//! Header rules shared by both providers.
//!
//! Everything a client sends is forwarded except hop-by-hop headers, Relay's
//! own `x-relay-*` headers and `accept-encoding` (Relay reads usage from the
//! stream, so it asks for identity encoding).

use http::{HeaderMap, HeaderName, header};

/// Prefix of Relay-owned request headers. Never forwarded upstream.
pub const RELAY_PREFIX: &str = "x-relay-";
/// Local client key header (observed mode).
pub const RELAY_KEY: &str = "x-relay-key";
/// Names the account an observed client belongs to, or pins a router-owned
/// request to one account.
pub const RELAY_ACCOUNT: &str = "x-relay-account";
/// Chooses the pool (`pooled` or `key`) for router-owned requests.
pub const RELAY_POOL: &str = "x-relay-pool";
/// Chooses the vendor (`kimi-com`, `xai`, …) for a request. Absent means
/// the wire's first-party vendor. A local routing hint; never forwarded.
pub const RELAY_VENDOR: &str = "x-relay-vendor";
/// Explicit session id (overrides provider session headers).
pub const RELAY_SESSION: &str = "x-relay-session";

const HOP_BY_HOP: [HeaderName; 8] = [
    header::CONNECTION,
    header::PROXY_AUTHENTICATE,
    header::PROXY_AUTHORIZATION,
    header::TE,
    header::TRAILER,
    header::TRANSFER_ENCODING,
    header::UPGRADE,
    header::HOST,
];

fn strip_connection_listed(headers: &mut HeaderMap) {
    let listed: Vec<HeaderName> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|n| HeaderName::from_bytes(n.trim().as_bytes()).ok())
        .collect();
    for name in listed {
        headers.remove(name);
    }
}

/// Prepares client request headers for upstream.
pub fn upstream_request(headers: &mut HeaderMap) {
    strip_connection_listed(headers);
    for h in HOP_BY_HOP {
        headers.remove(h);
    }
    headers.remove("keep-alive");
    headers.remove(header::CONTENT_LENGTH);
    headers.remove(header::ACCEPT_ENCODING);
    let relay: Vec<HeaderName> = headers
        .keys()
        .filter(|k| k.as_str().starts_with(RELAY_PREFIX))
        .cloned()
        .collect();
    for k in relay {
        headers.remove(k);
    }
}

/// Prepares upstream response headers for the client.
pub fn downstream_response(headers: &mut HeaderMap) {
    strip_connection_listed(headers);
    for h in HOP_BY_HOP {
        headers.remove(h);
    }
    headers.remove("keep-alive");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_relay_and_hop_headers_but_keeps_provider_headers() {
        let mut h = HeaderMap::new();
        h.insert("x-relay-key", "k".parse().unwrap());
        h.insert("x-relay-account", "a".parse().unwrap());
        h.insert(RELAY_VENDOR, "kimi-com".parse().unwrap());
        h.insert(header::CONNECTION, "keep-alive, x-foo".parse().unwrap());
        h.insert("x-foo", "1".parse().unwrap());
        h.insert(header::HOST, "127.0.0.1".parse().unwrap());
        h.insert(header::ACCEPT_ENCODING, "gzip".parse().unwrap());
        h.insert(header::AUTHORIZATION, "Bearer user-oauth".parse().unwrap());
        h.insert("anthropic-beta", "oauth-2025-04-20,x".parse().unwrap());
        h.insert("x-claude-code-session-id", "s".parse().unwrap());
        upstream_request(&mut h);
        let names: Vec<&str> = h.keys().map(|k| k.as_str()).collect();
        assert!(!names.iter().any(|n| n.starts_with("x-relay")));
        assert!(!names.contains(&"x-foo"));
        assert!(!names.contains(&"host"));
        assert!(!names.contains(&"accept-encoding"));
        // Credential and betas byte-identical.
        assert_eq!(h[header::AUTHORIZATION], "Bearer user-oauth");
        assert_eq!(h["anthropic-beta"], "oauth-2025-04-20,x");
        assert_eq!(h["x-claude-code-session-id"], "s");
    }
}
