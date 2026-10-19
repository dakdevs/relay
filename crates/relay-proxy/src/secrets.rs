//! Resolves [`SecretRef`]s. Values are read only for configured, enabled
//! accounts, are never logged, and are never written back: a CLI that owns a
//! login file keeps ownership of its refresh.

use relay_core::config::{SecretFormat, SecretRef};
use relay_providers::auth::{Credential, ParseError, Secret, parse_secret};
use std::io::Read;
use std::path::Path;

/// Why a secret is unavailable. Text is safe to display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable(pub String);

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Largest secret file read.
const MAX_SECRET_FILE: u64 = 1024 * 1024;

fn read_file(path: &Path) -> Result<String, Unavailable> {
    let f = std::fs::File::open(path)
        .map_err(|e| Unavailable(format!("cannot read {}: {}", path.display(), e.kind())))?;
    let mut text = String::new();
    f.take(MAX_SECRET_FILE)
        .read_to_string(&mut text)
        .map_err(|e| Unavailable(format!("cannot read {}: {}", path.display(), e.kind())))?;
    Ok(text)
}

#[cfg(target_os = "macos")]
fn read_keychain(service: &str, account: &str) -> Result<String, Unavailable> {
    let out = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", service, "-a", account, "-w"])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| Unavailable(format!("keychain unavailable: {}", e.kind())))?;
    if !out.status.success() {
        return Err(Unavailable(format!(
            "keychain item {service}/{account} not found or access denied"
        )));
    }
    String::from_utf8(out.stdout).map_err(|_| Unavailable("keychain item is not text".into()))
}

#[cfg(not(target_os = "macos"))]
fn read_keychain(service: &str, account: &str) -> Result<String, Unavailable> {
    let out = std::process::Command::new("secret-tool")
        .args(["lookup", "service", service, "account", account])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| {
            Unavailable(format!(
                "Secret Service unavailable (secret-tool): {}",
                e.kind()
            ))
        })?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err(Unavailable(format!(
            "secret {service}/{account} not found or access denied"
        )));
    }
    String::from_utf8(out.stdout).map_err(|_| Unavailable("secret is not text".into()))
}

fn parse(text: &str, format: SecretFormat) -> Result<Credential, Unavailable> {
    parse_secret(text, format).map_err(|ParseError(m)| Unavailable(m.to_string()))
}

/// Reads and parses a secret. Blocking (file, process); call off the
/// request path.
pub fn resolve(r: &SecretRef) -> Result<Credential, Unavailable> {
    match r {
        SecretRef::Env { env } => {
            let v = std::env::var(env)
                .map_err(|_| Unavailable(format!("environment variable {env} is not set")))?;
            parse(&v, SecretFormat::Raw)
        }
        SecretRef::File { file, format } => parse(&read_file(file)?, *format),
        SecretRef::Keychain {
            keychain_service,
            keychain_account,
            format,
        } => parse(&read_keychain(keychain_service, keychain_account)?, *format),
    }
}

/// File modification time, used to notice a CLI refreshing its login.
pub fn modified(r: &SecretRef) -> Option<std::time::SystemTime> {
    match r {
        SecretRef::File { file, .. } => std::fs::metadata(file).ok()?.modified().ok(),
        _ => None,
    }
}

/// Loads the local client key, generating `local-key` (0600) on first use.
pub fn local_key(r: Option<&SecretRef>, file: &Path) -> Result<Secret, Unavailable> {
    if let Some(r) = r {
        return resolve(r).map(|c| c.token);
    }
    match std::fs::read_to_string(file) {
        Ok(t) if !t.trim().is_empty() => return Ok(Secret::new(t.trim().to_string())),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Unavailable(format!("cannot read local key: {}", e.kind()))),
    }
    let mut raw = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut raw))
        .map_err(|e| Unavailable(format!("no randomness source: {}", e.kind())))?;
    let key: String = std::iter::once("relay-".to_string())
        .chain(raw.iter().map(|b| format!("{b:02x}")))
        .collect();
    crate::store::write_private(file, key.as_bytes())
        .map_err(|e| Unavailable(format!("cannot write local key: {}", e.kind())))?;
    Ok(Secret::new(key))
}

/// Constant-time comparison for the local key.
pub fn key_matches(expected: &Secret, given: &[u8]) -> bool {
    let e = expected.expose().as_bytes();
    if e.len() != given.len() {
        return false;
    }
    e.iter().zip(given).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_key_is_generated_once_and_private() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("local-key");
        let a = local_key(None, &f).unwrap();
        let b = local_key(None, &f).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.expose().len(), "relay-".len() + 64);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&f).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn missing_sources_explain_without_values() {
        let e = resolve(&SecretRef::Env {
            env: "RELAY_TEST_SURELY_UNSET_VAR".into(),
        })
        .unwrap_err();
        assert!(e.0.contains("RELAY_TEST_SURELY_UNSET_VAR"));
        let e = resolve(&SecretRef::File {
            file: "/nonexistent/relay/auth.json".into(),
            format: SecretFormat::CodexAuthJson,
        })
        .unwrap_err();
        assert!(e.0.contains("cannot read"));
    }

    #[test]
    fn file_secret_parses_format() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("auth.json");
        std::fs::write(
            &f,
            r#"{"tokens":{"access_token":"tok","account_id":"acc"}}"#,
        )
        .unwrap();
        let c = resolve(&SecretRef::File {
            file: f,
            format: SecretFormat::CodexAuthJson,
        })
        .unwrap();
        assert_eq!(c.token.expose(), "tok");
    }

    #[test]
    fn key_compare() {
        let k = Secret::new("abc".into());
        assert!(key_matches(&k, b"abc"));
        assert!(!key_matches(&k, b"abd"));
        assert!(!key_matches(&k, b"ab"));
    }
}
