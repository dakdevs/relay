//! Runtime bridge. A dedicated thread owns the proxy `Handle`, the config
//! store and the usage reader. It waits on UI commands and snapshot changes
//! (no timers while idle), builds presentation models off the UI thread and
//! sends them over a channel. The UI thread never blocks on the proxy, disk
//! or SQLite.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{
    StreamExt,
    channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded},
};
use gpui::{App, AppContext, BorrowAppContext, Entity};
use relay_core::{
    config::{AccountConfig, Config, PeriodKind},
    snapshot::{AccountState, AccountView, Capabilities, RoutingStatus, Snapshot, group_accounts},
};
use relay_proxy::{Handle, StartError, paths::Paths, setup::SetupEntry, store::ConfigStore};
use relay_usage::{Range as UsageRange, Reader};

use crate::{
    adapter,
    model::{Overview, Range, Through, UsageRow},
    store::{AccountEdit, Command, ConfigEdit, ConnectEvent, Op, Scope, Setup, Store},
};

/// Messages from the bridge thread to the UI.
pub enum Update {
    Overview(Box<Overview>),
    Usage(Vec<UsageRow>),
    Config(Box<Config>),
    /// Reply to a correlated connect request.
    Connect(ConnectEvent),
    Error(Scope, String),
    Paused(bool),
    /// The proxy drained and stopped; the app may exit.
    Stopped,
}

/// Through Relay and usage tables are re-read at most this often while
/// traffic flows (SQLite reads are cheap, but there is no need to chase).
const USAGE_REFRESH: Duration = Duration::from_secs(5);

/// Starts the bridge and returns the shared UI store.
pub fn start(cx: &mut App) -> Entity<Store> {
    let (cmd_tx, cmd_rx) = unbounded::<Command>();
    let (up_tx, mut up_rx) = unbounded::<Update>();
    let store = cx.new(|_| Store::new(Overview::starting(String::new(), now_ms()), Some(cmd_tx)));
    let spawned = std::thread::Builder::new()
        .name("relay-bridge".into())
        .spawn(move || run(cmd_rx, up_tx));
    if let Err(e) = spawned {
        store.update(cx, |s, _| {
            s.set_error(
                Scope::Routing,
                format!("Could not start Relay's runtime thread: {e}"),
            )
        });
    }
    let ui_store = store.clone();
    cx.spawn(async move |cx| {
        while let Some(update) = up_rx.next().await {
            let stop = matches!(update, Update::Stopped);
            cx.update(|cx| {
                if let Update::Config(c) = &update {
                    let pref = c.ui.theme;
                    let system = cx.window_appearance();
                    cx.update_global::<crate::widgets::Ui, _>(|ui, _| {
                        ui.preference = pref;
                        ui.theme = crate::widgets::Ui::resolve(pref, system);
                    });
                    cx.refresh_windows();
                }
                ui_store.update(cx, |s, cx| {
                    match update {
                        Update::Overview(o) => s.overview = *o,
                        Update::Usage(rows) => s.usage_rows = rows,
                        Update::Config(c) => s.config = *c,
                        Update::Connect(event) => s.push_connect_event(event),
                        Update::Error(scope, e) => s.set_error(scope, e),
                        Update::Paused(p) => s.paused = p,
                        Update::Stopped => {}
                    }
                    cx.notify();
                });
                if stop {
                    cx.quit();
                }
            });
            if stop {
                break;
            }
        }
    })
    .detach();
    store
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

struct Bridge {
    paths: Option<Paths>,
    config_store: Option<ConfigStore>,
    handle: Option<Handle>,
    reader: Option<Reader>,
    failure: Option<String>,
    through: Through,
    range: Range,
    last_usage: Option<Instant>,
    paused: bool,
    tx: UnboundedSender<Update>,
}

fn run(mut commands: UnboundedReceiver<Command>, tx: UnboundedSender<Update>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let _ = tx.unbounded_send(Update::Error(
                Scope::Routing,
                format!("Could not start Relay's runtime: {e}"),
            ));
            return;
        }
    };
    runtime.block_on(async move {
        let mut b = Bridge {
            paths: None,
            config_store: None,
            handle: None,
            reader: None,
            failure: None,
            through: Through::default(),
            range: Range::Week,
            last_usage: None,
            paused: false,
            tx,
        };
        b.boot();
        let mut updates = b.handle.as_ref().map(Handle::subscribe);
        loop {
            let changed = async {
                match updates.as_mut() {
                    Some(rx) => rx.changed().await.is_ok(),
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                command = commands.next() => {
                    let Some(command) = command else { break };
                    if command == Command::Quit {
                        if let Some(h) = b.handle.take() {
                            // Drains in-flight requests for up to 10 s, off the UI thread.
                            h.shutdown();
                        }
                        let _ = b.tx.unbounded_send(Update::Stopped);
                        break;
                    }
                    let restarted = b.command(command);
                    if restarted {
                        updates = b.handle.as_ref().map(Handle::subscribe);
                    }
                }
                ok = changed => {
                    if ok {
                        b.publish(false);
                    } else {
                        updates = None;
                    }
                }
            }
        }
    });
}

