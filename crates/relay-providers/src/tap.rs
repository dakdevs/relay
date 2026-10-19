//! Reads usage, terminal events and in-stream errors from a response body
//! as it passes through. The tap never alters or delays bytes.

use crate::sse::Scanner;
use relay_core::model::Provider;
use relay_core::usage::TokenUsage;
use serde_json::Value;

/// Largest non-streaming body buffered for usage parsing.
pub const MAX_JSON_BODY: usize = 8 * 1024 * 1024;

/// First event that decides whether a stream is real output or a rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decisive {
    /// Output, a tool call or any other content: the stream must commit.
    Output,
    /// An explicit rejection before any output (overload, rate or usage
    /// limit). `retry_after_ms` from `resets_in_seconds` when present.
    Rejection {
        /// Error type or code.
        kind: String,
        /// Provider-suggested wait.
        retry_after_ms: Option<i64>,
    },
    /// Some other error before output.
    Error {
        /// Error type or code.
        kind: String,
    },
}

/// Usage tap for one response.
#[derive(Debug)]
pub struct Tap {
    provider: Provider,
    streaming: bool,
    scanner: Scanner,
    json: Vec<u8>,
    json_overflow: bool,
    /// Usage seen so far.
    pub usage: TokenUsage,
    /// Whether usage was reported at all.
    pub usage_seen: bool,
    /// Terminal event seen (`message_stop`, `response.completed`, …) or a
    /// complete JSON body.
    pub terminal: bool,
    /// First decisive event.
    pub decisive: Option<Decisive>,
    /// Error after output started (type or code).
    pub late_error: Option<String>,
    /// Bytes observed.
    pub bytes: u64,
}

fn is_rejection(kind: &str) -> bool {
    matches!(
        kind,
        "overloaded_error"
            | "rate_limit_error"
            | "rate_limit_exceeded"
            | "usage_limit_reached"
            | "insufficient_quota"
            | "server_is_overloaded"
    )
}

fn u(v: &Value, k: &str) -> Option<u64> {
    v.get(k)?.as_u64()
}

/// Anthropic usage object → normalised usage. Fields absent from `v` keep
/// their previous value, because `message_delta` usage is cumulative and may
/// omit input fields.
fn anthropic_usage(v: &Value, into: &mut TokenUsage) {
    if let Some(x) = u(v, "input_tokens") {
        into.input = x;
    }
    if let Some(x) = u(v, "cache_read_input_tokens") {
        into.cache_read = x;
    }
    if let Some(x) = u(v, "cache_creation_input_tokens") {
        into.cache_write = x;
    }
    if let Some(x) = u(v, "output_tokens") {
        into.output = x;
    }
}

/// Responses usage → normalised usage (`input_tokens` includes cached).
fn openai_usage(v: &Value) -> TokenUsage {
    let input = u(v, "input_tokens").unwrap_or(0);
    let cached = v
        .get("input_tokens_details")
        .and_then(|d| u(d, "cached_tokens"))
        .unwrap_or(0)
        .min(input);
    TokenUsage {
        input: input - cached,
        cache_read: cached,
        cache_write: 0,
        output: u(v, "output_tokens").unwrap_or(0),
        reasoning: v
            .get("output_tokens_details")
            .and_then(|d| u(d, "reasoning_tokens"))
            .unwrap_or(0),
    }
}

