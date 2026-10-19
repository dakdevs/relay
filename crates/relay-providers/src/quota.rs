//! Quota from response headers. Header names come from observed provider
//! traffic (CLIProxyAPI source) and are undocumented for subscriptions;
//! missing, malformed or out-of-range values are skipped, never guessed.

use http::HeaderMap;
use relay_core::UnixMs;
use relay_core::model::Provider;
use relay_core::quota::{Credits, QuotaUpdate, QuotaWindow};

fn get<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name)?.to_str().ok().map(str::trim)
}

fn num(h: &HeaderMap, name: &str) -> Option<f64> {
    get(h, name)?.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn pct_ok(p: f64) -> Option<f32> {
    (0.0..=100.0).contains(&p).then_some(p as f32)
}

/// Parses `retry-after` (seconds) or `retry-after-ms`.
pub fn retry_after_ms(h: &HeaderMap) -> Option<i64> {
    if let Some(ms) = num(h, "retry-after-ms").filter(|v| *v >= 0.0) {
        return Some(ms as i64);
    }
    num(h, "retry-after")
        .filter(|v| *v >= 0.0)
        .map(|s| (s * 1000.0) as i64)
}

/// Parses a duration like `1s`, `6m0s`, `20ms`, `1h2m3.5s` into ms.
fn go_duration_ms(s: &str) -> Option<i64> {
    let mut total = 0.0f64;
    let mut rest = s.trim();
    if rest.is_empty() {
        return None;
    }
    while !rest.is_empty() {
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        let value: f64 = rest[..end].parse().ok()?;
        rest = &rest[end..];
        let (factor, len) = if rest.starts_with("ms") {
            (1.0, 2)
        } else if rest.starts_with('h') {
            (3_600_000.0, 1)
        } else if rest.starts_with('m') {
            (60_000.0, 1)
        } else if rest.starts_with('s') {
            (1000.0, 1)
        } else {
            return None;
        };
        total += value * factor;
        rest = &rest[len..];
    }
    Some(total as i64)
}

fn window_label(minutes: Option<u32>, fallback: &str) -> String {
    match minutes {
        Some(300) => "5-hour".into(),
        Some(10_080) => "Weekly".into(),
        Some(m) if m % 1440 == 0 => format!("{}-day", m / 1440),
        Some(m) if m % 60 == 0 => format!("{}-hour", m / 60),
        Some(m) => format!("{m}-minute"),
        None => fallback.into(),
    }
}

fn anthropic_claim_label(claim: &str) -> String {
    match claim {
        "5h" => "5-hour".into(),
        "7d" => "Weekly".into(),
        other => match other.strip_prefix("7d_") {
            Some(model) => {
                let mut m = model.to_string();
                if let Some(first) = m.get_mut(..1) {
                    first.make_ascii_uppercase();
                }
                format!("Weekly · {m}")
            }
            None => other.to_string(),
        },
    }
}

fn anthropic(h: &HeaderMap, now: UnixMs, out: &mut QuotaUpdate) {
    const P: &str = "anthropic-ratelimit-unified-";
    // Subscription windows: anthropic-ratelimit-unified-<claim>-utilization (0–1).
    for (name, _) in h.iter() {
        let Some(claim) = name
            .as_str()
            .strip_prefix(P)
            .and_then(|r| r.strip_suffix("-utilization"))
        else {
            continue;
        };
        let Some(used) = num(h, name.as_str()).and_then(|u| pct_ok(u * 100.0)) else {
            continue;
        };
        let reset = num(h, &format!("{P}{claim}-reset"))
            .filter(|r| *r > 0.0)
            .map(|r| (r * 1000.0) as i64);
        out.windows.push(QuotaWindow {
            key: claim.to_string(),
            label: anthropic_claim_label(claim),
            used_pct: Some(used),
            resets_at: reset,
            window_minutes: match claim {
                "5h" => Some(300),
                c if c.starts_with("7d") => Some(10_080),
                _ => None,
            },
            status: get(h, &format!("{P}{claim}-status")).map(str::to_string),
            observed_at: now,
        });
    }
    // Overall rejection: cooldown until the unified reset when no retry-after.
    if get(h, "anthropic-ratelimit-unified-status") == Some("rejected")
        && out.retry_after_ms.is_none()
        && let Some(reset) = num(h, "anthropic-ratelimit-unified-reset").filter(|r| *r > 0.0)
    {
        out.retry_after_ms = Some(((reset * 1000.0) as i64 - now).max(0));
    }
    // API-key rate limits: limit / remaining / reset (RFC 3339).
    for (kind, label) in [
        ("requests", "Requests / min"),
        ("tokens", "Tokens / min"),
        ("input-tokens", "Input tokens / min"),
        ("output-tokens", "Output tokens / min"),
    ] {
        let p = format!("anthropic-ratelimit-{kind}-");
        let (Some(limit), Some(remaining)) = (
            num(h, &format!("{p}limit")),
            num(h, &format!("{p}remaining")),
        ) else {
            continue;
        };
        if limit <= 0.0 || remaining < 0.0 || remaining > limit {
            continue;
        }
        let reset = get(h, &format!("{p}reset"))
            .and_then(|r| chrono::DateTime::parse_from_rfc3339(r).ok())
            .map(|d| d.timestamp_millis());
        out.windows.push(QuotaWindow {
            key: kind.to_string(),
            label: label.into(),
            used_pct: pct_ok((1.0 - remaining / limit) * 100.0),
            resets_at: reset,
            window_minutes: Some(1),
            status: None,
            observed_at: now,
        });
    }
}

fn codex_window(h: &HeaderMap, prefix: &str, key: String, now: UnixMs) -> Option<QuotaWindow> {
    let used = num(h, &format!("{prefix}used-percent")).and_then(pct_ok)?;
    let minutes = num(h, &format!("{prefix}window-minutes"))
        .filter(|m| *m > 0.0 && *m < 1e7)
        .map(|m| m as u32);
    let reset = num(h, &format!("{prefix}reset-at"))
        .filter(|r| *r > 0.0)
        .map(|r| (r * 1000.0) as i64)
        .or_else(|| {
            num(h, &format!("{prefix}reset-after-seconds"))
                .filter(|s| *s >= 0.0)
                .map(|s| now + (s * 1000.0) as i64)
        });
    let fallback = if key.ends_with("primary") {
        "Primary"
    } else {
        "Secondary"
    };
    let mut label = window_label(minutes, fallback);
    if let Some(limit) = key
        .strip_suffix("-primary")
        .or_else(|| key.strip_suffix("-secondary"))
    {
        label = format!("{label} · {limit}");
    }
    Some(QuotaWindow {
        key,
        label,
        used_pct: Some(used),
        resets_at: reset,
        window_minutes: minutes,
        status: None,
        observed_at: now,
    })
}

fn openai(h: &HeaderMap, now: UnixMs, out: &mut QuotaUpdate) {
    // Codex subscription windows, including per-limit namespaces
    // x-codex-<name>-{primary,secondary}-*.
    for (name, _) in h.iter() {
        let n = name.as_str();
        let Some(rest) = n
            .strip_prefix("x-codex-")
            .and_then(|r| r.strip_suffix("-used-percent"))
        else {
            continue;
        };
        let key = match rest {
            "primary" | "secondary" => rest.to_string(),
            r if r.ends_with("-primary") || r.ends_with("-secondary") => r.to_string(),
            _ => continue,
        };
        if let Some(w) = codex_window(h, &format!("x-codex-{rest}-"), key, now) {
            out.windows.push(w);
        }
    }
    out.windows.sort_by(|a, b| a.key.cmp(&b.key));
    if let Some(plan) = get(h, "x-codex-plan-type").filter(|p| !p.is_empty() && p.len() < 64) {
        out.plan = Some(plan.to_string());
    }
    let has = get(h, "x-codex-credits-has-credits").and_then(|v| v.parse::<bool>().ok());
    let unlimited = get(h, "x-codex-credits-unlimited").and_then(|v| v.parse::<bool>().ok());
    let balance = get(h, "x-codex-credits-balance")
        .filter(|b| !b.is_empty() && b.len() < 32)
        .map(str::to_string);
    if has.is_some() || unlimited.is_some() || balance.is_some() {
        out.credits = Some(Credits {
            has_credits: has,
            unlimited,
            balance,
            observed_at: now,
        });
    }
    // OpenAI Platform rate limits.
    for (kind, label) in [("requests", "Requests / min"), ("tokens", "Tokens / min")] {
        let (Some(limit), Some(remaining)) = (
            num(h, &format!("x-ratelimit-limit-{kind}")),
            num(h, &format!("x-ratelimit-remaining-{kind}")),
        ) else {
            continue;
        };
        if limit <= 0.0 || remaining < 0.0 || remaining > limit {
            continue;
        }
        out.windows.push(QuotaWindow {
            key: kind.to_string(),
            label: label.into(),
            used_pct: pct_ok((1.0 - remaining / limit) * 100.0),
            resets_at: get(h, &format!("x-ratelimit-reset-{kind}"))
                .and_then(go_duration_ms)
                .map(|ms| now + ms),
            window_minutes: None,
            status: None,
            observed_at: now,
        });
    }
}

/// Parses every quota signal in `h`.
pub fn parse(provider: Provider, h: &HeaderMap, now: UnixMs) -> QuotaUpdate {
    let mut out = QuotaUpdate {
        retry_after_ms: retry_after_ms(h),
        ..Default::default()
    };
    match provider {
        Provider::Anthropic => anthropic(h, now, &mut out),
        Provider::Openai => openai(h, now, &mut out),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        h
    }

    #[test]
    fn anthropic_unified_windows() {
        let h = headers(&[
            ("anthropic-ratelimit-unified-5h-utilization", "0.42"),
            ("anthropic-ratelimit-unified-5h-reset", "1760800000"),
            ("anthropic-ratelimit-unified-5h-status", "allowed"),
            ("anthropic-ratelimit-unified-7d-utilization", "0.9"),
            ("anthropic-ratelimit-unified-7d_opus-utilization", "0.1"),
        ]);
        let u = parse(Provider::Anthropic, &h, 5);
        let five = u.windows.iter().find(|w| w.key == "5h").unwrap();
        assert!((five.used_pct.unwrap() - 42.0).abs() < 1e-3);
        assert_eq!(five.resets_at, Some(1_760_800_000_000));
        assert_eq!(five.status.as_deref(), Some("allowed"));
        assert_eq!(five.observed_at, 5);
        let opus = u.windows.iter().find(|w| w.key == "7d_opus").unwrap();
        assert_eq!(opus.label, "Weekly · Opus");
        assert_eq!(u.windows.len(), 3);
    }

    #[test]
    fn malformed_values_are_skipped() {
        let h = headers(&[
            ("anthropic-ratelimit-unified-5h-utilization", "abc"),
            ("anthropic-ratelimit-unified-7d-utilization", "-0.5"),
            ("anthropic-ratelimit-unified-7d_x-utilization", "1.7"),
            ("x-codex-primary-used-percent", "NaN"),
            ("retry-after", "-3"),
        ]);
        assert!(parse(Provider::Anthropic, &h, 0).is_empty());
        assert!(parse(Provider::Openai, &h, 0).windows.is_empty());
    }

    #[test]
    fn anthropic_rejection_sets_cooldown_from_unified_reset() {
        let h = headers(&[
            ("anthropic-ratelimit-unified-status", "rejected"),
            ("anthropic-ratelimit-unified-reset", "100"),
        ]);
        assert_eq!(
            parse(Provider::Anthropic, &h, 40_000).retry_after_ms,
            Some(60_000)
        );
        let h = headers(&[
            ("anthropic-ratelimit-unified-status", "rejected"),
            ("anthropic-ratelimit-unified-reset", "100"),
            ("retry-after", "7"),
        ]);
        assert_eq!(
            parse(Provider::Anthropic, &h, 40_000).retry_after_ms,
            Some(7_000)
        );
    }

    #[test]
    fn anthropic_api_key_limits() {
        let h = headers(&[
            ("anthropic-ratelimit-requests-limit", "50"),
            ("anthropic-ratelimit-requests-remaining", "40"),
            ("anthropic-ratelimit-requests-reset", "2026-10-18T09:42:00Z"),
        ]);
        let u = parse(Provider::Anthropic, &h, 0);
        assert_eq!(u.windows[0].key, "requests");
        assert!((u.windows[0].used_pct.unwrap() - 20.0).abs() < 1e-3);
        assert_eq!(
            u.windows[0].resets_at,
            Some(
                chrono::DateTime::parse_from_rfc3339("2026-10-18T09:42:00Z")
                    .unwrap()
                    .timestamp_millis()
            )
        );
    }

    #[test]
    fn codex_windows_plan_and_credits() {
        let h = headers(&[
            ("x-codex-primary-used-percent", "37"),
            ("x-codex-primary-window-minutes", "300"),
            ("x-codex-primary-reset-after-seconds", "60"),
            ("x-codex-secondary-used-percent", "12.5"),
            ("x-codex-secondary-window-minutes", "10080"),
            ("x-codex-secondary-reset-at", "1760900000"),
            ("x-codex-gpt-5-primary-used-percent", "3"),
            ("x-codex-plan-type", "plus"),
            ("x-codex-credits-has-credits", "true"),
            ("x-codex-credits-balance", "12.50"),
        ]);
        let u = parse(Provider::Openai, &h, 1_000);
        let p = u.windows.iter().find(|w| w.key == "primary").unwrap();
        assert_eq!(p.label, "5-hour");
        assert_eq!(p.resets_at, Some(61_000));
        let s = u.windows.iter().find(|w| w.key == "secondary").unwrap();
        assert_eq!(s.label, "Weekly");
        assert_eq!(s.resets_at, Some(1_760_900_000_000));
        let m = u.windows.iter().find(|w| w.key == "gpt-5-primary").unwrap();
        assert_eq!(m.label, "Primary · gpt-5");
        assert_eq!(u.plan.as_deref(), Some("plus"));
        let c = u.credits.unwrap();
        assert_eq!(
            (c.has_credits, c.balance.as_deref()),
            (Some(true), Some("12.50"))
        );
    }

    #[test]
    fn openai_platform_limits_and_durations() {
        assert_eq!(go_duration_ms("6m0s"), Some(360_000));
        assert_eq!(go_duration_ms("20ms"), Some(20));
        assert_eq!(go_duration_ms("1h2m3.5s"), Some(3_723_500));
        assert_eq!(go_duration_ms("soon"), None);
        let h = headers(&[
            ("x-ratelimit-limit-tokens", "1000"),
            ("x-ratelimit-remaining-tokens", "250"),
            ("x-ratelimit-reset-tokens", "1s"),
        ]);
        let u = parse(Provider::Openai, &h, 0);
        assert!((u.windows[0].used_pct.unwrap() - 75.0).abs() < 1e-3);
        assert_eq!(u.windows[0].resets_at, Some(1_000));
    }

    #[test]
    fn retry_after_forms() {
        assert_eq!(
            retry_after_ms(&headers(&[("retry-after", "3")])),
            Some(3_000)
        );
        assert_eq!(
            retry_after_ms(&headers(&[("retry-after-ms", "250")])),
            Some(250)
        );
        assert_eq!(
            retry_after_ms(&headers(&[("retry-after", "Wed, 21 Oct 2015")])),
            None
        );
    }
}
