//! Configuration types and validation. Persistence lives in `relay-proxy`.
//!
//! Secrets are never stored in config: an account names *where* its
//! credential lives ([`SecretRef`]), and Relay resolves it at runtime.

use crate::model::{AccountId, AccountMode, CredentialKind, Pool, Provider, Vendor};
use crate::select::Strategy;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Whole configuration file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Local listener.
    pub server: ServerConfig,
    /// Placement and session behaviour.
    pub routing: RoutingConfig,
    /// Upstream origins Relay may contact.
    pub upstreams: UpstreamsConfig,
    /// Usage recording.
    pub usage: UsageConfig,
    /// Desktop preferences (no effect on the proxy).
    pub ui: UiPrefs,
    /// Accounts, in display and fill-first order.
    pub accounts: Vec<AccountConfig>,
}

/// Local listener settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// Loopback address to bind. Non-loopback addresses are rejected.
    pub listen: SocketAddr,
    /// Where the local client key lives. `None` uses a generated key in
    /// `local-key` (mode 0600) next to the config file.
    pub local_key: Option<SecretRef>,
    /// Largest accepted request body.
    pub max_body_bytes: usize,
    /// How long a request may wait for a free account slot before `429`.
    pub queue_wait_ms: u64,
    /// Upstream connection setup timeout (TCP connect and TLS handshake).
    pub connect_timeout_ms: u64,
    /// Longest wait for upstream response headers after the request is
    /// sent. Expiry is reported as ambiguous and never retried.
    pub response_header_timeout_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            listen: SocketAddr::from(([127, 0, 0, 1], 8787)),
            local_key: None,
            max_body_bytes: 32 * 1024 * 1024,
            queue_wait_ms: 2_000,
            connect_timeout_ms: 10_000,
            response_header_timeout_ms: 600_000,
        }
    }
}

/// Placement settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RoutingConfig {
    /// Placement strategy for new sessions.
    pub strategy: Strategy,
    /// Keep a session on the account it started on.
    pub sticky: bool,
    /// Forget a session binding after this long without requests.
    pub session_ttl_minutes: u32,
    /// Skip accounts with less than this percent left while any account has more.
    pub skip_below_pct: f32,
    /// Pool used by router-owned requests without `x-relay-pool`.
    pub default_pool: Pool,
    /// How long to hold response headers while watching for an in-stream
    /// rejection that can still fail over. 0 disables the hold.
    pub bootstrap_hold_ms: u64,
    /// Idle quota polling. Always off in this version: no documented,
    /// non-billed quota read exists for subscriptions.
    pub idle_poll: bool,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        RoutingConfig {
            strategy: Strategy::QuotaAware,
            sticky: true,
            session_ttl_minutes: 6 * 60,
            skip_below_pct: 5.0,
            default_pool: Pool::Pooled,
            bootstrap_hold_ms: 1_500,
            idle_poll: false,
        }
    }
}

/// Upstream allow-list. Relay never forwards to an origin outside it, so it
/// cannot be used as a generic credential-forwarding proxy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UpstreamsConfig {
    /// Allowed origins (`scheme://host[:port]`).
    pub allowed: Vec<String>,
}

/// Documented Anthropic API origin.
pub const ANTHROPIC_API: &str = "https://api.anthropic.com";
/// Documented OpenAI API base.
pub const OPENAI_API: &str = "https://api.openai.com/v1";
/// ChatGPT-login Codex backend. Not a documented public API.
pub const CHATGPT_CODEX: &str = "https://chatgpt.com/backend-api/codex";
/// Kimi Code base on kimi.com: Messages at `/coding/v1/messages`.
pub const KIMI_COM_CODING: &str = "https://api.kimi.com/coding";
/// Kimi Code Responses base on kimi.com: `/coding/v1/responses`.
pub const KIMI_COM_CODING_V1: &str = "https://api.kimi.com/coding/v1";
/// Kimi Code base on kimi.ai: Messages at `/coding/v1/messages`.
pub const KIMI_AI_CODING: &str = "https://api.kimi.ai/coding";
/// Kimi Code Responses base on kimi.ai: `/coding/v1/responses`.
pub const KIMI_AI_CODING_V1: &str = "https://api.kimi.ai/coding/v1";
/// xAI public API. Not the Grok CLI chat proxy, which needs CLI identity.
pub const XAI_API: &str = "https://api.x.ai/v1";
/// Meta AI API. A sign-in may report another base URL; Relay ignores it.
pub const META_API: &str = "https://api.meta.ai/v1";

