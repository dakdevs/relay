//! UI state shared by the popover and the main window. One entity; views
//! observe it and re-render only when it is notified (new snapshot or a user
//! action), never on a timer.

use std::collections::HashSet;

use futures::channel::mpsc::UnboundedSender;

use relay_core::config::{AccountConfig, Config};

use crate::model::{Overview, Period, Range, UsageRow};

/// Copyable values that point a T3 instance at Relay (never real secrets).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Setup {
    pub account_id: String,
    /// Display lines, e.g. env assignments or `-c` launch arguments.
    pub lines: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Quotas,
    Accounts,
    Usage,
    Routing,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Quotas, Tab::Accounts, Tab::Usage, Tab::Routing];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Quotas => "Quotas",
            Tab::Accounts => "Accounts",
            Tab::Usage => "Usage",
            Tab::Routing => "Routing",
        }
    }
}

/// Requests from the UI to the runtime bridge. The bridge does the work off
/// the UI thread and publishes a new snapshot when state changes.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    SetPaused(bool),
    /// Validate, persist and hot-apply a new account.
    AddAccount(Box<AccountConfig>),
    /// Persist and hot-apply routing/UI preferences.
    SaveConfig(Box<Config>),
    /// Fetch copyable T3 setup values for an account.
    ClientSetup(String),
    /// Re-read quota for one account via a documented, non-billed usage read.
    RefreshQuota(String),
    RefreshAll,
    SetAccountPaused {
        id: String,
        paused: bool,
    },
    UsageRange(Range),
    Retry,
    /// Re-evaluate relative times and freshness now (a window is visible).
    Republish,
    Quit,
}

pub struct Store {
    pub overview: Overview,
    pub period: Period,
    pub paused: bool,
    /// Popover group keys that are expanded.
    pub expanded: HashSet<String>,
    /// Main-window ledger groups that are collapsed.
    pub collapsed: HashSet<String>,
    pub tab: Tab,
    pub usage_range: Range,
    pub usage_rows: Vec<UsageRow>,
    pub search: String,
    /// Day highlighted on the Usage chart by pointer or arrow keys.
    pub hover_day: Option<usize>,
    /// Usage chart bounds from the last paint, for pointer → day mapping.
    pub chart_bounds: std::rc::Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
    pub provider_filter: Option<&'static str>,
    pub version: &'static str,
    /// Last loaded configuration (accounts, routing, UI prefs).
    pub config: Config,
    /// Setup values for the account just connected.
    pub setup: Option<Setup>,
    /// Last error from a command, for display.
    pub error: Option<String>,
    commands: Option<UnboundedSender<Command>>,
}

impl Store {
    pub fn new(overview: Overview, commands: Option<UnboundedSender<Command>>) -> Self {
        Self {
            overview,
            period: Period::Week,
            paused: false,
            expanded: HashSet::new(),
            collapsed: HashSet::new(),
            tab: Tab::Quotas,
            usage_range: Range::Week,
            usage_rows: Vec::new(),
            search: String::new(),
            hover_day: None,
            chart_bounds: Default::default(),
            provider_filter: None,
            version: crate::DISPLAY_VERSION,
            config: Config::default(),
            setup: None,
            error: None,
            commands,
        }
    }

    pub fn send(&self, command: Command) {
        if let Some(tx) = &self.commands {
            let _ = tx.unbounded_send(command);
        }
    }

    pub fn toggle_expanded(&mut self, key: &str) -> bool {
        if !self.expanded.remove(key) {
            self.expanded.insert(key.to_string());
            true
        } else {
            false
        }
    }

    /// Edits a copy of the config and asks the bridge to save and apply it.
    pub fn edit_config(&mut self, edit: impl FnOnce(&mut Config)) {
        let mut next = self.config.clone();
        edit(&mut next);
        if next != self.config {
            self.config = next.clone();
            self.send(Command::SaveConfig(Box::new(next)));
        }
    }

    pub fn toggle_paused(&mut self) {
        self.paused = !self.paused;
        self.send(Command::SetPaused(self.paused));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::channel::mpsc::unbounded;

    #[test]
    fn pause_toggle_sends_the_new_state() {
        let (tx, mut rx) = unbounded();
        let mut s = Store::new(Overview::starting("127.0.0.1:8317".into(), 0), Some(tx));
        s.toggle_paused();
        s.toggle_paused();
        assert_eq!(rx.try_recv().ok(), Some(Command::SetPaused(true)));
        assert_eq!(rx.try_recv().ok(), Some(Command::SetPaused(false)));
    }

    #[test]
    fn config_edits_send_only_real_changes() {
        let (tx, mut rx) = unbounded();
        let mut s = Store::new(Overview::starting(String::new(), 0), Some(tx));
        s.edit_config(|_| {});
        assert!(rx.try_recv().is_err());
        s.edit_config(|c| c.routing.sticky = !c.routing.sticky);
        assert!(
            matches!(rx.try_recv().ok(), Some(Command::SaveConfig(c)) if c.routing.sticky == s.config.routing.sticky)
        );
    }

    #[test]
    fn expansion_toggles_per_group() {
        let mut s = Store::new(Overview::starting(String::new(), 0), None);
        assert!(s.toggle_expanded("a"));
        assert!(s.expanded.contains("a"));
        assert!(!s.toggle_expanded("a"));
        assert!(s.expanded.is_empty());
    }
}
