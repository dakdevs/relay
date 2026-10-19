//! Account selection for router-owned (pooled or key) requests.
//!
//! Selection runs once per new session (or per request without a session) on
//! a slice prepared by the caller. It does not allocate and does not lock.

use crate::UnixMs;
use serde::{Deserialize, Serialize};

/// Placement strategy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    /// Most headroom wins; near-ties go to the account whose window resets
    /// soonest, so quota about to expire is used first.
    #[default]
    QuotaAware,
    /// Use accounts in configured order until each runs low.
    FillFirst,
    /// Rotate through eligible accounts.
    RoundRobin,
}

/// Headroom difference treated as a tie by [`Strategy::QuotaAware`].
pub const TIE_EPSILON: f32 = 0.05;

/// One account as seen by the selector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// Enabled, supports the model, not cooling down, not saturated, and
    /// credential available.
    pub eligible: bool,
    /// Fraction left (0–1) of the binding window; `None` when unknown.
    pub headroom: Option<f32>,
    /// Reset time of the binding window.
    pub resets_at: Option<UnixMs>,
}

impl Candidate {
    /// Unknown headroom ranks as full: an account Relay has no data for is
    /// the one that produces data when chosen, and a wrong guess costs one
    /// rejected first turn, which fails over.
    fn effective_headroom(&self) -> f32 {
        self.headroom.unwrap_or(1.0)
    }
}

/// Picks a candidate index or `None` when nothing is eligible.
///
/// `skip_below` (0–1) excludes accounts below that headroom unless every
/// eligible account is below it. `cursor` breaks remaining ties and drives
/// round-robin; callers advance it per decision.
pub fn select(
    candidates: &[Candidate],
    strategy: Strategy,
    skip_below: f32,
    cursor: usize,
) -> Option<usize> {
    let n = candidates.len();
    if n == 0 {
        return None;
    }
    let any_above = candidates
        .iter()
        .any(|c| c.eligible && c.effective_headroom() >= skip_below);
    let usable = |c: &Candidate| c.eligible && (!any_above || c.effective_headroom() >= skip_below);
    // Visit in rotated order so ties rotate without extra state.
    let rotated = (0..n).map(|i| (cursor + i) % n);

    match strategy {
        Strategy::FillFirst => (0..n).find(|&i| usable(&candidates[i])),
        Strategy::RoundRobin => rotated.clone().find(|&i| usable(&candidates[i])),
        Strategy::QuotaAware => {
            let best = candidates
                .iter()
                .filter(|c| usable(c))
                .map(Candidate::effective_headroom)
                .fold(f32::NEG_INFINITY, f32::max);
            if best == f32::NEG_INFINITY {
                return None;
            }
            let mut pick: Option<usize> = None;
            for i in rotated {
                let c = &candidates[i];
                if !usable(c) || c.effective_headroom() < best - TIE_EPSILON {
                    continue;
                }
                pick = match pick {
                    None => Some(i),
                    Some(p) => {
                        let current = candidates[p].resets_at.unwrap_or(UnixMs::MAX);
                        let challenger = c.resets_at.unwrap_or(UnixMs::MAX);
                        if challenger < current {
                            Some(i)
                        } else {
                            Some(p)
                        }
                    }
                };
            }
            pick
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(eligible: bool, headroom: Option<f32>, resets_at: Option<i64>) -> Candidate {
        Candidate {
            eligible,
            headroom,
            resets_at,
        }
    }

    #[test]
    fn quota_aware_picks_most_headroom_and_skips_ineligible() {
        let cands = [
            c(true, Some(0.30), None),
            c(false, Some(0.95), None), // cooling down
            c(true, Some(0.60), None),
        ];
        assert_eq!(select(&cands, Strategy::QuotaAware, 0.0, 0), Some(2));
    }

    #[test]
    fn near_tie_prefers_sooner_reset() {
        let cands = [
            c(true, Some(0.80), Some(9_000)),
            c(true, Some(0.77), Some(1_000)),
        ];
        // 0.77 is within 5 points of 0.80 and resets sooner.
        assert_eq!(select(&cands, Strategy::QuotaAware, 0.0, 0), Some(1));
        let far = [
            c(true, Some(0.80), Some(9_000)),
            c(true, Some(0.70), Some(1_000)),
        ];
        assert_eq!(select(&far, Strategy::QuotaAware, 0.0, 0), Some(0));
    }

    #[test]
    fn exact_ties_rotate_with_cursor() {
        let cands = [c(true, Some(0.5), None), c(true, Some(0.5), None)];
        assert_eq!(select(&cands, Strategy::QuotaAware, 0.0, 0), Some(0));
        assert_eq!(select(&cands, Strategy::QuotaAware, 0.0, 1), Some(1));
    }

    #[test]
    fn none_when_nothing_eligible() {
        assert_eq!(select(&[], Strategy::QuotaAware, 0.0, 0), None);
        let cands = [c(false, Some(1.0), None)];
        for s in [
            Strategy::QuotaAware,
            Strategy::FillFirst,
            Strategy::RoundRobin,
        ] {
            assert_eq!(select(&cands, s, 0.0, 0), None);
        }
    }

    #[test]
    fn skip_below_applies_only_when_something_is_above() {
        let cands = [c(true, Some(0.05), None), c(true, Some(0.50), None)];
        assert_eq!(select(&cands, Strategy::FillFirst, 0.10, 0), Some(1));
        let all_low = [c(true, Some(0.05), None), c(true, Some(0.08), None)];
        assert_eq!(select(&all_low, Strategy::FillFirst, 0.10, 0), Some(0));
    }

    #[test]
    fn round_robin_rotates_over_eligible() {
        let cands = [
            c(true, None, None),
            c(false, None, None),
            c(true, None, None),
        ];
        assert_eq!(select(&cands, Strategy::RoundRobin, 0.0, 0), Some(0));
        assert_eq!(select(&cands, Strategy::RoundRobin, 0.0, 1), Some(2));
        assert_eq!(select(&cands, Strategy::RoundRobin, 0.0, 3), Some(0));
    }

    #[test]
    fn unknown_headroom_is_tried() {
        let cands = [c(true, Some(0.4), None), c(true, None, None)];
        assert_eq!(select(&cands, Strategy::QuotaAware, 0.0, 0), Some(1));
    }

    /// Property: the result is always eligible, for many generated inputs.
    #[test]
    fn result_is_always_eligible() {
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..5_000 {
            let n = (next() % 8) as usize;
            let cands: Vec<Candidate> = (0..n)
                .map(|_| {
                    let r = next();
                    c(
                        r % 3 != 0,
                        (r % 5 != 0).then_some((r % 101) as f32 / 100.0),
                        (r % 2 == 0).then_some((r % 1000) as i64),
                    )
                })
                .collect();
            let skip = (next() % 30) as f32 / 100.0;
            for s in [
                Strategy::QuotaAware,
                Strategy::FillFirst,
                Strategy::RoundRobin,
            ] {
                let got = select(&cands, s, skip, next() as usize);
                match got {
                    Some(i) => assert!(cands[i].eligible),
                    None => assert!(cands.iter().all(|c| !c.eligible)),
                }
            }
        }
    }
}
