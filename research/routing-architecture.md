# Routing architecture research

Status: research and proposal only. Nothing here is implemented, benchmarked, or
platform-verified. Implementation waits for approval of the Paper UI design.

Research date: 2026-10-18. Sources: Context7 (`/router-for-me/cliproxyapi`,
`/router-for-me/cliproxyapidocs`, `/pingdotgg/t3code`, `/websites/rs_gpui`,
`/tauri-apps/tray-icon`, `/websites/rs_tokio_tokio`), the crates.io API, and the
GitHub API against `zed-industries/zed@56934ec0`. Provider settings, terms and
retry safety were corrected on review; see `compatibility-review.md`.

## 1. Findings

### 1.1 t3code compatibility: configuration hypothesis, not yet proven

t3code launches the stock CLIs, so the router only has to speak the wire APIs
those CLIs already use. **No t3code instance has completed a turn through a
router.** The rows below are configurations derived from source and docs; each
needs the live check in §4. See `compatibility-review.md` for citations.

The credential setting decides which account pays, so each provider has two
distinct configurations. They are not interchangeable.

| t3code provider | Subscription mode (user's own login reaches upstream)                                                                                                                                                                                                                                                                                                                                                                               | Key mode (router holds an API key)                                                                                                                                                                                                                                           | Wire API the router must serve                                                                              |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Claude instance | Instance env sets **only** `ANTHROPIC_BASE_URL=http://127.0.0.1:<port>`, plus its own `CLAUDE_CONFIG_DIR` logged in to claude.ai. Do **not** set `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY` or `apiKeyHelper`: any of them turns the subscription login off. Local router auth and instance attribution go in `ANTHROPIC_CUSTOM_HEADERS` (for example `x-router-instance: <id>`); the router strips that header before forwarding. | `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN=<router key>` (Sensitive), `ANTHROPIC_API_KEY=` (empty), `/logout` in that config dir. This is the pattern t3code documents for OpenRouter. Billing moves to the router's Console key; t3code stops showing subscription limits. | Anthropic Messages: `POST /v1/messages` (SSE), `POST /v1/messages/count_tokens`; tolerate `HEAD /api/hello` |
| Codex instance  | `setupMode: "existing"` with its own `homePath` logged in with ChatGPT. `launchArgs`: `-c model_provider="router"`, `model_providers.router={base_url="http://127.0.0.1:<port>/v1", wire_api="responses", requires_openai_auth=true, supports_websockets=false, http_headers={"x-router-instance"="<id>"}}`. No `env_key`: with `requires_openai_auth=true` Codex ignores it.                                                       | `setupMode: "existing"`; same provider block with `env_key="ROUTER_KEY"`, `requires_openai_auth=false`; `ROUTER_KEY` in instance env. Router holds an OpenAI Platform key.                                                                                                   | OpenAI Responses: `POST /v1/responses` (SSE), `POST /v1/responses/compact`, `GET /v1/models`                |

**Managed Codex cannot use the router in either mode.** `CodexManagedRuntime`
pins its own `openai_token_sharing` provider to `https://api.openai.com/v1`,
injects the T3-owned token as `ACCESS_TOKEN`, and deletes `OPENAI_BASE_URL`,
`OPENAI_API_KEY` and `T3CODE_CODEX_LAUNCH_ARGS`. Its `-c` args are fixed in code;
user `launchArgs` are replaced. A managed instance keeps working unrouted; to
route Codex, the user adds a separate `existing` instance.

Implications:

- `supports_websockets=false` keeps v1 to HTTP+SSE.
- In subscription mode the CLI's OAuth token **transits the router in memory**.
  The router must forward it unchanged, never log or persist it, and forward
  `anthropic-beta` verbatim (stripping its OAuth capability causes `401`). The
  earlier claim that "the router does not read the CLIs' own credentials" holds
  only in key mode.
- In subscription mode the account is fixed by the t3code instance. The router
  cannot pick an account unless it holds tokens for several accounts (§2.6).
- The user creates the t3code instances. The router can show copyable values but
  must not write t3code settings.

Unverified, needs a live check:

- Whether Codex with a custom `base_url` still shows the ChatGPT account and
  plan in t3code. Codex's workspace routing runs only when the base URL is unset
  or ends in `/backend-api/codex`, so a `/v1` router URL likely skips it [inf].
- Which upstream the router must forward a ChatGPT-login Codex request to.
  CLIProxyAPI uses `chatgpt.com/backend-api/codex/responses`, which OpenAI does
  not document as a public API.
- Whether Claude `rate_limit_event` still reaches t3code in subscription mode.

### 1.2 CLIProxyAPI (Go) as a reference

Behaviour worth copying, from `config.example.yaml`, `sdk/cliproxy/auth/*` and
`internal/runtime/executor/*`:

- **Strategies:** `round-robin` (default), `weighted-round-robin` and
  `fill-first`.
- **Retry rounds:** `request-retry` (default 3) adds credential rounds on
  HTTP 403, 408, 429, 500, 502, 503 and 504 and on transport errors.
  `max-retry-credentials` caps how many credentials each round tries. Do not
  copy the transport-error retry: it can re-execute an ambiguous attempt
  (§2.2).
- **Cooldown on 429:** the cooldown comes from `Retry-After`, with a floor.
  Without `Retry-After`, it uses exponential backoff (`BackoffLevel`). When the
  limit is credential-scoped, the cooldown spreads to every model on that
  credential.
- **Stream bootstrap failover:** the router buffers up to 48 frames or 1 MiB
  before committing response headers. If an overload or rate-limit error is
  hidden inside an HTTP 200 stream during that window, it fails over to another
  credential. After commit, it never retries. `streaming.bootstrap-retries`
  defaults to 0.
- **Session affinity:** the key is `provider::sessionID::model`. Identity
  precedence is explicit harness header or body > derived ID > message-prefix
  hash. Forks and subagents get alias handling.
- **Passive quota capture:** the router reads quota headers from every response.
  - Codex: `x-codex-{primary,secondary}-{used-percent,window-minutes,reset-at,reset-after-seconds}`, `x-codex-plan-type`, `x-codex-credits-*`.
  - Claude: `anthropic-ratelimit-unified-*`.
  - Both: `retry-after`.
- **Usage records:** each record has input, output, reasoning, cached and total
  tokens, plus latency, TTFT, failure, provider, model and auth index. Records go
  into a queue that is drained asynchronously.
- **Legacy settings:** `quota-exceeded.switch-project` and
  `switch-preview-model` are now compatibility-only and do nothing in v8. Do not
  copy them.

Scope cut for "minimal": no model aliasing or payload rewriting, no
Gemini/Antigravity support, no management HTTP API, no plugin system, and no LCP
Merkle matching.

### 1.3 GPUI on macOS and Linux

- **crates.io is stale:** `gpui` 0.2.2 was published 2025-10-22. Zed `main` was
  pushed 2026-10-18 and split the platform layer into separate crates:
  `gpui_platform`, `gpui_macos`, `gpui_linux`, `gpui_wgpu` and `gpui_tokio`. The
  README now requires `gpui_platform::application()`, which crates.io lacks.
  **Decision: pin both crates by git `rev` to a Zed commit.** Expect breaking
  changes on every bump; the README says GPUI is pre-1.0.
- **macOS:** renders with Metal. Requires full Xcode, not just the command-line
  tools, and the `font-kit` feature; without `font-kit`, no glyphs render.
- **Linux:** `gpui_linux` features `wayland` and `x11` (both on by default)
  choose the backend at runtime with `guess_compositor()`. Rendering goes through
  `gpui_wgpu` (Vulkan). Windows use client-side decorations on Wayland.
- **No tray API:** a search of the Zed tree for status-item or tray crates finds
  nothing. GPUI can only render the expanded quota panel as a window; the
  menu-bar icon needs a separate crate.
- **Tokio integration:** `gpui_tokio` exists. **Decision:** run the proxy on its
  own multi-thread Tokio runtime in dedicated threads, and send only snapshots to
  GPUI. The UI thread must never sit on the routing path.

### 1.4 Tray: `tray-icon` 0.26.1 (with muda 0.21.2)

| Platform                                 | Constraint                                                                                    | Consequence                                                                                                                                                                               |
| ---------------------------------------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS                                    | Create the icon on the main thread after the event loop is running.                           | Build it inside GPUI's `Application::run` callback, which runs on the main thread with NSApp active. Use `with_icon_as_template(true)` and `with_title("62%")` for an at-a-glance number. |
| Linux, default `libappindicator` backend | Needs a GTK main loop on the creating thread. Once set, a menu cannot be removed. No tooltip. | GPUI does not run GTK, so **do not use this backend.**                                                                                                                                    |
| Linux, `ksni` feature                    | Pure D-Bus StatusNotifierItem on its own worker thread. Tooltip and title are supported.      | **Use `default-features = false, features = ["ksni"]`.** GNOME needs the AppIndicator extension to show SNI icons. Document this as a requirement rather than work around it.             |

Tray scope: native menus cannot render custom rows. The menu shows one line per
group, for example "Claude · 3 accounts · avg 41% · resets 2h". Native submenus
expand to per-account lines, which covers the "expandable" requirement. Items
"Open dashboard…" and "Quit" open or close a small GPUI window that shows the
detailed analytics. Whether the tray menu text gets live updates while open
varies by platform; on macOS, menus usually refresh when reopened. Verify per
platform.

## 2. Proposed minimal architecture

```
t3code ─► claude/codex CLI ─HTTP/SSE─► [axum ingress] ─► Router (pure, sync, O(accounts))
                                             │                    │ reads ArcSwap<Snapshot>
                                             │                    ▼
                                             └────────► upstream (hyper-util pooled client, rustls)
                                                                  │ response headers ─► quota parse (inline, header-only)
                                                                  │ SSE bytes ─► passthrough + usage tap (no re-serialise)
                                             events ─► bounded mpsc (try_send, drop+count) ─► relay-usage writer
                                                                  ├─ aggregates (per account/model/window)
                                                                  ├─ SQLite (batched, dedicated thread)
                                                                  └─ publishes ArcSwap<UiSnapshot> @ ≤2 Hz ─► GPUI window + tray
```

One desktop binary with two runtimes: GPUI on the main thread, and Tokio on its
own threads. The proxy runtime never depends on the UI.

### 2.0 Rust Cargo workspace

The whole application is Rust in one Cargo workspace. There is no JS/TS
shell, Node or Bun sidecar, webview or embedded web frontend at runtime. Paper
is a design tool for approving the UI before `relay-desktop` is built; it ships
nothing.

```
Cargo.toml              # [workspace], shared deps, lints, profiles
Cargo.lock              # committed; CI uses --locked
rust-toolchain.toml     # pinned toolchain (see below)
crates/
  relay-core/           # lib
  relay-providers/      # lib
  relay-usage/          # lib
  relay-proxy/          # lib + headless bin `relay-proxy`
  relay-desktop/        # bin `relay`: GPUI window + native tray
```

| Crate             | Owns                                                                                                                                                                                                                                                | I/O                                                | Workspace deps                                 |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- | ---------------------------------------------- |
| `relay-core`      | `Account`, `Model`, `QuotaState`, `Snapshot`, `select()`, session binding, stateless-eligibility and the attempt-classification state machine (§2.2), aggregation math (§2.5)                                                                       | None; no async runtime                             | —                                              |
| `relay-providers` | Anthropic Messages and OpenAI Responses adapters: routing-key extraction, credential mode (pass-through, key, pool) and header rules, router-header stripping, quota header parsers, SSE usage extractors, mapping provider errors to core outcomes | None; pure functions over `http` and `bytes` types | `relay-core`                                   |
| `relay-usage`     | Usage event types, bounded ingest channel, SQLite store (WAL, batched writes on a dedicated thread), retention, rolling aggregates and read queries for the UI                                                                                      | Local disk only                                    | `relay-core`                                   |
| `relay-proxy`     | axum ingress, `hyper-util` + `rustls` upstream client, stream forwarder, admission, retries, cancellation, publishing `ArcSwap<Snapshot>`; `start(config) -> Handle`                                                                                | Network                                            | `relay-core`, `relay-providers`, `relay-usage` |
| `relay-desktop`   | GPUI dashboard window, `tray-icon` (`ksni` on Linux), starting and stopping the proxy runtime, rendering `UiSnapshot`                                                                                                                               | UI                                                 | `relay-core`, `relay-usage`, `relay-proxy`     |

Boundary rules:

- Dependencies point downward only. Nothing depends on `relay-desktop`, and
  only `relay-desktop` depends on `gpui`, `gpui_platform` or `tray-icon`. CI
  asserts that `cargo tree -p relay-proxy` contains no `gpui` crate.
- `relay-core` and `relay-providers` hold the logic most worth testing and need
  no network, so selector, parser and retry-classification tests run without
  sockets. Moving the provider-specific parsers out of core keeps core free of
  wire formats.
- The headless `relay-proxy` binary serves integration and E2E tests, and Linux
  sessions without a tray. It is a second target in an existing crate, not a
  new crate.
- Not split further: a CLI, config or management-API crate would have nothing
  to own in v1.

Workspace manifest:

- `[workspace]` with `members = ["crates/*"]` and `resolver = "3"`.
- `[workspace.package]`: `edition = "2024"`, `license`, `rust-version`. Each
  crate sets `edition.workspace = true`.
- `[workspace.dependencies]` declares every third-party version once (tokio,
  axum, hyper-util, rustls, bytes, http, serde, serde_json, arc-swap, dashmap,
  rusqlite, tray-icon, and `gpui` / `gpui_platform` by git `rev`, §1.3). Crates
  use `dep.workspace = true`.
- `[workspace.lints]`: `rust.unsafe_code = "deny"`, `rust.missing_docs = "warn"`
  for library crates, `clippy.all = "warn"` (CI promotes warnings to errors).
  Each crate sets `[lints] workspace = true`. If the macOS tray fallback (§6)
  needs `objc2` `unsafe`, it gets a scoped `#[allow(unsafe_code)]` inside
  `relay-desktop` only.
- `rust-toolchain.toml` pins the stable toolchain that the pinned Zed revision
  builds with. Edition 2024 needs Rust 1.85 or later. GPUI's actual minimum is
  unverified [inf].

Naming: crate and directory names are kebab-case. Rust module files stay
`snake_case.rs`, because module names must be identifiers. That conflicts with
the user's kebab-case filename preference; `#[path]` workarounds are not
proposed.

Credential handling depends on the mode (§1.1, §2.6):

- **Subscription pass-through:** the router stores nothing. The CLI's token
  arrives per request and is forwarded unchanged.
- **Key mode:** the router stores references to API keys, such as a keychain
  item name, plus an `account_id`.
- **Own-account pool (opt-in, §2.6):** the router holds OAuth tokens for several
  of the user's own accounts.

In every mode the router never prints or logs tokens. Connecting accounts and
OAuth flows are out of scope for this task.

### 2.1 Hot path, step by step

1. **Parse the routing key.** Read `provider` from the endpoint, `model` from a
   JSON field, and the session ID from headers. To get the model, the router
   must buffer the request body: Claude Code and Codex send the full
   conversation on every turn, often over 1 MB. That buffering is unavoidable.
   Read only the `model` field, using `serde_json` with a borrowed struct that
   ignores other fields, or a byte scan. Forward the original `Bytes` unchanged.
   Never re-serialize the body.
2. **Select an account.** Call `select(snapshot, provider, model, session)` on
   an `arc_swap::ArcSwap<Snapshot>` load. This takes no lock and does not await.
   Pseudocode:
   - `eligible` = enabled ∧ supports model ∧ not in cooldown (`now < next_recover_at`) ∧ in-flight < cap.
   - If a sticky binding exists in a `DashMap<SessionKey, (AccountId, Instant)>`
     with a TTL, and that account is eligible, return it.
   - Otherwise pick by strategy. **Default `quota-aware`:** choose the highest
     `headroom = 1 - max(primary_used, secondary_used)`. Among accounts within
     ε = 5 percentage points of the best, prefer the one whose window resets
     soonest, so quota that is about to expire is used first. Break remaining
     ties with an `AtomicUsize` round-robin cursor. `fill-first` and
     `round-robin` are config alternatives.
   - Bind the session to the chosen account.
3. **Admission and backpressure.** Use a per-account `Semaphore`. Take a permit
   with `try_acquire_owned`; if none is free, try the next candidate. If all
   candidates are saturated, wait on a bounded global queue with a deadline
   (default 2 s), then return `429` with `Retry-After`. Never queue without a
   bound.
4. **Forward upstream.** Use a pooled `hyper-util` client with HTTP/2 where the
   upstream supports it and keep-alive. In pass-through mode, forward the
   client's credential headers untouched and strip only router-owned headers.
   In key or pool mode, inject the chosen account's credential and keep the
   client's other headers (`User-Agent`, `anthropic-*`, `originator`) as sent;
   never synthesize or spoof client identity. Pass-through never refreshes;
   key and pool modes refresh in a background task, plus one single-flight
   refresh on a 401.
5. **Parse quota from response headers** on the response-header future. This is
   O(number of headers) and allocation-light. Write the result into the
   account's `AtomicU16` and `AtomicU64` fields so the selector sees it on the
   next request without republishing the snapshot.
6. **Stream the response.** Use `Body` passthrough, so frames are forwarded as
   `Bytes` without copying. A tap scans each SSE event for the usage-bearing
   event only: `message_delta` and `message_stop` usage for Anthropic, and
   `response.completed` usage for Responses. Data stays in the stream either
   way; the tap only reads.
7. **Finish.** Emit `UsageEvent` with `try_send` to a bounded mpsc of 8192
   events. If the channel is full, drop the event and increment
   `telemetry_dropped`. Telemetry can never apply backpressure to a request.

### 2.2 Retries and duplicate execution

"Nothing committed to the client yet" is **necessary but not sufficient**. The
real question is whether the upstream may already have executed the request.
Once the full request body has been written, a reset, timeout or truncated
stream is **ambiguous**: the provider may have generated output, billed it,
counted it against quota, and run server-side tools (Anthropic web search,
web fetch, code execution; Responses hosted tools). No idempotency key for
Messages or Responses was found in the sources reviewed, so the router cannot
deduplicate upstream and must not retry ambiguous attempts.

Each attempt is classified before any retry decision:

| Attempt outcome                                                                                                                                              | Same account                                                                          | Different account                                                                                                                                            | Notes                                                                                                                                                                                                                                                                                                  |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Proven not sent:** DNS, TCP connect or TLS handshake failure; HTTP/2 `REFUSED_STREAM`; GOAWAY whose last-stream ID is below this stream                    | Retry                                                                                 | Retry if stateless-eligible (below)                                                                                                                          | Pool reuse: a stale HTTP/1.1 keep-alive connection that resets after the request was written is **not** proven-not-sent. Retry only failures the client library reports as occurring before the request was written; whether `hyper-util` exposes that distinction reliably is [inf] and needs a test. |
| **Explicit pre-execution rejection** before any output: 429, 529, `overloaded_error`, `anthropic-ratelimit-unified-*` rejection, Codex `usage_limit_reached` | No; mark cooldown from `retry-after` / reset fields                                   | Only if stateless-eligible and the router chose the account (key or pool mode). In pass-through, return the error unchanged so the CLI's own backoff applies | The provider stated it did not run the request.                                                                                                                                                                                                                                                        |
| 500, 502, 503, 504, 408 before any output                                                                                                                    | No                                                                                    | No, unless stateless-eligible **and** no server-side tools declared                                                                                          | A gateway 502/504 can follow upstream execution; treat as ambiguous when server tools are present. Otherwise return unchanged.                                                                                                                                                                         |
| **Ambiguous:** reset or timeout after the body was written; EOF before `message_stop` / `response.completed`                                                 | **Never**                                                                             | **Never**                                                                                                                                                    | Return a retryable error, record `outcome=ambiguous`; the CLI decides (both CLIs have their own retry).                                                                                                                                                                                                |
| HTTP 200 with an in-stream rejection inside the bootstrap window (≤ 16 frames, 256 KiB or 1.5 s, headers held)                                               | No                                                                                    | Only if the event is an explicit rejection (row 2), no output item, content block or server-tool event was seen, and stateless-eligible                      | Otherwise commit and pass through.                                                                                                                                                                                                                                                                     |
| Any byte forwarded to the client                                                                                                                             | Never                                                                                 | Never                                                                                                                                                        | Pass the upstream error through; record `phase=mid_stream`.                                                                                                                                                                                                                                            |
| 400, 404, 413, context-length or capability rejection                                                                                                        | Never                                                                                 | Never                                                                                                                                                        | Forward the body unmodified; the CLI matches on its wording.                                                                                                                                                                                                                                           |
| 401 or 403                                                                                                                                                   | Key/pool mode: one single-flight refresh, then mark `auth_error`. Pass-through: never | Never                                                                                                                                                        | The router does not own pass-through tokens.                                                                                                                                                                                                                                                           |

**Stateless-eligible** means: no `previous_response_id`, no reasoning or
`encrypted_content` item, no compaction capsule, no signed thinking block, no
`container`, no server-side tool declared, and no sticky session binding. Most
turns after the first are not eligible, so quota-aware choice happens **at
session start** (bind), and later turns stay on their account.

Budget: at most 3 accounts and 10 s total. Cooldown from `retry-after`,
otherwise 1 s → 2 s → … capped at 5 min.

Billing: every attempt that reached the upstream may be billed on its own
account. The router records each attempt and its outcome so analytics show
retry cost; it does not claim at-most-once billing.

### 2.3 Cancellation

- Client disconnect drops the axum response body. Dropping the forwarding stream
  drops the upstream hyper body, which resets the HTTP/2 stream or closes the
  HTTP/1 connection. Write no extra code for this path, but **test it**: the
  upstream connection must close within 100 ms.
- Each request holds a `tokio_util::sync::CancellationToken` child of the
  shutdown token. On app quit, stop accepting connections, let in-flight streams
  drain for up to 10 s, then cancel.
- The bootstrap buffer loop and the backoff sleep use
  `select! { _ = token.cancelled() => … }`. Both futures are cancel-safe.
- The `OwnedSemaphorePermit` is released by `Drop` on every exit path.

### 2.4 Telemetry without hot-path blocking

- The hot path does only atomic stores and `try_send`. It performs no I/O and
  takes no locks shared with the UI.
- `relay-usage` owns the channel receiver and a dedicated writer thread. It
  writes rolling aggregates, keyed by account × model × hour, to SQLite in WAL
  mode, batched every 1 s or every 256 events. Raw events are kept for 7 days and hourly
  aggregates for 90 days.
- It publishes `UiSnapshot` through ArcSwap at most twice per second. GPUI
  receives a notification on a channel, reads the snapshot and re-renders. The
  tray rebuilds its menu items only when a group's text changes.

### 2.5 Quota aggregation rules (the "grouped / averaged" numbers)

A group is all accounts with the same provider and plan type. Per window
(Claude 5 h and 7 d, Codex primary and secondary):

- **Display "avg used %"** as the mean `used_pct` over accounts with **fresh**
  data, meaning observed less than 15 minutes ago and the window has not reset
  since. When a window resets (`now ≥ reset_at`), treat `used_pct` as 0 rather
  than stale.
- **Show the extremes:** `max used %` and the **pooled headroom**,
  `Σ(1 - used_i)`, expressed in account-equivalents, for example "1.7 accounts
  left". A plain mean hides one account at 100%, and these two numbers expose
  it.
- **Next reset** is `min(reset_at)` over accounts that are exhausted or above
  90%.
- Leave stale or unknown accounts out of the averages and count them in an
  "n stale" badge.
- Don't sum percentages across windows of different lengths, and don't average
  across providers.
- **Token analytics** are summed, never averaged: input, output, cached and
  reasoning tokens per account and model over today, 7 days and 30 days, plus
  request count, error rate, p50 and p95 TTFT, and retries.

The UI should show the stale count and the time each value was observed, because
quota data is passive and arrives only with responses.

Open question: proactive polling of each account's quota endpoint, as Codex's
`rateLimits/read` does, would make values fresh while idle but adds calls on the
user's subscriptions. Proposal: **off by default.**

### 2.6 Subscription routing modes

The user asked for routing across several subscriptions. Technical feasibility
and authorization evidence are separate; the citations are in
`compatibility-review.md`.

| Mode                                                                                                                                                   | Who picks the account                                                                | Technically feasible                                             | Authorization evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Proposal                                                                     |
| ------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------ | ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| **A. Pass-through observer**: one t3code instance per account, each with its own login                                                                 | The user, by choosing the instance; the router recommends the one with most headroom | Claude: documented. Codex: plausible, unverified upstream (§1.1) | Claude: documented gateway mode; the legal page does not prevent signing in to the unmodified binary with one's own subscription. Codex: Auth docs call `requires_openai_auth` "useful when you access OpenAI models through an LLM proxy server"; no upstream named                                                                                                                                                                                                                    | v1 default                                                                   |
| **B. Own-account pool**: router holds OAuth tokens for several of the user's **own** accounts and binds each new session to the one with most headroom | Router, at session start                                                             | Yes; CLIProxyAPI does it                                         | **Not explicitly permitted, not explicitly prohibited for a single owner.** Anthropic's legal page forbids developers to "collect, store, or intermediate Claude.ai credentials", in a bullet addressed to developers of products and services; whether a personal tool falls under it is unresolved. OpenAI's Terms forbid circumventing "rate limits or restrictions"; whether binding sessions to several owned accounts counts is unresolved. Enforcement can happen without notice | Opt-in after the user accepts the risk; design-ready but disabled by default |
| **C. Routing for other people**, or sharing one account's login                                                                                        | Router                                                                               | Yes                                                              | Prohibited: Anthropic forbids routing plan credentials "on behalf of their users" and sharing logins; OpenAI forbids making an account "available to anyone else"                                                                                                                                                                                                                                                                                                                       | Out of scope                                                                 |
| **D. Key mode** (Console / Platform API keys)                                                                                                          | Router                                                                               | Yes                                                              | Documented gateway use, billed to the key owner                                                                                                                                                                                                                                                                                                                                                                                                                                         | Supported; never substituted silently for A or B                             |

Mode B constraints, if the user enables it:

- Bind to localhost; one local user; no remote ingress.
- Requests come from the genuine CLIs. Keep their `User-Agent`, `anthropic-*`
  and `originator` headers; do not add CLIProxyAPI-style cloaking, UA or TLS
  fingerprint spoofing.
- Bind per session; migrate only stateless-eligible requests (§2.2).
- Token acquisition is out of scope for this task. Importing a CLI's login and
  refreshing it from the router may rotate the refresh token and log that CLI
  out [inf]; one writer per token must be designed before Mode B ships.

## 3. Performance gates (proposed, not yet measured)

No published routing-overhead benchmark exists for CLIProxyAPI, so these are
targets to verify, not claims. They run on a local mock upstream with
deterministic latency.

| ID  | Metric                                                                                           | Gate                                                                    | Harness                                                               |
| --- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------- | --------------------------------------------------------------------- |
| P1  | `select()` with 16 accounts and 10k live sessions                                                | p99 < 2 µs, 0 allocations                                               | `criterion` in `relay-core`, plus a `dhat` allocation assertion       |
| P2  | Added latency to the first upstream byte (proxy vs direct) at 64 concurrent requests, 32 KB body | p50 < 0.5 ms, p99 < 2 ms                                                | `oha` or a custom Tokio load generator against a mock that echoes SSE |
| P3  | Added TTFT for the downstream client, without bootstrap hold                                     | p99 < 1 ms                                                              | Same as P2                                                            |
| P4  | Streaming throughput, 1k events/s per stream × 64 streams                                        | Proxy CPU < 1 core on M-series; no event reordering or loss             | Mock upstream; compare checksums of the byte streams                  |
| P5  | Overhead for a 2 MB request body                                                                 | p99 < 5 ms; RSS growth per in-flight request ≈ body size, no extra copy | Same harness, plus a `/proc` or `task_info` RSS sample                |
| P6  | Telemetry isolation: SQLite writer stalled for 5 s                                               | P2 gates still pass; `telemetry_dropped` increases; no request errors   | Fault-injected store                                                  |
| P7  | Disconnect propagation                                                                           | Upstream connection closed < 100 ms after the client drops              | Mock records the close time                                           |
| P8  | Idle footprint                                                                                   | RSS < 60 MB with the window closed, wakeups < 1/s                       | Instruments (macOS) or `pidstat` (Linux), manual check                |
| P9  | UI isolation                                                                                     | Saturating the proxy (P4) never pushes the GPUI frame time beyond 16 ms | GPUI `profiler` feature or a manual trace                             |

CI fails if any automated gate (P1–P7) regresses by more than 10% against the
stored baseline.

## 4. Tests

The rule for every test: an oracle that is independent of the production logic,
and a plausible defect that would make it fail.

- **Selector (`relay-core`):** table-driven cases with hand-computed expected
  winners:
  - an account in cooldown is skipped;
  - a sticky session survives a better-headroom account, then moves when its
    account is exhausted;
  - the ε-tie rule picks the account that resets sooner;
  - saturated accounts are skipped;
  - the selector returns `None` when no account is eligible.

  A property test (`proptest`) checks that the result is always eligible and
  that the selector never chooses a cooled-down account.

- **Quota parsers (`relay-providers`):** fixtures copied from the header and event names CLIProxyAPI
  handles. Missing, malformed or negative values must leave the state unchanged.
- **Aggregation:** a reset window counts as 0; stale accounts are excluded and
  counted; pooled headroom matches hand arithmetic; mixed-provider input is
  rejected.
- **Retry state machine against a mock upstream.** The mock counts executions
  per account; that count is the oracle for duplicate execution.
  - connect refused → retried; mock execution count stays 1;
  - mock reads the full body, then resets the connection → **not** retried,
    client gets a retryable error, `outcome=ambiguous`, execution count 1;
  - EOF before `message_stop` → not retried;
  - 429 on a stateless first turn (key mode) → retried on another account with
    correct cooldown;
  - 429 on a turn carrying `previous_response_id` or a signed thinking block →
    not migrated; error returned unchanged;
  - 429 in pass-through → returned unchanged, never retried;
  - 502 with a server-side tool declared → not retried;
  - 400 → not retried, body byte-identical;
  - in-stream `usage_limit_reached` inside the window → fails over and the
    client sees only the second stream; the same error after the first delta →
    no retry, exactly one stream;
  - 401 in key mode → exactly one refresh, even with N concurrent requests
    (single-flight); 401 in pass-through → no refresh.
- **Credential hygiene (pass-through):** the upstream mock receives the client's
  `Authorization` and `anthropic-beta` byte-identical, never receives
  `x-router-instance`, and a log capture contains no token bytes.
- **Byte fidelity:** for recorded Claude and Codex SSE fixtures, the client
  receives output identical to the upstream byte for byte, and the extracted
  usage matches the fixture's usage block.
- **Cancellation and backpressure:** drop the client and confirm the mock
  upstream sees the close (P7); exceed capacity and get a 429 with `Retry-After`
  after the deadline; a full telemetry channel produces no error.
- **End-to-end (manual, gated on approval and on the user connecting accounts):**
  t3code Claude and Codex `existing` instances configured as in §1.1, each
  completing one turn through the router in subscription mode and in key mode.
  Record for each: turn completes, t3code still shows subscription limits (or
  not), Codex account and plan UI state, and which upstream the Codex request
  reached. Until this runs, t3code integration is unproven.

### 4.1 Check commands

Rust checks are the primary gates, run from the workspace root:

| Gate                       | Command                                                                                                              | Where                             |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| Format                     | `cargo fmt --all --check`                                                                                            | CI, macOS and Linux               |
| Lint                       | `cargo clippy --workspace --all-targets --locked -- -D warnings`                                                     | CI, macOS and Linux               |
| Unit and integration tests | `cargo test --workspace --locked` (proxy integration tests in `crates/relay-proxy/tests/` against the mock upstream) | CI, macOS and Linux               |
| UI independence            | `cargo tree -p relay-proxy` contains no `gpui` crate                                                                 | CI                                |
| Benchmarks P1–P7           | `cargo bench -p relay-core` and `-p relay-proxy`, compared with the stored baseline                                  | CI                                |
| Proxy E2E                  | headless `relay-proxy` binary with t3code instances (§4 end-to-end)                                                  | Manual, needs the user's accounts |
| Platform E2E               | §5 matrix                                                                                                            | Manual, real desktop sessions     |

The user's global instructions also ask for `bun test` and `bun run test:e2e`.
The workspace has no JS package, so those commands do not apply. Do not add a
`package.json`, wrapper scripts or placeholder tests to satisfy them; report
them as not applicable. Prettier applies only to the Markdown docs.

## 5. Platform verification matrix

| Check                                        | macOS 15/26 (arm64) | Ubuntu 24.04 GNOME, Wayland + AppIndicator extension    | KDE Plasma 6, Wayland | X11 session (any) |
| -------------------------------------------- | ------------------- | ------------------------------------------------------- | --------------------- | ----------------- |
| `relay-desktop` builds from pinned Zed rev   | CI                  | CI (deps: vulkan, wayland, xkbcommon, fontconfig, dbus) | —                     | —                 |
| GPUI window opens and renders                | Manual              | Manual                                                  | Manual                | Manual            |
| Tray icon and title visible                  | Manual              | Manual (fails without extension → documented)           | Manual                | Manual            |
| Submenus expand; text updates on reopen      | Manual              | Manual                                                  | Manual                | Manual            |
| App without window (tray-only) keeps running | Manual              | Manual                                                  | Manual                | Manual            |
| Proxy gates P1–P7                            | CI                  | CI (headless)                                           | —                     | —                 |

The CI hosts are GitHub-hosted `macos-latest` and `ubuntu-latest`. Tray and
window checks need a real desktop session, so they remain manual and will be
reported as such.

## 6. Risks and unknowns

1. **GPUI churn.** Without a crates.io release that matches the README, a git pin
   is mandatory, and API breakage is likely on every bump.
2. **Event-loop coexistence.** It is unverified that `tray-icon` on macOS works
   inside GPUI's NSApp run loop. Prototype this first. Fallback: create the
   NSStatusItem through `objc2` directly.
3. **Linux tray visibility** depends on the desktop. GNOME needs an extension.
4. **Upstream terms.** Assessed in §2.6 and `compatibility-review.md`.
   Pass-through of one's own login is the best-supported subscription path.
   Router-chosen pooling across one's own accounts is unresolved, not proven
   prohibited; routing for others is prohibited. Account enforcement is the
   risk the user accepts by enabling Mode B.
5. **Undocumented upstream surfaces.** The quota header names come from
   CLIProxyAPI source, and `chatgpt.com/backend-api/codex` is not a documented
   public API. Both may change without notice. Parsers must tolerate missing
   fields.
6. **t3code integration** is unproven until the §4 end-to-end check runs.

## 7. Decisions needed from the user

- Enable Mode B (own-account pool, §2.6) after reviewing the unresolved terms,
  or stay with Mode A (pass-through, instance chooses account) plus key mode.
- Default strategy `quota-aware` with session stickiness (applies to Modes B
  and D; Mode A only recommends). Alternatives: `fill-first` or `round-robin`.
- Proactive quota polling: off by default?
- Bootstrap hold window of 1.5 s, which adds TTFT only when the upstream is
  slow to start.
- Which providers ship in v1: Claude and Codex only, as proposed.