/// Canonical upstream base for a vendor, wire and credential kind; `None`
/// when the vendor does not serve that wire or credential kind. This
/// catalog is the only place new-vendor destinations come from.
pub fn vendor_base(vendor: Vendor, wire: Provider, kind: CredentialKind) -> Option<&'static str> {
    if !vendor.serves(wire) || !vendor.accepts(kind) {
        return None;
    }
    Some(match (vendor, wire, kind) {
        (Vendor::Anthropic, _, _) => ANTHROPIC_API,
        (Vendor::Openai, _, CredentialKind::ApiKey) => OPENAI_API,
        (Vendor::Openai, _, CredentialKind::Subscription) => CHATGPT_CODEX,
        (Vendor::KimiCom, Provider::Anthropic, _) => KIMI_COM_CODING,
        (Vendor::KimiCom, Provider::Openai, _) => KIMI_COM_CODING_V1,
        (Vendor::KimiAi, Provider::Anthropic, _) => KIMI_AI_CODING,
        (Vendor::KimiAi, Provider::Openai, _) => KIMI_AI_CODING_V1,
        (Vendor::Xai, _, _) => XAI_API,
        (Vendor::Meta, _, _) => META_API,
    })
}

/// The non-first-party vendor whose domain contains `host` (canonical, see
/// [`canonical_host`]), on any port. An account of any other vendor may
/// never target it, so a token can't be sent to the wrong vendor.
fn vendor_owning(host: &str) -> Option<Vendor> {
    [
        (Vendor::KimiCom, "kimi.com"),
        (Vendor::KimiAi, "kimi.ai"),
        (Vendor::Xai, "x.ai"),
        (Vendor::Meta, "meta.ai"),
    ]
    .into_iter()
    .find(|(_, d)| {
        host.strip_suffix(d)
            .is_some_and(|rest| rest.is_empty() || rest.ends_with('.'))
    })
    .map(|(v, _)| v)
}

/// Valid device ids are 1–128 chars of `[A-Za-z0-9-_.]`, safe as a header
/// value.
fn device_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

impl Default for UpstreamsConfig {
    fn default() -> Self {
        UpstreamsConfig {
            allowed: vec![
                "https://api.anthropic.com".into(),
                "https://api.openai.com".into(),
                "https://chatgpt.com".into(),
            ],
        }
    }
}

/// Usage recording settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UsageConfig {
    /// Record usage at all.
    pub enabled: bool,
    /// Keep per-request rows this long.
    pub raw_retention_days: u32,
    /// Keep hourly totals this long.
    pub hourly_retention_days: u32,
}

impl Default for UsageConfig {
    fn default() -> Self {
        UsageConfig {
            enabled: true,
            raw_retention_days: 7,
            hourly_retention_days: 400,
        }
    }
}

/// Through Relay period shown by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PeriodKind {
    /// Calendar week.
    #[default]
    Week,
    /// Calendar month.
    Month,
}

/// Appearance preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow the OS.
    #[default]
    System,
    /// Light.
    Light,
    /// Dark.
    Dark,
}

/// Durable desktop preferences.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiPrefs {
    /// Through Relay period.
    pub period: PeriodKind,
    /// Theme.
    pub theme: Theme,
    /// First weekday, 0 = Monday … 6 = Sunday. `None` follows the locale.
    pub week_start: Option<u8>,
    /// Expanded provider groups in the tray popover (group keys).
    pub expanded: Vec<String>,
}