impl Bridge {
    fn boot(&mut self) {
        let paths = match Paths::for_user() {
            Ok(p) => p,
            Err(e) => return self.fail(format!("Cannot find Relay's data folder: {e}")),
        };
        let store = match ConfigStore::open(&paths) {
            Ok(s) => s,
            Err(e) => {
                self.paths = Some(paths);
                return self.fail(format!("Cannot read Relay's settings: {e}"));
            }
        };
        let _ = self
            .tx
            .unbounded_send(Update::Config(Box::new(store.config().clone())));
        self.config_store = Some(store);
        self.paths = Some(paths);
        self.start_proxy();
    }

    fn start_proxy(&mut self) {
        let (Some(paths), Some(store)) = (&self.paths, &self.config_store) else {
            return;
        };
        match relay_proxy::start(paths, store.config().clone()) {
            Ok(handle) => {
                self.reader = Reader::open(handle.usage_db()).ok();
                self.failure = None;
                self.handle = Some(handle);
            }
            Err(e) => {
                self.reader = Reader::open(&paths.usage_db).ok();
                self.handle = None;
                self.failure = Some(match e {
                    StartError::AddrInUse(addr) => format!("{addr} is already in use"),
                    other => other.to_string(),
                });
            }
        }
        self.publish(true);
    }

    /// Stops the current proxy (if any) and starts one from the saved config.
    fn restart(&mut self) -> bool {
        if let Some(h) = self.handle.take() {
            h.shutdown();
        }
        self.start_proxy();
        if let Some(h) = &self.handle {
            h.set_paused(self.paused);
        }
        true
    }

    fn fail(&mut self, reason: String) {
        self.failure = Some(reason);
        self.publish(true);
    }

    fn address(&self) -> String {
        self.config_store.as_ref().map_or_else(
            || Config::default().server.listen.to_string(),
            |s| s.config().server.listen.to_string(),
        )
    }

    /// Current snapshot, or one built from config when the proxy is down
    /// (accounts listed, quota unknown, routing failed).
    fn snapshot(&self) -> Arc<Snapshot> {
        if let Some(h) = &self.handle {
            return h.snapshot();
        }
        let now = now_ms();
        let accounts = self
            .config_store
            .as_ref()
            .map_or_else(Vec::new, |s| offline_accounts(&s.config().accounts));
        Arc::new(Snapshot {
            version: 0,
            generated_at: now,
            routing: RoutingStatus::Failed {
                reason: self
                    .failure
                    .clone()
                    .unwrap_or_else(|| "Relay is not running".into()),
            },
            providers: group_accounts(accounts, now),
            telemetry_dropped: 0,
            in_flight: 0,
        })
    }

    fn refresh_usage(&mut self, force: bool) {
        if !force && self.last_usage.is_some_and(|t| t.elapsed() < USAGE_REFRESH) {
            return;
        }
        self.last_usage = Some(Instant::now());
        let week_start = relay_usage::reader::week_start_from_pref(
            self.config_store
                .as_ref()
                .and_then(|s| s.config().ui.week_start),
        );
        let Some(reader) = self.reader.as_mut() else {
            return;
        };
        if let (Ok(week), Ok(month)) = (
            reader.through_relay(PeriodKind::Week, week_start),
            reader.through_relay(PeriodKind::Month, week_start),
        ) {
            self.through = adapter::through(&week, &month);
        }
        let range = match self.range {
            Range::Day => UsageRange::Day,
            Range::Week => UsageRange::Week,
            Range::Month => UsageRange::Month,
        };
        if let Ok(totals) = reader.account_totals(range) {
            let config = self
                .config_store
                .as_ref()
                .map(|s| s.config().clone())
                .unwrap_or_default();
            let _ = self
                .tx
                .unbounded_send(Update::Usage(usage_rows(&totals, &config)));
        }
    }