/// Error kind and `resets_in_seconds` from an error object. Looks at the
/// nested `error` object first, then top-level fields, and prefers a known
/// rejection kind over a generic one, so `{"type":"error","code":
/// "rate_limit_exceeded"}` classifies as a rate limit.
fn error_kind(v: &Value) -> Option<(String, Option<i64>)> {
    let nested = v.get("error").filter(|e| e.is_object());
    let candidates: Vec<&str> = [
        nested.and_then(|e| e.get("type")),
        nested.and_then(|e| e.get("code")),
        v.get("code"),
        v.get("type"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .filter(|k| !k.is_empty())
    .collect();
    let kind = candidates
        .iter()
        .find(|k| is_rejection(k))
        .or_else(|| candidates.iter().find(|k| **k != "error"))
        .or_else(|| candidates.first())?
        .to_string();
    let retry = [nested, Some(v)]
        .into_iter()
        .flatten()
        .find_map(|e| e.get("resets_in_seconds").and_then(Value::as_f64))
        .filter(|s| *s >= 0.0)
        .map(|s| (s * 1000.0) as i64);
    Some((kind, retry))
}

impl Tap {
    /// New tap. `streaming` follows the response content type.
    pub fn new(provider: Provider, streaming: bool) -> Tap {
        Tap {
            provider,
            streaming,
            scanner: Scanner::default(),
            json: Vec::new(),
            json_overflow: false,
            usage: TokenUsage::default(),
            usage_seen: false,
            terminal: false,
            decisive: None,
            late_error: None,
            bytes: 0,
        }
    }

    fn decide(&mut self, d: Decisive) {
        if self.decisive.is_none() {
            self.decisive = Some(d);
        }
    }

    fn error(&mut self, v: &Value) {
        let (kind, retry) = error_kind(v).unwrap_or_else(|| ("error".into(), None));
        if self.decisive.is_some() {
            self.late_error = Some(kind);
        } else if is_rejection(&kind) {
            self.decide(Decisive::Rejection {
                kind,
                retry_after_ms: retry,
            });
        } else {
            self.decide(Decisive::Error { kind });
        }
    }

    fn on_event(&mut self, event: &str, data: &[u8]) {
        // Streams without `event:` lines carry the type in the data.
        let parsed: Option<Value> = if data.is_empty() {
            None
        } else {
            serde_json::from_slice(data).ok()
        };
        let owned_type;
        let event = if event.is_empty() {
            owned_type = parsed
                .as_ref()
                .and_then(|v| v.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            owned_type.as_str()
        } else {
            event
        };
        match self.provider {
            Provider::Anthropic => match event {
                "ping" | "" => {}
                "message_start" => {
                    if let Some(us) = parsed.as_ref().and_then(|v| v.get("message")?.get("usage")) {
                        anthropic_usage(us, &mut self.usage);
                        self.usage_seen = true;
                    }
                }
                "message_delta" => {
                    if let Some(us) = parsed.as_ref().and_then(|v| v.get("usage")) {
                        anthropic_usage(us, &mut self.usage);
                        self.usage_seen = true;
                    }
                    self.decide(Decisive::Output);
                }
                "message_stop" => {
                    self.decide(Decisive::Output);
                    self.terminal = true;
                }
                "error" => {
                    if let Some(v) = &parsed {
                        self.error(v);
                    }
                }
                _ => self.decide(Decisive::Output),
            },
            Provider::Openai => match event {
                "" | "response.created" | "response.in_progress" | "response.queued" => {}
                "response.completed" | "response.incomplete" => {
                    if let Some(us) = parsed
                        .as_ref()
                        .and_then(|v| v.get("response")?.get("usage"))
                    {
                        self.usage = openai_usage(us);
                        self.usage_seen = true;
                    }
                    self.decide(Decisive::Output);
                    self.terminal = true;
                }
                "response.failed" | "error" => {
                    if let Some(v) = &parsed {
                        let inner = v
                            .get("response")
                            .filter(|r| r.get("error").is_some())
                            .unwrap_or(v);
                        self.error(inner);
                        if let Some(us) = v.get("response").and_then(|r| r.get("usage")) {
                            self.usage = openai_usage(us);
                            self.usage_seen = true;
                        }
                    }
                    self.terminal = event == "response.failed";
                }
                _ => self.decide(Decisive::Output),
            },
        }
    }

    fn wanted(provider: Provider, event: &str) -> bool {
        match provider {
            Provider::Anthropic => {
                matches!(event, "" | "message_start" | "message_delta" | "error")
            }
            Provider::Openai => matches!(
                event,
                "" | "response.completed" | "response.incomplete" | "response.failed" | "error"
            ),
        }
    }

    /// Observes a body chunk.
    pub fn feed(&mut self, chunk: &[u8]) {
        self.bytes += chunk.len() as u64;
        if self.streaming {
            let provider = self.provider;
            let mut scanner = std::mem::take(&mut self.scanner);
            scanner.feed(
                chunk,
                &|e: &str| Tap::wanted(provider, e),
                &mut |e: &str, d: &[u8]| self.on_event(e, d),
            );
            self.scanner = scanner;
        } else if !self.json_overflow {
            if self.json.len() + chunk.len() > MAX_JSON_BODY {
                self.json_overflow = true;
                self.json = Vec::new();
            } else {
                self.json.extend_from_slice(chunk);
            }
        }
    }

    /// Ends the body. For non-streaming responses this parses the buffered
    /// JSON; `success` is whether the HTTP status was 2xx.
    pub fn finish(&mut self, success: bool) {
        if self.streaming {
            let mut scanner = std::mem::take(&mut self.scanner);
            scanner.finish(&mut |e: &str, d: &[u8]| self.on_event(e, d));
            self.scanner = scanner;
            return;
        }
        if self.json_overflow || self.json.is_empty() {
            return;
        }
        let Ok(v) = serde_json::from_slice::<Value>(&self.json) else {
            return;
        };
        self.json = Vec::new();
        if success {
            self.terminal = true;
            self.decide(Decisive::Output);
            if let Some(us) = v.get("usage") {
                match self.provider {
                    Provider::Anthropic => anthropic_usage(us, &mut self.usage),
                    Provider::Openai => self.usage = openai_usage(us),
                }
                self.usage_seen = true;
            }
        } else {
            self.error(&v);
        }
    }

    /// Lines skipped for exceeding the scanner cap.
    pub fn overflowed(&self) -> u32 {
        self.scanner.overflowed + u32::from(self.json_overflow)
    }
}

/// Classifies a non-2xx error body (already fully read). Returns the error
/// kind and `resets_in_seconds` when present.
pub fn error_body(body: &[u8]) -> Option<(String, Option<i64>)> {
    error_kind(&serde_json::from_slice::<Value>(body).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUDE_STREAM: &[u8] = b"event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"usage\":{\"input_tokens\":12,\"cache_creation_input_tokens\":100,\"cache_read_input_tokens\":2000,\"output_tokens\":1}}}\n\n\
event: ping\ndata: {\"type\":\"ping\"}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":57}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

    const CODEX_STREAM: &[u8] = b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n\
event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"x\"}\n\n\
event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":1000,\"input_tokens_details\":{\"cached_tokens\":800},\"output_tokens\":300,\"output_tokens_details\":{\"reasoning_tokens\":200},\"total_tokens\":1300}}}\n\n";

    #[test]
    fn claude_usage_is_cumulative_not_summed() {
        for split in [1, 7, 64, CLAUDE_STREAM.len()] {
            let mut t = Tap::new(Provider::Anthropic, true);
            for c in CLAUDE_STREAM.chunks(split) {
                t.feed(c);
            }
            t.finish(true);
            assert_eq!(
                t.usage,
                TokenUsage {
                    input: 12,
                    cache_read: 2000,
                    cache_write: 100,
                    output: 57,
                    reasoning: 0
                }
            );
            assert!(t.terminal);
            assert_eq!(t.decisive, Some(Decisive::Output));
        }
    }

    #[test]
    fn codex_usage_splits_cached_and_keeps_reasoning_inside_output() {
        let mut t = Tap::new(Provider::Openai, true);
        t.feed(CODEX_STREAM);
        t.finish(true);
        assert_eq!(t.usage.input, 200);
        assert_eq!(t.usage.cache_read, 800);
        assert_eq!(t.usage.output, 300);
        assert_eq!(t.usage.reasoning, 200);
        assert_eq!(t.usage.total(), 1300); // matches provider total_tokens
        assert!(t.terminal);
    }

    #[test]
    fn truncated_stream_is_not_terminal() {
        let cut = &CLAUDE_STREAM[..CLAUDE_STREAM.len() - 40];
        let mut t = Tap::new(Provider::Anthropic, true);
        t.feed(cut);
        t.finish(true);
        assert!(!t.terminal);
    }

    #[test]
    fn in_stream_rejection_before_output_is_decisive() {
        let s = b"event: response.created\ndata: {}\n\n\
event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"usage_limit_reached\",\"resets_in_seconds\":90}}\n\n";
        let mut t = Tap::new(Provider::Openai, true);
        t.feed(s);
        assert_eq!(
            t.decisive,
            Some(Decisive::Rejection {
                kind: "usage_limit_reached".into(),
                retry_after_ms: Some(90_000)
            })
        );
        let late = b"event: content_block_start\ndata: {}\n\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\"}}\n\n";
        let mut t = Tap::new(Provider::Anthropic, true);
        t.feed(late);
        assert_eq!(t.decisive, Some(Decisive::Output));
        assert_eq!(t.late_error.as_deref(), Some("overloaded_error"));
    }

    #[test]
    fn non_streaming_json_usage() {
        let mut t = Tap::new(Provider::Anthropic, false);
        t.feed(br#"{"id":"m","usage":{"input_tokens":3,"output_tokens":4}}"#);
        t.finish(true);
        assert_eq!(t.usage.total(), 7);
        assert!(t.terminal);
        let mut t = Tap::new(Provider::Anthropic, false);
        t.feed(br#"{"type":"error","error":{"type":"overloaded_error","message":"x"}}"#);
        t.finish(false);
        assert!(matches!(t.decisive, Some(Decisive::Rejection { .. })));
    }

    #[test]
    fn top_level_code_is_not_shadowed_by_generic_type() {
        let mut t = Tap::new(Provider::Openai, true);
        t.feed(b"event: error\ndata: {\"type\":\"error\",\"code\":\"rate_limit_exceeded\",\"message\":\"x\"}\n\n");
        assert!(matches!(
            t.decisive,
            Some(Decisive::Rejection { ref kind, .. }) if kind == "rate_limit_exceeded"
        ));
        assert_eq!(
            error_body(br#"{"type":"error","error":{"type":"overloaded_error"}}"#)
                .unwrap()
                .0,
            "overloaded_error"
        );
        assert_eq!(
            error_body(br#"{"type":"error","error":{"type":"api_error"}}"#)
                .unwrap()
                .0,
            "api_error"
        );
    }

    #[test]
    fn response_failed_before_output_is_an_error_not_success() {
        let s = b"event: response.created\ndata: {}\n\n\
event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"server_error\",\"message\":\"x\"}}}\n\n";
        let mut t = Tap::new(Provider::Openai, true);
        t.feed(s);
        t.finish(true);
        assert_eq!(
            t.decisive,
            Some(Decisive::Error {
                kind: "server_error".into()
            })
        );
        assert!(t.terminal);
    }

    #[test]
    fn error_body_kinds() {
        assert_eq!(
            error_body(br#"{"error":{"type":"usage_limit_reached","resets_in_seconds":5}}"#),
            Some(("usage_limit_reached".into(), Some(5_000)))
        );
        assert_eq!(
            error_body(br#"{"error":{"code":"insufficient_quota"}}"#)
                .unwrap()
                .0,
            "insufficient_quota"
        );
        assert_eq!(error_body(b"<html>"), None);
    }
}
