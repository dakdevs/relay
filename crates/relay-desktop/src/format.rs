//! Text formatting for numbers, durations and times shown in the UI.

use std::time::Duration;

/// Compact token count: `7.4M`, `412K`, `1.14M`, `980`.
pub fn tokens(n: u64) -> String {
    let f = n as f64;
    if n >= 1_000_000_000 {
        trim(format!("{:.1}", f / 1e9)) + "B"
    } else if n >= 1_000_000 {
        trim(format!("{:.1}", f / 1e6)) + "M"
    } else if n >= 10_000 {
        format!("{:.0}K", f / 1e3)
    } else if n >= 1_000 {
        trim(format!("{:.1}", f / 1e3)) + "K"
    } else {
        n.to_string()
    }
}

fn trim(s: String) -> String {
    s.strip_suffix(".0").map(str::to_owned).unwrap_or(s)
}

/// Integer with thousands separators: `3,090`.
pub fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Percentage left, rounded down so "0%" only ever means exhausted.
pub fn percent(left: f32) -> String {
    let v = left.clamp(0.0, 999.0);
    let shown = if v > 0.0 && v < 1.0 { 1.0 } else { v.floor() };
    format!("{shown:.0}%")
}

/// Short duration: `38m`, `2h 10m`, `1h 05m`, `4d`, `<1m`.
pub fn duration_short(d: Duration) -> String {
    let mins = d.as_secs() / 60;
    if mins < 1 {
        "<1m".into()
    } else if mins < 60 {
        format!("{mins}m")
    } else if mins < 24 * 60 {
        let (h, m) = (mins / 60, mins % 60);
        if m == 0 {
            format!("{h}h")
        } else {
            format!("{h}h {m:02}m")
        }
    } else {
        format!("{}d", mins / (24 * 60))
    }
}

/// `seen 2m ago`, `seen just now`.
pub fn seen_ago(d: Duration) -> String {
    if d.as_secs() < 60 {
        "seen just now".into()
    } else {
        format!("seen {} ago", duration_short(d))
    }
}

/// Signed percent change label: `↑ 17%`, `↓ 4%`, `0%`.
pub fn delta(pct: f64) -> String {
    let r = pct.round();
    if r > 0.0 {
        format!("↑ {r:.0}%")
    } else if r < 0.0 {
        format!("↓ {:.0}%", -r)
    } else {
        "0%".into()
    }
}

/// Local wall-clock reset time: `Oct 22, 09:00`.
pub fn clock(ms: i64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_millis_opt(ms) {
        chrono::LocalResult::Single(t) | chrono::LocalResult::Ambiguous(t, _) => {
            t.format("%b %-d, %H:%M").to_string()
        }
        chrono::LocalResult::None => String::new(),
    }
}

/// Ledger reset cell: `in 4d · Oct 22, 09:00`.
pub fn reset_cell(reset_at: i64, now: i64) -> String {
    let d = Duration::from_millis((reset_at - now).max(0) as u64);
    format!("in {} · {}", duration_short(d), clock(reset_at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_cell_combines_relative_and_wall_clock() {
        use chrono::TimeZone;
        let at = chrono::Local
            .with_ymd_and_hms(2026, 10, 22, 9, 0, 0)
            .unwrap()
            .timestamp_millis();
        assert_eq!(clock(at), "Oct 22, 09:00");
        assert_eq!(
            reset_cell(at, at - (4 * 86_400 + 600) * 1000),
            "in 4d · Oct 22, 09:00"
        );
    }

    #[test]
    fn token_compaction_matches_design_samples() {
        assert_eq!(tokens(7_400_000), "7.4M");
        assert_eq!(tokens(18_800_000), "18.8M");
        assert_eq!(tokens(412_000), "412K");
        assert_eq!(tokens(52_000), "52K");
        assert_eq!(tokens(3_000_000), "3M");
        assert_eq!(tokens(1_500), "1.5K");
        assert_eq!(tokens(980), "980");
        assert_eq!(tokens(0), "0");
    }

    #[test]
    fn grouping() {
        assert_eq!(grouped(3090), "3,090");
        assert_eq!(grouped(1_204_000), "1,204,000");
        assert_eq!(grouped(97), "97");
        assert_eq!(grouped(100), "100");
    }

    #[test]
    fn percent_never_rounds_remaining_quota_to_zero() {
        assert_eq!(percent(0.0), "0%");
        assert_eq!(percent(0.4), "1%");
        assert_eq!(percent(48.9), "48%");
        assert_eq!(percent(100.0), "100%");
    }

    #[test]
    fn durations() {
        assert_eq!(duration_short(Duration::from_secs(38 * 60)), "38m");
        assert_eq!(duration_short(Duration::from_secs(130 * 60)), "2h 10m");
        assert_eq!(duration_short(Duration::from_secs(65 * 60)), "1h 05m");
        assert_eq!(duration_short(Duration::from_secs(3 * 3600)), "3h");
        assert_eq!(duration_short(Duration::from_secs(4 * 86_400 + 7200)), "4d");
        assert_eq!(duration_short(Duration::from_secs(20)), "<1m");
        assert_eq!(seen_ago(Duration::from_secs(120)), "seen 2m ago");
    }

    #[test]
    fn deltas() {
        assert_eq!(delta(17.2), "↑ 17%");
        assert_eq!(delta(-3.6), "↓ 4%");
        assert_eq!(delta(0.2), "0%");
    }
}
