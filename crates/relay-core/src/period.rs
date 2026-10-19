//! "Through Relay" period comparison: this week or month so far against the
//! same elapsed span of the previous one, in local calendar days.
//!
//! Null (no recording) and zero (recorded, no traffic) are different: a day
//! is `None` when Relay recorded no hour of it, and `Some(0)` when Relay was
//! running but nothing passed through.

use crate::config::PeriodKind;
use chrono::{Datelike, Days, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Recorded totals for one local day, by local hour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DayHours {
    /// Bit `h` set when Relay was recording during hour `h`.
    pub covered: u32,
    /// Tokens per hour.
    pub tokens: [u64; 24],
}

impl DayHours {
    /// Day total, `None` when no hour was recorded.
    pub fn total(&self) -> Option<u64> {
        (self.covered != 0).then(|| self.tokens.iter().sum())
    }

    /// Total for hours before `hour`; `None` when none of them was recorded.
    pub fn total_before(&self, hour: u32) -> Option<u64> {
        let mask = if hour >= 24 {
            u32::MAX
        } else {
            (1u32 << hour) - 1
        };
        (self.covered & mask != 0 || hour == 0)
            .then(|| self.tokens[..hour.min(24) as usize].iter().sum())
    }
}

/// One day on the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayBucket {
    /// Local date.
    pub date: NaiveDate,
    /// Tokens, `None` when not recorded.
    pub tokens: Option<u64>,
    /// The current, still-growing day.
    pub partial: bool,
}

/// Comparison label state (board 9).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "pct")]
pub enum Comparison {
    /// Percent change against the prior equivalent span.
    Delta(f64),
    /// First day of the period; nothing comparable yet.
    TooEarly,
    /// No prior data, or a prior total of zero against current traffic.
    NoComparison,
    /// Some prior day in the equivalent span was not recorded.
    PriorIncomplete,
    /// Some current day was not recorded.
    Incomplete,
    /// Both spans fully recorded and both zero.
    NoTraffic,
}

/// Everything the sparkline needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThroughRelay {
    /// Week or month.
    pub kind: PeriodKind,
    /// Days in the whole current period (axis length).
    pub period_days: u32,
    /// Period start through today; the last bucket is partial.
    pub current: Vec<DayBucket>,
    /// The whole prior period.
    pub prior: Vec<DayBucket>,
    /// Current tokens so far.
    pub current_total: u64,
    /// Prior tokens over the same elapsed span; `None` when not fully recorded.
    pub prior_equivalent_total: Option<u64>,
    /// Label state.
    pub comparison: Comparison,
    /// Set when the prior month is shorter than today's day of month, so the
    /// prior span ends on this (its last) day.
    pub prior_truncated_to: Option<NaiveDate>,
}

/// First day of the period containing `day`.
pub fn period_start(kind: PeriodKind, day: NaiveDate, week_start: Weekday) -> NaiveDate {
    match kind {
        PeriodKind::Week => {
            let back =
                (day.weekday().num_days_from_monday() + 7 - week_start.num_days_from_monday()) % 7;
            day - Days::new(back as u64)
        }
        PeriodKind::Month => day.with_day(1).unwrap_or(day),
    }
}

/// First day of the period after the one starting at `start`.
pub fn next_period_start(kind: PeriodKind, start: NaiveDate) -> NaiveDate {
    match kind {
        PeriodKind::Week => start + Days::new(7),
        PeriodKind::Month => start
            .checked_add_months(chrono::Months::new(1))
            .unwrap_or(start),
    }
}

fn prior_period_start(kind: PeriodKind, start: NaiveDate) -> NaiveDate {
    match kind {
        PeriodKind::Week => start - Days::new(7),
        PeriodKind::Month => start
            .checked_sub_months(chrono::Months::new(1))
            .unwrap_or(start),
    }
}

