//! Request handling: authenticate, inspect, place, forward, classify.

use crate::body::{BoxError, InFlight, Record, RespBody, TapBody};
use crate::secrets::key_matches;
use crate::state::{AccountRt, Registry, Shared, now_ms};
use bytes::Bytes;
use http::header::{self, HeaderMap, HeaderValue};
use http::{Method, Request, Response, StatusCode, Uri};
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::Client as HyperClient;
use hyper_util::client::legacy::connect::HttpConnector;
use relay_core::attempt::{
    Decision, MAX_ACCOUNTS_PER_REQUEST, Outcome, RETRY_BUDGET_MS, RequestTraits, decide,
};
use relay_core::model::{AccountId, AccountMode, Pool, Provider};
use relay_core::select::{Candidate, select};
use relay_providers::auth::Credential;
use relay_providers::headers::{
    RELAY_ACCOUNT, RELAY_KEY, RELAY_POOL, downstream_response, upstream_request,
};
use relay_providers::request::{Endpoint, RequestInfo, inspect, session_id};
use relay_providers::tap::{Decisive, Tap, error_body};
use relay_providers::{auth, quota};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

type Https = HttpsConnector<HttpConnector>;

/// Bounds the whole connection setup (TCP connect and TLS handshake). A
/// timeout here is a connect error, so the request is known not sent.
#[derive(Clone)]
pub struct TimedConnector {
    inner: Https,
    timeout: Duration,
}

impl tower_service::Service<Uri> for TimedConnector {
    type Response = <Https as tower_service::Service<Uri>>::Response;
    type Error = BoxError;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, BoxError>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), BoxError>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        let fut = self.inner.call(uri);
        let timeout = self.timeout;
        Box::pin(async move {
            match tokio::time::timeout(timeout, fut).await {
                Ok(r) => r,
                Err(_) => Err("upstream connect (TCP and TLS) timed out".into()),
            }
        })
    }
}

/// Upstream HTTP client with deadlines.
#[derive(Clone)]
pub struct Client {
    inner: HyperClient<TimedConnector, Full<Bytes>>,
    /// Longest wait for response headers once the request is sent.
    header_timeout: Duration,
}

/// Builds the pooled upstream client (rustls, HTTP/1.1 and HTTP/2 via ALPN).
pub fn client(connect_timeout: Duration, header_timeout: Duration) -> Client {
    let mut http = HttpConnector::new();
    http.enforce_http(false);
    http.set_nodelay(true);
    http.set_connect_timeout(Some(connect_timeout));
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_provider_and_webpki_roots(Arc::new(rustls::crypto::ring::default_provider()))
        .expect("ring provider supports default protocol versions")
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .wrap_connector(http);
    Client {
        inner: HyperClient::builder(hyper_util::rt::TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(90))
            .build(TimedConnector {
                inner: https,
                timeout: connect_timeout,
            }),
        header_timeout,
    }
}

/// Bootstrap window limits for in-stream rejection failover.
const HOLD_FRAMES: usize = 16;
const HOLD_BYTES: usize = 256 * 1024;
/// Error bodies buffered for classification.
const ERROR_BODY: usize = 256 * 1024;

fn json_error(
    provider: Option<Provider>,
    status: StatusCode,
    kind: &str,
    message: &str,
) -> Response<RespBody> {
    let body = match provider {
        Some(Provider::Openai) => serde_json::json!({
            "error": { "type": kind, "code": kind, "message": message }
        }),
        _ => serde_json::json!({
            "type": "error",
            "error": { "type": kind, "message": message }
        }),
    };
    let mut r = Response::new(RespBody::full(body.to_string()));
    *r.status_mut() = status;
    let h = r.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(v) = HeaderValue::try_from(kind) {
        h.insert("x-relay-error", v);
    }
    r
}

fn header_str<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name)?
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

fn bearer(h: &HeaderMap) -> Option<&[u8]> {
    let v = h.get(header::AUTHORIZATION)?.as_bytes();
    v.strip_prefix(b"Bearer ")
        .or_else(|| v.strip_prefix(b"bearer "))
}

/// How the client authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Auth {
    /// Local key as the API credential: Relay chooses the account.
    Owned,
    /// `x-relay-key` with the client's own credential: pass-through.
    Pass,
}

