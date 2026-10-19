# Integration contract: backend crates ↔ `relay-desktop`

Owner: core (`relay-core`, `relay-providers`, `relay-usage`, `relay-proxy`, root
manifest). Desktop consumes only the items below; anything else is internal.
All items below are implemented and tested (`cargo test --workspace --exclude relay-desktop`).

## Workspace

- Toolchain `1.98.1` via `rust-toolchain.toml` (Zed `56934ec0` pin). Homebrew
  `cargo` ignores it; use rustup's cargo
  (`~/.rustup/toolchains/1.98.1-*/bin` first in `PATH`).
- Version: `[workspace.package] version = "20261018.1.0"`; display
  `YYYYMMDD.N` is `relay_core::DISPLAY_VERSION` _(landed)_.
- CI gates: `cargo fmt --all --check`,
  `cargo clippy --workspace --exclude relay-desktop --all-targets --locked -- -D warnings`,
  `cargo test --workspace --exclude relay-desktop --locked`. Tests need no
  env, network or credentials; they bind ephemeral `127.0.0.1` ports and use
  temp dirs only.

## Threading rule

The proxy runs on its own multi-thread Tokio runtime in dedicated threads.
The UI never sits on the request path: it reads `Arc<Snapshot>` and awaits a
`watch` change notification. Usage queries are synchronous SQLite reads
(single-digit ms); call them from a background executor, never from render.

## `relay-core` _(landed)_

| Item                                                                                                                                                                                                                  | Use                                                                                                                                 |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `snapshot::Snapshot { version, generated_at, routing, providers, telemetry_dropped, in_flight }`                                                                                                                      | Whole UI state. Empty `providers` = first run.                                                                                      |
| `snapshot::RoutingStatus::{Stopped, Running{addr}, Paused{addr}, Failed{reason}}`                                                                                                                                     | Footer / not-routing popover.                                                                                                       |
| `snapshot::ProviderView { provider, groups }`                                                                                                                                                                         | Provider sections, `Provider::ALL` order, only providers with accounts.                                                             |
| `snapshot::GroupView { key, plan, credential, windows: Vec<WindowAggregate>, accounts }`                                                                                                                              | Plan group; `key` is stable for expand state.                                                                                       |
| `aggregate::WindowAggregate { key, label, accounts, fresh, stale, unknown, avg_left_pct: Option, min_left_pct, pooled_left_pct, first_reset, last_reset }`                                                            | Group bar: avg over fresh only, counts printed; pooled = "`pooled_left_pct`% of `fresh` × 100%"; resets "first + range".            |
| `snapshot::AccountView { id, provider, label, identifier, plan, mode, credential, state, state_detail, in_flight, sessions, last_seen, windows, credits, cooldown_until, models_seen, models_allowed, capabilities }` | Account card / ledger row.                                                                                                          |
| `snapshot::AccountState::{InUse, Healthy, Low, Exhausted, Stale, Unknown, Disconnected, Paused}`                                                                                                                      | State tag.                                                                                                                          |
| `snapshot::Capability::{Supported, Unsupported(reason)}`                                                                                                                                                              | Reset limits / Refresh quota: both `Unsupported` with a reason in v1 (no documented provider API). Render disabled with the reason. |
| `quota::QuotaWindow { key, label, used_pct: Option<f32>, resets_at, window_minutes, status, observed_at }` + `.freshness(now)`, `.effective_left(now)`                                                                | Per-window bar. `None` → "—", never 0%.                                                                                             |
| `quota::Credits { has_credits, unlimited, balance, observed_at }`                                                                                                                                                     | Only shown when `Some`.                                                                                                             |
| `period::through_relay(kind, today, hour, week_start, &days) -> ThroughRelay`                                                                                                                                         | Pure; desktop normally gets `ThroughRelay` from `relay-usage` instead.                                                              |
| `period::ThroughRelay { kind, period_days, current: Vec<DayBucket>, prior, current_total, prior_equivalent_total, comparison, prior_truncated_to }`                                                                   | Sparkline. `DayBucket { date, tokens: Option<u64>, partial }`: `None` = gap, `Some(0)` = baseline dot, `partial` = hollow end dot.  |
| `period::Comparison::{Delta(f64), TooEarly, NoComparison, PriorIncomplete, Incomplete, NoTraffic}`                                                                                                                    | Delta label or muted label.                                                                                                         |
| `config::{Config, ServerConfig, RoutingConfig, UpstreamsConfig, UsageConfig, UiPrefs, AccountConfig, SecretRef, SecretFormat, PeriodKind, Theme}`                                                                     | Settings forms. `Config::validate() -> Result<(), Vec<ConfigError>>` gives display-ready messages.                                  |
| `model::{Provider, AccountMode, Pool, CredentialKind, AccountId, mask_identifier}`                                                                                                                                    | `mask_identifier` before storing an identifier.                                                                                     |
| `usage::TokenUsage { input, cache_read, cache_write, output, reasoning }`                                                                                                                                             | Disjoint buckets; `reasoning ⊂ output`; `.total()` = Through Relay measure.                                                         |

Group = provider + plan + credential kind. No "next account" field exists
anywhere, by design.

## `relay-proxy` _(landed)_

