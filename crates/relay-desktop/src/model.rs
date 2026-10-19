//! Presentation model: display-ready data for the popover and main window.
//!
//! Built from the core snapshot by `bridge.rs` once per published snapshot
//! (at most twice a second), never per frame. Views only read it. Numbers are
//! quota *left*; `None` means unknown and is never drawn as 0%.

use crate::assets::Mark;

pub type UnixMs = i64;

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Starting,
    Running,
    Paused,
    /// The proxy is not accepting requests (e.g. the port is taken).
    NotRouting {
        title: String,
        detail: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeTag {
    Pooled,
    Observed,
    Key,
    /// A group holding both observed and router-owned accounts.
    Mixed,
}

impl ModeTag {
    pub fn text(self) -> &'static str {
        match self {
            ModeTag::Pooled | ModeTag::Key => "Pooled",
            ModeTag::Observed => "Observed",
            ModeTag::Mixed => "Mixed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Healthy,
    Low,
    Exhausted,
    Stale,
    Unknown,
    Disconnected,
    Paused,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccountWindow {
    /// "5-hour", "Weekly", "Fable 7d", "Tokens/min".
    pub label: String,
    pub left: Option<f32>,
    pub resets_at: Option<UnixMs>,
    pub stale: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Account {
    pub id: String,
    pub provider: relay_core::model::Provider,
    pub label: String,
    pub masked: String,
    pub plan: String,
    pub credential: String,
    pub state: State,
    /// Sessions currently bound to this account.
    pub in_use: u32,
    pub seen_at: Option<UnixMs>,
    pub windows: Vec<AccountWindow>,
    /// Only when the provider reports them.
    pub credits: Option<String>,
    /// One-line reason, e.g. "T3 instance not found", "Paused by you · not routed".
    pub note: Option<String>,
    /// `None` when the provider action is supported; otherwise why not.
    pub reset_unsupported: Option<String>,
    pub refresh_unsupported: Option<String>,
    pub mode: ModeTag,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupWindow {
    pub label: String,
    /// Mean left over fresh accounts; `None` when no account is fresh.
    pub avg_left: Option<f32>,
    pub fresh: usize,
    pub stale: usize,
    /// Accounts in the group (the N in "X% of N × 100%").
    pub accounts: usize,
    /// Sum of each fresh account's left, in percent.
    pub pooled_left: f32,
    pub first_reset: Option<UnixMs>,
    pub last_reset: Option<UnixMs>,
    /// Per-account values in account order, for summary segments.
    pub segments: Vec<(Option<f32>, bool)>,
    /// Model-specific limit (nested and collapsible in the summary).
    pub model_limit: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// Stable key (provider + plan + credential) for expansion state.
    pub key: String,
    pub title: String,
    pub mark: Option<Mark>,
    /// "3 subscriptions", "2 keys".
    pub count_label: String,
    pub mode: ModeTag,
    pub windows: Vec<GroupWindow>,
    pub accounts: Vec<Account>,
}

impl Group {
    pub fn stale_count(&self) -> usize {
        self.accounts
            .iter()
            .filter(|a| a.state == State::Stale)
            .count()
    }

    /// Accounts carrying sessions now, by label. Never a prediction.
    pub fn in_use(&self) -> Vec<(&str, u32)> {
        self.accounts
            .iter()
            .filter(|a| a.in_use > 0)
            .map(|a| (a.label.as_str(), a.in_use))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub provider: &'static str,
    pub mark: Mark,
    pub groups: Vec<Group>,
}

impl Section {
    pub fn account_count(&self) -> usize {
        self.groups.iter().map(|g| g.accounts.len()).sum()
    }
}

/// Comparison label states (board 9).
#[derive(Clone, Debug, PartialEq)]
pub enum Comparison {
    Delta(f64),
    TooEarly,
    NoComparison,
    PriorIncomplete,
    Incomplete,
}

impl Comparison {
    pub fn muted_label(&self) -> Option<&'static str> {
        match self {
            Comparison::Delta(_) => None,
            Comparison::TooEarly => Some("Too early"),
            Comparison::NoComparison => Some("No comparison"),
            Comparison::PriorIncomplete => Some("Prior incomplete"),
            Comparison::Incomplete => Some("Incomplete"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Series {
    /// Daily buckets for the current span; the last one is partial.
    pub current: Vec<Option<u64>>,
    /// Same-elapsed prior span, aligned by day index.
    pub prior: Vec<Option<u64>>,
    /// Number of day slots on the x axis (7 for a week, days in month).
    pub slots: usize,
    pub day_labels: Vec<String>,
    /// "Thu Oct 15" for each current and prior bucket (hover/focus detail).
    pub current_dates: Vec<String>,
    pub prior_dates: Vec<String>,
    pub total: Option<u64>,
    pub prior_total: Option<u64>,
    /// "vs 6.3M · same span last week".
    pub caption: String,
    pub legend_current: String,
    pub legend_prior: String,
    pub comparison: Option<Comparison>,
    /// Long span description for the Usage tab: "Mon 00:00 → Sun 09:41, local time".
    pub span: String,
}

impl Series {
    pub fn has_traffic(&self) -> bool {
        self.total.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Period {
    #[default]
    Week,
    Month,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Through {
    pub week: Series,
    pub month: Series,
}

impl Through {
    pub fn get(&self, period: Period) -> &Series {
        match period {
            Period::Week => &self.week,
            Period::Month => &self.month,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct UsageRow {
    pub label: String,
    pub sublabel: String,
    pub mark: Option<Mark>,
    pub input: u64,
    pub output: u64,
    pub cached: u64,
    pub requests: u64,
    pub errors: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Range {
    Day,
    #[default]
    Week,
    Month,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Overview {
    pub status: Status,
    pub address: String,
    pub sections: Vec<Section>,
    pub through: Through,
    pub now: UnixMs,
}

impl Overview {
    pub fn starting(address: String, now: UnixMs) -> Self {
        Self {
            status: Status::Starting,
            address,
            sections: Vec::new(),
            through: Through::default(),
            now,
        }
    }

    pub fn account_count(&self) -> usize {
        self.sections.iter().map(Section::account_count).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.account_count() == 0
    }

    /// Native fallback menu text (board 7): status, one line per group with
    /// its comparable windows, then accounts in use. Never a prediction.
    pub fn menu(&self, paused: bool) -> crate::tray::MenuModel {
        let status = match &self.status {
            Status::Starting => "Relay — Starting".to_string(),
            Status::Running => "Relay — Running".to_string(),
            Status::Paused => "Relay — Paused".to_string(),
            Status::NotRouting { title, .. } => format!("Relay — Not routing · {title}"),
        };
        let mut rows = Vec::new();
        for g in self.sections.iter().flat_map(|s| &s.groups) {
            let mut line = g.title.clone();
            for w in g.windows.iter().filter(|w| !w.model_limit).take(2) {
                let value = w.avg_left.map_or("—".to_string(), crate::format::percent);
                line.push_str(&format!(" · {} {}", w.label.to_lowercase(), value));
            }
            let stale = g.stale_count();
            if stale > 0 {
                line.push_str(&format!(" · {stale} stale"));
            }
            rows.push((line, false));
            for (label, n) in g.in_use() {
                let s = if n == 1 { "session" } else { "sessions" };
                rows.push((format!("{label} — in use by {n} {s}"), true));
            }
        }
        crate::tray::MenuModel {
            status,
            rows,
            paused,
        }
    }
}

#[cfg(test)]
pub mod fixtures {
    //! Hand-built overviews for view-model tests (values mirror board 1).
    use super::*;

    pub fn window(label: &str, left: Option<f32>) -> AccountWindow {
        AccountWindow {
            label: label.into(),
            left,
            resets_at: None,
            stale: false,
        }
    }

    pub fn account(label: &str, in_use: u32, state: State) -> Account {
        Account {
            id: label.to_lowercase(),
            provider: relay_core::model::Provider::Anthropic,
            label: label.into(),
            masked: "d••@example.com".into(),
            plan: "Max 20×".into(),
            credential: "subscription".into(),
            state,
            in_use,
            seen_at: None,
            windows: vec![window("5-hour", Some(50.0))],
            credits: None,
            note: None,
            reset_unsupported: Some("No documented reset action".into()),
            refresh_unsupported: Some("No documented usage read".into()),
            mode: ModeTag::Pooled,
        }
    }

    pub fn group_window(label: &str, avg: Option<f32>, fresh: usize) -> GroupWindow {
        GroupWindow {
            label: label.into(),
            avg_left: avg,
            fresh,
            stale: 0,
            accounts: fresh,
            pooled_left: avg.unwrap_or(0.0) * fresh as f32,
            first_reset: None,
            last_reset: None,
            segments: vec![],
            model_limit: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fixtures::*, *};

    fn overview() -> Overview {
        let claude = Group {
            key: "anthropic/max/sub".into(),
            title: "Claude Max".into(),
            mark: Some(Mark::Claude),
            count_label: "3 subscriptions".into(),
            mode: ModeTag::Pooled,
            windows: vec![
                group_window("5-hour", Some(48.0), 3),
                group_window("Weekly", Some(71.0), 3),
                {
                    let mut w = group_window("Fable 7d", Some(97.0), 3);
                    w.model_limit = true;
                    w
                },
            ],
            accounts: vec![
                account("Personal", 0, State::Healthy),
                account("Work", 2, State::Healthy),
                account("Secondary", 0, State::Exhausted),
            ],
        };
        let codex = Group {
            key: "openai/plus/sub".into(),
            title: "ChatGPT Plus · Codex".into(),
            mark: Some(Mark::ChatGpt),
            count_label: "2".into(),
            mode: ModeTag::Observed,
            windows: vec![
                group_window("5-hour", Some(22.0), 1),
                group_window("Weekly", None, 0),
            ],
            accounts: vec![
                account("Dev", 1, State::Low),
                account("Team", 0, State::Stale),
            ],
        };
        Overview {
            status: Status::Running,
            address: "127.0.0.1:8317".into(),
            sections: vec![
                Section {
                    provider: "Anthropic",
                    mark: Mark::Anthropic,
                    groups: vec![claude],
                },
                Section {
                    provider: "OpenAI",
                    mark: Mark::OpenAi,
                    groups: vec![codex],
                },
            ],
            through: Through::default(),
            now: 0,
        }
    }

    #[test]
    fn native_menu_matches_board_seven_order_and_text() {
        let menu = overview().menu(false);
        assert_eq!(menu.status, "Relay — Running");
        assert_eq!(
            menu.rows,
            vec![
                ("Claude Max · 5-hour 48% · weekly 71%".to_string(), false),
                ("Work — in use by 2 sessions".to_string(), true),
                (
                    "ChatGPT Plus · Codex · 5-hour 22% · weekly — · 1 stale".to_string(),
                    false
                ),
                ("Dev — in use by 1 session".to_string(), true),
            ]
        );
    }

    #[test]
    fn counts_and_in_use_are_current_state_only() {
        let o = overview();
        assert_eq!(o.account_count(), 5);
        assert!(!o.is_empty());
        let g = &o.sections[0].groups[0];
        assert_eq!(g.in_use(), vec![("Work", 2)]);
        assert_eq!(o.sections[1].groups[0].stale_count(), 1);
    }

    #[test]
    fn comparison_labels() {
        assert_eq!(Comparison::Delta(17.0).muted_label(), None);
        assert_eq!(Comparison::TooEarly.muted_label(), Some("Too early"));
        assert_eq!(
            Comparison::PriorIncomplete.muted_label(),
            Some("Prior incomplete")
        );
    }
}