fn authenticate(sh: &Shared, h: &HeaderMap) -> Option<Auth> {
    let key = &sh.local_key;
    let owned = bearer(h).is_some_and(|b| key_matches(key, b))
        || h.get("x-api-key")
            .is_some_and(|v| key_matches(key, v.as_bytes()));
    if owned {
        return Some(Auth::Owned);
    }
    h.get(RELAY_KEY)
        .is_some_and(|v| key_matches(key, v.as_bytes()))
        .then_some(Auth::Pass)
}

/// Per-request context.
struct Ctx {
    sh: Arc<Shared>,
    reg: Arc<Registry>,
    client: Client,
    endpoint: Endpoint,
    provider: Provider,
    method: Method,
    query: Option<String>,
    headers: HeaderMap,
    body: Bytes,
    info: RequestInfo,
    started: Instant,
}

impl Ctx {
    fn traits(&self) -> RequestTraits {
        RequestTraits {
            stateless: !self.info.stateful,
        }
    }

    fn err(&self, status: StatusCode, kind: &str, msg: &str) -> Response<RespBody> {
        json_error(Some(self.provider), status, kind, msg)
    }
}

/// Entry point for every request.
pub async fn handle(sh: Arc<Shared>, client: Client, req: Request<Incoming>) -> Response<RespBody> {
    let started = Instant::now();
    let (parts, body) = req.into_parts();
    let path = parts.uri.path();
    if path == "/api/hello" && (parts.method == Method::HEAD || parts.method == Method::GET) {
        return Response::new(RespBody::empty());
    }
    if path == "/relay/health" && parts.method == Method::GET {
        let mut r = Response::new(RespBody::full(
            serde_json::json!({ "status": "ok", "version": relay_core::DISPLAY_VERSION })
                .to_string(),
        ));
        r.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        return r;
    }
    let Some((endpoint, hint)) = Endpoint::from_path(path) else {
        return json_error(
            None,
            StatusCode::NOT_FOUND,
            "not_found_error",
            "Relay serves /v1/messages, /v1/messages/count_tokens, /v1/responses, /v1/responses/compact and /v1/models",
        );
    };
    let expected = if endpoint == Endpoint::Models {
        Method::GET
    } else {
        Method::POST
    };
    if parts.method != expected {
        return json_error(
            hint,
            StatusCode::METHOD_NOT_ALLOWED,
            "invalid_request_error",
            "method not allowed",
        );
    }
    let Some(auth) = authenticate(&sh, &parts.headers) else {
        return json_error(
            hint,
            StatusCode::UNAUTHORIZED,
            "authentication_error",
            "Missing or wrong Relay local key",
        );
    };
    let reg = sh.registry.load_full();
    let named = header_str(&parts.headers, RELAY_ACCOUNT).map(str::to_string);
    let provider = hint
        .or_else(|| {
            named
                .as_deref()
                .and_then(|n| reg.account(n))
                .map(|a| a.cfg.provider)
        })
        .unwrap_or(if parts.headers.contains_key("anthropic-version") {
            Provider::Anthropic
        } else {
            Provider::Openai
        });

    let limit = reg.config.server.max_body_bytes;
    let body = match Limited::new(body, limit).collect().await {
        Ok(c) => c.to_bytes(),
        Err(e)
            if e.downcast_ref::<http_body_util::LengthLimitError>()
                .is_some() =>
        {
            return json_error(
                Some(provider),
                StatusCode::PAYLOAD_TOO_LARGE,
                "request_too_large",
                "Request body exceeds Relay's server.max_body_bytes",
            );
        }
        Err(_) => {
            return json_error(
                Some(provider),
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "Could not read request body",
            );
        }
    };
    let info = if parts.method == Method::POST {
        inspect(provider, &body)
    } else {
        RequestInfo::default()
    };
    let ctx = Ctx {
        sh,
        reg,
        client,
        endpoint,
        provider,
        method: parts.method,
        query: parts.uri.query().map(str::to_string),
        headers: parts.headers,
        body,
        info,
        started,
    };
    match auth {
        Auth::Pass => pass_through(&ctx, named.as_deref()).await,
        Auth::Owned => owned(&ctx, named.as_deref()).await,
    }
}

