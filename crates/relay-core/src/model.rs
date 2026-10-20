//! Identifiers and enums that describe providers, accounts and routing modes.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Upstream provider family. Each speaks one wire API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// Anthropic Messages API (Claude Code).
    Anthropic,
    /// OpenAI Responses API (Codex).
    Openai,
}

impl Provider {
    /// All providers, in display order.
    pub const ALL: [Provider; 2] = [Provider::Anthropic, Provider::Openai];

    /// Human-readable provider name.
    pub fn label(self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic",
            Provider::Openai => "OpenAI",
        }
    }

    /// Stable lowercase key used in config, storage and logs.
    pub fn key(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::Openai => "openai",
        }
    }

    /// Parses [`Provider::key`].
    pub fn from_key(key: &str) -> Option<Provider> {
        match key {
            "anthropic" => Some(Provider::Anthropic),
            "openai" => Some(Provider::Openai),
            _ => None,
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

/// Who operates an account's upstream, separate from the wire protocol
/// ([`Provider`]) it speaks. Kimi serves both wires; xAI and Meta serve
/// Responses only. Routing never crosses vendors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Vendor {
    /// Anthropic (Claude subscriptions and Console keys).
    Anthropic,
    /// OpenAI (ChatGPT/Codex logins and Platform keys).
    Openai,
    /// Kimi Code on kimi.com.
    KimiCom,
    /// Kimi Code on kimi.ai.
    KimiAi,
    /// xAI (Grok) public API.
    Xai,
    /// Meta AI API.
    Meta,
}

impl Vendor {
    /// All vendors, in display order.
    pub const ALL: [Vendor; 6] = [
        Vendor::Anthropic,
        Vendor::Openai,
        Vendor::KimiCom,
        Vendor::KimiAi,
        Vendor::Xai,
        Vendor::Meta,
    ];

    /// Stable kebab-case key used in config, headers (`x-relay-vendor`) and
    /// group keys. Also the auth engine's vendor id.
    pub fn key(self) -> &'static str {
        match self {
            Vendor::Anthropic => "anthropic",
            Vendor::Openai => "openai",
            Vendor::KimiCom => "kimi-com",
            Vendor::KimiAi => "kimi-ai",
            Vendor::Xai => "xai",
            Vendor::Meta => "meta",
        }
    }

    /// Parses [`Vendor::key`].
    pub fn from_key(key: &str) -> Option<Vendor> {
        Vendor::ALL.into_iter().find(|v| v.key() == key.trim())
    }

    /// Human-readable vendor name.
    pub fn label(self) -> &'static str {
        match self {
            Vendor::Anthropic => "Anthropic",
            Vendor::Openai => "OpenAI",
            Vendor::KimiCom => "Kimi",
            Vendor::KimiAi => "Kimi (kimi.ai)",
            Vendor::Xai => "xAI",
            Vendor::Meta => "Meta",
        }
    }

    /// The vendor that owns `wire`; the default for accounts and requests
    /// that name no vendor.
    pub fn first_party(wire: Provider) -> Vendor {
        match wire {
            Provider::Anthropic => Vendor::Anthropic,
            Provider::Openai => Vendor::Openai,
        }
    }

    /// Whether this vendor owns a wire protocol.
    pub fn is_first_party(self) -> bool {
        matches!(self, Vendor::Anthropic | Vendor::Openai)
    }

    /// Wire protocols the vendor serves without translation.
    pub fn wires(self) -> &'static [Provider] {
        match self {
            Vendor::Anthropic => &[Provider::Anthropic],
            Vendor::Openai | Vendor::Xai | Vendor::Meta => &[Provider::Openai],
            Vendor::KimiCom | Vendor::KimiAi => &[Provider::Anthropic, Provider::Openai],
        }
    }

    /// Whether the vendor serves `wire`.
    pub fn serves(self, wire: Provider) -> bool {
        self.wires().contains(&wire)
    }

    /// Whether routing to this vendor has live proof. `false` means the
    /// route follows the vendor's documented or observed shape but has not
    /// been confirmed with a real account; never present it as working.
    pub fn verified(self) -> bool {
        self.is_first_party()
    }

    /// Whether the vendor accepts `kind` on Relay's routes. Kimi Code is a
    /// subscription login only; its platform keys use another origin.
    pub fn accepts(self, kind: CredentialKind) -> bool {
        !matches!(
            (self, kind),
            (Vendor::KimiCom | Vendor::KimiAi, CredentialKind::ApiKey)
        )
    }

    /// Whether router-owned accounts need a per-install device id that the
    /// vendor bound at sign-in (Kimi's `X-Msh-Device-Id`).
    pub fn needs_device_id(self) -> bool {
        matches!(self, Vendor::KimiCom | Vendor::KimiAi)
    }
}

impl fmt::Display for Vendor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

/// How Relay treats an account. Set per account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountMode {
    /// The client (one T3 instance) keeps its own login; Relay forwards the
    /// client's credential unchanged and only records usage and quota.
    Observed,
    /// Opt-in own-account subscription pool: Relay holds a reference to this
    /// account's subscription token and binds each new session to the pooled
    /// account with most headroom. Provider terms for this are unresolved.
    Pooled,
    /// API key billed to the key owner (Anthropic Console / OpenAI Platform).
    Key,
}

