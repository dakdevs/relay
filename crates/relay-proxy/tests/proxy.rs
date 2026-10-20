//! Protocol and end-to-end tests against controllable local upstreams.
//! Everything binds ephemeral 127.0.0.1 ports in temp dirs; no network,
//! credentials or user settings are touched.

mod mock;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use mock::{Act, Mock};
use relay_core::config::{AccountConfig, Config, SecretFormat, SecretRef};
use relay_core::model::{AccountId, AccountMode, CredentialKind, Pool, Provider, Vendor};
use relay_core::select::Strategy;
use relay_core::snapshot::{AccountState, RoutingStatus};
use relay_proxy::paths::Paths;
use relay_proxy::{Handle, StartError, start};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CLAUDE_SSE: &[u8] = b"event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"usage\":{\"input_tokens\":12,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":30,\"output_tokens\":1}}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t\",\"name\":\"Bash\",\"input\":{}}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"cmd\\\": \\\"ls\\\"}\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":9}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

const CODEX_SSE: &[u8] = b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n\
event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n\
event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":100,\"input_tokens_details\":{\"cached_tokens\":40},\"output_tokens\":20,\"output_tokens_details\":{\"reasoning_tokens\":5}}}}\n\n";

const CODEX_LIMIT: &[u8] = b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n\
event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"usage_limit_reached\",\"resets_in_seconds\":120}}\n\n";

const FIRST_TURN: &str =
    r#"{"model":"gpt-5","stream":true,"input":[{"role":"user","content":"hi"}]}"#;

fn account(id: &str, provider: Provider, mode: AccountMode, upstream: &str) -> AccountConfig {
    AccountConfig {
        id: AccountId(id.into()),
        provider,
        vendor: None,
        device_id: None,
        label: id.into(),
        mode,
        plan: None,
        identifier: None,
        credential_kind: None,
        upstream: Some(upstream.into()),
        credential: None,
        paused: false,
        max_in_flight: 8,
        models: vec![],
    }
}

struct Env {
    _dir: tempfile::TempDir,
    paths: Paths,
    handle: Option<Handle>,
    key: String,
}

impl Env {
    fn base(&self) -> String {
        format!("http://{}", self.handle.as_ref().unwrap().addr())
    }
    fn handle(&self) -> &Handle {
        self.handle.as_ref().unwrap()
    }
    fn stop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.shutdown();
        }
    }
}

/// Starts a proxy with `accounts` on an ephemeral port. Router-owned
/// accounts get a raw-format credential file holding `token-<id>`.
fn proxy(mut accounts: Vec<AccountConfig>, edit: impl FnOnce(&mut Config)) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    let mut cfg = Config::default();
    cfg.server.listen = "127.0.0.1:0".parse().unwrap();
    for a in &mut accounts {
        let url = a.upstream.clone().unwrap();
        if !cfg.upstreams.allowed.contains(&url) {
            cfg.upstreams.allowed.push(url);
        }
        if a.mode.router_owned() {
            let f = dir.path().join(format!("{}.token", a.id));
            std::fs::write(&f, format!("token-{}", a.id)).unwrap();
            a.credential = Some(SecretRef::File {
                file: f,
                format: SecretFormat::Raw,
            });
        }
    }
    cfg.accounts = accounts;
    cfg.routing.strategy = Strategy::FillFirst;
    edit(&mut cfg);
    let handle = start(&paths, cfg).unwrap();
    let key = handle.reveal_local_key();
    // Credentials load on a background thread.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let snap = handle.snapshot();
        let pending = snap
            .providers
            .iter()
            .flat_map(|p| &p.groups)
            .flat_map(|g| &g.accounts)
            .any(|a| a.mode.router_owned() && a.state == AccountState::Disconnected);
        if !pending {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Env {
        _dir: dir,
        paths,
        handle: Some(handle),
        key,
    }
}

type C = Client<HttpConnector, Full<Bytes>>;

fn client() -> C {
    Client::builder(TokioExecutor::new()).build_http()
}

struct Resp {
    status: u16,
    headers: http::HeaderMap,
    body: Vec<u8>,
}

async fn send(c: &C, url: String, headers: &[(&str, &str)], body: &str) -> Resp {
    let mut b = http::Request::post(url);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let r = c
        .request(b.body(Full::new(Bytes::from(body.to_string()))).unwrap())
        .await
        .unwrap();
    let status = r.status().as_u16();
    let headers = r.headers().clone();
    let body = r
        .into_body()
        .collect()
        .await
        .map(|c| c.to_bytes().to_vec())
        .unwrap_or_default();
    Resp {
        status,
        headers,
        body,
    }
}

fn owned_headers<'a>(env: &'a Env, extra: &[(&'a str, &'a str)]) -> Vec<(&'a str, String)> {
    let mut h = vec![("authorization", format!("Bearer {}", env.key))];
    h.extend(extra.iter().map(|(k, v)| (*k, v.to_string())));
    h
}

async fn send_owned(c: &C, env: &Env, path: &str, extra: &[(&str, &str)], body: &str) -> Resp {
    let h = owned_headers(env, extra);
    let refs: Vec<(&str, &str)> = h.iter().map(|(k, v)| (*k, v.as_str())).collect();
    send(c, format!("{}{path}", env.base()), &refs, body).await
}