/// Builds the comparison for local `today` at local `hour` (0–23) from
/// recorded hourly data.
pub fn through_relay(
    kind: PeriodKind,
    today: NaiveDate,
    hour: u32,
    week_start: Weekday,
    days: &BTreeMap<NaiveDate, DayHours>,
) -> ThroughRelay {
    let start = period_start(kind, today, week_start);
    let end = next_period_start(kind, start);
    let prior_start = prior_period_start(kind, start);
    let get = |d: NaiveDate| days.get(&d).copied().unwrap_or_default();

    let elapsed = (today - start).num_days() as u64; // index of today
    let current: Vec<DayBucket> = (0..=elapsed)
        .map(|i| {
            let date = start + Days::new(i);
            DayBucket {
                date,
                tokens: get(date).total(),
                partial: i == elapsed,
            }
        })
        .collect();
    let prior_len = (start - prior_start).num_days() as u64;
    let prior: Vec<DayBucket> = (0..prior_len)
        .map(|i| {
            let date = prior_start + Days::new(i);
            DayBucket {
                date,
                tokens: get(date).total(),
                partial: false,
            }
        })
        .collect();

    let current_total: u64 = current.iter().filter_map(|b| b.tokens).sum();

    // Equivalent prior span: same day offsets, and on the matching day only
    // the hours already elapsed today. A shorter prior month ends on its
    // last day.
    let truncated = elapsed >= prior_len;
    let mut prior_sum: Option<u64> = Some(0);
    let full_days = elapsed.min(prior_len);
    for b in &prior[..full_days as usize] {
        prior_sum = prior_sum.zip(b.tokens).map(|(a, t)| a + t);
    }
    if !truncated {
        let day = get(prior_start + Days::new(elapsed));
        prior_sum = prior_sum.zip(day.total_before(hour)).map(|(a, t)| a + t);
    }

    let current_complete = current.iter().all(|b| b.tokens.is_some());
    let prior_any = prior.iter().any(|b| b.tokens.is_some());
    let comparison = if elapsed == 0 {
        Comparison::TooEarly
    } else if !current_complete {
        Comparison::Incomplete
    } else if !prior_any {
        Comparison::NoComparison
    } else {
        match prior_sum {
            None => Comparison::PriorIncomplete,
            Some(0) if current_total == 0 => Comparison::NoTraffic,
            Some(0) => Comparison::NoComparison,
            Some(p) => Comparison::Delta((current_total as f64 - p as f64) / p as f64 * 100.0),
        }
    };

    ThroughRelay {
        kind,
        period_days: (end - start).num_days() as u32,
        current,
        prior,
        current_total,
        prior_equivalent_total: prior_sum,
        comparison,
        prior_truncated_to: truncated.then(|| start - Days::new(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn full(tokens_per_hour: u64) -> DayHours {
        DayHours {
            covered: (1 << 24) - 1,
            tokens: [tokens_per_hour; 24],
        }
    }

    #[test]
    fn week_start_respects_first_weekday() {
        // 2026-10-18 is a Sunday.
        assert_eq!(
            period_start(PeriodKind::Week, d(2026, 10, 18), Weekday::Mon),
            d(2026, 10, 12)
        );
        assert_eq!(
            period_start(PeriodKind::Week, d(2026, 10, 18), Weekday::Sun),
            d(2026, 10, 18)
        );
        assert_eq!(
            period_start(PeriodKind::Month, d(2026, 10, 18), Weekday::Mon),
            d(2026, 10, 1)
        );
    }

    #[test]
    fn delta_uses_same_elapsed_hours() {
        // Week starting Mon 2026-10-12; today Wed 10-14 at 12:00.
        let mut days = BTreeMap::new();
        for i in 5..=7 {
            days.insert(d(2026, 10, i), full(10)); // prior Mon–Wed: 240/day
        }
        for i in 8..=11 {
            days.insert(d(2026, 10, i), full(1));
        }
        days.insert(d(2026, 10, 12), full(20));
        days.insert(d(2026, 10, 13), full(20));
        let mut today = DayHours {
            covered: (1 << 13) - 1,
            ..Default::default()
        };
        today.tokens[..12].fill(20);
        days.insert(d(2026, 10, 14), today);
        let tr = through_relay(PeriodKind::Week, d(2026, 10, 14), 12, Weekday::Mon, &days);
        // Prior equivalent: Mon+Tue full (480) + Wed hours 0..12 (120) = 600.
        assert_eq!(tr.prior_equivalent_total, Some(600));
        // Current: 480 + 480 + 240 = 1200.
        assert_eq!(tr.current_total, 1200);
        assert_eq!(tr.comparison, Comparison::Delta(100.0));
        assert_eq!(tr.current.len(), 3);
        assert!(tr.current[2].partial);
        assert_eq!(tr.period_days, 7);
        assert_eq!(tr.prior.len(), 7);
    }

    #[test]
    fn null_and_zero_differ() {
        let mut days = BTreeMap::new();
        days.insert(d(2026, 10, 12), full(0)); // recorded zero
        let tr = through_relay(PeriodKind::Week, d(2026, 10, 13), 5, Weekday::Mon, &days);
        assert_eq!(tr.current[0].tokens, Some(0));
        assert_eq!(tr.current[1].tokens, None);
        assert_eq!(tr.comparison, Comparison::Incomplete);
    }

    #[test]
    fn labels_for_missing_prior() {
        let mut days = BTreeMap::new();
        days.insert(d(2026, 10, 12), full(1));
        days.insert(d(2026, 10, 13), full(1));
        // No prior at all.
        let tr = through_relay(PeriodKind::Week, d(2026, 10, 13), 5, Weekday::Mon, &days);
        assert_eq!(tr.comparison, Comparison::NoComparison);
        // Prior Monday recorded, prior Tuesday hours missing.
        days.insert(d(2026, 10, 5), full(1));
        let tr = through_relay(PeriodKind::Week, d(2026, 10, 13), 5, Weekday::Mon, &days);
        assert_eq!(tr.comparison, Comparison::PriorIncomplete);
        // First day of the period.
        let tr = through_relay(PeriodKind::Week, d(2026, 10, 12), 5, Weekday::Mon, &days);
        assert_eq!(tr.comparison, Comparison::TooEarly);
    }

    #[test]
    fn zero_prior_and_zero_current_is_no_traffic() {
        let mut days = BTreeMap::new();
        for i in 5..=13 {
            days.insert(d(2026, 10, i), full(0));
        }
        let tr = through_relay(PeriodKind::Week, d(2026, 10, 13), 5, Weekday::Mon, &days);
        assert_eq!(tr.comparison, Comparison::NoTraffic);
        days.insert(d(2026, 10, 12), full(1));
        let tr = through_relay(PeriodKind::Week, d(2026, 10, 13), 5, Weekday::Mon, &days);
        assert_eq!(tr.comparison, Comparison::NoComparison);
    }

    #[test]
    fn shorter_prior_month_is_truncated_and_labelled() {
        // Today 2026-03-30; February 2026 has 28 days.
        let mut days = BTreeMap::new();
        for i in 1..=28 {
            days.insert(d(2026, 2, i), full(1));
        }
        for i in 1..=30 {
            days.insert(d(2026, 3, i), full(2));
        }
        let tr = through_relay(PeriodKind::Month, d(2026, 3, 30), 10, Weekday::Mon, &days);
        assert_eq!(tr.prior_truncated_to, Some(d(2026, 2, 28)));
        assert_eq!(tr.prior_equivalent_total, Some(28 * 24));
        assert_eq!(tr.period_days, 31);
    }
}