impl AccountMode {
    /// Whether Relay chooses this account (and injects its credential).
    pub fn router_owned(self) -> bool {
        !matches!(self, AccountMode::Observed)
    }

    /// Display label.
    pub fn label(self) -> &'static str {
        match self {
            AccountMode::Observed => "Observed",
            AccountMode::Pooled => "Pooled",
            AccountMode::Key => "API key",
        }
    }
}

/// Which pool a router-owned request draws from. Subscription and API-key
/// accounts are never mixed, so a key is never silently substituted for a
/// subscription (or the reverse).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pool {
    /// Accounts in [`AccountMode::Pooled`].
    Pooled,
    /// Accounts in [`AccountMode::Key`].
    Key,
}

impl Pool {
    /// The account mode that belongs to this pool.
    pub fn mode(self) -> AccountMode {
        match self {
            Pool::Pooled => AccountMode::Pooled,
            Pool::Key => AccountMode::Key,
        }
    }

    /// Parses the `x-relay-pool` header value.
    pub fn from_key(key: &str) -> Option<Pool> {
        match key.trim() {
            "pooled" | "subscription" => Some(Pool::Pooled),
            "key" | "api" | "api-key" => Some(Pool::Key),
            _ => None,
        }
    }
}

/// Kind of credential that pays for an account's traffic; part of the group key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialKind {
    /// A subscription login (Claude.ai / ChatGPT).
    Subscription,
    /// An API key.
    ApiKey,
}

impl CredentialKind {
    /// Credential kind implied by a mode. Observed accounts default to
    /// subscription; config may override it.
    pub fn for_mode(mode: AccountMode) -> CredentialKind {
        match mode {
            AccountMode::Key => CredentialKind::ApiKey,
            _ => CredentialKind::Subscription,
        }
    }
}

/// Stable account identifier from config, e.g. `claude-personal`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(pub String);

impl AccountId {
    /// Valid ids are 1–64 chars of `[a-z0-9-_]`, so they are safe in headers,
    /// file names and SQL text.
    pub fn is_valid(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    }

    /// Borrowed id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Masks an identifier for display: `dak@example.com` → `d••@example.com`,
/// `sk-ant-abc123xyz` → `sk-…xyz`. Never returns more than the first and
/// last few characters.
pub fn mask_identifier(raw: &str) -> String {
    let raw = raw.trim();
    if let Some((user, domain)) = raw.split_once('@') {
        let first: String = user.chars().take(1).collect();
        return format!("{first}••@{domain}");
    }
    let chars: Vec<char> = raw.chars().collect();
    if chars.len() <= 6 {
        return "••••".into();
    }
    let head: String = chars[..3].iter().collect();
    let tail: String = chars[chars.len() - 3..].iter().collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_email_and_keys_without_leaking_middle() {
        assert_eq!(mask_identifier("dak@example.com"), "d••@example.com");
        assert_eq!(mask_identifier("sk-ant-api03-SECRETVALUE-xyz"), "sk-…xyz");
        assert_eq!(mask_identifier("short"), "••••");
        assert!(!mask_identifier("sk-ant-api03-SECRETVALUE-xyz").contains("SECRET"));
    }

    #[test]
    fn account_id_rules() {
        assert!(AccountId::is_valid("claude-work_2"));
        assert!(!AccountId::is_valid("Claude"));
        assert!(!AccountId::is_valid("a b"));
        assert!(!AccountId::is_valid(""));
        assert!(!AccountId::is_valid(&"a".repeat(65)));
    }

    #[test]
    fn vendor_keys_round_trip_and_wires_are_exact() {
        #[derive(Deserialize)]
        struct W {
            v: Vendor,
        }
        for v in Vendor::ALL {
            assert_eq!(Vendor::from_key(v.key()), Some(v));
            let w: W = toml::from_str(&format!("v = \"{}\"", v.key())).unwrap();
            assert_eq!(w.v, v);
        }
        assert_eq!(Vendor::from_key("kimi"), None);
        assert_eq!(Vendor::first_party(Provider::Anthropic), Vendor::Anthropic);
        assert_eq!(Vendor::first_party(Provider::Openai), Vendor::Openai);
        assert!(
            Vendor::KimiAi.serves(Provider::Anthropic) && Vendor::KimiAi.serves(Provider::Openai)
        );
        assert!(!Vendor::Xai.serves(Provider::Anthropic));
        assert!(!Vendor::Meta.serves(Provider::Anthropic));
        assert!(!Vendor::Anthropic.serves(Provider::Openai));
        // No live proof yet for the new vendors.
        assert!(!Vendor::KimiCom.verified() && !Vendor::Xai.verified() && !Vendor::Meta.verified());
        assert!(!Vendor::KimiCom.accepts(CredentialKind::ApiKey));
        assert!(Vendor::Xai.accepts(CredentialKind::ApiKey));
    }

    #[test]
    fn pools_never_cross_modes() {
        assert_eq!(Pool::Pooled.mode(), AccountMode::Pooled);
        assert_eq!(Pool::Key.mode(), AccountMode::Key);
        assert_eq!(Pool::from_key("subscription"), Some(Pool::Pooled));
        assert_eq!(Pool::from_key("bogus"), None);
    }
}