async fn pass_through(ctx: &Ctx, named: Option<&str>) -> Response<RespBody> {
    let Some(name) = named else {
        return ctx.err(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "Observed clients must send x-relay-account with their Relay account id",
        );
    };
    let Some(acc) = ctx.reg.account(name).cloned() else {
        return ctx.err(
            StatusCode::FORBIDDEN,
            "permission_error",
            "x-relay-account names no configured account",
        );
    };
    if acc.cfg.provider != ctx.provider || acc.cfg.mode != AccountMode::Observed {
        return ctx.err(
            StatusCode::FORBIDDEN,
            "permission_error",
            "x-relay-account must name an observed account of this provider",
        );
    }
    let mut attempt: u8 = 0;
    loop {
        let permit = acc.sem.clone().try_acquire_owned().ok();
        // Observed traffic is never refused for saturation: the client
        // already chose its account, so the slot count is advisory.
        match attempt_once(ctx, &acc, attempt, false, permit).await {
            Attempt::Commit(r) => return r,
            Attempt::Failed {
                outcome, response, ..
            } => {
                if decide(outcome, ctx.traits(), false, attempt) == Decision::RetrySame {
                    attempt += 1;
                    continue;
                }
                return response;
            }
        }
    }
}

/// Pool from `x-relay-pool`, else the configured default. `None` when the
/// header names no pool.
fn pool_of(ctx: &Ctx) -> Option<Pool> {
    match header_str(&ctx.headers, RELAY_POOL) {
        None => Some(ctx.reg.config.routing.default_pool),
        Some(v) => Pool::from_key(v),
    }
}

/// Re-reads an account's credential source if due (single-flight; the read
/// runs on the blocking pool). Returns whether the credential is usable.
async fn refresh(acc: Arc<AccountRt>) -> bool {
    let _guard = acc.reloading.lock().await;
    let a = acc.clone();
    tokio::task::spawn_blocking(move || a.refresh_credential_blocking())
        .await
        .unwrap_or(false)
}

/// Bound on waiting for credential refreshes when nothing else is eligible.
const REFRESH_WAIT: Duration = Duration::from_secs(2);

/// [`pick`], plus credential recovery: accounts whose credential is due a
/// refresh are refreshed in the background, so a CLI that renewed its
/// login is picked up without user action. The request waits for those
/// refreshes (bounded) only when no account is eligible without them.
async fn pick_recovering(
    ctx: &Ctx,
    pool: Pool,
    tried: &[AccountId],
    now: i64,
) -> Option<Arc<AccountRt>> {
    let due: Vec<_> = ctx
        .reg
        .pool(ctx.provider, pool)
        .filter(|a| !tried.contains(a.id()) && a.needs_refresh(now))
        .map(|a| tokio::spawn(refresh(a.clone())))
        .collect();
    if let Some(a) = pick(ctx, pool, tried, now) {
        return Some(a);
    }
    if due.is_empty() {
        return None;
    }
    let _ = tokio::time::timeout(REFRESH_WAIT, async {
        for h in due {
            let _ = h.await;
        }
    })
    .await;
    pick(ctx, pool, tried, now_ms())
}

/// Picks the next account for a router-owned request.
fn pick(ctx: &Ctx, pool: Pool, tried: &[AccountId], now: i64) -> Option<Arc<AccountRt>> {
    let model = ctx.info.model.as_deref();
    let accounts: Vec<&Arc<AccountRt>> = ctx
        .reg
        .pool(ctx.provider, pool)
        .filter(|a| !tried.contains(a.id()))
        .collect();
    let routing = &ctx.reg.config.routing;
    let cursor = ctx.sh.cursor.fetch_add(1, Ordering::Relaxed);
    // Prefer accounts with a free slot; if all are saturated, pick anyway
    // and let the bounded admission wait decide.
    [true, false].into_iter().find_map(|need_slot| {
        let cands: Vec<Candidate> = accounts
            .iter()
            .map(|a| a.candidate(model, now, need_slot))
            .collect();
        select(
            &cands,
            routing.strategy,
            routing.skip_below_pct / 100.0,
            cursor,
        )
        .map(|i| accounts[i].clone())
    })
}

