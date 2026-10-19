//! Quota windows as reported by providers, and how they age.
//!
//! Quota data is passive: it arrives only with responses. A value is *fresh*
//! for [`FRESH_FOR_MS`] after it was observed, unless its window has reset
//! since, in which case the window is known to be empty again.

use crate::UnixMs;
use serde::{Deserialize, Serialize};

/// How long an observed quota value counts as fresh.
pub const FRESH_FOR_MS: i64 = 15 * 60 * 1000;

/// Percent-left threshold below which an account is shown as low.
pub const LOW_LEFT_PCT: f32 = 20.0;

/// One provider-reported quota window for one account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaWindow {
    /// Stable key, e.g. `5h`, `7d`, `7d_opus`, `primary`, `secondary`,
    /// `requests`, `tokens`.
    pub key: String,
    /// Display label, e.g. `5-hour`, `Weekly`, `Requests / min`.
    pub label: String,
    /// Percent used, 0–100. `None` when the provider did not report it.
    pub used_pct: Option<f32>,
    /// When the window resets.
    pub resets_at: Option<UnixMs>,
    /// Window length when reported.
    pub window_minutes: Option<u32>,
    /// Provider's own status for the window (`allowed`, `allowed_warning`,
    /// `rejected`) when reported.
    pub status: Option<String>,
    /// When Relay observed this value.
    pub observed_at: UnixMs,
}

/// Freshness of one window at a given time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Freshness {
    /// Observed within [`FRESH_FOR_MS`].
    Fresh,
    /// The window's reset time has passed since it was observed; it is
    /// known to be empty (0% used) until traffic reports otherwise.
    Reset,
    /// Observed too long ago.
    Stale,
    /// No usable value.
    Unknown,
}

impl QuotaWindow {
    /// Freshness at `now`.
    pub fn freshness(&self, now: UnixMs) -> Freshness {
        if self.used_pct.is_none() {
            return Freshness::Unknown;
        }
        if let Some(reset) = self.resets_at
            && now >= reset
            && self.observed_at < reset
        {
            return Freshness::Reset;
        }
        if now - self.observed_at <= FRESH_FOR_MS {
            Freshness::Fresh
        } else {
            Freshness::Stale
        }
    }

    /// Effective percent used at `now`: the observed value, 0 after a reset,
    /// `None` when stale or unknown.
    pub fn effective_used(&self, now: UnixMs) -> Option<f32> {
        match self.freshness(now) {
            Freshness::Fresh => self.used_pct,
            Freshness::Reset => Some(0.0),
            Freshness::Stale | Freshness::Unknown => None,
        }
    }

    /// Effective percent left at `now`.
    pub fn effective_left(&self, now: UnixMs) -> Option<f32> {
        self.effective_used(now)
            .map(|u| (100.0 - u).clamp(0.0, 100.0))
    }

    /// Whether the window is known to be exhausted now: fresh data says so,
    /// or stale data says so and its reset is still ahead. Stale exhaustion
    /// without a future reset is unknown, so the account can be probed again
    /// (a renewed rejection then cools it down).
    pub fn is_exhausted(&self, now: UnixMs) -> bool {
        let full =
            self.status.as_deref() == Some("rejected") || self.used_pct.is_some_and(|u| u >= 100.0);
        match self.freshness(now) {
            Freshness::Fresh => full,
            Freshness::Stale => full && self.resets_at.is_some_and(|r| r > now),
            Freshness::Reset | Freshness::Unknown => false,
        }
    }
}

/// Provider-reported credits that can reset a window, when reported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Credits {
    /// Whether the account has credits.
    pub has_credits: Option<bool>,
    /// Unlimited credits.
    pub unlimited: Option<bool>,
    /// Balance as reported (provider units, kept as text).
    pub balance: Option<String>,
    /// When observed.
    pub observed_at: UnixMs,
}