    fn publish(&mut self, force_usage: bool) {
        self.refresh_usage(force_usage);
        // Evaluate freshness and relative times at the real current time:
        // while idle the proxy publishes nothing, but "resets in" still moves.
        let mut snapshot = (*self.snapshot()).clone();
        snapshot.generated_at = now_ms();
        let overview = adapter::overview(&snapshot, self.through.clone(), &self.address());
        let _ = self.tx.unbounded_send(Update::Overview(Box::new(overview)));
    }

    /// Applies `edit` to the canonical config, persists it (validated,
    /// atomic) and hot-applies it. Returns a display-ready error on failure.
    fn save(
        &mut self,
        edit: impl FnOnce(&mut ConfigStore) -> Result<(), relay_proxy::store::StoreError>,
    ) -> Result<(), String> {
        let Some(store) = self.config_store.as_mut() else {
            return Err("Relay's settings are not available".into());
        };
        if let Err(e) = edit(store) {
            // The canonical config is unchanged; republish it so any
            // optimistic UI value rolls back to what is actually saved.
            let canonical = store.config().clone();
            let _ = self.tx.unbounded_send(Update::Config(Box::new(canonical)));
            return Err(e.to_string());
        }
        let config = store.config().clone();
        let mut result = Ok(());
        if let Some(h) = &self.handle {
            match h.apply_config(config.clone()) {
                Err(errs) => {
                    result = Err(errs
                        .iter()
                        .map(|e| e.0.clone())
                        .collect::<Vec<_>>()
                        .join("; "));
                }
                Ok(()) => {
                    // A new local address restarts the proxy (see Edit);
                    // anything else that needs a restart is said, not hidden.
                    if let Some(notice) = restart_notice(&h.restart_required()) {
                        self.error(Scope::Routing, notice);
                    }
                }
            }
        }
        let _ = self.tx.unbounded_send(Update::Config(Box::new(config)));
        self.publish(false);
        result
    }

    fn error(&self, scope: Scope, message: String) {
        let _ = self.tx.unbounded_send(Update::Error(scope, message));
    }

    fn connect(&self, event: ConnectEvent) {
        let _ = self.tx.unbounded_send(Update::Connect(event));
    }

    /// Saves an account, then (only on success) reports it and sends its
    /// setup values, all tagged with `op`. A failure replies `Failed`.
    fn add_account(&mut self, op: Op, account: AccountConfig) {
        let id = account.id.0.clone();
        match self.save(|s| s.add_account(account)) {
            Ok(()) => {
                self.connect(ConnectEvent::Saved { op, id: id.clone() });
                self.send_setup(op, &id);
            }
            Err(message) => self.connect(ConnectEvent::Failed { op, message }),
        }
    }

    /// Merges sheet-editable fields onto the canonical account. The
    /// provider is part of the account's identity and can't change here.
    fn update_account(&mut self, op: Op, id: &str, edit: &AccountEdit) {
        let Some(existing) = self
            .config_store
            .as_ref()
            .and_then(|s| s.config().account(id).cloned())
        else {
            return self.connect(ConnectEvent::Failed {
                op,
                message: format!("No account named {id}"),
            });
        };
        let merged = edit.merge_onto(&existing);
        match self.save(|s| s.replace_account(merged)) {
            Ok(()) => {
                self.connect(ConnectEvent::Saved {
                    op,
                    id: id.to_string(),
                });
                self.send_setup(op, id);
            }
            Err(message) => self.connect(ConnectEvent::Failed { op, message }),
        }
    }

    fn send_setup(&self, op: Op, id: &str) {
        match self.handle.as_ref().and_then(|h| h.client_setup(id)) {
            Some(cs) => self.connect(ConnectEvent::Setup {
                op,
                setup: Setup {
                    account_id: id.to_string(),
                    lines: setup_lines(&cs.entries),
                    notes: cs.notes,
                },
            }),
            None => self.connect(ConnectEvent::Failed {
                op,
                message: match &self.failure {
                    Some(f) => format!("Setup values need Relay running: {f}"),
                    None => format!("No account named {id}"),
                },
            }),
        }
    }

