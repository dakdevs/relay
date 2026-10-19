//! Configuration types and validation. Persistence lives in `relay-proxy`.
//!
//! Secrets are never stored in config: an account names *where* its
//! credential lives ([`SecretRef`]), and Relay resolves it at runtime.

use crate::model::{AccountId, AccountMode, CredentialKind, Pool, Provider};
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
    /// Provider.
    pub provider: Provider,
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
    /// Upstream base URL. Defaults per provider and mode; must be allowed.
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

    /// Upstream base URL with defaults applied.
    pub fn upstream_base(&self) -> &str {
        if let Some(u) = &self.upstream {
            return u.trim_end_matches('/');
        }
        match (self.provider, self.credential_kind()) {
            (Provider::Anthropic, _) => ANTHROPIC_API,
            (Provider::Openai, CredentialKind::ApiKey) => OPENAI_API,
            (Provider::Openai, CredentialKind::Subscription) => CHATGPT_CODEX,
        }
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
pub fn split_url(url: &str) -> Option<(&str, &str, Option<u16>, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !h.ends_with(']') || authority.starts_with('[') => {
            (h, Some(p.parse().ok()?))
        }
        _ => (authority, None),
    };
    Some((scheme, host, port, path))
}

fn origin_of(url: &str) -> Option<String> {
    let (scheme, host, port, _) = split_url(url)?;
    Some(match port {
        Some(p) => format!("{scheme}://{}:{p}", host.to_ascii_lowercase()),
        None => format!("{scheme}://{}", host.to_ascii_lowercase()),
    })
}

fn is_loopback_host(host: &str) -> bool {
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
            let base = acc.upstream_base();
            match origin_of(base) {
                Some(o) if allowed.contains(&o) => {}
                Some(o) => err(format!(
                    "account {id}: upstream {o} is not in upstreams.allowed"
                )),
                None => err(format!(
                    "account {id}: upstream {base:?} is not an http(s) URL"
                )),
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
