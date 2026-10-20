//! Copyable client settings for a T3 provider instance. Relay never writes
//! T3 or CLI settings itself; the user pastes these.

use relay_core::config::AccountConfig;
use relay_core::model::{AccountMode, Pool, Provider};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Placeholder shown instead of the local key.
pub const KEY_PLACEHOLDER: &str = "<relay local key>";

/// One value to copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupEntry {
    /// Where it goes (env var name, `launchArgs`, …).
    pub name: String,
    /// Value. Contains [`KEY_PLACEHOLDER`] where the local key belongs.
    pub value: String,
    /// Mark as sensitive in T3.
    pub sensitive: bool,
}

/// Settings for one account's client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientSetup {
    /// Provider.
    pub provider: Provider,
    /// Mode.
    pub mode: AccountMode,
    /// Values.
    pub entries: Vec<SetupEntry>,
    /// What to leave unset and why.
    pub notes: Vec<String>,
}

fn entry(name: &str, value: String, sensitive: bool) -> SetupEntry {
    SetupEntry {
        name: name.into(),
        value,
        sensitive,
    }
}

/// Builds the settings for `account` against a proxy at `addr`.
pub fn client_setup(account: &AccountConfig, addr: SocketAddr) -> ClientSetup {
    let base = format!("http://{addr}");
    let id = account.id.as_str();
    let pool = match account.mode {
        AccountMode::Key => Some(Pool::Key),
        AccountMode::Pooled => Some(Pool::Pooled),
        AccountMode::Observed => None,
    };
    let pool_key = |p: Pool| match p {
        Pool::Pooled => "pooled",
        Pool::Key => "key",
    };
    // Non-first-party vendors are selected explicitly; without the header a
    // request routes to the wire's first-party vendor.
    let vendor = account.vendor();
    let (vline, vtoml) = if vendor.is_first_party() {
        (String::new(), String::new())
    } else {
        (
            format!("\nx-relay-vendor: {}", vendor.key()),
            format!(",\"x-relay-vendor\"=\"{}\"", vendor.key()),
        )
    };
    let (entries, mut notes) = match (account.provider, pool) {
        (Provider::Anthropic, None) => (
            vec![
                entry("ANTHROPIC_BASE_URL", base, false),
                entry(
                    "ANTHROPIC_CUSTOM_HEADERS",
                    format!("x-relay-key: {KEY_PLACEHOLDER}\nx-relay-account: {id}{vline}"),
                    true,
                ),
            ],
            vec![
                "Keep this instance's own claude.ai login (its CLAUDE_CONFIG_DIR).".into(),
                "Do not set ANTHROPIC_AUTH_TOKEN, ANTHROPIC_API_KEY or apiKeyHelper: any of them replaces the subscription login.".into(),
            ],
        ),
        (Provider::Anthropic, Some(p)) => (
            vec![
                entry("ANTHROPIC_BASE_URL", base, false),
                entry("ANTHROPIC_AUTH_TOKEN", KEY_PLACEHOLDER.into(), true),
                entry("ANTHROPIC_API_KEY", String::new(), false),
                entry("ANTHROPIC_CUSTOM_HEADERS", format!("x-relay-pool: {}{vline}", pool_key(p)), false),
            ],
            vec![
                "Run /logout in this instance's config directory so the CLI sends Relay's key.".into(),
                "Relay picks the account; T3 may no longer show subscription limits for this instance.".into(),
            ],
        ),
        (Provider::Openai, None) => (
            vec![
                entry(
                    "launchArgs",
                    format!(
                        "-c model_provider=\"relay\" -c 'model_providers.relay={{name=\"Relay\",base_url=\"{base}/v1\",wire_api=\"responses\",requires_openai_auth=true,supports_websockets=false,http_headers={{\"x-relay-account\"=\"{id}\"{vtoml}}},env_http_headers={{\"x-relay-key\"=\"RELAY_KEY\"}}}}'"
                    ),
                    false,
                ),
                entry("RELAY_KEY", KEY_PLACEHOLDER.into(), true),
            ],
            vec![
                "Use setupMode \"existing\" with this account's own CODEX_HOME logged in with ChatGPT; managed Codex can't be routed.".into(),
                "Relay forwards the ChatGPT login unchanged to the account's upstream (default chatgpt.com/backend-api/codex, not a documented public API).".into(),
            ],
        ),
        (Provider::Openai, Some(p)) => (
            vec![
                entry(
                    "launchArgs",
                    format!(
                        "-c model_provider=\"relay\" -c 'model_providers.relay={{name=\"Relay\",base_url=\"{base}/v1\",wire_api=\"responses\",env_key=\"RELAY_KEY\",requires_openai_auth=false,supports_websockets=false,http_headers={{\"x-relay-pool\"=\"{}\"{vtoml}}}}}'",
                        pool_key(p)
                    ),
                    false,
                ),
                entry("RELAY_KEY", KEY_PLACEHOLDER.into(), true),
            ],
            vec!["Use setupMode \"existing\"; Relay picks the account.".into()],
        ),
    };
    if !vendor.is_first_party() {
        // Only first-party routes are proven; first-party notes would be
        // wrong here.
        notes = vec![
            format!(
                "Use {} model ids; Relay forwards the request without translation.",
                vendor.label()
            ),
            format!(
                "Relay's {} route is unverified with a live account.",
                vendor.label()
            ),
        ];
    }
    ClientSetup {
        provider: account.provider,
        mode: account.mode,
        entries,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_core::model::AccountId;

    fn acc(provider: Provider, mode: AccountMode) -> AccountConfig {
        AccountConfig {
            id: AccountId("work".into()),
            provider,
            vendor: None,
            device_id: None,
            label: "Work".into(),
            mode,
            plan: None,
            identifier: None,
            credential_kind: None,
            upstream: None,
            credential: None,
            paused: false,
            max_in_flight: 8,
            models: vec![],
        }
    }

    #[test]
    fn observed_claude_never_sets_a_credential_variable() {
        let s = client_setup(
            &acc(Provider::Anthropic, AccountMode::Observed),
            "127.0.0.1:8787".parse().unwrap(),
        );
        let names: Vec<&str> = s.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["ANTHROPIC_BASE_URL", "ANTHROPIC_CUSTOM_HEADERS"]);
        assert!(s.entries[1].value.contains("x-relay-account: work"));
        assert!(s.entries[1].sensitive);
    }

    #[test]
    fn codex_observed_requires_openai_auth_and_key_mode_does_not() {
        let addr = "127.0.0.1:9".parse().unwrap();
        let o = client_setup(&acc(Provider::Openai, AccountMode::Observed), addr);
        assert!(o.entries[0].value.contains("requires_openai_auth=true"));
        assert!(!o.entries[0].value.contains("env_key"));
        let k = client_setup(&acc(Provider::Openai, AccountMode::Key), addr);
        assert!(k.entries[0].value.contains("requires_openai_auth=false"));
        assert!(k.entries[0].value.contains("\"x-relay-pool\"=\"key\""));
        // The real key never appears.
        assert!(k.entries.iter().all(|e| !e.value.starts_with("relay-")));
    }

    #[test]
    fn other_vendors_name_themselves_and_claim_nothing_verified() {
        let addr = "127.0.0.1:9".parse().unwrap();
        let mut a = acc(Provider::Anthropic, AccountMode::Pooled);
        a.vendor = Some(relay_core::model::Vendor::KimiCom);
        let s = client_setup(&a, addr);
        let h = &s
            .entries
            .iter()
            .find(|e| e.name == "ANTHROPIC_CUSTOM_HEADERS")
            .unwrap()
            .value;
        assert_eq!(h, "x-relay-pool: pooled\nx-relay-vendor: kimi-com");
        assert!(s.notes.iter().any(|n| n.contains("unverified")));
        let mut x = acc(Provider::Openai, AccountMode::Observed);
        x.vendor = Some(relay_core::model::Vendor::Xai);
        let s = client_setup(&x, addr);
        assert!(
            s.entries[0]
                .value
                .contains(r#""x-relay-account"="work","x-relay-vendor"="xai""#)
        );
        // First-party output is unchanged.
        let o = client_setup(&acc(Provider::Openai, AccountMode::Observed), addr);
        assert!(!o.entries[0].value.contains("x-relay-vendor"));
    }
}