    /// Returns true when the proxy was (re)started.
    fn command(&mut self, command: Command) -> bool {
        match command {
            Command::SetPaused(p) => {
                self.paused = p;
                if let Some(h) = &self.handle {
                    h.set_paused(p);
                }
                let _ = self.tx.unbounded_send(Update::Paused(p));
            }
            Command::AddAccount { op, account } => self.add_account(op, *account),
            Command::UpdateAccount { op, id, edit } => self.update_account(op, &id, &edit),
            Command::Edit(edit) => {
                let before = self.config_store.as_ref().map(|s| s.config().server.listen);
                if let Err(e) = self.save(|s| s.update(|c| edit.apply(c))) {
                    self.error(Scope::Routing, e);
                    return false;
                }
                let after = self.config_store.as_ref().map(|s| s.config().server.listen);
                // The listen address is fixed for a handle's life, so a new
                // address restarts the proxy (draining in-flight requests).
                if matches!(edit, ConfigEdit::ListenPort(_)) && before != after {
                    return self.restart();
                }
            }
            Command::SetAccountPaused { id, paused } => {
                if let Err(e) = self.save(|s| s.set_paused(&id, paused)) {
                    self.error(Scope::Accounts, e);
                }
            }
            Command::ClientSetup { op, id } => self.send_setup(op, &id),
            Command::UsageRange(range) => {
                self.range = range;
                self.refresh_usage(true);
            }
            // Capabilities are Unsupported in this version; the buttons are
            // disabled with the reason, so nothing is sent upstream.
            Command::RefreshQuota(_) | Command::RefreshAll => {}
            Command::Republish => self.publish(false),
            Command::Retry => {
                if self.handle.is_none() {
                    self.start_proxy();
                    return self.handle.is_some();
                }
            }
            Command::Quit => {}
        }
        false
    }
}

/// "Applies after Relay restarts: …" for settings the running proxy can't
/// hot-apply, excluding the local address, which the bridge restarts for.
pub fn restart_notice(required: &[String]) -> Option<String> {
    let rest: Vec<&str> = required
        .iter()
        .map(String::as_str)
        .filter(|r| !r.starts_with("Local address"))
        .collect();
    (!rest.is_empty()).then(|| format!("Applies after Relay restarts: {}.", rest.join(", ")))
}

/// `NAME=value` lines for display and copying. Values carry the local-key
/// placeholder, never the key itself.
pub fn setup_lines(entries: &[SetupEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|e| {
            if e.name == "launchArgs" {
                e.value.clone()
            } else {
                format!("{}={}", e.name, e.value)
            }
        })
        .collect()
}

/// Config accounts as views with no quota, for the not-routing state.
pub fn offline_accounts(accounts: &[AccountConfig]) -> Vec<AccountView> {
    accounts
        .iter()
        .map(|a| AccountView {
            id: a.id.clone(),
            provider: a.provider,
            label: a.label.clone(),
            identifier: a.identifier.clone(),
            plan: a.plan.clone(),
            mode: a.mode,
            credential: a.credential_kind(),
            state: if a.paused {
                AccountState::Paused
            } else {
                AccountState::Unknown
            },
            state_detail: None,
            in_flight: 0,
            sessions: 0,
            last_seen: None,
            windows: Vec::new(),
            credits: None,
            cooldown_until: None,
            models_seen: Vec::new(),
            models_allowed: a.models.clone(),
            capabilities: Capabilities::for_account(a.provider, a.credential_kind()),
        })
        .collect()
}