async fn owned(ctx: &Ctx, named: Option<&str>) -> Response<RespBody> {
    if ctx.sh.paused.load(Ordering::Relaxed) {
        return ctx.err(
            StatusCode::SERVICE_UNAVAILABLE,
            "relay_paused",
            "Relay routing is paused",
        );
    }
    let Some(pool) = pool_of(ctx) else {
        return ctx.err(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "x-relay-pool must be pooled or key",
        );
    };
    let routing = &ctx.reg.config.routing;
    let session = routing
        .sticky
        .then(|| session_id(ctx.provider, &ctx.headers))
        .flatten()
        .map(str::to_string);
    let ttl = Duration::from_secs(u64::from(routing.session_ttl_minutes) * 60);
    let pinned = match named {
        Some(n) => match ctx.reg.account(n) {
            Some(a) if a.cfg.provider == ctx.provider && a.cfg.mode == pool.mode() => {
                Some(a.clone())
            }
            _ => {
                return ctx.err(
                    StatusCode::FORBIDDEN,
                    "permission_error",
                    "x-relay-account must name an account in the selected pool",
                );
            }
        },
        None => None,
    };
    let traits = ctx.traits();
    let mut tried: Vec<AccountId> = Vec::new();
    let mut attempt: u8 = 0;
    let mut last: Option<Response<RespBody>> = None;
    let mut retry_same: Option<Arc<AccountRt>> = None;
    loop {
        let now = now_ms();
        if attempt > 0 && ctx.started.elapsed() > Duration::from_millis(RETRY_BUDGET_MS) {
            break;
        }
        let acc = if let Some(a) = retry_same.take() {
            a
        } else if tried.len() >= MAX_ACCOUNTS_PER_REQUEST {
            break;
        } else if let Some(p) = &pinned {
            if !tried.is_empty() {
                break;
            }
            p.clone()
        } else {
            let bound = session
                .as_deref()
                .filter(|_| tried.is_empty())
                .and_then(|s| ctx.sh.sessions.get(ctx.provider, s, ttl))
                .and_then(|id| ctx.reg.account(id.as_str()).cloned())
                .filter(|a| a.cfg.mode == pool.mode() && a.cfg.provider == ctx.provider);
            match bound {
                Some(b) if b.candidate(ctx.info.model.as_deref(), now, false).eligible => b,
                // The bound account can't serve, and the request may move.
                Some(_) if traits.stateless => {
                    match pick_recovering(ctx, pool, &tried, now).await {
                        Some(a) => a,
                        None => break,
                    }
                }
                // Continuation state is bound to the account: send it there
                // and let the provider answer, unless it can't be used at all.
                Some(b) if !b.cfg.paused && b.credential_ok(now) => b,
                Some(_) => {
                    return ctx.err(StatusCode::SERVICE_UNAVAILABLE, "session_account_unavailable", "This session's account is paused or disconnected, and its conversation state cannot move to another account");
                }
                None => match pick_recovering(ctx, pool, &tried, now).await {
                    Some(a) => a,
                    None => break,
                },
            }
        };
        if !tried.contains(acc.id()) {
            tried.push(acc.id().clone());
        }
        // Admission: bounded wait for a slot on the chosen account.
        let permit = match acc.sem.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                let wait = Duration::from_millis(ctx.reg.config.server.queue_wait_ms);
                match tokio::time::timeout(wait, acc.sem.clone().acquire_owned()).await {
                    Ok(Ok(p)) => p,
                    _ => {
                        let mut r = ctx.err(
                            StatusCode::TOO_MANY_REQUESTS,
                            "relay_saturated",
                            "All slots for the chosen account are busy",
                        );
                        r.headers_mut()
                            .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
                        return r;
                    }
                }
            }
        };
        let usable = if acc.needs_refresh(now_ms()) {
            refresh(acc.clone()).await
        } else {
            acc.credential_ok(now_ms())
        };
        if !usable {
            drop(permit);
            if pinned.is_none() && traits.stateless {
                // Credential vanished between selection and use; nothing has
                // been sent, so another account may take it.
                continue;
            }
            return ctx.err(
                StatusCode::SERVICE_UNAVAILABLE,
                "account_disconnected",
                "The account's credential is unavailable",
            );
        }
        match attempt_once(ctx, &acc, attempt, true, Some(permit)).await {
            Attempt::Commit(r) => {
                if let Some(s) = &session {
                    ctx.sh.sessions.bind(ctx.provider, s, acc.id());
                    ctx.sh.changed();
                }
                return r;
            }
            Attempt::Failed {
                outcome,
                response,
                auth_retry,
            } => {
                if auth_retry {
                    retry_same = Some(acc);
                    attempt += 1;
                    continue;
                }
                match decide(outcome, traits, true, attempt) {
                    Decision::RetrySame => retry_same = Some(acc),
                    Decision::Failover if pinned.is_none() => {}
                    _ => return response,
                }
                last = Some(response);
                attempt += 1;
            }
        }
    }
    last.unwrap_or_else(|| {
        let mut r = ctx.err(StatusCode::SERVICE_UNAVAILABLE, "no_eligible_account", "No account in this pool can take the request now (paused, cooling down, exhausted, disconnected or not allowed for this model). Relay never falls back to another pool.");
        let soonest = ctx
            .reg
            .pool(ctx.provider, pool)
            .map(|a| a.cooldown_until.load(Ordering::Relaxed))
            .filter(|c| *c > now_ms())
            .min();
        if let Some(c) = soonest
            && let Ok(v) = HeaderValue::try_from(((c - now_ms()) / 1000 + 1).to_string())
        {
            r.headers_mut().insert(header::RETRY_AFTER, v);
        }
        r
    })
}

