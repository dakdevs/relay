//! Request inspection: routing key and account-movability, from one bounded
//! pass over the buffered body. The body is never re-serialised; Relay
//! forwards the original bytes.

use http::HeaderMap;
use memchr::memmem;
use relay_core::model::{Provider, Vendor};
use serde::Deserialize;
use serde::de::IgnoredAny;
use std::borrow::Cow;

/// Which wire endpoint a request targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    /// `POST /v1/messages`
    Messages,
    /// `POST /v1/messages/count_tokens`
    CountTokens,
    /// `POST /v1/responses`
    Responses,
    /// `POST /v1/responses/compact`
    ResponsesCompact,
    /// `GET /v1/models`
    Models,
}

impl Endpoint {
    /// Maps a request path (without query) and the provider hint from
    /// headers to an endpoint. `/v1/models` is shared, so it needs a hint.
    pub fn from_path(path: &str) -> Option<(Endpoint, Option<Provider>)> {
        Some(match path {
            "/v1/messages" => (Endpoint::Messages, Some(Provider::Anthropic)),
            "/v1/messages/count_tokens" => (Endpoint::CountTokens, Some(Provider::Anthropic)),
            "/v1/responses" => (Endpoint::Responses, Some(Provider::Openai)),
            "/v1/responses/compact" => (Endpoint::ResponsesCompact, Some(Provider::Openai)),
            "/v1/models" => (Endpoint::Models, None),
            _ => return None,
        })
    }

    /// Whether responses carry generated tokens worth recording.
    pub fn generates(self) -> bool {
        matches!(
            self,
            Endpoint::Messages | Endpoint::Responses | Endpoint::ResponsesCompact
        )
    }

    /// Whether `vendor` serves this endpoint on `wire`. First-party vendors
    /// serve every endpoint of their wire. Others serve only what their
    /// upstream is known to implement (CLIProxyAPI 3f0cef0): Kimi Code
    /// Messages, token counting and Responses; xAI Responses and compact
    /// (`api.x.ai`); Meta Responses (compact answers 501). No model listing
    /// is known for them, so `/v1/models` is refused rather than guessed.
    pub fn served_by(self, vendor: Vendor, wire: Provider) -> bool {
        if !vendor.serves(wire) {
            return false;
        }
        match vendor {
            Vendor::Anthropic | Vendor::Openai => true,
            Vendor::KimiCom | Vendor::KimiAi => matches!(
                self,
                Endpoint::Messages | Endpoint::CountTokens | Endpoint::Responses
            ),
            Vendor::Xai => matches!(self, Endpoint::Responses | Endpoint::ResponsesCompact),
            Vendor::Meta => self == Endpoint::Responses,
        }
    }

    /// Upstream path suffix appended to the account's base URL. Anthropic
    /// bases are origins (`https://api.anthropic.com`), OpenAI bases already
    /// end in their API root (`…/v1` or `…/backend-api/codex`).
    pub fn upstream_path(self, provider: Provider) -> &'static str {
        match (provider, self) {
            (Provider::Anthropic, Endpoint::Messages) => "/v1/messages",
            (Provider::Anthropic, Endpoint::CountTokens) => "/v1/messages/count_tokens",
            (Provider::Anthropic, _) => "/v1/models",
            (Provider::Openai, Endpoint::Responses) => "/responses",
            (Provider::Openai, Endpoint::ResponsesCompact) => "/responses/compact",
            (Provider::Openai, _) => "/models",
        }
    }
}

/// What Relay needs from a request body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestInfo {
    /// `model` field.
    pub model: Option<String>,
    /// `stream: true`.
    pub stream: bool,
    /// Carries state or references bound to an account, so the request must
    /// not move to another account.
    pub stateful: bool,
    /// The body was not a JSON object (forwarded anyway; never moved).
    pub unparsed: bool,
}

#[derive(Deserialize)]
struct Probe<'a> {
    #[serde(borrow, default)]
    model: Option<Cow<'a, str>>,
    #[serde(default)]
    stream: Option<bool>,
    #[serde(default)]
    previous_response_id: Option<IgnoredAny>,
    #[serde(default)]
    conversation: Option<IgnoredAny>,
    #[serde(default)]
    container: Option<IgnoredAny>,
}