pub fn usage_rows(totals: &[relay_usage::AccountTotals], config: &Config) -> Vec<UsageRow> {
    totals
        .iter()
        .map(|t| {
            let acc = config.accounts.iter().find(|a| a.id == t.account_id);
            let provider = t.provider.or(acc.map(|a| a.provider));
            let (group, mark) = match (provider, acc) {
                (Some(p), Some(a)) => {
                    let (title, mark) =
                        adapter::group_title(p, a.credential_kind(), a.plan.as_deref());
                    (
                        format!("{title} · {}", p.label()),
                        mark.or(Some(adapter::provider_mark(p))),
                    )
                }
                (Some(p), None) => (p.label().to_string(), Some(adapter::provider_mark(p))),
                _ => (String::new(), None),
            };
            UsageRow {
                label: acc.map_or_else(
                    || format!("{} (removed)", t.account_id),
                    |a| a.label.clone(),
                ),
                sublabel: group,
                mark,
                input: t.usage.input,
                output: t.usage.output,
                cached: t.usage.cache_read + t.usage.cache_write,
                requests: t.requests,
                errors: t.errors,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_core::{
        model::{AccountId, AccountMode, Provider},
        usage::TokenUsage,
    };

    fn cfg_account(
        id: &str,
        label: &str,
        provider: Provider,
        mode: AccountMode,
        paused: bool,
    ) -> AccountConfig {
        AccountConfig {
            id: AccountId(id.into()),
            provider,
            label: label.into(),
            mode,
            plan: Some("Max 20×".into()),
            identifier: Some("d••@x.io".into()),
            credential_kind: None,
            upstream: None,
            credential: None,
            paused,
            max_in_flight: 8,
            models: vec![],
        }
    }

    /// A bridge over a temp data dir with the proxy on an ephemeral
    /// loopback port. No user files, no fixed ports.
    fn test_bridge(dir: &std::path::Path) -> (Bridge, UnboundedReceiver<Update>) {
        let paths = Paths::in_dir(dir);
        let mut store = ConfigStore::open(&paths).unwrap();
        store
            .update(|c| c.server.listen = "127.0.0.1:0".parse().unwrap())
            .unwrap();
        let (tx, rx) = unbounded();
        let mut b = Bridge {
            paths: Some(paths),
            config_store: Some(store),
            handle: None,
            reader: None,
            failure: None,
            through: Through::default(),
            range: Range::Week,
            last_usage: None,
            paused: false,
            tx,
        };
        b.start_proxy();
        assert!(
            b.handle.is_some(),
            "proxy should start on an ephemeral port"
        );
        (b, rx)
    }

    fn on_disk(dir: &std::path::Path) -> Config {
        relay_proxy::store::load(&dir.join("config.toml")).unwrap()
    }

    fn drain(rx: &mut UnboundedReceiver<Update>) -> Vec<Update> {
        let mut out = Vec::new();
        while let Ok(u) = rx.try_recv() {
            out.push(u);
        }
        out
    }

    fn observed(id: &str, label: &str) -> AccountConfig {
        let mut a = cfg_account(id, label, Provider::Anthropic, AccountMode::Observed, false);
        a.plan = None;
        a
    }

    #[test]
    fn applying_a_new_port_restarts_the_proxy_there() {
        let dir = tempfile::tempdir().unwrap();
        let (mut b, _rx) = test_bridge(dir.path());
        b.command(Command::SetPaused(true));
        // A free port, released before Relay binds it.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert!(
            b.command(Command::Edit(ConfigEdit::ListenPort(port))),
            "port change restarts"
        );
        let h = b.handle.as_ref().expect("restarted");
        assert_eq!(h.addr().port(), port);
        assert!(h.is_paused(), "routing pause survives the restart");
        assert_eq!(on_disk(dir.path()).server.listen.port(), port);
        // The same port again changes nothing and does not restart.
        assert!(!b.command(Command::Edit(ConfigEdit::ListenPort(port))));
    }

    /// Regression (review P1): a settings edit made from a UI copy that
    /// predates an account add or pause must not drop or revert them.
    #[test]
    fn stale_ui_settings_edits_never_overwrite_accounts_or_pauses() {
        let dir = tempfile::tempdir().unwrap();
        let (mut b, mut rx) = test_bridge(dir.path());
        // The UI store still holds the config from before the add.
        let (ui_tx, mut ui_rx) = unbounded();
        let mut ui = Store::new(Overview::starting(String::new(), 0), Some(ui_tx));
        ui.config = b.config_store.as_ref().unwrap().config().clone();

        b.command(Command::AddAccount {
            op: 1,
            account: Box::new(observed("anthropic-work", "Work")),
        });
        b.command(Command::AddAccount {
            op: 2,
            account: Box::new(observed("anthropic-home", "Home")),
        });
        b.command(Command::SetAccountPaused {
            id: "anthropic-home".into(),
            paused: true,
        });
        // Rapid settings edits from the stale copy, delivered in order.
        let sticky = !ui.config.routing.sticky;
        ui.edit(ConfigEdit::Sticky(sticky));
        ui.edit(ConfigEdit::Strategy(
            relay_core::select::Strategy::RoundRobin,
        ));
        ui.edit(ConfigEdit::SkipBelowPct(10.0));
        while let Ok(cmd) = ui_rx.try_recv() {
            b.command(cmd);
        }

        let disk = on_disk(dir.path());
        let ids: Vec<&str> = disk.accounts.iter().map(|a| a.id.0.as_str()).collect();
        assert_eq!(ids, ["anthropic-work", "anthropic-home"]);
        assert!(disk.accounts[1].paused, "pause kept");
        assert_eq!(disk.routing.sticky, sticky);
        assert_eq!(
            disk.routing.strategy,
            relay_core::select::Strategy::RoundRobin
        );
        assert_eq!(disk.routing.skip_below_pct, 10.0);
        assert!(
            !drain(&mut rx)
                .iter()
                .any(|u| matches!(u, Update::Error(..)))
        );
    }

    fn saved(u: &Update, op: Op) -> bool {
        matches!(u, Update::Connect(ConnectEvent::Saved { op: o, .. }) if *o == op)
    }

    fn setup_for(u: &Update, op: Op) -> bool {
        matches!(u, Update::Connect(ConnectEvent::Setup { op: o, .. }) if *o == op)
    }

    fn failed(u: &Update, op: Op) -> bool {
        matches!(u, Update::Connect(ConnectEvent::Failed { op: o, .. }) if *o == op)
    }

    /// Regression (review r1 P2, r2 P2): Back→Continue and Reconnect edits
    /// merge onto the canonical account. Custom fields the sheet doesn't show
    /// survive, the provider can't change, and no second account appears.
    #[test]
    fn account_edits_merge_and_never_duplicate_or_drop_custom_fields() {
        let dir = tempfile::tempdir().unwrap();
        let (mut b, mut rx) = test_bridge(dir.path());
        let mut custom = observed("anthropic-dev", "Dev");
        custom.identifier = Some("d••@acme.co".into());
        custom.models = vec!["claude-opus-*".into()];
        custom.max_in_flight = 3;
        custom.upstream = Some(relay_core::config::ANTHROPIC_API.into());
        custom.plan = Some("Max 20×".into());
        b.command(Command::AddAccount {
            op: 7,
            account: Box::new(custom.clone()),
        });
        b.command(Command::SetAccountPaused {
            id: "anthropic-dev".into(),
            paused: true,
        });
        let first = drain(&mut rx);
        let s = first.iter().position(|u| saved(u, 7));
        let p = first.iter().position(|u| setup_for(u, 7));
        assert!(
            s.is_some() && p.is_some() && s < p,
            "saved, then setup, same op"
        );

        // Back (or Reconnect) → rename → Continue.
        let edit = AccountEdit {
            label: "Dev laptop".into(),
            identifier: None,
            mode: AccountMode::Observed,
            credential: None,
            credential_kind: None,
        };
        b.command(Command::UpdateAccount {
            op: 8,
            id: "anthropic-dev".into(),
            edit: Box::new(edit.clone()),
        });
        assert!(drain(&mut rx).iter().any(|u| saved(u, 8)));
        let disk = on_disk(dir.path());
        assert_eq!(disk.accounts.len(), 1, "no second account");
        let a = &disk.accounts[0];
        assert_eq!(a.label, "Dev laptop");
        assert_eq!(a.provider, Provider::Anthropic, "provider unchanged");
        assert_eq!(
            (
                &a.identifier,
                &a.models,
                a.max_in_flight,
                &a.upstream,
                &a.plan,
                a.paused
            ),
            (
                &custom.identifier,
                &custom.models,
                3,
                &custom.upstream,
                &custom.plan,
                true
            )
        );

        // Updating an account that doesn't exist fails for that op only.
        b.command(Command::UpdateAccount {
            op: 9,
            id: "nope".into(),
            edit: Box::new(edit),
        });
        let out = drain(&mut rx);
        assert!(out.iter().any(|u| failed(u, 9)));
        assert!(!out.iter().any(|u| saved(u, 9) || setup_for(u, 9)));

        // A failing add (duplicate id) replies Failed and sends no setup.
        b.command(Command::AddAccount {
            op: 10,
            account: Box::new(observed("anthropic-dev", "Dupe")),
        });
        let out = drain(&mut rx);
        assert!(out.iter().any(|u| failed(u, 10)));
        assert!(!out.iter().any(|u| saved(u, 10) || setup_for(u, 10)));
        assert_eq!(on_disk(dir.path()).accounts.len(), 1);
    }

    /// Regression (review r2 P3): an invalid edit is reported with the
    /// original reason, and the canonical (unchanged) config is republished
    /// so the UI shows what is actually saved.
    #[test]
    fn failed_edits_republish_the_saved_value_with_the_reason() {
        let dir = tempfile::tempdir().unwrap();
        let (mut b, mut rx) = test_bridge(dir.path());
        let saved_skip = on_disk(dir.path()).routing.skip_below_pct;
        b.command(Command::Edit(ConfigEdit::SkipBelowPct(150.0)));
        let out = drain(&mut rx);
        let rolled_back = out
            .iter()
            .any(|u| matches!(u, Update::Config(c) if c.routing.skip_below_pct == saved_skip));
        assert!(rolled_back, "canonical config republished");
        assert!(out.iter().any(
            |u| matches!(u, Update::Error(Scope::Routing, m) if m.contains("skip_below_pct"))
        ));
        assert_eq!(on_disk(dir.path()).routing.skip_below_pct, saved_skip);
    }

    #[test]
    fn restart_notice_skips_the_address_the_bridge_restarts_for() {
        assert_eq!(
            restart_notice(&["Local address (127.0.0.1:9000)".into()]),
            None
        );
        assert_eq!(
            restart_notice(&[
                "Local address (127.0.0.1:9000)".into(),
                "Upstream connect timeout".into()
            ]),
            Some("Applies after Relay restarts: Upstream connect timeout.".into())
        );
        assert_eq!(restart_notice(&[]), None);
    }

    #[test]
    fn setup_lines_never_contain_the_key() {
        let lines = setup_lines(&[
            SetupEntry {
                name: "ANTHROPIC_BASE_URL".into(),
                value: "http://127.0.0.1:8787".into(),
                sensitive: false,
            },
            SetupEntry {
                name: "RELAY_KEY".into(),
                value: relay_proxy::setup::KEY_PLACEHOLDER.into(),
                sensitive: true,
            },
            SetupEntry {
                name: "launchArgs".into(),
                value: "-c model_provider=\"relay\"".into(),
                sensitive: false,
            },
        ]);
        assert_eq!(lines[0], "ANTHROPIC_BASE_URL=http://127.0.0.1:8787");
        assert_eq!(
            lines[1],
            format!("RELAY_KEY={}", relay_proxy::setup::KEY_PLACEHOLDER)
        );
        assert_eq!(lines[2], "-c model_provider=\"relay\"");
    }

    #[test]
    fn offline_accounts_have_unknown_quota_not_zero() {
        let views = offline_accounts(&[
            cfg_account(
                "a",
                "Personal",
                Provider::Anthropic,
                AccountMode::Observed,
                false,
            ),
            cfg_account("b", "Key B", Provider::Anthropic, AccountMode::Key, true),
        ]);
        assert_eq!(views[0].state, AccountState::Unknown);
        assert!(views[0].windows.is_empty());
        assert_eq!(views[1].state, AccountState::Paused);
        let o = adapter::overview(
            &Snapshot {
                version: 0,
                generated_at: 0,
                routing: RoutingStatus::Failed {
                    reason: "127.0.0.1:8787 is already in use".into(),
                },
                providers: group_accounts(views, 0),
                telemetry_dropped: 0,
                in_flight: 0,
            },
            Through::default(),
            "127.0.0.1:8787",
        );
        assert_eq!(o.account_count(), 2);
        assert!(
            matches!(o.status, crate::model::Status::NotRouting { ref title, .. } if title == "Port 8787 is in use")
        );
    }

    #[test]
    fn usage_rows_use_config_labels_and_sum_cache_buckets() {
        let config = Config {
            accounts: vec![cfg_account(
                "claude-work",
                "Work",
                Provider::Anthropic,
                AccountMode::Pooled,
                false,
            )],
            ..Default::default()
        };
        let totals = [relay_usage::AccountTotals {
            account_id: AccountId("claude-work".into()),
            provider: Some(Provider::Anthropic),
            model: None,
            usage: TokenUsage {
                input: 10,
                cache_read: 5,
                cache_write: 2,
                output: 7,
                reasoning: 1,
            },
            requests: 3,
            errors: 1,
            retries: 0,
            ambiguous: 0,
            mean_ttft_ms: None,
        }];
        let rows = usage_rows(&totals, &config);
        assert_eq!(rows[0].label, "Work");
        assert_eq!(rows[0].sublabel, "Claude Max · Anthropic");
        assert_eq!((rows[0].input, rows[0].cached, rows[0].output), (10, 7, 7));
    }
}
