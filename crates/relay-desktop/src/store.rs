//! UI state shared by the popover and the main window. One entity; views
//! observe it and re-render only when it is notified (new snapshot or a user
//! action), never on a timer.

use std::collections::{HashMap, HashSet};

use futures::channel::mpsc::UnboundedSender;

use relay_core::{
    config::{AccountConfig, Config, SecretRef},
    model::{AccountMode, CredentialKind},
    select::Strategy,
};

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

/// One setting change. The bridge applies it to its canonical config, so a
/// stale UI copy can never overwrite accounts or other settings.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigEdit {
    Strategy(Strategy),
    Sticky(bool),
    SkipBelowPct(f32),
    ListenPort(u16),
}

impl ConfigEdit {
    pub fn apply(&self, c: &mut Config) {
        match self {
            ConfigEdit::Strategy(s) => c.routing.strategy = *s,
            ConfigEdit::Sticky(on) => c.routing.sticky = *on,
            ConfigEdit::SkipBelowPct(p) => c.routing.skip_below_pct = *p,
            ConfigEdit::ListenPort(port) => c.server.listen.set_port(*port),
        }
    }
}

/// The fields the connect sheet can change on an existing account. The
/// bridge merges them onto its canonical copy, so provider, plan, upstream,
/// models, concurrency and pause are never touched by the sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct AccountEdit {
    pub label: String,
    /// New masked identifier; `None` keeps the saved one.
    pub identifier: Option<String>,
    pub mode: AccountMode,
    pub credential: Option<SecretRef>,
    pub credential_kind: Option<CredentialKind>,
}

impl AccountEdit {
    pub fn merge_onto(&self, existing: &AccountConfig) -> AccountConfig {
        let mut next = existing.clone();
        next.label = self.label.clone();
        if let Some(id) = &self.identifier {
            next.identifier = Some(id.clone());
        }
        next.mode = self.mode;
        next.credential = self.credential.clone();
        next.credential_kind = self.credential_kind;
        next
    }
}

/// Correlates a connect-sheet request with its replies, so a late reply to
/// an earlier request can neither advance nor cancel the current one.
pub type Op = u64;

/// Replies to correlated connect requests.
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectEvent {
    Saved { op: Op, id: String },
    Setup { op: Op, setup: Setup },
    Failed { op: Op, message: String },
}

/// Where an error is shown, so one action's failure never masks another's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Routing settings, port, restart notices.
    Routing,
    /// Open at login.
    Login,
    /// Account rows: pause, resume.
    Accounts,
    /// Popover and app-wide (pause routing, runtime).
    App,
}

/// Shown when a command can't reach the runtime thread.
pub const NOT_RUNNING: &str =
    "Relay's runtime isn't running, so this change wasn't sent. Quit and reopen Relay.";

