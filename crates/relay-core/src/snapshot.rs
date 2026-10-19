//! The read-only view the desktop app renders. Built off the request path
//! and published by `relay-proxy` at most twice per second, only on change.

use crate::UnixMs;
use crate::aggregate::{WindowAggregate, aggregate_window, window_keys};
use crate::model::{AccountId, AccountMode, CredentialKind, Provider};
use crate::quota::{Credits, Freshness, LOW_LEFT_PCT, QuotaWindow};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Whole UI state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Monotonic publish counter.
    pub version: u64,
    /// When built.
    pub generated_at: UnixMs,
    /// Listener and routing state.
    pub routing: RoutingStatus,
    /// Providers that have at least one account, in [`Provider::ALL`] order.
    pub providers: Vec<ProviderView>,
    /// Usage events dropped because the recorder queue was full.
    pub telemetry_dropped: u64,
    /// Requests currently in flight across all accounts.
    pub in_flight: u32,
}

/// Listener and routing state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum RoutingStatus {
    /// Not started.
    #[default]
    Stopped,
    /// Listening and routing.
    Running {
        /// Bound address.
        addr: SocketAddr,
    },
    /// Listening; router-owned requests are refused, observed traffic still
    /// passes so running CLI sessions are not broken.
    Paused {
        /// Bound address.
        addr: SocketAddr,
    },
    /// Could not start.
    Failed {
        /// Why, for display.
        reason: String,
    },
}

/// One provider section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderView {
    /// Provider.
    pub provider: Provider,
    /// Groups (plan + credential kind), in first-configured order.
    pub groups: Vec<GroupView>,
}

/// Accounts with the same provider, plan and credential kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupView {
    /// Stable key `provider:plan:credential`, for UI expand state.
    pub key: String,
    /// Plan label; `None` when unknown.
    pub plan: Option<String>,
    /// Credential kind.
    pub credential: CredentialKind,
    /// One aggregate per window key seen in the group.
    pub windows: Vec<WindowAggregate>,
    /// Accounts in config order.
    pub accounts: Vec<AccountView>,
}

/// Display state of an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountState {
    /// Serving requests right now.
    InUse,
    /// Fresh data, comfortable headroom.
    Healthy,
    /// Fresh data, below [`LOW_LEFT_PCT`] left.
    Low,
    /// A window is exhausted or the account is cooling down.
    Exhausted,
    /// Data exists but is older than the freshness limit.
    Stale,
    /// No quota data yet.
    Unknown,
    /// Credential missing, expired or rejected.
    Disconnected,
    /// Paused by the user.
    Paused,
}

/// Whether an action is available, with the reason when not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "reason")]
pub enum Capability {
    /// Available.
    Supported,
    /// Not available, with a reason to show.
    Unsupported(String),
}

/// Per-account actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Provider's own "reset limits" action.
    pub reset_limits: Capability,
    /// Documented non-billed quota read.
    pub refresh_quota: Capability,
}

impl Capabilities {
    /// What this version supports for an account. Neither action has a
    /// documented provider API for subscriptions, so both are unsupported
    /// with the reason instead of being faked.
    pub fn for_account(provider: Provider, credential: CredentialKind) -> Capabilities {
        let refresh = match credential {
            CredentialKind::ApiKey => {
                "API rate limits arrive with each response; no separate read exists"
            }
            CredentialKind::Subscription => match provider {
                Provider::Anthropic => {
                    "No documented usage read for Claude subscriptions; values update with traffic"
                }
                Provider::Openai => {
                    "No documented usage read for ChatGPT plans; values update with traffic"
                }
            },
        };
        Capabilities {
            reset_limits: Capability::Unsupported(
                "The provider has no documented reset action for this account".into(),
            ),
            refresh_quota: Capability::Unsupported(refresh.into()),
        }
    }
}

/// One account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountView {
    /// Config id.
    pub id: AccountId,
    /// Provider.
    pub provider: Provider,
    /// Display label.
    pub label: String,
    /// Masked identifier.
    pub identifier: Option<String>,
    /// Plan label (provider-reported, else configured).
    pub plan: Option<String>,
    /// Mode.
    pub mode: AccountMode,
    /// Credential kind.
    pub credential: CredentialKind,
    /// Display state.
    pub state: AccountState,
    /// Detail for `Disconnected` (e.g. "Token expired; run the CLI to sign in again").
    pub state_detail: Option<String>,
    /// Requests in flight.
    pub in_flight: u32,
    /// Live sticky sessions bound here.
    pub sessions: u32,
    /// Last response from upstream.
    pub last_seen: Option<UnixMs>,
    /// Windows as reported.
    pub windows: Vec<QuotaWindow>,
    /// Credits when reported.
    pub credits: Option<Credits>,
    /// Cooling down until.
    pub cooldown_until: Option<UnixMs>,
    /// Models seen through this account, most recent first (bounded).
    pub models_seen: Vec<String>,
    /// Configured model allow-list (empty = all).
    pub models_allowed: Vec<String>,
    /// Actions.
    pub capabilities: Capabilities,
}

/// Inputs for an account's display state.
#[derive(Debug, Clone, Copy)]
pub struct StateInputs {
    /// Paused by the user.
    pub paused: bool,
    /// Credential problem.
    pub disconnected: bool,
    /// Requests in flight.
    pub in_flight: u32,
    /// Cooling down now.
    pub cooling_down: bool,
}