/// After a 401: re-reads the credential source once (single-flight).
/// Returns true when it now holds a usable token different from `before`,
/// i.e. the CLI renewed its login and the request may be retried once.
async fn reload_after_401(acc: &Arc<AccountRt>, before: Option<Arc<Credential>>) -> bool {
    let _guard = acc.reloading.lock().await;
    let current = acc.credential.load_full();
    let differs = |c: &Option<Arc<Credential>>| match (&before, c) {
        (Some(a), Some(b)) => a.token != b.token,
        (None, Some(_)) => true,
        _ => false,
    };
    if differs(&current) && acc.credential_ok(now_ms()) {
        return true;
    }
    let a = acc.clone();
    let ok = tokio::task::spawn_blocking(move || a.load_credential())
        .await
        .unwrap_or(false);
    ok && differs(&acc.credential.load_full())
}

enum Attempt {
    Commit(Response<RespBody>),
    Failed {
        outcome: Outcome,
        response: Response<RespBody>,
        /// 401 on a router-owned account whose credential changed on reload.
        auth_retry: bool,
    },
}

fn upstream_uri(ctx: &Ctx, acc: &AccountRt) -> Option<Uri> {
    let mut s = String::with_capacity(96);
    s.push_str(acc.cfg.upstream_base());
    s.push_str(ctx.endpoint.upstream_path(ctx.provider));
    if let Some(q) = &ctx.query {
        s.push('?');
        s.push_str(q);
    }
    s.parse().ok()
}

fn is_event_stream(h: &HeaderMap) -> bool {
    h.get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/event-stream"))
}