/// All quota knowledge for one account. Updates merge by window key so a
/// response that omits a window never erases it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QuotaState {
    /// Windows by insertion order (stable for display).
    pub windows: Vec<QuotaWindow>,
    /// Plan type when the provider reports it (e.g. Codex `x-codex-plan-type`).
    pub plan: Option<String>,
    /// Credits when reported.
    pub credits: Option<Credits>,
}

/// A parsed quota update from one response.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuotaUpdate {
    /// Windows present in the response.
    pub windows: Vec<QuotaWindow>,
    /// Plan when present.
    pub plan: Option<String>,
    /// Credits when present.
    pub credits: Option<Credits>,
    /// `retry-after` in milliseconds when present.
    pub retry_after_ms: Option<i64>,
}

impl QuotaUpdate {
    /// True when nothing was parsed.
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
            && self.plan.is_none()
            && self.credits.is_none()
            && self.retry_after_ms.is_none()
    }
}

impl QuotaState {
    /// Merges an update. Returns true if anything changed.
    pub fn merge(&mut self, update: &QuotaUpdate) -> bool {
        let mut changed = false;
        for w in &update.windows {
            match self.windows.iter_mut().find(|x| x.key == w.key) {
                Some(existing) => {
                    if existing != w {
                        // Keep fields the update omitted, except a reset time
                        // already past at the new observation and a status
                        // that a new usage value supersedes.
                        let merged = QuotaWindow {
                            used_pct: w.used_pct.or(existing.used_pct),
                            resets_at: w
                                .resets_at
                                .or(existing.resets_at.filter(|r| *r > w.observed_at)),
                            window_minutes: w.window_minutes.or(existing.window_minutes),
                            status: w.status.clone().or_else(|| {
                                w.used_pct
                                    .is_none()
                                    .then(|| existing.status.clone())
                                    .flatten()
                            }),
                            ..w.clone()
                        };
                        changed |= *existing != merged;
                        *existing = merged;
                    }
                }
                None => {
                    self.windows.push(w.clone());
                    changed = true;
                }
            }
        }
        if update.plan.is_some() && update.plan != self.plan {
            self.plan = update.plan.clone();
            changed = true;
        }
        if update.credits.is_some() && update.credits != self.credits {
            self.credits = update.credits.clone();
            changed = true;
        }
        changed
    }

    /// Headroom used for selection: the minimum percent left across fresh
    /// windows, as a 0–1 fraction. `None` when no window is fresh.
    pub fn headroom(&self, now: UnixMs) -> Option<f32> {
        self.windows
            .iter()
            .filter_map(|w| w.effective_left(now))
            .fold(None, |acc: Option<f32>, left| {
                Some(acc.map_or(left, |a| a.min(left)))
            })
            .map(|pct| pct / 100.0)
    }