/// One account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountConfig {
    /// Stable id (`[a-z0-9-_]`, ≤64). Clients name it in `x-relay-account`.
    pub id: AccountId,
    /// Wire protocol the account serves (Messages or Responses).
    pub provider: Provider,
    /// Vendor operating the upstream. `None` is the wire's first-party
    /// vendor, so configs written before vendors existed are unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<Vendor>,
    /// Per-install device id the vendor bound at sign-in (Kimi
    /// `X-Msh-Device-Id`). An identifier, not a secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// Stable display label (Personal, Work, …).
    pub label: String,
    /// Routing mode.
    pub mode: AccountMode,
    /// Plan label, e.g. `Max 20×`, `Plus`. Provider-reported plan wins when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// Masked identifier for display. Stored already masked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    /// Overrides the credential kind implied by `mode` (an observed client
    /// may log in with an API key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_kind: Option<CredentialKind>,
    /// Upstream base URL. Defaults per vendor, wire and credential kind;
    /// must be allowed. New vendors accept only their catalog base, or an
    /// allowed loopback origin for local testing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    /// Credential location for pooled and key accounts. Observed accounts
    /// must not have one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<SecretRef>,
    /// Paused accounts receive no new router-owned sessions.
    #[serde(default)]
    pub paused: bool,
    /// Concurrent in-flight requests allowed.
    #[serde(default = "default_in_flight")]
    pub max_in_flight: u32,
    /// Model allow-list. Entries ending in `*` match by prefix. Empty allows all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
}

fn default_in_flight() -> u32 {
    8
}

impl AccountConfig {
    /// Credential kind for grouping.
    pub fn credential_kind(&self) -> CredentialKind {
        self.credential_kind
            .unwrap_or_else(|| CredentialKind::for_mode(self.mode))
    }

    /// Vendor with the first-party default applied.
    pub fn vendor(&self) -> Vendor {
        self.vendor
            .unwrap_or_else(|| Vendor::first_party(self.provider))
    }

    /// Upstream base URL with defaults applied. Empty when the vendor does
    /// not serve this wire or credential kind ([`Config::validate`]
    /// rejects that).
    pub fn upstream_base(&self) -> &str {
        if let Some(u) = &self.upstream {
            return u.trim_end_matches('/');
        }
        vendor_base(self.vendor(), self.provider, self.credential_kind()).unwrap_or("")
    }

    /// Whether the allow-list admits `model`.
    pub fn supports_model(&self, model: &str) -> bool {
        self.models.is_empty()
            || self.models.iter().any(|m| match m.strip_suffix('*') {
                Some(prefix) => model.starts_with(prefix),
                None => m == model,
            })
    }
}

/// Where a secret lives. Relay reads it only when an account using it is
/// enabled, never logs it, and never writes it back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum SecretRef {
    /// An environment variable of the Relay process.
    Env {
        /// Variable name.
        env: String,
    },
    /// A file. Relay re-reads it when it changes, so a CLI that refreshes
    /// its own login keeps ownership of the refresh.
    File {
        /// Path.
        file: PathBuf,
        /// How to extract the token.
        #[serde(default)]
        format: SecretFormat,
    },
    /// macOS Keychain generic password, or Secret Service on Linux.
    Keychain {
        /// Service name.
        keychain_service: String,
        /// Account name.
        keychain_account: String,
        /// How to extract the token.
        #[serde(default)]
        format: SecretFormat,
    },
}

/// Secret content format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecretFormat {
    /// The whole value (trimmed) is the token.
    #[default]
    Raw,
    /// Codex `auth.json`: `tokens.access_token` and `tokens.account_id`.
    CodexAuthJson,
    /// Claude Code credentials JSON: `claudeAiOauth.accessToken` and `expiresAt`.
    ClaudeCredentialsJson,
}

/// A configuration problem, phrased for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(pub String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