```rust
// Paths (config, data, local key, usage db). RELAY_HOME overrides the root.
relay_proxy::paths::Paths::for_user() -> std::io::Result<Paths>
relay_proxy::paths::Paths::in_dir(dir: &Path) -> Paths
Paths { dir, config_file, local_key_file, usage_db }

// Durable config. Every mutation validates, then writes atomically (0600).
relay_proxy::store::ConfigStore::open(paths: &Paths) -> Result<ConfigStore, StoreError>
ConfigStore::config(&self) -> &Config
ConfigStore::update(&mut self, edit: impl FnOnce(&mut Config)) -> Result<(), StoreError>
ConfigStore::add_account(&mut self, account: AccountConfig) -> Result<(), StoreError>
ConfigStore::replace_account(&mut self, account: AccountConfig) -> Result<(), StoreError>
ConfigStore::remove_account(&mut self, id: &str) -> Result<(), StoreError>
ConfigStore::set_paused(&mut self, id: &str, paused: bool) -> Result<(), StoreError>

// Proxy lifecycle. Binds synchronously, so a port conflict is returned here.
relay_proxy::start(paths: &Paths, config: Config) -> Result<Handle, StartError>
StartError::{AddrInUse(SocketAddr), Bind(io::Error), Config(Vec<ConfigError>), Usage(String), Runtime(io::Error)}
Handle::addr(&self) -> SocketAddr
Handle::snapshot(&self) -> Arc<Snapshot>                   // lock-free load
Handle::subscribe(&self) -> tokio::sync::watch::Receiver<Arc<Snapshot>>  // executor-agnostic
Handle::set_paused(&self, paused: bool)
Handle::apply_config(&self, config: Config) -> Result<(), Vec<ConfigError>>  // hot reload accounts/routing
Handle::client_setup(&self, account: &str) -> Option<ClientSetup>         // copyable T3 values
Handle::reveal_local_key(&self) -> String                                 // explicit user action only
Handle::is_paused(&self) -> bool
Handle::usage_db(&self) -> &Path
Handle::shutdown(self)                                                    // stops accepting, drains ≤10 s
```

Snapshots publish at most every 500 ms and only when something changed
(request start/end, quota update, pause, config). When idle the publisher
sleeps until the next time-driven transition (a window reset or a value
turning stale), so a hidden UI costs no wakeups.

`ClientSetup` gives the per-account T3 values from
`research/routing-architecture.md` §1.1: Claude
`ANTHROPIC_BASE_URL` and `ANTHROPIC_CUSTOM_HEADERS` (observed) or
`ANTHROPIC_AUTH_TOKEN` (pooled/key); Codex `launchArgs` for
`setupMode: existing`. Secret values are represented by a placeholder; the
desktop copies the real key only via `reveal_local_key`.

Headless binary: `relay-proxy [--home DIR] [--config FILE] [run|check|version|help]`.
`check` validates and reports credential availability without creating files.

## `relay-usage` _(landed)_

```rust
relay_usage::Reader::open(path: &Path) -> rusqlite::Result<Reader>       // read-only connection
Reader::through_relay(&mut self, kind: PeriodKind, week_start: chrono::Weekday) -> rusqlite::Result<ThroughRelay>
Reader::account_totals(&mut self, range: Range) -> rusqlite::Result<Vec<AccountTotals>>
Range::{Day, Week, Month}  // trailing 24 h, 7 d, 30 d
AccountTotals { account_id, provider: Option<Provider>, model: Option<String>, usage: TokenUsage, requests, errors, retries, ambiguous, mean_ttft_ms: Option<u64> }
relay_usage::reader::week_start_from_pref(pref: Option<u8>) -> Weekday     // 0 = Monday; default Monday
Reader::model_totals(&mut self, range: Range) -> rusqlite::Result<Vec<AccountTotals>>  // per account × model
```

A missing database (Relay never ran) reads as all-`None` buckets, not zeros.

## Account flows

- **Observed** (default, Mode A): `AccountConfig { mode: Observed, credential: None }`.
  The T3 instance keeps its own login and sends `x-relay-key` plus
  `x-relay-account: <id>`; Relay forwards the client credential unchanged.
- **Pooled** (opt-in, Mode B; terms unresolved): `credential: Some(SecretRef)`
  pointing at the user's own login file or keychain item. Relay re-reads it
  when it changes and never refreshes it, so the CLI keeps refresh ownership.
- **Key** (Mode D): `credential: Some(SecretRef::Env | Keychain)`.

Pools never mix: a router-owned request uses `x-relay-pool: pooled|key`, else
`routing.default_pool`. No eligible account in that pool → `503`, never a
fallback to the other pool.

## Retry rule

Relay moves a request to another account only when the attempt provably
never reached the provider (DNS, TCP connect or TLS handshake failure,
including the connect deadline) or the provider explicitly refused it before
any output (429, 529, `overloaded_error`, rate-limit or
`usage_limit_reached`), the request carries no account-bound state, and
Relay chose the account. An explicit refusal means no output was generated;
it does not prove the provider counted or billed nothing, so a moved request
may cost something on both accounts. Any 5xx or 408, a missing response
before `server.response_header_timeout_ms`, and every other ambiguous
failure is returned unchanged and never retried. Clients may still retry on
their own, so Relay does not guarantee at-most-once execution.

## Credential recovery and restarts

- A pooled or key account whose credential is missing, expired or rejected
  (401) is re-read at most every 2 s while requests need it, and at once when
  its file changes; a healthy file credential is checked for changes every
  30 s. A rejected token stays out until the source holds a different one.
  Relay never refreshes logins itself. A 403 is a permission or model denial
  and does not disconnect the account unless the body says
  `authentication_error`.
- `Handle::restart_required() -> Vec<String>` lists applied settings that only
  take effect after a restart (local address, local key source, upstream
  timeouts, usage recording and retention). Show it after `apply_config`.
- `ServerConfig.response_header_timeout_ms` (default 600000) is new; configs
  without it keep working.