/// Derives the display state. Order matters: user and credential state win
/// over activity, which wins over quota.
pub fn account_state(windows: &[QuotaWindow], inputs: StateInputs, now: UnixMs) -> AccountState {
    if inputs.paused {
        return AccountState::Paused;
    }
    if inputs.disconnected {
        return AccountState::Disconnected;
    }
    if inputs.in_flight > 0 {
        return AccountState::InUse;
    }
    if inputs.cooling_down || windows.iter().any(|w| w.is_exhausted(now)) {
        return AccountState::Exhausted;
    }
    let mut fresh = false;
    let mut stale = false;
    let mut low = false;
    for w in windows {
        match w.freshness(now) {
            Freshness::Fresh | Freshness::Reset => {
                fresh = true;
                low |= w.effective_left(now).is_some_and(|l| l < LOW_LEFT_PCT);
            }
            Freshness::Stale => stale = true,
            Freshness::Unknown => {}
        }
    }
    match (fresh, stale, low) {
        (true, _, true) => AccountState::Low,
        (true, _, false) => AccountState::Healthy,
        (false, true, _) => AccountState::Stale,
        _ => AccountState::Unknown,
    }
}

/// Groups accounts by provider, then plan and credential kind, and computes
/// window aggregates. Accounts keep their input order.
pub fn group_accounts(accounts: Vec<AccountView>, now: UnixMs) -> Vec<ProviderView> {
    let mut providers: Vec<ProviderView> = Vec::new();
    for provider in Provider::ALL {
        let mut groups: Vec<GroupView> = Vec::new();
        for acc in accounts.iter().filter(|a| a.provider == provider) {
            let key = format!(
                "{}:{}:{}",
                provider.key(),
                acc.plan.as_deref().unwrap_or("-"),
                match acc.credential {
                    CredentialKind::Subscription => "subscription",
                    CredentialKind::ApiKey => "api-key",
                }
            );
            match groups.iter_mut().find(|g| g.key == key) {
                Some(g) => g.accounts.push(acc.clone()),
                None => groups.push(GroupView {
                    key,
                    plan: acc.plan.clone(),
                    credential: acc.credential,
                    windows: Vec::new(),
                    accounts: vec![acc.clone()],
                }),
            }
        }
        for g in &mut groups {
            let keys = window_keys(g.accounts.iter().map(|a| a.windows.as_slice()));
            g.windows = keys
                .iter()
                .map(|k| aggregate_window(k, g.accounts.iter().map(|a| a.windows.as_slice()), now))
                .collect();
        }
        if !groups.is_empty() {
            providers.push(ProviderView { provider, groups });
        }
    }
    providers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(id: &str, provider: Provider, plan: Option<&str>, used: Option<f32>) -> AccountView {
        AccountView {
            id: AccountId(id.into()),
            provider,
            label: id.into(),
            identifier: None,
            plan: plan.map(Into::into),
            mode: AccountMode::Observed,
            credential: CredentialKind::Subscription,
            state: AccountState::Unknown,
            state_detail: None,
            in_flight: 0,
            sessions: 0,
            last_seen: None,
            windows: used
                .map(|u| QuotaWindow {
                    key: "5h".into(),
                    label: "5-hour".into(),
                    used_pct: Some(u),
                    resets_at: None,
                    window_minutes: None,
                    status: None,
                    observed_at: 0,
                })
                .into_iter()
                .collect(),
            credits: None,
            cooldown_until: None,
            models_seen: vec![],
            models_allowed: vec![],
            capabilities: Capabilities::for_account(provider, CredentialKind::Subscription),
        }
    }

    #[test]
    fn groups_by_provider_then_plan_and_never_mixes_providers() {
        let out = group_accounts(
            vec![
                view("a", Provider::Openai, Some("Plus"), Some(10.0)),
                view("b", Provider::Anthropic, Some("Max"), Some(20.0)),
                view("c", Provider::Anthropic, Some("Max"), Some(60.0)),
                view("d", Provider::Anthropic, Some("Pro"), None),
            ],
            0,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].provider, Provider::Anthropic);
        assert_eq!(out[0].groups.len(), 2);
        let max = &out[0].groups[0];
        assert_eq!(max.accounts.len(), 2);
        assert_eq!(max.windows[0].avg_left_pct, Some(60.0));
        assert!(out[0].groups[1].windows.is_empty());
        assert_eq!(out[1].groups[0].windows[0].avg_left_pct, Some(90.0));
    }

    #[test]
    fn empty_input_gives_empty_providers() {
        assert!(group_accounts(vec![], 0).is_empty());
    }

    #[test]
    fn state_precedence() {
        let low = view("x", Provider::Anthropic, None, Some(90.0)).windows;
        let base = StateInputs {
            paused: false,
            disconnected: false,
            in_flight: 0,
            cooling_down: false,
        };
        assert_eq!(account_state(&low, base, 0), AccountState::Low);
        assert_eq!(account_state(&[], base, 0), AccountState::Unknown);
        assert_eq!(
            account_state(
                &low,
                StateInputs {
                    in_flight: 1,
                    ..base
                },
                0
            ),
            AccountState::InUse
        );
        assert_eq!(
            account_state(
                &low,
                StateInputs {
                    cooling_down: true,
                    ..base
                },
                0
            ),
            AccountState::Exhausted
        );
        assert_eq!(
            account_state(
                &low,
                StateInputs {
                    paused: true,
                    disconnected: true,
                    ..base
                },
                0
            ),
            AccountState::Paused
        );
        assert_eq!(
            account_state(&low, base, crate::quota::FRESH_FOR_MS + 1),
            AccountState::Stale
        );
    }
}