/// Splits a URL into `(scheme, host, port, path)` without a URL crate.
///
/// Strict, so no other parser can read a different host: rejects userinfo,
/// `?`, `#`, `%`, `\`, whitespace, control and non-ASCII characters
/// anywhere, hosts that are neither DNS names (`[A-Za-z0-9-]` labels, one
/// optional trailing dot) nor bracketed IPv6 literals, and bad ports. The
/// host is returned as written; compare it with [`canonical_host`].
pub fn split_url(url: &str) -> Option<(&str, &str, Option<u16>, &str)> {
    if url
        .bytes()
        .any(|b| matches!(b, b'?' | b'#' | b'%' | b'\\' | b'@') || !b.is_ascii_graphic())
    {
        return None;
    }
    let (scheme, rest) = url.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (ip, after) = inner.split_once(']')?;
        ip.parse::<std::net::Ipv6Addr>().ok()?;
        let port = match after {
            "" => None,
            p => Some(p.strip_prefix(':')?.parse().ok()?),
        };
        (&authority[..ip.len() + 2], port)
    } else {
        let (host, port) = match authority.split_once(':') {
            Some((h, p)) => (h, Some(p.parse().ok()?)),
            None => (authority, None),
        };
        let name = host.strip_suffix('.').unwrap_or(host);
        let dns = !name.is_empty()
            && name.split('.').all(|l| {
                !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            });
        if !dns {
            return None;
        }
        (host, port)
    };
    Some((scheme, host, port, path))
}

/// Lower-case host without the trailing DNS dot: the form every origin
/// comparison uses.
fn canonical_host(host: &str) -> String {
    host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase()
}

/// `scheme://host[:port]` with a canonical host and the scheme's default
/// port omitted, so spellings of one origin compare equal.
fn origin_of(url: &str) -> Option<String> {
    let (scheme, host, port, _) = split_url(url)?;
    let host = canonical_host(host);
    Some(
        match port.filter(|p| !matches!((scheme, *p), ("https", 443) | ("http", 80))) {
            Some(p) => format!("{scheme}://{host}:{p}"),
            None => format!("{scheme}://{host}"),
        },
    )
}