fn account_view(env: &Env, id: &str) -> relay_core::snapshot::AccountView {
    // The publisher coalesces at 2 Hz; wait for a fresh snapshot.
    std::thread::sleep(Duration::from_millis(700));
    env.handle()
        .snapshot()
        .providers
        .iter()
        .flat_map(|p| &p.groups)
        .flat_map(|g| &g.accounts)
        .find(|a| a.id.as_str() == id)
        .cloned()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_config_runs_with_true_empty_state() {
    let env = proxy(vec![], |_| {});
    let snap = env.handle().snapshot();
    assert!(snap.providers.is_empty());
    assert!(matches!(snap.routing, RoutingStatus::Running { .. }));
    // Health is local; unknown paths are refused, never forwarded.
    let c = client();
    let r = c
        .request(
            http::Request::get(format!("{}/relay/health", env.base()))
                .body(Full::default())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let r = send(
        &c,
        format!("{}/v0/management/api-call", env.base()),
        &[],
        "{}",
    )
    .await;
    assert_eq!(r.status, 404);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn port_conflict_is_reported_at_start() {
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.server.listen = taken.local_addr().unwrap();
    match start(&Paths::in_dir(dir.path()), cfg) {
        Err(StartError::AddrInUse(a)) => assert_eq!(a, taken.local_addr().unwrap()),
        other => panic!("expected AddrInUse, got {:?}", other.err()),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pass_through_keeps_client_credentials_and_bytes() {
    let up = Mock::start(
        Act::sse(vec![&CLAUDE_SSE[..100], &CLAUDE_SSE[100..]])
            .with_header("anthropic-ratelimit-unified-5h-utilization", "0.25")
            .with_header("anthropic-ratelimit-unified-5h-reset", "4102444800"),
    )
    .await;
    let mut env = proxy(
        vec![account(
            "personal",
            Provider::Anthropic,
            AccountMode::Observed,
            &up.url(),
        )],
        |_| {},
    );
    let c = client();
    let key = env.key.clone();
    let body = r#"{"model":"claude-sonnet-5-5","stream":true,"messages":[{"role":"user","content":"hi"}]}"#;
    let r = send(
        &c,
        format!("{}/v1/messages?beta=true", env.base()),
        &[
            ("authorization", "Bearer user-oauth-token"),
            (
                "anthropic-beta",
                "oauth-2025-04-20,interleaved-thinking-2025-05-14",
            ),
            ("anthropic-version", "2023-06-01"),
            ("x-relay-key", &key),
            ("x-relay-account", "personal"),
            ("x-claude-code-session-id", "s1"),
        ],
        body,
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body, CLAUDE_SSE, "SSE must pass through byte for byte");
    let seen = up.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].path, "/v1/messages?beta=true");
    assert_eq!(
        seen[0].header("authorization"),
        Some("Bearer user-oauth-token")
    );
    assert_eq!(
        seen[0].header("anthropic-beta"),
        Some("oauth-2025-04-20,interleaved-thinking-2025-05-14")
    );
    assert!(
        seen[0]
            .headers
            .iter()
            .all(|(k, _)| !k.starts_with("x-relay"))
    );
    assert_eq!(seen[0].body, body.as_bytes());

    let view = account_view(&env, "personal");
    assert_eq!(view.windows[0].key, "5h");
    assert_eq!(view.windows[0].used_pct, Some(25.0));
    assert_eq!(view.state, AccountState::Healthy);
    // No local key or client token in the snapshot.
    let dump = format!("{:?}", env.handle().snapshot());
    assert!(!dump.contains(&key) && !dump.contains("user-oauth-token"));

    let db = env.paths.usage_db.clone();
    env.stop();
    let mut reader = relay_usage::Reader::open(&db).unwrap();
    let totals = reader.account_totals(relay_usage::Range::Day).unwrap();
    assert_eq!(totals.len(), 1);
    assert_eq!(totals[0].usage.input, 12);
    assert_eq!(totals[0].usage.cache_read, 30);
    assert_eq!(
        totals[0].usage.output, 9,
        "message_delta usage is cumulative, not added"
    );
    assert_eq!(totals[0].requests, 1);
    assert_eq!(totals[0].errors, 0, "a fully delivered stream is a success");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_or_missing_local_key_is_denied_before_upstream() {
    let up = Mock::start(Act::json(200, "{}")).await;
    let env = proxy(
        vec![account(
            "a",
            Provider::Anthropic,
            AccountMode::Observed,
            &up.url(),
        )],
        |_| {},
    );
    let c = client();
    for headers in [
        vec![("x-relay-key", "nope"), ("x-relay-account", "a")],
        vec![("authorization", "Bearer nope")],
        vec![],
    ] {
        let r = send(&c, format!("{}/v1/messages", env.base()), &headers, "{}").await;
        assert_eq!(r.status, 401);
    }
    // Pass-through must name an observed account.
    let key = env.key.clone();
    let r = send(
        &c,
        format!("{}/v1/messages", env.base()),
        &[("x-relay-key", &key)],
        "{}",
    )
    .await;
    assert_eq!(r.status, 400);
    let r = send(
        &c,
        format!("{}/v1/messages", env.base()),
        &[("x-relay-key", &key), ("x-relay-account", "zzz")],
        "{}",
    )
    .await;
    assert_eq!(r.status, 403);
    assert_eq!(up.count(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ambiguous_reset_is_never_retried() {
    let a = Mock::start(Act::Reset).await;
    let b = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(
        vec![
            account("a", Provider::Openai, AccountMode::Key, &a.url()),
            account("b", Provider::Openai, AccountMode::Key, &b.url()),
        ],
        |c| c.routing.default_pool = Pool::Key,
    );
    let c = client();
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.status, 502);
    assert_eq!(r.headers["x-relay-error"], "upstream_ambiguous");
    assert_eq!(a.count(), 1, "executed once");
    assert_eq!(b.count(), 0, "never re-executed elsewhere");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proven_not_sent_moves_to_next_account() {
    // A port with nothing listening: connect is refused.
    let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_url = format!("http://{}", dead.local_addr().unwrap());
    drop(dead);
    let b = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(
        vec![
            account("a", Provider::Openai, AccountMode::Key, &dead_url),
            account("b", Provider::Openai, AccountMode::Key, &b.url()),
        ],
        |c| c.routing.default_pool = Pool::Key,
    );
    let c = client();
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body, CODEX_SSE);
    assert_eq!(b.count(), 1);
    assert_eq!(b.seen()[0].header("authorization"), Some("Bearer token-b"));
    assert_eq!(b.seen()[0].path, "/responses");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejection_on_first_turn_fails_over_and_cools_down() {
    let a = Mock::start(
        Act::json(429, r#"{"error":{"type":"rate_limit_exceeded"}}"#)
            .with_header("retry-after", "30"),
    )
    .await;
    let b = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(
        vec![
            account("a", Provider::Openai, AccountMode::Pooled, &a.url()),
            account("b", Provider::Openai, AccountMode::Pooled, &b.url()),
        ],
        |_| {},
    );
    let c = client();
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.status, 200);
    assert_eq!((a.count(), b.count()), (1, 1));
    let view = account_view(&env, "a");
    assert_eq!(view.state, AccountState::Exhausted);
    let until = view.cooldown_until.unwrap() - relay_proxy::state::now_ms();
    assert!(
        (25_000..=30_500).contains(&until),
        "cooldown from retry-after, got {until}"
    );
    // While cooling down, A is skipped without a request.
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.status, 200);
    assert_eq!((a.count(), b.count()), (1, 2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stateful_or_pass_through_rejections_are_returned_unchanged() {
    let body429 = r#"{"error":{"type":"rate_limit_exceeded","message":"slow down"}}"#;
    let a = Mock::start(Act::json(429, body429)).await;
    let b = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let o = Mock::start(Act::json(429, body429)).await;
    let env = proxy(
        vec![
            account("a", Provider::Openai, AccountMode::Pooled, &a.url()),
            account("b", Provider::Openai, AccountMode::Pooled, &b.url()),
            account("o", Provider::Openai, AccountMode::Observed, &o.url()),
        ],
        |_| {},
    );
    let c = client();
    let r = send_owned(
        &c,
        &env,
        "/v1/responses",
        &[],
        r#"{"model":"gpt-5","previous_response_id":"resp_1"}"#,
    )
    .await;
    assert_eq!(r.status, 429);
    assert_eq!(r.body, body429.as_bytes());
    assert_eq!((a.count(), b.count()), (1, 0));

    let key = env.key.clone();
    let r = send(
        &c,
        format!("{}/v1/responses", env.base()),
        &[
            ("authorization", "Bearer chatgpt-login"),
            ("x-relay-key", &key),
            ("x-relay-account", "o"),
        ],
        FIRST_TURN,
    )
    .await;
    assert_eq!(r.status, 429);
    assert_eq!(r.body, body429.as_bytes());
    assert_eq!(o.count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_and_client_errors_are_not_retried() {
    let a = Mock::start(Act::json(502, r#"{"error":"bad gateway"}"#)).await;
    let b = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(
        vec![
            account("a", Provider::Openai, AccountMode::Key, &a.url()),
            account("b", Provider::Openai, AccountMode::Key, &b.url()),
        ],
        |c| c.routing.default_pool = Pool::Key,
    );
    let c = client();
    let r = send_owned(
        &c,
        &env,
        "/v1/responses",
        &[],
        r#"{"model":"gpt-5","tools":[{"type":"web_search"}]}"#,
    )
    .await;
    assert_eq!(r.status, 502);
    assert_eq!(b.count(), 0);
    // A 5xx doesn't prove the request didn't run, so even a stateless first
    // turn without provider-side tools stays on its account.
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.status, 502);
    assert_eq!((a.count(), b.count()), (2, 0));

    let bad = r#"{"type":"error","error":{"type":"invalid_request_error","message":"prompt is too long"}}"#;
    let x = Mock::start(Act::json(400, bad)).await;
    let y = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    let env2 = proxy(
        vec![
            account("x", Provider::Anthropic, AccountMode::Key, &x.url()),
            account("y", Provider::Anthropic, AccountMode::Key, &y.url()),
        ],
        |c| c.routing.default_pool = Pool::Key,
    );
    let r = send_owned(
        &c,
        &env2,
        "/v1/messages",
        &[],
        r#"{"model":"m","messages":[]}"#,
    )
    .await;
    assert_eq!(r.status, 400);
    assert_eq!(r.body, bad.as_bytes(), "error bodies are byte-identical");
    assert_eq!((x.count(), y.count()), (1, 0));
    assert_eq!(x.seen()[0].header("x-api-key"), Some("token-x"));
    assert_eq!(x.seen()[0].header("authorization"), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn in_stream_limit_fails_over_only_before_output() {
    let a = Mock::start(Act::sse(vec![CODEX_LIMIT])).await;
    let b = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(
        vec![
            account("a", Provider::Openai, AccountMode::Pooled, &a.url()),
            account("b", Provider::Openai, AccountMode::Pooled, &b.url()),
        ],
        |_| {},
    );
    let c = client();
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body, CODEX_SSE, "client sees only the second stream");
    assert_eq!((a.count(), b.count()), (1, 1));
    let view = account_view(&env, "a");
    assert!(
        view.cooldown_until.is_some(),
        "resets_in_seconds sets the cooldown"
    );

    // Same error after output: one stream, no retry.
    let late: Vec<u8> = [&CODEX_SSE[..CODEX_SSE.len() - 1], CODEX_LIMIT].concat();
    let p = Mock::start(Act::sse(vec![&late])).await;
    let q = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env2 = proxy(
        vec![
            account("p", Provider::Openai, AccountMode::Pooled, &p.url()),
            account("q", Provider::Openai, AccountMode::Pooled, &q.url()),
        ],
        |_| {},
    );
    let r = send_owned(&c, &env2, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.body, late);
    assert_eq!((p.count(), q.count()), (1, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sessions_stick_and_new_sessions_go_to_most_headroom() {
    let a =
        Mock::start(Act::sse(vec![CODEX_SSE]).with_header("x-codex-primary-used-percent", "90"))
            .await;
    let b =
        Mock::start(Act::sse(vec![CODEX_SSE]).with_header("x-codex-primary-used-percent", "10"))
            .await;
    let env = proxy(
        vec![
            account("a", Provider::Openai, AccountMode::Pooled, &a.url()),
            account("b", Provider::Openai, AccountMode::Pooled, &b.url()),
        ],
        |c| c.routing.strategy = Strategy::QuotaAware,
    );
    let c = client();
    // Learn both accounts' quota (unknown ranks as full, so each gets one).
    send_owned(
        &c,
        &env,
        "/v1/responses",
        &[("x-relay-account", "a")],
        FIRST_TURN,
    )
    .await;
    send_owned(
        &c,
        &env,
        "/v1/responses",
        &[("x-relay-account", "b")],
        FIRST_TURN,
    )
    .await;
    assert_eq!((a.count(), b.count()), (1, 1));
    // Bind session s1 to a explicitly, then continue it without the pin.
    send_owned(
        &c,
        &env,
        "/v1/responses",
        &[("x-relay-account", "a"), ("session_id", "s1")],
        FIRST_TURN,
    )
    .await;
    for _ in 0..3 {
        let r = send_owned(
            &c,
            &env,
            "/v1/responses",
            &[("session_id", "s1")],
            r#"{"model":"gpt-5","previous_response_id":"r"}"#,
        )
        .await;
        assert_eq!(r.status, 200);
    }
    assert_eq!(
        a.count(),
        5,
        "running session stays on its account despite less headroom"
    );
    // A new session goes to the account with most headroom.
    send_owned(
        &c,
        &env,
        "/v1/responses",
        &[("session_id", "s2")],
        FIRST_TURN,
    )
    .await;
    assert_eq!(b.count(), 2);
    assert_eq!(account_view(&env, "a").sessions, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pools_never_mix() {
    let k = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(
        vec![account("k", Provider::Openai, AccountMode::Key, &k.url())],
        |_| {},
    );
    let c = client();
    // Default pool is the subscription pool, which is empty: no silent
    // substitution of the API key.
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.status, 503);
    assert_eq!(r.headers["x-relay-error"], "no_eligible_account");
    assert_eq!(k.count(), 0);
    let r = send_owned(
        &c,
        &env,
        "/v1/responses",
        &[("x-relay-pool", "key")],
        FIRST_TURN,
    )
    .await;
    assert_eq!(r.status, 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_disconnect_closes_upstream_promptly() {
    let chunks: Vec<(Duration, Vec<u8>)> = (0..100)
        .map(|_| {
            (
                Duration::from_millis(20),
                b"event: ping\ndata: {}\n\n".to_vec(),
            )
        })
        .collect();
    let up = Mock::start(Act::Respond {
        status: 200,
        headers: vec![("content-type".into(), "text/event-stream".into())],
        chunks,
    })
    .await;
    let env = proxy(
        vec![account(
            "a",
            Provider::Anthropic,
            AccountMode::Observed,
            &up.url(),
        )],
        |_| {},
    );
    let mut s = tokio::net::TcpStream::connect(env.handle().addr())
        .await
        .unwrap();
    let body = r#"{"model":"m","stream":true}"#;
    let req = format!(
        "POST /v1/messages HTTP/1.1\r\nhost: x\r\nx-relay-key: {}\r\nx-relay-account: a\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
        env.key,
        body.len(),
        body
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut buf = [0u8; 256];
    let n = s.read(&mut buf).await.unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).starts_with("HTTP/1.1 200"));
    let dropped = Instant::now();
    drop(s);
    let deadline = Instant::now() + Duration::from_secs(2);
    while up.closed_at().is_empty() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let closed = up.closed_at();
    assert_eq!(closed.len(), 1, "upstream saw the close");
    // Bounded by one 20 ms chunk interval plus scheduling; P7 measures it.
    assert!(closed[0].duration_since(dropped) < Duration::from_millis(250));
    assert_eq!(account_view(&env, "a").in_flight, 0, "slot released");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saturation_returns_429_after_bounded_wait() {
    let slow: Vec<(Duration, Vec<u8>)> = vec![(Duration::from_millis(800), CODEX_SSE.to_vec())];
    let up = Mock::start(Act::Respond {
        status: 200,
        headers: vec![("content-type".into(), "text/event-stream".into())],
        chunks: slow,
    })
    .await;
    let mut acc = account("a", Provider::Openai, AccountMode::Pooled, &up.url());
    acc.max_in_flight = 1;
    let env = proxy(vec![acc], |c| {
        c.server.queue_wait_ms = 100;
        c.routing.bootstrap_hold_ms = 0;
    });
    let c = client();
    let base = env.base();
    let key = env.key.clone();
    let first = {
        let c = c.clone();
        let base = base.clone();
        let key = key.clone();
        tokio::spawn(async move {
            send(
                &c,
                format!("{base}/v1/responses"),
                &[("authorization", &format!("Bearer {key}"))],
                FIRST_TURN,
            )
            .await
        })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    let t = Instant::now();
    let r = send(
        &c,
        format!("{base}/v1/responses"),
        &[("authorization", &format!("Bearer {key}"))],
        FIRST_TURN,
    )
    .await;
    assert_eq!(r.status, 429);
    assert_eq!(r.headers["retry-after"], "1");
    assert!(t.elapsed() < Duration::from_millis(600));
    assert_eq!(first.await.unwrap().status, 200);
    assert_eq!(up.count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_failure_marks_disconnected_without_retry() {
    let a = Mock::start(Act::json(
        401,
        r#"{"type":"error","error":{"type":"authentication_error"}}"#,
    ))
    .await;
    let b = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    let env = proxy(
        vec![
            account("a", Provider::Anthropic, AccountMode::Pooled, &a.url()),
            account("b", Provider::Anthropic, AccountMode::Pooled, &b.url()),
        ],
        |_| {},
    );
    let c = client();
    let r = send_owned(
        &c,
        &env,
        "/v1/messages",
        &[],
        r#"{"model":"m","messages":[]}"#,
    )
    .await;
    assert_eq!(r.status, 401);
    assert_eq!((a.count(), b.count()), (1, 0));
    let seen = a.seen();
    assert_eq!(seen[0].header("authorization"), Some("Bearer token-a"));
    assert_eq!(seen[0].header("anthropic-beta"), Some("oauth-2025-04-20"));
    let view = account_view(&env, "a");
    assert_eq!(view.state, AccountState::Disconnected);
    assert!(view.state_detail.unwrap().contains("rejected"));
    // A disconnected account is skipped for new work.
    let r = send_owned(
        &c,
        &env,
        "/v1/messages",
        &[],
        r#"{"model":"m","messages":[]}"#,
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(b.count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pause_stops_routing_but_not_observed_clients() {
    let up = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(
        vec![
            account("p", Provider::Openai, AccountMode::Pooled, &up.url()),
            account("o", Provider::Openai, AccountMode::Observed, &up.url()),
        ],
        |_| {},
    );
    env.handle().set_paused(true);
    let c = client();
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.status, 503);
    let key = env.key.clone();
    let r = send(
        &c,
        format!("{}/v1/responses", env.base()),
        &[
            ("authorization", "Bearer login"),
            ("x-relay-key", &key),
            ("x-relay-account", "o"),
        ],
        FIRST_TURN,
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(up.count(), 1);
    std::thread::sleep(Duration::from_millis(700));
    assert!(matches!(
        env.handle().snapshot().routing,
        RoutingStatus::Paused { .. }
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_reload_and_setup_values() {
    let up = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(vec![], |_| {});
    let mut cfg = Config::default();
    cfg.server.listen = "127.0.0.1:0".parse().unwrap();
    cfg.upstreams.allowed.push(up.url());
    let mut a = account("w", Provider::Openai, AccountMode::Observed, &up.url());
    a.credential_kind = Some(CredentialKind::Subscription);
    cfg.accounts.push(a);
    env.handle().apply_config(cfg).unwrap();
    let setup = env.handle().client_setup("w").unwrap();
    assert!(
        setup.entries[0]
            .value
            .contains(&format!("{}/v1", env.base()))
    );
    assert!(!format!("{setup:?}").contains(&env.key));
    // Upstreams outside the allow-list are refused at config time.
    let mut bad = Config::default();
    bad.accounts.push(account(
        "x",
        Provider::Openai,
        AccountMode::Observed,
        "https://example.com",
    ));
    assert!(env.handle().apply_config(bad).is_err());
}

fn write_claude_login(path: &std::path::Path, token: &str, expires_at: i64) {
    std::fs::write(
        path,
        format!(r#"{{"claudeAiOauth":{{"accessToken":"{token}","expiresAt":{expires_at}}}}}"#),
    )
    .unwrap();
}

/// Lifetime of the short-lived test login; long enough for a loaded CI host
/// to start the proxy and serve one request before it expires.
const EXPIRES_IN_MS: i64 = 3_000;

async fn sleep_until_ms(at: i64) {
    let left = at - relay_proxy::state::now_ms();
    if left > 0 {
        tokio::time::sleep(Duration::from_millis(left as u64)).await;
    }
}

const MESSAGES: &str =
    r#"{"model":"claude-sonnet-5-5","messages":[{"role":"user","content":"hi"}]}"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_login_recovers_when_the_cli_renews_it() {
    let up = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    let login_dir = tempfile::tempdir().unwrap();
    let login = login_dir.path().join("credentials.json");
    let now = relay_proxy::state::now_ms();
    write_claude_login(&login, "old-token", now + EXPIRES_IN_MS);
    let path = login.clone();
    let env = proxy(
        vec![account(
            "a",
            Provider::Anthropic,
            AccountMode::Pooled,
            &up.url(),
        )],
        move |c| {
            c.accounts[0].credential = Some(SecretRef::File {
                file: path,
                format: SecretFormat::ClaudeCredentialsJson,
            })
        },
    );
    let c = client();
    assert_eq!(
        send_owned(&c, &env, "/v1/messages", &[], MESSAGES)
            .await
            .status,
        200
    );
    sleep_until_ms(now + EXPIRES_IN_MS + 200).await;
    // Expired: nothing eligible, and no request reaches the provider.
    let r = send_owned(&c, &env, "/v1/messages", &[], MESSAGES).await;
    assert_eq!(r.status, 503);
    assert_eq!(up.count(), 1);
    assert_eq!(account_view(&env, "a").state, AccountState::Disconnected);
    // The CLI refreshes its own login file; Relay picks it up on the next
    // request without a restart or config change.
    write_claude_login(&login, "new-token", now + 3_600_000);
    let r = send_owned(&c, &env, "/v1/messages", &[], MESSAGES).await;
    assert_eq!(r.status, 200);
    assert_eq!(
        up.seen()[1].header("authorization"),
        Some("Bearer new-token")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_token_stays_out_until_replaced() {
    let up = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    up.push(Act::json(
        401,
        r#"{"type":"error","error":{"type":"authentication_error"}}"#,
    ));
    let env = proxy(
        vec![account(
            "a",
            Provider::Anthropic,
            AccountMode::Pooled,
            &up.url(),
        )],
        |_| {},
    );
    let c = client();
    assert_eq!(
        send_owned(&c, &env, "/v1/messages", &[], MESSAGES)
            .await
            .status,
        401
    );
    assert_eq!(up.count(), 1, "same token on reload: no retry");
    // Re-reading the same rejected token does not resurrect it.
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    assert_eq!(
        send_owned(&c, &env, "/v1/messages", &[], MESSAGES)
            .await
            .status,
        503
    );
    assert_eq!(up.count(), 1);
    std::fs::write(env._dir.path().join("a.token"), "token-renewed").unwrap();
    let r = send_owned(&c, &env, "/v1/messages", &[], MESSAGES).await;
    assert_eq!(r.status, 200);
    assert_eq!(
        up.seen()[1].header("authorization"),
        Some("Bearer token-renewed")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forbidden_model_does_not_disconnect_the_account() {
    let up = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    let denied =
        r#"{"type":"error","error":{"type":"permission_error","message":"model not allowed"}}"#;
    up.push(Act::json(403, denied));
    let env = proxy(
        vec![account(
            "a",
            Provider::Anthropic,
            AccountMode::Pooled,
            &up.url(),
        )],
        |_| {},
    );
    let c = client();
    let r = send_owned(
        &c,
        &env,
        "/v1/messages",
        &[],
        r#"{"model":"claude-opus-x","messages":[]}"#,
    )
    .await;
    assert_eq!(r.status, 403);
    assert_eq!(r.body, denied.as_bytes());
    assert_ne!(account_view(&env, "a").state, AccountState::Disconnected);
    let r = send_owned(&c, &env, "/v1/messages", &[], MESSAGES).await;
    assert_eq!(r.status, 200);
    assert_eq!(up.count(), 2);
}

fn cooldown_left(env: &Env, id: &str) -> i64 {
    account_view(env, id).cooldown_until.unwrap_or(0) - relay_proxy::state::now_ms()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_cooldown_per_rejection_and_failures_do_not_reset_backoff() {
    let limit = r#"{"error":{"type":"rate_limit_exceeded"}}"#;
    let failed: &[u8] = b"event: response.created\ndata: {}\n\n\
event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"server_error\"}}}\n\n";
    let up = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    up.push(Act::json(429, limit)); // no retry-after: backoff level 0 → 1 s
    up.push(Act::sse(vec![failed])); // must not reset the backoff
    up.push(Act::json(429, limit)); // level 1 → 2 s
    up.push(Act::json(429, limit).with_header("retry-after", "1")); // header wins
    let env = proxy(
        vec![account(
            "a",
            Provider::Openai,
            AccountMode::Pooled,
            &up.url(),
        )],
        |c| c.routing.bootstrap_hold_ms = 0,
    );
    let c = client();
    assert_eq!(
        send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN)
            .await
            .status,
        429
    );
    let left = cooldown_left(&env, "a");
    assert!(
        left <= 1_000 && left > 0,
        "first backoff is 1 s, got {left}"
    );
    tokio::time::sleep(Duration::from_millis(400)).await;
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.body, failed);
    assert_eq!(
        send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN)
            .await
            .status,
        429
    );
    let left = cooldown_left(&env, "a");
    assert!(
        (1_000..=2_000).contains(&left),
        "second backoff is 2 s, got {left}"
    );
    tokio::time::sleep(Duration::from_millis(left as u64 + 50)).await;
    assert_eq!(
        send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN)
            .await
            .status,
        429
    );
    let left = cooldown_left(&env, "a");
    assert!(
        left <= 1_000 && left > 0,
        "retry-after 1 s is honored, got {left}"
    );

    let mut env = env;
    let db = env.paths.usage_db.clone();
    env.stop();
    let mut reader = relay_usage::Reader::open(&db).unwrap();
    let t = &reader.account_totals(relay_usage::Range::Day).unwrap()[0];
    assert_eq!(
        (t.requests, t.errors),
        (4, 4),
        "response.failed counts as an error"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn snapshot_settles_after_bursts() {
    let up = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    let env = proxy(
        vec![account(
            "a",
            Provider::Anthropic,
            AccountMode::Observed,
            &up.url(),
        )],
        |_| {},
    );
    let c = client();
    let key = env.key.clone();
    let base = env.base();
    for _ in 0..5 {
        let mut tasks = Vec::new();
        for _ in 0..16 {
            let (c, key, base) = (c.clone(), key.clone(), base.clone());
            tasks.push(tokio::spawn(async move {
                send(
                    &c,
                    format!("{base}/v1/messages"),
                    &[("x-relay-key", &key), ("x-relay-account", "a")],
                    MESSAGES,
                )
                .await
                .status
            }));
        }
        for t in tasks {
            assert_eq!(t.await.unwrap(), 200);
        }
    }
    let view = account_view(&env, "a");
    assert_eq!(view.in_flight, 0, "the last change is always published");
    assert_ne!(view.state, AccountState::InUse);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_response_headers_time_out_as_ambiguous() {
    let a = Mock::start(Act::Delay(
        Duration::from_millis(800),
        Box::new(Act::sse(vec![CODEX_SSE])),
    ))
    .await;
    let b = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(
        vec![
            account("a", Provider::Openai, AccountMode::Pooled, &a.url()),
            account("b", Provider::Openai, AccountMode::Pooled, &b.url()),
        ],
        |c| c.server.response_header_timeout_ms = 200,
    );
    let c = client();
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.status, 504);
    assert_eq!(r.headers["x-relay-error"], "upstream_ambiguous");
    assert_eq!((a.count(), b.count()), (1, 0), "sent once, never retried");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_only_settings_are_reported() {
    let env = proxy(vec![], |_| {});
    assert!(env.handle().restart_required().is_empty());
    let mut cfg = Config::default();
    cfg.server.listen = "127.0.0.1:0".parse().unwrap();
    cfg.usage.raw_retention_days = 3;
    env.handle().apply_config(cfg).unwrap();
    assert_eq!(
        env.handle().restart_required(),
        ["Usage recording and retention"]
    );
}

async fn wait_for(cond: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !cond() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(cond(), "condition not reached");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn late_401_for_old_token_does_not_poison_renewed_login() {
    let unauthorized = || {
        Act::json(
            401,
            r#"{"type":"error","error":{"type":"authentication_error"}}"#,
        )
    };
    let up = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    // Both concurrent requests are answered 401 for the old token, one
    // much later than the other.
    up.push(Act::Delay(
        Duration::from_millis(300),
        Box::new(unauthorized()),
    ));
    up.push(Act::Delay(
        Duration::from_millis(1_200),
        Box::new(unauthorized()),
    ));
    let env = proxy(
        vec![account(
            "a",
            Provider::Anthropic,
            AccountMode::Pooled,
            &up.url(),
        )],
        |_| {},
    );
    let c = client();
    let first = {
        let (c, base, key) = (c.clone(), env.base(), env.key.clone());
        tokio::spawn(async move {
            send(
                &c,
                format!("{base}/v1/messages"),
                &[("authorization", &format!("Bearer {key}"))],
                MESSAGES,
            )
            .await
        })
    };
    let second = {
        let (c, base, key) = (c.clone(), env.base(), env.key.clone());
        tokio::spawn(async move {
            send(
                &c,
                format!("{base}/v1/messages"),
                &[("authorization", &format!("Bearer {key}"))],
                MESSAGES,
            )
            .await
        })
    };
    // Both sent with the old token; the CLI renews its login before either
    // 401 arrives.
    wait_for(|| up.count() == 2).await;
    std::fs::write(env._dir.path().join("a.token"), "token-renewed").unwrap();
    let (r1, r2) = (first.await.unwrap(), second.await.unwrap());
    assert_eq!(
        (r1.status, r2.status),
        (200, 200),
        "each retries once with the renewed token"
    );
    let seen = up.seen();
    assert_eq!(seen.len(), 4);
    assert!(
        seen[..2]
            .iter()
            .all(|s| s.header("authorization") == Some("Bearer token-a"))
    );
    assert!(
        seen[2..]
            .iter()
            .all(|s| s.header("authorization") == Some("Bearer token-renewed"))
    );
    // The late 401 did not reject the renewed token.
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    assert_eq!(
        send_owned(&c, &env, "/v1/messages", &[], MESSAGES)
            .await
            .status,
        200
    );
    assert_ne!(account_view(&env, "a").state, AccountState::Disconnected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bound_stateful_session_recovers_its_own_account_login() {
    let up = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    let other = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    let login_dir = tempfile::tempdir().unwrap();
    let login = login_dir.path().join("credentials.json");
    let now = relay_proxy::state::now_ms();
    write_claude_login(&login, "old-token", now + EXPIRES_IN_MS);
    let path = login.clone();
    let env = proxy(
        vec![
            account("a", Provider::Anthropic, AccountMode::Pooled, &up.url()),
            account("b", Provider::Anthropic, AccountMode::Pooled, &other.url()),
        ],
        move |c| {
            c.accounts[0].credential = Some(SecretRef::File {
                file: path,
                format: SecretFormat::ClaudeCredentialsJson,
            })
        },
    );
    let c = client();
    let s = [("x-claude-code-session-id", "s1")];
    assert_eq!(
        send_owned(&c, &env, "/v1/messages", &s, MESSAGES)
            .await
            .status,
        200
    );
    assert_eq!(up.count(), 1, "session bound to a");
    sleep_until_ms(now + EXPIRES_IN_MS + 200).await;
    write_claude_login(&login, "new-token", now + 3_600_000);
    // A continuation with signed thinking can't move; it must recover a's
    // own login without any unrelated request in between.
    let cont = r#"{"model":"claude-sonnet-5-5","messages":[{"role":"assistant","content":[{"type":"thinking","thinking":"x","signature":"sig"}]},{"role":"user","content":"go on"}]}"#;
    let r = send_owned(&c, &env, "/v1/messages", &s, cont).await;
    assert_eq!(r.status, 200);
    assert_eq!((up.count(), other.count()), (2, 0));
    assert_eq!(
        up.seen()[1].header("authorization"),
        Some("Bearer new-token")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_account_after_failover_still_reloads_on_401() {
    let a = Mock::start(Act::json(
        429,
        r#"{"error":{"type":"rate_limit_exceeded"}}"#,
    ))
    .await;
    let b = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    b.push(Act::Delay(
        Duration::from_millis(300),
        Box::new(Act::json(401, r#"{"error":{"type":"invalid_api_key"}}"#)),
    ));
    let env = proxy(
        vec![
            account("a", Provider::Openai, AccountMode::Pooled, &a.url()),
            account("b", Provider::Openai, AccountMode::Pooled, &b.url()),
        ],
        |_| {},
    );
    let c = client();
    let req = {
        let (c, base, key) = (c.clone(), env.base(), env.key.clone());
        tokio::spawn(async move {
            send(
                &c,
                format!("{base}/v1/responses"),
                &[("authorization", &format!("Bearer {key}"))],
                FIRST_TURN,
            )
            .await
        })
    };
    wait_for(|| b.count() == 1).await;
    std::fs::write(env._dir.path().join("b.token"), "token-b2").unwrap();
    let r = req.await.unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(b.count(), 2);
    assert_eq!(b.seen()[1].header("authorization"), Some("Bearer token-b2"));
}

// ---- Vendors that share a wire: Kimi, xAI, Meta ----

fn vendor_account(
    id: &str,
    provider: Provider,
    vendor: Vendor,
    mode: AccountMode,
    upstream: &str,
) -> AccountConfig {
    let mut a = account(id, provider, mode, upstream);
    a.vendor = Some(vendor);
    if vendor.needs_device_id() {
        a.device_id = Some(format!("device-{id}"));
    }
    a
}

const CLAUDE_BODY: &str =
    r#"{"model":"kimi-k2","stream":true,"messages":[{"role":"user","content":"hi"}]}"#;

fn assert_no_relay_headers(s: &mock::Seen) {
    assert!(
        s.headers
            .iter()
            .all(|(k, _)| !k.to_ascii_lowercase().starts_with("x-relay-")),
        "relay headers leaked: {:?}",
        s.headers.iter().map(|(k, _)| k).collect::<Vec<_>>()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kimi_serves_both_wires_with_its_paths_and_honest_headers() {
    // Kimi answers with first-party-looking quota headers; they must not be
    // read as Kimi quota.
    let up = Mock::start(
        Act::sse(vec![CLAUDE_SSE])
            .with_header("anthropic-ratelimit-unified-5h-utilization", "0.25")
            .with_header("anthropic-ratelimit-unified-5h-reset", "4102444800"),
    )
    .await;
    let env = proxy(
        vec![
            vendor_account(
                "kimi-msg",
                Provider::Anthropic,
                Vendor::KimiCom,
                AccountMode::Pooled,
                &format!("{}/coding", up.url()),
            ),
            vendor_account(
                "kimi-resp",
                Provider::Openai,
                Vendor::KimiCom,
                AccountMode::Pooled,
                &format!("{}/coding/v1", up.url()),
            ),
        ],
        |_| {},
    );
    let c = client();
    let kimi = [
        ("x-relay-vendor", "kimi-com"),
        ("anthropic-version", "2023-06-01"),
    ];
    let r = send_owned(&c, &env, "/v1/messages?beta=true", &kimi, CLAUDE_BODY).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body, CLAUDE_SSE, "stream bytes unchanged");
    let r = send_owned(&c, &env, "/v1/messages/count_tokens", &kimi, CLAUDE_BODY).await;
    assert_eq!(r.status, 200);
    let r = send_owned(&c, &env, "/v1/responses", &kimi[..1], FIRST_TURN).await;
    assert_eq!(r.status, 200);
    let seen = up.seen();
    assert_eq!(
        seen.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(),
        [
            "/coding/v1/messages?beta=true",
            "/coding/v1/messages/count_tokens",
            "/coding/v1/responses"
        ]
    );
    for (s, id) in seen.iter().zip(["kimi-msg", "kimi-msg", "kimi-resp"]) {
        assert_eq!(
            s.header("authorization"),
            Some(format!("Bearer token-{id}").as_str())
        );
        assert_eq!(
            s.header("x-msh-device-id"),
            Some(format!("device-{id}").as_str())
        );
        assert_eq!(s.header("x-msh-platform"), Some("Relay"));
        assert_eq!(s.header("x-msh-version"), Some(relay_core::DISPLAY_VERSION));
        // Claude-only OAuth capability flag is never added for Kimi.
        assert!(s.header("anthropic-beta").is_none());
        assert!(s.header("x-api-key").is_none());
        assert_no_relay_headers(s);
    }
    // Endpoints Kimi isn't known to serve are refused before any send.
    let r = send_owned(&c, &env, "/v1/responses/compact", &kimi[..1], FIRST_TURN).await;
    assert_eq!(r.status, 404);
    assert_eq!(up.count(), 3);
    // Quota is "not reported", never borrowed from Anthropic header names.
    let v = account_view(&env, "kimi-msg");
    assert_eq!(v.vendor, Vendor::KimiCom);
    assert!(v.windows.is_empty());
    let snap = env.handle().snapshot();
    assert!(snap.providers.iter().all(|p| p.vendor == Vendor::KimiCom));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vendors_are_isolated_for_default_pins_and_sessions() {
    let kimi = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    let claude = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    // Kimi first: fill-first would pick it if vendors were not filtered.
    let env = proxy(
        vec![
            vendor_account(
                "kimi",
                Provider::Anthropic,
                Vendor::KimiCom,
                AccountMode::Pooled,
                &kimi.url(),
            ),
            account(
                "claude",
                Provider::Anthropic,
                AccountMode::Pooled,
                &claude.url(),
            ),
        ],
        |_| {},
    );
    let c = client();
    let s = ("x-claude-code-session-id", "shared-session");
    let kv = ("x-relay-vendor", "kimi-com");
    // No vendor header: first-party only, also for an existing client.
    let r = send_owned(&c, &env, "/v1/messages", &[s], CLAUDE_BODY).await;
    assert_eq!(r.status, 200);
    assert_eq!((claude.count(), kimi.count()), (1, 0));
    // Same session id on Kimi binds separately.
    let r = send_owned(&c, &env, "/v1/messages", &[s, kv], CLAUDE_BODY).await;
    assert_eq!(r.status, 200);
    assert_eq!((claude.count(), kimi.count()), (1, 1));
    let r = send_owned(&c, &env, "/v1/messages", &[s], CLAUDE_BODY).await;
    assert_eq!(r.status, 200);
    assert_eq!(
        (claude.count(), kimi.count()),
        (2, 1),
        "session stays on its vendor"
    );
    // Cross-vendor pins are refused in both directions, even when the
    // vendor is the first-party default.
    let pin_kimi = ("x-relay-account", "kimi");
    let pin_claude = ("x-relay-account", "claude");
    for h in [vec![pin_kimi], vec![pin_claude, kv]] {
        let r = send_owned(&c, &env, "/v1/messages", &h, CLAUDE_BODY).await;
        assert_eq!(r.status, 403, "{h:?}");
    }
    // A pin that agrees with the vendor works.
    let r = send_owned(&c, &env, "/v1/messages", &[pin_kimi, kv], CLAUDE_BODY).await;
    assert_eq!(r.status, 200);
    // Unknown vendors and wires a vendor doesn't speak are refused.
    let r = send_owned(
        &c,
        &env,
        "/v1/messages",
        &[("x-relay-vendor", "kimi")],
        CLAUDE_BODY,
    )
    .await;
    assert_eq!(r.status, 400);
    let r = send_owned(
        &c,
        &env,
        "/v1/messages",
        &[("x-relay-vendor", "xai")],
        CLAUDE_BODY,
    )
    .await;
    assert_eq!(r.status, 404);
    assert_eq!((claude.count(), kimi.count()), (2, 2));
    // Snapshot sections never combine vendors.
    let v = account_view(&env, "claude");
    assert_eq!(v.vendor, Vendor::Anthropic);
    let snap = env.handle().snapshot();
    let sections: Vec<_> = snap
        .providers
        .iter()
        .map(|p| (p.provider, p.vendor))
        .collect();
    assert_eq!(
        sections,
        [
            (Provider::Anthropic, Vendor::Anthropic),
            (Provider::Anthropic, Vendor::KimiCom)
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn observed_kimi_forwards_client_login_only_with_matching_vendor() {
    let up = Mock::start(Act::sse(vec![CLAUDE_SSE])).await;
    let mut a = vendor_account(
        "kimi-cli",
        Provider::Anthropic,
        Vendor::KimiAi,
        AccountMode::Observed,
        &up.url(),
    );
    a.device_id = None;
    let env = proxy(vec![a], |_| {});
    let c = client();
    let key = env.key.clone();
    let base = [
        ("authorization", "Bearer kimi-user-token"),
        ("x-msh-device-id", "client-device"),
        ("x-relay-key", key.as_str()),
        ("x-relay-account", "kimi-cli"),
    ];
    let url = format!("{}/v1/messages", env.base());
    let r = send(&c, url.clone(), &base, CLAUDE_BODY).await;
    assert_eq!(r.status, 403, "pin without x-relay-vendor");
    assert_eq!(up.count(), 0);
    let mut h = base.to_vec();
    h.push(("x-relay-vendor", "kimi-ai"));
    let r = send(&c, url, &h, CLAUDE_BODY).await;
    assert_eq!(r.status, 200);
    let s = &up.seen()[0];
    assert_eq!(s.header("authorization"), Some("Bearer kimi-user-token"));
    assert_eq!(s.header("x-msh-device-id"), Some("client-device"));
    assert_no_relay_headers(s);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xai_and_meta_get_bearer_only_and_never_fail_over_across_vendors() {
    let xai =
        Mock::start(Act::json(429, r#"{"error":"rate_limited"}"#).with_header("retry-after", "30"))
            .await;
    let meta = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let openai = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(
        vec![
            vendor_account(
                "xai",
                Provider::Openai,
                Vendor::Xai,
                AccountMode::Key,
                &xai.url(),
            ),
            vendor_account(
                "meta",
                Provider::Openai,
                Vendor::Meta,
                AccountMode::Key,
                &meta.url(),
            ),
            account("openai", Provider::Openai, AccountMode::Key, &openai.url()),
        ],
        |c| c.routing.default_pool = Pool::Key,
    );
    let c = client();
    let r = send_owned(
        &c,
        &env,
        "/v1/responses",
        &[("x-relay-vendor", "xai")],
        FIRST_TURN,
    )
    .await;
    assert_ne!(
        r.status, 200,
        "xAI's rejection is not hidden by another vendor"
    );
    assert_eq!((xai.count(), meta.count(), openai.count()), (1, 0, 0));
    let s = &xai.seen()[0];
    assert_eq!(s.path, "/responses");
    assert_eq!(s.header("authorization"), Some("Bearer token-xai"));
    assert!(s.header("chatgpt-account-id").is_none());
    assert!(
        s.headers
            .iter()
            .all(|(k, _)| !k.starts_with("x-msh-") && !k.starts_with("x-grok"))
    );
    assert_no_relay_headers(s);
    // The cooled-down xAI account isn't replaced by Meta or OpenAI.
    let r = send_owned(
        &c,
        &env,
        "/v1/responses",
        &[("x-relay-vendor", "xai")],
        FIRST_TURN,
    )
    .await;
    assert_eq!(r.status, 503);
    assert_eq!((meta.count(), openai.count()), (0, 0));

    let r = send_owned(
        &c,
        &env,
        "/v1/responses",
        &[("x-relay-vendor", "meta")],
        FIRST_TURN,
    )
    .await;
    assert_eq!(r.status, 200);
    let s = &meta.seen()[0];
    assert_eq!(s.header("authorization"), Some("Bearer token-meta"));
    assert!(s.header("x-client-id").is_none());
    assert_no_relay_headers(s);
    // Meta has no compact route; refused before sending.
    let r = send_owned(
        &c,
        &env,
        "/v1/responses/compact",
        &[("x-relay-vendor", "meta")],
        FIRST_TURN,
    )
    .await;
    assert_eq!(r.status, 404);
    assert_eq!(meta.count(), 1);
    // First-party default is untouched.
    let r = send_owned(&c, &env, "/v1/responses", &[], FIRST_TURN).await;
    assert_eq!(r.status, 200);
    assert_eq!(openai.count(), 1);
    assert_eq!(
        openai.seen()[0].header("authorization"),
        Some("Bearer token-openai")
    );
    assert!(account_view(&env, "xai").windows.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xai_ambiguous_send_is_never_retried_on_another_account() {
    let a = Mock::start(Act::Reset).await;
    let b = Mock::start(Act::sse(vec![CODEX_SSE])).await;
    let env = proxy(
        vec![
            vendor_account(
                "xa",
                Provider::Openai,
                Vendor::Xai,
                AccountMode::Pooled,
                &a.url(),
            ),
            vendor_account(
                "xb",
                Provider::Openai,
                Vendor::Xai,
                AccountMode::Pooled,
                &b.url(),
            ),
        ],
        |_| {},
    );
    let c = client();
    let r = send_owned(
        &c,
        &env,
        "/v1/responses/compact",
        &[("x-relay-vendor", "xai")],
        FIRST_TURN,
    )
    .await;
    assert_eq!(r.status, 502);
    assert_eq!(r.headers["x-relay-error"], "upstream_ambiguous");
    assert_eq!((a.count(), b.count()), (1, 0));
    assert_eq!(a.seen()[0].path, "/responses/compact");
}