    /// Earliest reset among windows that bind (highest effective use).
    pub fn binding_reset(&self, now: UnixMs) -> Option<UnixMs> {
        self.windows
            .iter()
            .filter(|w| w.effective_used(now).is_some())
            .max_by(|a, b| {
                a.effective_used(now)
                    .partial_cmp(&b.effective_used(now))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|w| w.resets_at)
    }

    /// Whether any window is exhausted now.
    pub fn exhausted(&self, now: UnixMs) -> bool {
        self.windows.iter().any(|w| w.is_exhausted(now))
    }

    /// Earliest time at which this state's freshness classification changes.
    /// Lets snapshot publishers sleep instead of polling.
    pub fn next_transition(&self, now: UnixMs) -> Option<UnixMs> {
        self.windows
            .iter()
            .flat_map(|w| {
                let stale_at = w.observed_at + FRESH_FOR_MS;
                [
                    (stale_at > now).then_some(stale_at),
                    w.resets_at.filter(|r| *r > now),
                ]
            })
            .flatten()
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(key: &str, used: Option<f32>, reset: Option<i64>, observed: i64) -> QuotaWindow {
        QuotaWindow {
            key: key.into(),
            label: key.into(),
            used_pct: used,
            resets_at: reset,
            window_minutes: None,
            status: None,
            observed_at: observed,
        }
    }

    #[test]
    fn freshness_ages_and_resets() {
        let win = w("5h", Some(40.0), Some(10_000_000), 1_000);
        assert_eq!(win.freshness(1_000 + FRESH_FOR_MS), Freshness::Fresh);
        assert_eq!(win.freshness(1_001 + FRESH_FOR_MS), Freshness::Stale);
        // After the reset time the window is empty, not stale.
        assert_eq!(win.freshness(10_000_000), Freshness::Reset);
        assert_eq!(win.effective_used(10_000_000), Some(0.0));
        assert_eq!(w("x", None, None, 0).freshness(0), Freshness::Unknown);
    }

    #[test]
    fn headroom_is_min_left_over_fresh_windows() {
        let s = QuotaState {
            windows: vec![
                w("5h", Some(30.0), None, 0),
                w("7d", Some(85.0), None, 0),
                w("old", Some(1.0), None, -FRESH_FOR_MS - 1),
            ],
            ..Default::default()
        };
        // 7d binds: 15% left. The stale window is ignored.
        assert!((s.headroom(0).unwrap() - 0.15).abs() < 1e-6);
        assert_eq!(QuotaState::default().headroom(0), None);
    }

    #[test]
    fn merge_keeps_omitted_fields_and_reports_change() {
        let mut s = QuotaState::default();
        let mut up = QuotaUpdate {
            windows: vec![w("5h", Some(10.0), Some(500), 1)],
            ..Default::default()
        };
        assert!(s.merge(&up));
        assert!(!s.merge(&up));
        up.windows[0] = w("5h", Some(20.0), None, 2);
        assert!(s.merge(&up));
        assert_eq!(s.windows[0].resets_at, Some(500));
        assert_eq!(s.windows[0].used_pct, Some(20.0));
    }

    #[test]
    fn exhausted_by_status_or_full_but_not_after_reset() {
        let mut win = w("5h", Some(50.0), Some(100), 0);
        win.status = Some("rejected".into());
        assert!(win.is_exhausted(50));
        assert!(!win.is_exhausted(100));
        assert!(w("x", Some(100.0), None, 0).is_exhausted(0));
    }

    #[test]
    fn stale_exhaustion_without_future_reset_is_probed_again() {
        let full = w("5h", Some(100.0), None, 0);
        assert!(full.is_exhausted(FRESH_FOR_MS));
        assert!(!full.is_exhausted(FRESH_FOR_MS + 1));
        // Stale, but the provider said when it resets: still excluded until then.
        let later = w("5h", Some(100.0), Some(FRESH_FOR_MS * 4), 0);
        assert!(later.is_exhausted(FRESH_FOR_MS * 2));
        assert!(!later.is_exhausted(FRESH_FOR_MS * 4));
    }

    #[test]
    fn merge_drops_past_reset_and_superseded_status() {
        let mut s = QuotaState::default();
        let mut first = w("5h", Some(100.0), Some(1_000), 0);
        first.status = Some("rejected".into());
        s.merge(&QuotaUpdate {
            windows: vec![first],
            ..Default::default()
        });
        // Later observation without reset or status.
        s.merge(&QuotaUpdate {
            windows: vec![w("5h", Some(100.0), None, 2_000)],
            ..Default::default()
        });
        assert_eq!(s.windows[0].resets_at, None, "past reset is not carried");
        assert_eq!(s.windows[0].status, None);
        // Once stale it no longer excludes the account forever.
        assert!(!s.exhausted(2_000 + FRESH_FOR_MS + 1));
    }

    #[test]
    fn next_transition_is_earliest_future_event() {
        let s = QuotaState {
            windows: vec![w("5h", Some(1.0), Some(5_000), 0)],
            ..Default::default()
        };
        assert_eq!(s.next_transition(0), Some(5_000));
        assert_eq!(s.next_transition(5_000), Some(FRESH_FOR_MS));
    }
}
