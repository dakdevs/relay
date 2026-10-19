//! Group-level quota numbers ("grouped / averaged"), computed per window.
//!
//! A group is one provider + plan + credential kind, so only comparable
//! windows are combined. Unknown values are never treated as 0%.

use crate::UnixMs;
use crate::quota::{Freshness, QuotaWindow};
use serde::{Deserialize, Serialize};

/// Aggregate of one window key across a group's accounts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowAggregate {
    /// Window key (`5h`, `7d`, `primary`, …).
    pub key: String,
    /// Display label from the first account reporting the window.
    pub label: String,
    /// Accounts in the group (the `N` in "X% of N × 100%").
    pub accounts: u32,
    /// Accounts with a fresh value (or a known reset).
    pub fresh: u32,
    /// Accounts whose last value is stale.
    pub stale: u32,
    /// Accounts with no value for this window.
    pub unknown: u32,
    /// Mean percent left over fresh accounts; `None` when none are fresh.
    pub avg_left_pct: Option<f32>,
    /// Lowest percent left over fresh accounts.
    pub min_left_pct: Option<f32>,
    /// Sum of percent left over fresh accounts (pooled capacity, out of
    /// `fresh × 100`).
    pub pooled_left_pct: f32,
    /// Earliest future reset among accounts with a value.
    pub first_reset: Option<UnixMs>,
    /// Latest future reset among accounts with a value.
    pub last_reset: Option<UnixMs>,
}

/// Aggregates `key` over `accounts`, where each item is that account's list
/// of windows. Accounts lacking the window count as unknown.
pub fn aggregate_window<'a>(
    key: &str,
    accounts: impl IntoIterator<Item = &'a [QuotaWindow]>,
    now: UnixMs,
) -> WindowAggregate {
    let mut agg = WindowAggregate {
        key: key.to_string(),
        label: String::new(),
        accounts: 0,
        fresh: 0,
        stale: 0,
        unknown: 0,
        avg_left_pct: None,
        min_left_pct: None,
        pooled_left_pct: 0.0,
        first_reset: None,
        last_reset: None,
    };
    for windows in accounts {
        agg.accounts += 1;
        let Some(w) = windows.iter().find(|w| w.key == key) else {
            agg.unknown += 1;
            continue;
        };
        if agg.label.is_empty() {
            agg.label = w.label.clone();
        }
        match w.freshness(now) {
            Freshness::Fresh | Freshness::Reset => {
                let left = w.effective_left(now).unwrap_or(0.0);
                agg.fresh += 1;
                agg.pooled_left_pct += left;
                agg.min_left_pct = Some(agg.min_left_pct.map_or(left, |m| m.min(left)));
            }
            Freshness::Stale => agg.stale += 1,
            Freshness::Unknown => agg.unknown += 1,
        }
        if let Some(r) = w.resets_at.filter(|r| *r > now) {
            agg.first_reset = Some(agg.first_reset.map_or(r, |f| f.min(r)));
            agg.last_reset = Some(agg.last_reset.map_or(r, |l| l.max(r)));
        }
    }
    if agg.fresh > 0 {
        agg.avg_left_pct = Some(agg.pooled_left_pct / agg.fresh as f32);
    }
    if agg.label.is_empty() {
        agg.label = key.to_string();
    }
    agg
}

/// Window keys present in any account, in first-seen order.
pub fn window_keys<'a>(accounts: impl IntoIterator<Item = &'a [QuotaWindow]>) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for windows in accounts {
        for w in windows {
            if !keys.contains(&w.key) {
                keys.push(w.key.clone());
            }
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quota::FRESH_FOR_MS;

    fn w(key: &str, used: Option<f32>, reset: Option<i64>, observed: i64) -> QuotaWindow {
        QuotaWindow {
            key: key.into(),
            label: "5-hour".into(),
            used_pct: used,
            resets_at: reset,
            window_minutes: None,
            status: None,
            observed_at: observed,
        }
    }

    #[test]
    fn averages_only_fresh_and_counts_the_rest() {
        let now = 10 * FRESH_FOR_MS;
        let a = vec![w("5h", Some(20.0), Some(now + 1_000), now)]; // 80 left
        let b = vec![w("5h", Some(70.0), Some(now + 5_000), now)]; // 30 left
        let stale = vec![w("5h", Some(0.0), None, now - FRESH_FOR_MS - 1)];
        let unknown: Vec<QuotaWindow> = vec![];
        let agg = aggregate_window(
            "5h",
            [
                a.as_slice(),
                b.as_slice(),
                stale.as_slice(),
                unknown.as_slice(),
            ],
            now,
        );
        assert_eq!(
            (agg.accounts, agg.fresh, agg.stale, agg.unknown),
            (4, 2, 1, 1)
        );
        assert_eq!(agg.avg_left_pct, Some(55.0));
        assert_eq!(agg.min_left_pct, Some(30.0));
        assert_eq!(agg.pooled_left_pct, 110.0);
        assert_eq!(agg.first_reset, Some(now + 1_000));
        assert_eq!(agg.last_reset, Some(now + 5_000));
    }

    #[test]
    fn reset_window_counts_as_full_not_stale() {
        let a = vec![w("5h", Some(100.0), Some(500), 0)];
        let agg = aggregate_window("5h", [a.as_slice()], 600);
        assert_eq!(agg.fresh, 1);
        assert_eq!(agg.avg_left_pct, Some(100.0));
        assert_eq!(agg.first_reset, None);
    }

    #[test]
    fn no_fresh_means_no_average_not_zero() {
        let unknown: Vec<QuotaWindow> = vec![];
        let agg = aggregate_window("7d", [unknown.as_slice()], 0);
        assert_eq!(agg.avg_left_pct, None);
        assert_eq!(agg.unknown, 1);
    }

    #[test]
    fn keys_in_first_seen_order() {
        let a = vec![w("5h", None, None, 0), w("7d", None, None, 0)];
        let b = vec![w("7d_opus", None, None, 0), w("5h", None, None, 0)];
        assert_eq!(
            window_keys([a.as_slice(), b.as_slice()]),
            ["5h", "7d", "7d_opus"]
        );
    }
}
