//! Upstream credentials for router-owned accounts: extraction from the
//! supported secret formats, and injection into request headers.
//!
//! Relay never synthesises client identity: it adds only the credential and
//! the provider's required capability flag for that credential type.

use http::{HeaderMap, HeaderValue, header};
use relay_core::config::SecretFormat;
use relay_core::model::{CredentialKind, Provider, Vendor};
use std::fmt;

/// A secret string. `Debug` and `Display` never print it.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wraps a value.
    pub fn new(value: String) -> Secret {
        Secret(value)
    }

    /// The raw value. Call sites must not log it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(••••)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // Best effort: overwrite before freeing.
        let len = self.0.len();
        self.0.clear();
        self.0.extend(std::iter::repeat_n('\0', len));
    }
}

/// A resolved upstream credential.
#[derive(Debug, Clone)]
pub struct Credential {
    /// Token or key.
    pub token: Secret,
    /// ChatGPT workspace id (`ChatGPT-Account-ID`) for Codex logins.
    pub chatgpt_account_id: Option<String>,
    /// Expiry when the secret format reports it (Unix ms).
    pub expires_at: Option<i64>,
}

/// Why a secret could not be turned into a credential. Never contains the
/// secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub &'static str);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Extracts a credential from secret text in `format`.
pub fn parse_secret(text: &str, format: SecretFormat) -> Result<Credential, ParseError> {
    match format {
        SecretFormat::Raw => {
            let t = text.trim();
            if t.is_empty() {
                return Err(ParseError("secret is empty"));
            }
            if t.chars().any(|c| c.is_control() || c == ' ') {
                return Err(ParseError(
                    "secret contains whitespace or control characters",
                ));
            }
            Ok(Credential {
                token: Secret::new(t.to_string()),
                chatgpt_account_id: None,
                expires_at: None,
            })
        }
        SecretFormat::CodexAuthJson => {
            let v: serde_json::Value = serde_json::from_str(text)
                .map_err(|_| ParseError("auth.json is not valid JSON"))?;
            let tokens = v
                .get("tokens")
                .ok_or(ParseError("auth.json has no ChatGPT tokens"))?;
            let token = tokens
                .get("access_token")
                .and_then(|t| t.as_str())
                .filter(|t| !t.is_empty())
                .ok_or(ParseError("auth.json has no access token"))?;
            Ok(Credential {
                token: Secret::new(token.to_string()),
                chatgpt_account_id: tokens
                    .get("account_id")
                    .and_then(|a| a.as_str())
                    .map(str::to_string),
                expires_at: None,
            })
        }
        SecretFormat::ClaudeCredentialsJson => {
            let v: serde_json::Value = serde_json::from_str(text)
                .map_err(|_| ParseError("credentials file is not valid JSON"))?;
            let oauth = v
                .get("claudeAiOauth")
                .ok_or(ParseError("credentials file has no claude.ai login"))?;
            let token = oauth
                .get("accessToken")
                .and_then(|t| t.as_str())
                .filter(|t| !t.is_empty())
                .ok_or(ParseError("credentials file has no access token"))?;
            Ok(Credential {
                token: Secret::new(token.to_string()),
                chatgpt_account_id: None,
                expires_at: oauth.get("expiresAt").and_then(|e| e.as_i64()),
            })
        }
    }
}

/// Anthropic beta flag the API requires with claude.ai OAuth tokens. Added
/// only for pooled subscription accounts, and only when absent.
pub const ANTHROPIC_OAUTH_BETA: &str = "oauth-2025-04-20";