/// Requests from the UI to the runtime bridge. The bridge does the work off
/// the UI thread and publishes a new snapshot when state changes.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    SetPaused(bool),
    /// Validate, persist and hot-apply a new account; replies
    /// `ConnectEvent::Saved` then `Setup`, or `Failed`, all with `op`.
    AddAccount {
        op: Op,
        account: Box<AccountConfig>,
    },
    /// Merge editable fields onto the canonical account `id` (same replies).
    UpdateAccount {
        op: Op,
        id: String,
        edit: Box<AccountEdit>,
    },
    /// Apply one setting to the canonical config, persist and hot-apply.
    Edit(ConfigEdit),
    /// Fetch copyable T3 setup values for an account (`Setup` or `Failed`).
    ClientSetup {
        op: Op,
        id: String,
    },
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
    /// Recent replies to connect requests (bounded); the sheet matches by op.
    pub connect_events: Vec<ConnectEvent>,
    next_op: Op,
    errors: HashMap<Scope, String>,
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
            connect_events: Vec::new(),
            next_op: 1,
            errors: HashMap::new(),
            commands,
        }
    }

    /// Sends a command to the runtime thread. Fails (with a display-ready
    /// reason) when there is no runtime or it has stopped.
    pub fn send(&self, command: Command) -> Result<(), String> {
        match &self.commands {
            Some(tx) => tx
                .unbounded_send(command)
                .map_err(|_| NOT_RUNNING.to_string()),
            None => Err(NOT_RUNNING.to_string()),
        }
    }

    /// Sends, or records the failure under `scope`. Returns whether it was sent.
    pub fn send_or_report(&mut self, command: Command, scope: Scope) -> bool {
        match self.send(command) {
            Ok(()) => true,
            Err(e) => {
                self.set_error(scope, e);
                false
            }
        }
    }

    pub fn next_op(&mut self) -> Op {
        let op = self.next_op;
        self.next_op += 1;
        op
    }

    /// Keeps the last few replies; older ones can't match a live request.
    pub fn push_connect_event(&mut self, event: ConnectEvent) {
        const KEEP: usize = 16;
        self.connect_events.push(event);
        if self.connect_events.len() > KEEP {
            self.connect_events.remove(0);
        }
    }

    pub fn connect_event(&self, op: Op) -> impl Iterator<Item = &ConnectEvent> {
        self.connect_events.iter().filter(move |e| match e {
            ConnectEvent::Saved { op: o, .. }
            | ConnectEvent::Setup { op: o, .. }
            | ConnectEvent::Failed { op: o, .. } => *o == op,
        })
    }

    pub fn toggle_expanded(&mut self, key: &str) -> bool {
        if !self.expanded.remove(key) {
            self.expanded.insert(key.to_string());
            true
        } else {
            false
        }
    }

    /// Applies one setting optimistically and asks the bridge to apply the
    /// same edit to its canonical config. No-ops send nothing.
    pub fn edit(&mut self, edit: ConfigEdit) {
        let mut next = self.config.clone();
        edit.apply(&mut next);
        if next != self.config {
            let previous = std::mem::replace(&mut self.config, next);
            self.clear_error(Scope::Routing);
            if !self.send_or_report(Command::Edit(edit), Scope::Routing) {
                // Not sent: show the value that is actually saved.
                self.config = previous;
            }
        }
    }

    pub fn error(&self, scope: Scope) -> Option<&str> {
        self.errors.get(&scope).map(String::as_str)
    }

    pub fn set_error(&mut self, scope: Scope, message: impl Into<String>) {
        self.errors.insert(scope, message.into());
    }

    pub fn clear_error(&mut self, scope: Scope) {
        self.errors.remove(&scope);
    }

    pub fn toggle_paused(&mut self) {
        if self.send_or_report(Command::SetPaused(!self.paused), Scope::App) {
            self.paused = !self.paused;
        }
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
    fn edits_are_field_scoped_and_skip_no_ops() {
        let (tx, mut rx) = unbounded();
        let mut s = Store::new(Overview::starting(String::new(), 0), Some(tx));
        let sticky = s.config.routing.sticky;
        s.edit(ConfigEdit::Sticky(sticky));
        assert!(rx.try_recv().is_err());
        s.edit(ConfigEdit::Sticky(!sticky));
        assert_eq!(
            rx.try_recv().ok(),
            Some(Command::Edit(ConfigEdit::Sticky(!sticky)))
        );
        assert_eq!(s.config.routing.sticky, !sticky);
    }

    #[test]
    fn errors_are_scoped() {
        let mut s = Store::new(Overview::starting(String::new(), 0), None);
        s.set_error(Scope::Routing, "port taken");
        s.set_error(Scope::Login, "disk full");
        // A routing edit clears only routing errors, not a login failure.
        s.edit(ConfigEdit::Sticky(!s.config.routing.sticky));
        assert_eq!(
            s.error(Scope::Routing),
            Some(NOT_RUNNING),
            "unsent edit is reported"
        );
        assert_eq!(s.error(Scope::Login), Some("disk full"));
    }

    #[test]
    fn unsent_commands_report_instead_of_hanging() {
        // No runtime: nothing is pretended.
        let mut s = Store::new(Overview::starting(String::new(), 0), None);
        let sticky = s.config.routing.sticky;
        s.edit(ConfigEdit::Sticky(!sticky));
        assert_eq!(
            s.config.routing.sticky, sticky,
            "optimistic value rolled back"
        );
        assert_eq!(s.error(Scope::Routing), Some(NOT_RUNNING));
        s.toggle_paused();
        assert!(!s.paused);
        assert_eq!(s.error(Scope::App), Some(NOT_RUNNING));
        // A stopped runtime (receiver dropped) is reported the same way.
        let (tx, rx) = unbounded();
        drop(rx);
        let s = Store::new(Overview::starting(String::new(), 0), Some(tx));
        assert_eq!(s.send(Command::Republish), Err(NOT_RUNNING.to_string()));
    }

    #[test]
    fn account_edits_keep_fields_the_sheet_does_not_show() {
        use relay_core::model::{AccountId, Provider};
        let existing = AccountConfig {
            id: AccountId("openai-dev".into()),
            provider: Provider::Openai,
            label: "Dev".into(),
            mode: AccountMode::Observed,
            plan: Some("Plus".into()),
            identifier: Some("d••@acme.co".into()),
            credential_kind: None,
            upstream: Some("https://example.invalid/codex".into()),
            credential: None,
            paused: true,
            max_in_flight: 3,
            models: vec!["gpt-5*".into()],
        };
        let edit = AccountEdit {
            label: "Dev laptop".into(),
            identifier: None,
            mode: AccountMode::Observed,
            credential: None,
            credential_kind: None,
        };
        let merged = edit.merge_onto(&existing);
        assert_eq!(merged.label, "Dev laptop");
        assert_eq!(
            (
                &merged.id,
                merged.provider,
                &merged.plan,
                &merged.identifier,
                &merged.upstream,
                merged.paused,
                merged.max_in_flight,
                &merged.models
            ),
            (
                &existing.id,
                existing.provider,
                &existing.plan,
                &existing.identifier,
                &existing.upstream,
                true,
                3,
                &existing.models
            )
        );
        let renamed = AccountEdit {
            identifier: Some("x••@y.io".into()),
            ..edit
        }
        .merge_onto(&existing);
        assert_eq!(renamed.identifier.as_deref(), Some("x••@y.io"));
    }

    #[test]
    fn connect_events_are_bounded_and_matched_by_op() {
        let mut s = Store::new(Overview::starting(String::new(), 0), None);
        let a = s.next_op();
        let b = s.next_op();
        assert_ne!(a, b);
        s.push_connect_event(ConnectEvent::Failed {
            op: a,
            message: "late".into(),
        });
        s.push_connect_event(ConnectEvent::Saved {
            op: b,
            id: "x".into(),
        });
        assert_eq!(s.connect_event(b).count(), 1);
        assert!(matches!(
            s.connect_event(a).next(),
            Some(ConnectEvent::Failed { .. })
        ));
        for i in 0..40 {
            s.push_connect_event(ConnectEvent::Saved {
                op: 100 + i,
                id: "y".into(),
            });
        }
        assert!(s.connect_events.len() <= 16);
        assert_eq!(s.connect_event(a).count(), 0);
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