/// Object keys whose presence anywhere means account-bound content: signed
/// thinking, encrypted reasoning or compaction capsules, uploaded files.
const STATE_KEYS: [&[u8]; 3] = [b"\"signature\"", b"\"encrypted_content\"", b"\"file_id\""];

/// `"type"` values that mean account-bound items.
const STATE_TYPES: [&[u8]; 5] = [
    b"reasoning",
    b"redacted_thinking",
    b"compaction",
    b"item_reference",
    b"container_upload",
];

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i
}

/// Positions just after `"key"` followed by optional whitespace and `:`,
/// i.e. where `key` is an object key rather than text inside a string
/// (string content would have escaped quotes).
fn key_values<'a>(body: &'a [u8], key: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    memmem::find_iter(body, key).filter_map(move |at| {
        if at > 0 && body[at - 1] == b'\\' {
            return None;
        }
        let i = skip_ws(body, at + key.len());
        (body.get(i) == Some(&b':')).then(|| skip_ws(body, i + 1))
    })
}

/// One pass per marker over the bytes, tolerant of JSON whitespace. A false
/// positive only makes the request non-movable, which is the safe side.
fn has_account_state(body: &[u8]) -> bool {
    STATE_KEYS
        .iter()
        .any(|k| key_values(body, k).next().is_some())
        || key_values(body, b"\"type\"").any(|v| {
            body.get(v) == Some(&b'"')
                && STATE_TYPES.iter().any(|t| {
                    body[v + 1..].starts_with(t) && body.get(v + 1 + t.len()) == Some(&b'"')
                })
        })
}

/// Inspects a request body.
pub fn inspect(_provider: Provider, body: &[u8]) -> RequestInfo {
    let Ok(p) = serde_json::from_slice::<Probe>(body) else {
        return RequestInfo {
            stateful: true,
            unparsed: true,
            ..Default::default()
        };
    };
    let stateful = p.previous_response_id.is_some()
        || p.conversation.is_some()
        || p.container.is_some()
        || has_account_state(body);
    RequestInfo {
        model: p.model.map(Cow::into_owned),
        stream: p.stream.unwrap_or(false),
        stateful,
        unparsed: false,
    }
}