/// Removes the client's credential headers (which carry Relay's local key in
/// router-owned mode) and inserts `cred`.
pub fn inject(
    provider: Provider,
    kind: CredentialKind,
    headers: &mut HeaderMap,
    cred: &Credential,
) -> Result<(), ParseError> {
    headers.remove(header::AUTHORIZATION);
    headers.remove("x-api-key");
    headers.remove("chatgpt-account-id");
    let bearer = || {
        HeaderValue::try_from(format!("Bearer {}", cred.token.expose()))
            .map(|mut v| {
                v.set_sensitive(true);
                v
            })
            .map_err(|_| ParseError("credential is not a valid header value"))
    };
    match (provider, kind) {
        (Provider::Anthropic, CredentialKind::ApiKey) => {
            let mut v = HeaderValue::try_from(cred.token.expose())
                .map_err(|_| ParseError("credential is not a valid header value"))?;
            v.set_sensitive(true);
            headers.insert("x-api-key", v);
        }
        (Provider::Anthropic, CredentialKind::Subscription) => {
            headers.insert(header::AUTHORIZATION, bearer()?);
            // Keep every beta the client sent (possibly across several
            // header lines), de-duplicated in order, and add the OAuth one.
            let mut betas: Vec<String> = Vec::new();
            for v in headers.get_all("anthropic-beta") {
                for b in v.to_str().unwrap_or("").split(',') {
                    let b = b.trim();
                    if !b.is_empty() && !betas.iter().any(|x| x == b) {
                        betas.push(b.to_string());
                    }
                }
            }
            if !betas.iter().any(|b| b == ANTHROPIC_OAUTH_BETA) {
                betas.push(ANTHROPIC_OAUTH_BETA.to_string());
            }
            let v = HeaderValue::try_from(betas.join(","))
                .map_err(|_| ParseError("anthropic-beta is not a valid header value"))?;
            headers.insert("anthropic-beta", v);
        }
        (Provider::Openai, _) => {
            headers.insert(header::AUTHORIZATION, bearer()?);
            if let Some(id) = &cred.chatgpt_account_id
                && let Ok(v) = HeaderValue::try_from(id.as_str())
            {
                headers.insert("chatgpt-account-id", v);
            }
        }
    }
    Ok(())
}

/// Kimi client headers. Relay names itself honestly instead of copying a
/// Kimi CLI's identity.
const KIMI_PLATFORM: &str = "Relay";