fn is_loopback_host(host: &str) -> bool {
    let host = canonical_host(host);
    host == "localhost"
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

impl Config {
    /// Checks invariants. Returns every problem, not just the first.
    pub fn validate(&self) -> Result<(), Vec<ConfigError>> {
        let mut errs = Vec::new();
        let mut err = |s: String| errs.push(ConfigError(s));
        if !self.server.listen.ip().is_loopback() {
            err(format!(
                "server.listen {} is not a loopback address; Relay only serves this machine",
                self.server.listen
            ));
        }
        if self.server.connect_timeout_ms == 0 || self.server.response_header_timeout_ms == 0 {
            err("server timeouts must be greater than 0".into());
        }
        if self.server.max_body_bytes < 1024 {
            err("server.max_body_bytes must be at least 1024".into());
        }
        if !(0.0..=100.0).contains(&self.routing.skip_below_pct) {
            err("routing.skip_below_pct must be 0–100".into());
        }
        if self.routing.idle_poll {
            err(
                "routing.idle_poll is not supported: no documented non-billed quota read exists"
                    .into(),
            );
        }
        let mut allowed = Vec::new();
        for a in &self.upstreams.allowed {
            match origin_of(a) {
                Some(o) => {
                    let (scheme, host, _, _) = split_url(a).unwrap_or(("", "", None, ""));
                    if scheme == "http" && !is_loopback_host(host) {
                        err(format!(
                            "upstreams.allowed {a}: plain http is allowed only for loopback"
                        ));
                    }
                    allowed.push(o);
                }
                None => err(format!("upstreams.allowed {a}: not an http(s) origin")),
            }
        }
        let mut seen = std::collections::HashSet::new();
        for acc in &self.accounts {
            let id = acc.id.as_str();
            if !AccountId::is_valid(id) {
                err(format!("account id {id:?}: use 1–64 of a-z, 0-9, - and _"));
            }
            if !seen.insert(id) {
                err(format!("account id {id:?} is duplicated"));
            }
            if acc.label.trim().is_empty() {
                err(format!("account {id}: label is empty"));
            }
            if acc.max_in_flight == 0 {
                err(format!("account {id}: max_in_flight must be at least 1"));
            }
            match (acc.mode, &acc.credential) {
                (AccountMode::Observed, Some(_)) => err(format!(
                    "account {id}: observed accounts use the client's own login and must not name a credential"
                )),
                (AccountMode::Pooled | AccountMode::Key, None) => err(format!(
                    "account {id}: {} accounts need a credential reference",
                    acc.mode.label()
                )),
                _ => {}
            }
            if acc.mode == AccountMode::Pooled
                && acc.credential_kind() != CredentialKind::Subscription
            {
                err(format!("account {id}: pooled accounts are subscriptions"));
            }
            if acc.mode == AccountMode::Key && acc.credential_kind() != CredentialKind::ApiKey {
                err(format!("account {id}: key accounts use API keys"));
            }
            let vendor = acc.vendor();
            if !vendor.serves(acc.provider) {
                err(format!(
                    "account {id}: {} does not serve the {} wire",
                    vendor.label(),
                    acc.provider.label()
                ));
                continue;
            }
            if !vendor.accepts(acc.credential_kind()) {
                err(format!(
                    "account {id}: {} accounts are subscription sign-ins, not API keys",
                    vendor.label()
                ));
                continue;
            }
            match (&acc.device_id, vendor.needs_device_id()) {
                (Some(d), true) if !device_id_ok(d) => err(format!(
                    "account {id}: device_id must be 1–128 of A-Z, a-z, 0-9, -, _ and ."
                )),
                (Some(_), false) => err(format!(
                    "account {id}: device_id is only used by Kimi accounts"
                )),
                (None, true) if acc.mode.router_owned() => err(format!(
                    "account {id}: {} needs the device id from its sign-in",
                    vendor.label()
                )),
                _ => {}
            }
            let base = acc.upstream_base();
            let (Some(o), Some((_, host, _, _))) = (origin_of(base), split_url(base)) else {
                err(format!(
                    "account {id}: upstream {base:?} is not an http(s) URL"
                ));
                continue;
            };
            if let Some(other) = vendor_owning(&canonical_host(host)).filter(|v| *v != vendor) {
                err(format!(
                    "account {id}: upstream {o} belongs to {}, not {}",
                    other.label(),
                    vendor.label()
                ));
                continue;
            }
            let catalog = vendor_base(vendor, acc.provider, acc.credential_kind());
            let ok = if vendor.is_first_party() {
                allowed.contains(&o)
            } else {
                // Exactly the catalog base, or an allow-listed loopback
                // origin (local tests). Never another remote origin.
                Some(base) == catalog || (allowed.contains(&o) && is_loopback_host(host))
            };
            if !ok {
                err(if vendor.is_first_party() {
                    format!("account {id}: upstream {o} is not in upstreams.allowed")
                } else {
                    format!(
                        "account {id}: {} accounts must use {}",
                        vendor.label(),
                        catalog.unwrap_or("its catalog upstream")
                    )
                });
            }
        }
        if errs.is_empty() { Ok(()) } else { Err(errs) }
    }

    /// Account by id.
    pub fn account(&self, id: &str) -> Option<&AccountConfig> {
        self.accounts.iter().find(|a| a.id.as_str() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(id: &str, mode: AccountMode, credential: Option<SecretRef>) -> AccountConfig {
        AccountConfig {
            id: AccountId(id.into()),
            provider: Provider::Anthropic,
            vendor: None,
            device_id: None,
            label: "Personal".into(),
            mode,
            plan: None,
            identifier: None,
            credential_kind: None,
            upstream: None,
            credential,
            paused: false,
            max_in_flight: 4,
            models: vec![],
        }
    }

    fn env(name: &str) -> Option<SecretRef> {
        Some(SecretRef::Env { env: name.into() })
    }

    #[test]
    fn default_config_is_valid_and_empty() {
        let c = Config::default();
        assert!(c.validate().is_ok());
        assert!(c.accounts.is_empty());
        assert!(c.server.listen.ip().is_loopback());
    }

    #[test]
    fn rejects_non_loopback_listener() {
        let mut c = Config::default();
        c.server.listen = "0.0.0.0:8787".parse().unwrap();
        assert!(c.validate().is_err());
    }

    #[test]
    fn mode_and_credential_must_agree() {
        let mut c = Config {
            accounts: vec![acc("a", AccountMode::Observed, env("X"))],
            ..Default::default()
        };
        assert!(c.validate().is_err());
        c.accounts = vec![acc("a", AccountMode::Pooled, None)];
        assert!(c.validate().is_err());
        c.accounts = vec![acc("a", AccountMode::Pooled, env("X"))];
        assert!(c.validate().is_ok());
        // A key account can't claim to be a subscription.
        let mut k = acc("k", AccountMode::Key, env("K"));
        k.credential_kind = Some(CredentialKind::Subscription);
        c.accounts = vec![k];
        assert!(c.validate().is_err());
    }

    #[test]
    fn upstream_must_be_allowed() {
        let mut c = Config::default();
        let mut a = acc("a", AccountMode::Observed, None);
        a.upstream = Some("https://evil.example.com".into());
        c.accounts = vec![a.clone()];
        assert!(c.validate().is_err());
        c.upstreams.allowed.push("https://evil.example.com".into());
        assert!(c.validate().is_ok());
        // Plain http only to loopback.
        c.upstreams.allowed = vec!["http://example.com".into()];
        assert!(c.validate().is_err());
        c.upstreams.allowed = vec!["http://127.0.0.1:9000".into()];
        a.upstream = Some("http://127.0.0.1:9000".into());
        c.accounts = vec![a];
        assert!(c.validate().is_ok());
    }

    #[test]
    fn duplicate_and_invalid_ids() {
        let c = Config {
            accounts: vec![
                acc("a", AccountMode::Observed, None),
                acc("a", AccountMode::Observed, None),
                acc("Bad Id", AccountMode::Observed, None),
            ],
            ..Default::default()
        };
        assert_eq!(c.validate().unwrap_err().len(), 2);
    }

    #[test]
    fn default_upstreams_by_provider_and_credential() {
        let mut a = acc("a", AccountMode::Observed, None);
        assert_eq!(a.upstream_base(), ANTHROPIC_API);
        a.provider = Provider::Openai;
        assert_eq!(a.upstream_base(), CHATGPT_CODEX);
        a.credential_kind = Some(CredentialKind::ApiKey);
        assert_eq!(a.upstream_base(), OPENAI_API);
    }

    fn kimi(id: &str, wire: Provider, mode: AccountMode) -> AccountConfig {
        let mut a = acc(
            id,
            mode,
            (mode != AccountMode::Observed).then(|| env("K")).flatten(),
        );
        a.provider = wire;
        a.vendor = Some(Vendor::KimiCom);
        a.device_id = Some("0b9e7c1e-device".into());
        a
    }

    #[test]
    fn vendor_defaults_keep_old_configs_byte_identical() {
        let a = acc("a", AccountMode::Observed, None);
        assert_eq!(a.vendor(), Vendor::Anthropic);
        let text = toml::to_string(&a).unwrap();
        assert!(!text.contains("vendor") && !text.contains("device_id"));
    }

    #[test]
    fn vendor_catalog_bases_per_wire() {
        let mut k = kimi("k", Provider::Anthropic, AccountMode::Pooled);
        assert_eq!(k.upstream_base(), "https://api.kimi.com/coding");
        k.provider = Provider::Openai;
        assert_eq!(k.upstream_base(), "https://api.kimi.com/coding/v1");
        k.vendor = Some(Vendor::KimiAi);
        assert_eq!(k.upstream_base(), "https://api.kimi.ai/coding/v1");
        let mut x = acc("x", AccountMode::Key, env("X"));
        x.provider = Provider::Openai;
        x.vendor = Some(Vendor::Xai);
        assert_eq!(x.upstream_base(), XAI_API);
        x.vendor = Some(Vendor::Meta);
        assert_eq!(x.upstream_base(), META_API);
        // Catalog origins need no allow-list entry for their own vendor.
        let c = Config {
            accounts: vec![kimi("k", Provider::Anthropic, AccountMode::Pooled), x],
            ..Default::default()
        };
        assert_eq!(c.validate(), Ok(()));
    }

    #[test]
    fn vendor_must_serve_wire_and_credential() {
        let mut x = acc("x", AccountMode::Pooled, env("X"));
        x.vendor = Some(Vendor::Xai); // xAI has no Messages route.
        let c = Config {
            accounts: vec![x],
            ..Default::default()
        };
        assert!(c.validate().is_err());
        let mut k = kimi("k", Provider::Openai, AccountMode::Key);
        k.credential_kind = None;
        let c = Config {
            accounts: vec![k],
            ..Default::default()
        };
        assert!(c.validate().is_err());
        // An Anthropic account can't claim the OpenAI vendor.
        let mut a = acc("a", AccountMode::Observed, None);
        a.vendor = Some(Vendor::Openai);
        let c = Config {
            accounts: vec![a],
            ..Default::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn kimi_router_owned_needs_a_valid_device_id_only_kimi_has_one() {
        let mut k = kimi("k", Provider::Anthropic, AccountMode::Pooled);
        k.device_id = None;
        let mut c = Config {
            accounts: vec![k.clone()],
            ..Default::default()
        };
        assert!(c.validate().is_err());
        k.device_id = Some("bad id\r\n".into());
        c.accounts = vec![k.clone()];
        assert!(c.validate().is_err());
        // Observed Kimi clients send their own device headers.
        let mut o = kimi("o", Provider::Anthropic, AccountMode::Observed);
        o.device_id = None;
        c.accounts = vec![o];
        assert_eq!(c.validate(), Ok(()));
        let mut a = acc("a", AccountMode::Pooled, env("A"));
        a.device_id = Some("dev".into());
        c.accounts = vec![a];
        assert!(c.validate().is_err());
    }

    #[test]
    fn vendor_origins_are_strict_in_both_directions() {
        // A new vendor can't be pointed at another remote origin, even one
        // the user allow-listed, nor at another path on its own origin.
        let mut k = kimi("k", Provider::Anthropic, AccountMode::Pooled);
        k.upstream = Some("https://evil.example.com".into());
        let mut c = Config {
            accounts: vec![k.clone()],
            ..Default::default()
        };
        c.upstreams.allowed.push("https://evil.example.com".into());
        assert!(c.validate().is_err());
        k.upstream = Some("https://api.kimi.com/other".into());
        c.accounts = vec![k.clone()];
        assert!(c.validate().is_err());
        // Nor at another vendor's catalog origin.
        k.upstream = Some(KIMI_AI_CODING.into());
        c.accounts = vec![k.clone()];
        assert!(c.validate().is_err());
        // A first-party account can't reach a new vendor's origin even when
        // it is allow-listed, so a Claude token never goes to Kimi.
        let mut a = acc("a", AccountMode::Pooled, env("A"));
        a.upstream = Some(KIMI_COM_CODING.into());
        c.upstreams.allowed.push("https://api.kimi.com".into());
        c.accounts = vec![a];
        assert!(c.validate().is_err());
        // Loopback only when allow-listed, as for first-party accounts.
        k.upstream = Some("http://127.0.0.1:9000".into());
        c.accounts = vec![k.clone()];
        assert!(c.validate().is_err());
        c.upstreams.allowed.push("http://127.0.0.1:9000".into());
        assert_eq!(c.validate(), Ok(()));

        // Spelling variants of another vendor's host stay blocked for a
        // first-party account even when that exact spelling is allow-listed:
        // explicit or other ports, trailing DNS dot, letter case, other
        // hosts on the vendor's domain.
        for (upstream, allowed) in [
            (
                "https://api.kimi.com:443/coding",
                "https://api.kimi.com:443",
            ),
            ("https://api.kimi.com./coding", "https://api.kimi.com."),
            ("https://API.Kimi.COM/coding", "https://API.Kimi.COM"),
            (
                "https://api.kimi.ai:8443/coding",
                "https://api.kimi.ai:8443",
            ),
            ("https://api.x.ai:443/v1", "https://api.x.ai:443"),
            ("https://api.x.ai./v1", "https://api.x.ai."),
            ("https://api.meta.ai:443/v1", "https://api.meta.ai:443"),
            ("https://graph.meta.ai/v1", "https://graph.meta.ai"),
        ] {
            let mut a = acc("a", AccountMode::Pooled, env("A"));
            a.upstream = Some(upstream.into());
            let mut c = Config {
                accounts: vec![a],
                ..Default::default()
            };
            c.upstreams.allowed.push(allowed.into());
            assert!(c.validate().is_err(), "first-party → {upstream}");
            // The new vendor itself also accepts only its exact catalog base.
            let mut k = kimi("k", Provider::Anthropic, AccountMode::Pooled);
            k.vendor = Some(Vendor::Xai);
            k.provider = Provider::Openai;
            k.device_id = None;
            k.upstream = Some(upstream.into());
            c.accounts = vec![k];
            assert!(c.validate().is_err(), "vendor → {upstream}");
        }
        // Query, fragment, escapes and backslashes in the authority are
        // refused outright, so no parser can disagree about the host.
        for bad in [
            "https://api.anthropic.com?.api.kimi.com",
            "https://api.anthropic.com#.api.kimi.com",
            "https://api.kimi%2ecom/coding",
            "https://api.anthropic.com\\.api.kimi.com",
            "https://api.anthropic.com\\@api.kimi.com",
        ] {
            assert_eq!(split_url(bad), None, "{bad}");
            let mut a = acc("a", AccountMode::Pooled, env("A"));
            a.upstream = Some(bad.into());
            let mut c = Config {
                accounts: vec![a],
                ..Default::default()
            };
            c.upstreams.allowed.push(bad.into());
            assert!(c.validate().is_err(), "{bad}");
        }
        // Custom first-party gateways keep working where allow-listed, and
        // the default port is the same origin as no port.
        for (upstream, allowed) in [
            (
                "https://gateway.example.com:8443/anthropic",
                "https://gateway.example.com:8443",
            ),
            ("https://api.anthropic.com:443", "https://api.anthropic.com"),
            (
                "https://Gateway.Example.com./claude",
                "https://gateway.example.com",
            ),
        ] {
            let mut a = acc("a", AccountMode::Pooled, env("A"));
            a.upstream = Some(upstream.into());
            let mut c = Config {
                accounts: vec![a],
                ..Default::default()
            };
            c.upstreams.allowed.push(allowed.into());
            assert_eq!(c.validate(), Ok(()), "{upstream}");
        }
    }

    #[test]
    fn model_allow_list() {
        let mut a = acc("a", AccountMode::Observed, None);
        assert!(a.supports_model("anything"));
        a.models = vec!["claude-sonnet-*".into(), "claude-fable-5-1".into()];
        assert!(a.supports_model("claude-sonnet-5-5"));
        assert!(a.supports_model("claude-fable-5-1"));
        assert!(!a.supports_model("claude-opus-5-5"));
    }

    #[test]
    fn split_url_parses_ports_and_paths() {
        assert_eq!(
            split_url("https://chatgpt.com/backend-api/codex"),
            Some(("https", "chatgpt.com", None, "/backend-api/codex"))
        );
        assert_eq!(
            split_url("http://127.0.0.1:9000"),
            Some(("http", "127.0.0.1", Some(9000), ""))
        );
        assert_eq!(split_url("ftp://x"), None);
        assert_eq!(split_url("https://user@host"), None);
    }

    #[test]
    fn secret_refs_round_trip_through_toml_shapes() {
        let text = r#"
            [[accounts]]
            id = "work"
            provider = "openai"
            label = "Work"
            mode = "pooled"
            credential = { file = "/tmp/auth.json", format = "codex-auth-json" }

            [[accounts]]
            id = "key"
            provider = "anthropic"
            label = "Console"
            mode = "key"
            credential = { keychain_service = "relay", keychain_account = "console" }
        "#;
        let c: Config = toml::from_str(text).unwrap();
        assert_eq!(
            c.accounts[0].credential,
            Some(SecretRef::File {
                file: "/tmp/auth.json".into(),
                format: SecretFormat::CodexAuthJson
            })
        );
        assert!(matches!(
            c.accounts[1].credential,
            Some(SecretRef::Keychain { .. })
        ));
        assert!(c.validate().is_ok());
    }
}