/// Session identity from headers, in precedence order: Relay's explicit
/// header, then the CLI's own session header.
pub fn session_id(provider: Provider, h: &HeaderMap) -> Option<&str> {
    let names: &[&str] = match provider {
        Provider::Anthropic => &[crate::headers::RELAY_SESSION, "x-claude-code-session-id"],
        Provider::Openai => &[
            crate::headers::RELAY_SESSION,
            "session_id",
            "conversation_id",
        ],
    };
    names
        .iter()
        .filter_map(|n| h.get(*n)?.to_str().ok())
        .map(str::trim)
        .find(|v| !v.is_empty() && v.len() <= 256)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_turn_is_movable() {
        let body = br#"{"model":"claude-sonnet-5-5","stream":true,"system":[{"type":"text","text":"x"}],
            "messages":[{"role":"user","content":"hi"}],"tools":[{"name":"Bash","input_schema":{}}]}"#;
        let i = inspect(Provider::Anthropic, body);
        assert_eq!(i.model.as_deref(), Some("claude-sonnet-5-5"));
        assert!(i.stream);
        assert!(!i.stateful);
    }

    #[test]
    fn signed_thinking_is_stateful() {
        let body = br#"{"model":"m","messages":[{"role":"assistant","content":[{"type":"thinking","thinking":"...","signature":"abc"}]}]}"#;
        assert!(inspect(Provider::Anthropic, body).stateful);
    }

    #[test]
    fn responses_continuations_are_stateful() {
        assert!(
            inspect(
                Provider::Openai,
                br#"{"model":"m","previous_response_id":"resp_1"}"#
            )
            .stateful
        );
        assert!(
            !inspect(
                Provider::Openai,
                br#"{"model":"m","previous_response_id":null}"#
            )
            .stateful
        );
        assert!(
            inspect(
                Provider::Openai,
                br#"{"model":"m","input":[{"type":"reasoning","encrypted_content":"x"}]}"#
            )
            .stateful
        );
    }

    #[test]
    fn state_detection_tolerates_whitespace_and_finds_references() {
        let spaced = br#"{"model":"m","input":[ { "type" : "reasoning", "summary": [] } ]}"#;
        assert!(inspect(Provider::Openai, spaced).stateful);
        let enc = br#"{"model":"m","input":[{"type":"message"},{"encrypted_content" :"x"}]}"#;
        assert!(inspect(Provider::Openai, enc).stateful);
        let file = br#"{"model":"m","input":[{"role":"user","content":[{"type":"input_file","file_id": "file-1"}]}]}"#;
        assert!(inspect(Provider::Openai, file).stateful);
        let conv = br#"{"model":"m","conversation":"conv_1","input":"hi"}"#;
        assert!(inspect(Provider::Openai, conv).stateful);
        let item = br#"{"model":"m","input":[{"type":"item_reference","id":"rs_1"}]}"#;
        assert!(inspect(Provider::Openai, item).stateful);
    }

    #[test]
    fn marker_words_inside_text_do_not_pin() {
        let text = br#"{"model":"m","messages":[{"role":"user","content":"explain \"signature\": and type reasoning"}]}"#;
        assert!(!inspect(Provider::Anthropic, text).stateful);
        let other_type = br#"{"model":"m","input":[{"type":"reasoning_effort_note"}]}"#;
        assert!(!inspect(Provider::Openai, other_type).stateful);
    }

    #[test]
    fn escaped_model_names_parse() {
        let i = inspect(Provider::Openai, br#"{"model":"gpt-5"}"#);
        assert_eq!(i.model.as_deref(), Some("gpt-5"));
    }

    #[test]
    fn non_json_is_never_movable() {
        let i = inspect(Provider::Openai, b"not json");
        assert!(i.unparsed && i.stateful);
    }

    #[test]
    fn endpoints_map_to_upstream_paths() {
        let (e, p) = Endpoint::from_path("/v1/responses").unwrap();
        assert_eq!(p, Some(Provider::Openai));
        assert_eq!(e.upstream_path(Provider::Openai), "/responses");
        assert_eq!(
            Endpoint::Models.upstream_path(Provider::Anthropic),
            "/v1/models"
        );
        assert!(Endpoint::from_path("/v0/management/api-call").is_none());
    }

    #[test]
    fn vendor_endpoints_follow_known_upstream_routes() {
        use Endpoint::*;
        for e in [Messages, CountTokens, Models] {
            assert!(e.served_by(Vendor::Anthropic, Provider::Anthropic));
        }
        assert!(Messages.served_by(Vendor::KimiCom, Provider::Anthropic));
        assert!(CountTokens.served_by(Vendor::KimiAi, Provider::Anthropic));
        assert!(Responses.served_by(Vendor::KimiCom, Provider::Openai));
        assert!(!ResponsesCompact.served_by(Vendor::KimiCom, Provider::Openai));
        assert!(!Models.served_by(Vendor::KimiCom, Provider::Anthropic));
        assert!(ResponsesCompact.served_by(Vendor::Xai, Provider::Openai));
        assert!(!ResponsesCompact.served_by(Vendor::Meta, Provider::Openai));
        assert!(Responses.served_by(Vendor::Meta, Provider::Openai));
        // A vendor never serves a wire it doesn't speak.
        assert!(!Messages.served_by(Vendor::Xai, Provider::Anthropic));
        assert!(!Messages.served_by(Vendor::Openai, Provider::Anthropic));
    }

    #[test]
    fn session_header_precedence() {
        let mut h = HeaderMap::new();
        h.insert("x-claude-code-session-id", "cli".parse().unwrap());
        assert_eq!(session_id(Provider::Anthropic, &h), Some("cli"));
        h.insert("x-relay-session", "explicit".parse().unwrap());
        assert_eq!(session_id(Provider::Anthropic, &h), Some("explicit"));
        let mut o = HeaderMap::new();
        o.insert("session_id", "codex".parse().unwrap());
        assert_eq!(session_id(Provider::Openai, &o), Some("codex"));
    }
}