async fn attempt_once(
    ctx: &Ctx,
    acc: &Arc<AccountRt>,
    attempt: u8,
    owned: bool,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
) -> Attempt {
    let flight = InFlight::new(ctx.sh.clone(), acc.clone(), permit);
    let record = |status: u16, streaming: bool| Record {
        flight,
        status,
        started: ctx.started,
        attempt,
        model: ctx.info.model.clone(),
        record: ctx.endpoint.generates(),
        generating: ctx.endpoint.generates(),
        streaming,
    };
    if let Some(m) = &ctx.info.model {
        acc.saw_model(m);
    }
    let Some(uri) = upstream_uri(ctx, acc) else {
        let response = ctx.err(
            StatusCode::BAD_GATEWAY,
            "relay_config_error",
            "Account upstream URL is invalid",
        );
        record(0, false).emit(Outcome::NotSent, None);
        return Attempt::Failed {
            outcome: Outcome::ClientError,
            response,
            auth_retry: false,
        };
    };
    let mut headers = ctx.headers.clone();
    upstream_request(&mut headers);
    if owned {
        let cred = acc.credential.load_full();
        let injected = cred
            .as_deref()
            .map(|c| auth::inject(acc.cfg.provider, acc.cfg.credential_kind(), &mut headers, c));
        if !matches!(injected, Some(Ok(()))) {
            let response = ctx.err(
                StatusCode::SERVICE_UNAVAILABLE,
                "account_disconnected",
                "The account's credential is unavailable",
            );
            return Attempt::Failed {
                outcome: Outcome::AuthError,
                response,
                auth_retry: false,
            };
        }
    }
    let mut req = Request::new(Full::new(ctx.body.clone()));
    *req.method_mut() = ctx.method.clone();
    *req.uri_mut() = uri;
    *req.headers_mut() = headers;

    let sent = tokio::time::timeout(ctx.client.header_timeout, ctx.client.inner.request(req)).await;
    let resp = match sent {
        Ok(Ok(r)) => r,
        Err(_) => {
            // Sent, but no response headers in time: it may be running.
            record(0, false).emit(Outcome::Ambiguous, None);
            let mut response = ctx.err(StatusCode::GATEWAY_TIMEOUT, "upstream_ambiguous", "The upstream did not answer in time after the request was sent; it may have run. Relay did not retry it.");
            response
                .headers_mut()
                .insert("x-should-retry", HeaderValue::from_static("true"));
            return Attempt::Failed {
                outcome: Outcome::Ambiguous,
                response,
                auth_retry: false,
            };
        }
        Ok(Err(e)) => {
            // Only a connect failure (including the TLS handshake) proves
            // the request never left.
            let outcome = if e.is_connect() {
                Outcome::NotSent
            } else {
                Outcome::Ambiguous
            };
            record(0, false).emit(outcome, None);
            let mut response = if outcome == Outcome::NotSent {
                ctx.err(
                    StatusCode::BAD_GATEWAY,
                    "upstream_unreachable",
                    "Relay could not connect to the upstream",
                )
            } else {
                ctx.err(StatusCode::BAD_GATEWAY, "upstream_ambiguous", "The upstream connection failed after the request was sent; it may have run. Relay did not retry it.")
            };
            response
                .headers_mut()
                .insert("x-should-retry", HeaderValue::from_static("true"));
            return Attempt::Failed {
                outcome,
                response,
                auth_retry: false,
            };
        }
    };
    let now = now_ms();
    let update = quota::parse(acc.cfg.provider, resp.headers(), now);
    if acc.observe(&update, now) {
        ctx.sh.changed();
    }
    let status = resp.status();
    let streaming = is_event_stream(resp.headers());
    let (mut parts, mut body) = resp.into_parts();
    downstream_response(&mut parts.headers);
    let mut tap = Tap::new(acc.cfg.provider, streaming);

    if !status.is_success() {
        let outcome = Outcome::from_status(status.as_u16());
        // Buffer small error bodies to classify them and to allow failover.
        let mut held = VecDeque::new();
        let mut size = 0usize;
        let mut complete = false;
        loop {
            match body.frame().await {
                Some(Ok(f)) => {
                    if let Some(d) = f.data_ref() {
                        size += d.len();
                        tap.feed(d);
                    }
                    held.push_back(f);
                    if size > ERROR_BODY {
                        break;
                    }
                }
                Some(Err(_)) => break,
                None => {
                    complete = true;
                    break;
                }
            }
        }
        let error = if complete {
            let mut buf = Vec::with_capacity(size);
            for f in &held {
                if let Some(d) = f.data_ref() {
                    buf.extend_from_slice(d);
                }
            }
            error_body(&buf)
        } else {
            None
        };
        // A 403 is a permission or model denial unless the provider says
        // the credential itself failed.
        let outcome = if status == StatusCode::FORBIDDEN
            && error
                .as_ref()
                .is_some_and(|(k, _)| k == "authentication_error")
        {
            Outcome::AuthError
        } else {
            outcome
        };
        let body_rejection = matches!(tap.decisive, Some(Decisive::Rejection { .. }));
        let cooled = outcome == Outcome::Rejected || body_rejection;
        if cooled {
            // One cooldown per attempt. Both sources are the provider's own
            // statement; the longer wait wins.
            let body_wait = error.as_ref().and_then(|e| e.1);
            let retry = match (update.retry_after_ms, body_wait) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
            acc.cool_down(retry, now);
            ctx.sh.changed();
        }
        let auth_retry = owned && outcome == Outcome::AuthError && attempt == 0 && {
            let before = acc.credential.load_full();
            reload_after_401(acc, before).await
        };
        if outcome == Outcome::AuthError && owned && !auth_retry {
            acc.credential_rejected();
            ctx.sh.changed();
        }
        let rec = record(status.as_u16(), streaming);
        let response = Response::from_parts(
            parts,
            RespBody::Tap(Box::new(
                TapBody::new((!complete).then_some(body), held, tap, rec, outcome)
                    .cooled_if(cooled),
            )),
        );
        return Attempt::Failed {
            outcome,
            response,
            auth_retry,
        };
    }

    // 2xx. Hold the first frames when an in-stream rejection could still
    // fail over to another account without anything reaching the client.
    let mut held = VecDeque::new();
    let hold_ms = ctx.reg.config.routing.bootstrap_hold_ms;
    let may_fail_over = owned
        && streaming
        && hold_ms > 0
        && !ctx.info.stateful
        && ctx.endpoint.generates()
        && attempt + 1 < MAX_ACCOUNTS_PER_REQUEST as u8;
    if may_fail_over {
        let deadline = tokio::time::sleep(Duration::from_millis(hold_ms));
        tokio::pin!(deadline);
        let mut bytes = 0usize;
        let mut ended = false;
        while tap.decisive.is_none() && held.len() < HOLD_FRAMES && bytes < HOLD_BYTES {
            tokio::select! {
                f = body.frame() => match f {
                    Some(Ok(f)) => {
                        if let Some(d) = f.data_ref() {
                            bytes += d.len();
                            tap.feed(d);
                        }
                        held.push_back(f);
                    }
                    Some(Err(_)) => {
                        // Failed before anything reached the client: the
                        // upstream may have run, so this is ambiguous.
                        record(status.as_u16(), streaming).emit(Outcome::Ambiguous, Some(&tap));
                        let mut response = ctx.err(StatusCode::BAD_GATEWAY, "upstream_ambiguous", "The upstream stream failed before any output; it may have run. Relay did not retry it.");
                        response.headers_mut().insert("x-should-retry", HeaderValue::from_static("true"));
                        return Attempt::Failed { outcome: Outcome::Ambiguous, response, auth_retry: false };
                    }
                    None => { ended = true; break; }
                },
                _ = &mut deadline => break,
            }
        }
        if let Some(Decisive::Rejection { retry_after_ms, .. }) = tap.decisive.clone() {
            acc.cool_down(retry_after_ms, now_ms());
            ctx.sh.changed();
            // An explicit refusal before any output. The provider may still
            // have read (and counted) the input, so this is not proof that
            // nothing was billed. Keep the stream for the client in case
            // nothing else can serve it.
            let rec = record(status.as_u16(), streaming);
            let response = Response::from_parts(
                parts,
                RespBody::Tap(Box::new(
                    TapBody::new((!ended).then_some(body), held, tap, rec, Outcome::Rejected)
                        .cooled(),
                )),
            );
            return Attempt::Failed {
                outcome: Outcome::Rejected,
                response,
                auth_retry: false,
            };
        }
        let rec = record(status.as_u16(), streaming);
        return Attempt::Commit(Response::from_parts(
            parts,
            RespBody::Tap(Box::new(TapBody::new(
                (!ended).then_some(body),
                held,
                tap,
                rec,
                Outcome::Cancelled,
            ))),
        ));
    }
    let rec = record(status.as_u16(), streaming);
    Attempt::Commit(Response::from_parts(
        parts,
        RespBody::Tap(Box::new(TapBody::new(
            Some(body),
            held,
            tap,
            rec,
            Outcome::Cancelled,
        ))),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_shapes_follow_provider() {
        let a = json_error(
            Some(Provider::Anthropic),
            StatusCode::UNAUTHORIZED,
            "authentication_error",
            "x",
        );
        assert_eq!(a.status(), 401);
        assert_eq!(a.headers()["x-relay-error"], "authentication_error");
        let RespBody::Full(Some(b)) = a.into_body() else {
            panic!()
        };
        let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["type"], "error");
        let o = json_error(Some(Provider::Openai), StatusCode::BAD_REQUEST, "k", "m");
        let RespBody::Full(Some(b)) = o.into_body() else {
            panic!()
        };
        let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["error"]["code"], "k");
    }
}