/// [`inject`] for any vendor. First-party vendors behave exactly as
/// [`inject`]. Other vendors get only `Authorization: Bearer`, plus Kimi's
/// `X-Msh-*` headers naming Relay and the device id bound at sign-in
/// (required: without it nothing is sent). Client `oauth-*` Anthropic betas
/// are dropped; other betas pass. No vendor CLI identity (user agent,
/// client id, version) is ever synthesised.
pub fn inject_for(
    vendor: Vendor,
    wire: Provider,
    kind: CredentialKind,
    device_id: Option<&str>,
    headers: &mut HeaderMap,
    cred: &Credential,
) -> Result<(), ParseError> {
    if vendor.is_first_party() {
        return inject(wire, kind, headers, cred);
    }
    headers.remove(header::AUTHORIZATION);
    headers.remove("x-api-key");
    headers.remove("chatgpt-account-id");
    let msh: Vec<_> = headers
        .keys()
        .filter(|k| k.as_str().starts_with("x-msh-"))
        .cloned()
        .collect();
    for k in msh {
        headers.remove(k);
    }
    // `oauth-*` betas declare a Claude login capability that means nothing
    // to another vendor; other betas still describe the request.
    let betas: Vec<String> = headers
        .get_all("anthropic-beta")
        .iter()
        .flat_map(|v| v.to_str().unwrap_or("").split(','))
        .map(str::trim)
        .filter(|b| !b.is_empty() && !b.starts_with("oauth-"))
        .map(str::to_string)
        .collect();
    headers.remove("anthropic-beta");
    if !betas.is_empty() {
        let v = HeaderValue::try_from(betas.join(","))
            .map_err(|_| ParseError("anthropic-beta is not a valid header value"))?;
        headers.insert("anthropic-beta", v);
    }
    let mut v = HeaderValue::try_from(format!("Bearer {}", cred.token.expose()))
        .map_err(|_| ParseError("credential is not a valid header value"))?;
    v.set_sensitive(true);
    headers.insert(header::AUTHORIZATION, v);
    if vendor.needs_device_id() {
        let id = device_id
            .filter(|d| !d.is_empty())
            .ok_or(ParseError("the account has no device id from its sign-in"))?;
        let id = HeaderValue::try_from(id)
            .map_err(|_| ParseError("device id is not a valid header value"))?;
        headers.insert("x-msh-platform", HeaderValue::from_static(KIMI_PLATFORM));
        headers.insert(
            "x-msh-version",
            HeaderValue::from_static(relay_core::DISPLAY_VERSION),
        );
        headers.insert("x-msh-device-id", id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_secret() {
        let c = parse_secret("  sk-test-123  ", SecretFormat::Raw).unwrap();
        assert_eq!(c.token.expose(), "sk-test-123");
        assert!(!format!("{c:?}").contains("sk-test"));
    }

    #[test]
    fn codex_auth_json() {
        let text = r#"{"OPENAI_API_KEY":null,"tokens":{"access_token":"at-1","refresh_token":"rt","account_id":"acct-9"}}"#;
        let c = parse_secret(text, SecretFormat::CodexAuthJson).unwrap();
        assert_eq!(c.token.expose(), "at-1");
        assert_eq!(c.chatgpt_account_id.as_deref(), Some("acct-9"));
        assert!(parse_secret("{}", SecretFormat::CodexAuthJson).is_err());
        let err = parse_secret("{nope", SecretFormat::CodexAuthJson).unwrap_err();
        assert!(!err.0.contains("nope"));
    }

    #[test]
    fn claude_credentials_json() {
        let text = r#"{"claudeAiOauth":{"accessToken":"oat","refreshToken":"r","expiresAt":1760000000000}}"#;
        let c = parse_secret(text, SecretFormat::ClaudeCredentialsJson).unwrap();
        assert_eq!(c.token.expose(), "oat");
        assert_eq!(c.expires_at, Some(1_760_000_000_000));
    }

    #[test]
    fn raw_rejects_whitespace_inside() {
        assert!(parse_secret("a b", SecretFormat::Raw).is_err());
        assert!(parse_secret("   ", SecretFormat::Raw).is_err());
    }

    fn cred(t: &str) -> Credential {
        Credential {
            token: Secret::new(t.into()),
            chatgpt_account_id: Some("acct".into()),
            expires_at: None,
        }
    }

    #[test]
    fn inject_replaces_local_key_and_keeps_other_headers() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer local-key".parse().unwrap());
        h.insert(
            "anthropic-beta",
            "fine-grained-tool-streaming-2025-05-14".parse().unwrap(),
        );
        h.insert(header::USER_AGENT, "claude-cli/2.0".parse().unwrap());
        inject(
            Provider::Anthropic,
            CredentialKind::Subscription,
            &mut h,
            &cred("oat"),
        )
        .unwrap();
        assert_eq!(h[header::AUTHORIZATION], "Bearer oat");
        assert!(h[header::AUTHORIZATION].is_sensitive());
        assert_eq!(
            h["anthropic-beta"],
            "fine-grained-tool-streaming-2025-05-14,oauth-2025-04-20"
        );
        assert_eq!(h[header::USER_AGENT], "claude-cli/2.0");

        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer local-key".parse().unwrap());
        inject(
            Provider::Anthropic,
            CredentialKind::ApiKey,
            &mut h,
            &cred("sk-ant"),
        )
        .unwrap();
        assert!(h.get(header::AUTHORIZATION).is_none());
        assert_eq!(h["x-api-key"], "sk-ant");
        assert!(h.get("anthropic-beta").is_none());

        let mut h = HeaderMap::new();
        inject(
            Provider::Openai,
            CredentialKind::Subscription,
            &mut h,
            &cred("at"),
        )
        .unwrap();
        assert_eq!(h[header::AUTHORIZATION], "Bearer at");
        assert_eq!(h["chatgpt-account-id"], "acct");
    }

    #[test]
    fn all_beta_header_lines_survive_injection() {
        let mut h = HeaderMap::new();
        h.append("anthropic-beta", "a-2025,b-2025".parse().unwrap());
        h.append("anthropic-beta", "c-2025, a-2025".parse().unwrap());
        inject(
            Provider::Anthropic,
            CredentialKind::Subscription,
            &mut h,
            &cred("oat"),
        )
        .unwrap();
        let all: Vec<&str> = h
            .get_all("anthropic-beta")
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect();
        assert_eq!(all, ["a-2025,b-2025,c-2025,oauth-2025-04-20"]);
    }

    #[test]
    fn oauth_beta_not_duplicated() {
        let mut h = HeaderMap::new();
        h.insert("anthropic-beta", "oauth-2025-04-20".parse().unwrap());
        inject(
            Provider::Anthropic,
            CredentialKind::Subscription,
            &mut h,
            &cred("oat"),
        )
        .unwrap();
        assert_eq!(h["anthropic-beta"], "oauth-2025-04-20");
    }

    #[test]
    fn other_vendors_get_bearer_only_and_never_first_party_extras() {
        for (vendor, wire) in [
            (Vendor::Xai, Provider::Openai),
            (Vendor::Meta, Provider::Openai),
            (Vendor::KimiCom, Provider::Anthropic),
            (Vendor::KimiAi, Provider::Openai),
        ] {
            let mut h = HeaderMap::new();
            h.insert(header::AUTHORIZATION, "Bearer local-key".parse().unwrap());
            h.insert("x-api-key", "local-key".parse().unwrap());
            h.insert("chatgpt-account-id", "stale".parse().unwrap());
            h.insert("x-msh-platform", "kimi_cli".parse().unwrap());
            h.insert(header::USER_AGENT, "codex_cli_rs/1".parse().unwrap());
            inject_for(
                vendor,
                wire,
                CredentialKind::Subscription,
                Some("dev-1"),
                &mut h,
                &cred("vt"),
            )
            .unwrap();
            assert_eq!(h[header::AUTHORIZATION], "Bearer vt", "{vendor}");
            assert!(h[header::AUTHORIZATION].is_sensitive());
            assert!(h.get("x-api-key").is_none());
            // The Codex account id belongs to ChatGPT; never sent elsewhere.
            assert!(h.get("chatgpt-account-id").is_none());
            assert!(h.get("anthropic-beta").is_none());
            // The client's own user agent passes; Relay adds none.
            assert_eq!(h[header::USER_AGENT], "codex_cli_rs/1");
            if vendor.needs_device_id() {
                assert_eq!(h["x-msh-platform"], "Relay");
                assert_eq!(h["x-msh-version"], relay_core::DISPLAY_VERSION);
                assert_eq!(h["x-msh-device-id"], "dev-1");
            } else {
                assert!(h.keys().all(|k| !k.as_str().starts_with("x-msh-")));
            }
        }
    }

    #[test]
    fn other_vendors_drop_claude_oauth_betas_and_keep_the_rest() {
        let mut h = HeaderMap::new();
        h.append(
            "anthropic-beta",
            "oauth-2025-04-20,fine-grained-tool-streaming-2025-05-14"
                .parse()
                .unwrap(),
        );
        h.append(
            "anthropic-beta",
            "interleaved-thinking-2025-05-14".parse().unwrap(),
        );
        inject_for(
            Vendor::KimiCom,
            Provider::Anthropic,
            CredentialKind::Subscription,
            Some("dev-1"),
            &mut h,
            &cred("vt"),
        )
        .unwrap();
        let all: Vec<&str> = h
            .get_all("anthropic-beta")
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect();
        assert_eq!(
            all,
            ["fine-grained-tool-streaming-2025-05-14,interleaved-thinking-2025-05-14"]
        );
        let mut h = HeaderMap::new();
        h.insert("anthropic-beta", "oauth-2025-04-20".parse().unwrap());
        inject_for(
            Vendor::KimiAi,
            Provider::Anthropic,
            CredentialKind::Subscription,
            Some("dev-1"),
            &mut h,
            &cred("vt"),
        )
        .unwrap();
        assert!(h.get("anthropic-beta").is_none());
    }

    #[test]
    fn kimi_without_device_id_is_refused() {
        let mut h = HeaderMap::new();
        let e = inject_for(
            Vendor::KimiCom,
            Provider::Anthropic,
            CredentialKind::Subscription,
            None,
            &mut h,
            &cred("vt"),
        );
        assert!(e.is_err());
    }

    #[test]
    fn first_party_vendor_matches_inject() {
        let mut a = HeaderMap::new();
        let mut b = HeaderMap::new();
        inject(
            Provider::Openai,
            CredentialKind::Subscription,
            &mut a,
            &cred("at"),
        )
        .unwrap();
        inject_for(
            Vendor::Openai,
            Provider::Openai,
            CredentialKind::Subscription,
            None,
            &mut b,
            &cred("at"),
        )
        .unwrap();
        assert_eq!(a, b);
    }
}
