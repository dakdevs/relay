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
    store::{Command, Setup, Store},
};

/// Messages from the bridge thread to the UI.
pub enum Update {
    Overview(Box<Overview>),
    Usage(Vec<UsageRow>),
    Config(Box<Config>),
    Setup(Setup),
    Error(String),
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
            s.error = Some(format!("Could not start Relay's runtime thread: {e}"))
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
                        Update::Setup(setup) => s.setup = Some(setup),
                        Update::Error(e) => s.error = Some(e),
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
            let _ = tx.unbounded_send(Update::Error(format!(
                "Could not start Relay's runtime: {e}"
            )));
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

    fn save(
        &mut self,
        edit: impl FnOnce(&mut ConfigStore) -> Result<(), relay_proxy::store::StoreError>,
    ) {
        let Some(store) = self.config_store.as_mut() else {
            return self.error("Relay's settings are not available".into());
        };
        if let Err(e) = edit(store) {
            return self.error(e.to_string());
        }
        let config = store.config().clone();
        if let Some(h) = &self.handle {
            match h.apply_config(config.clone()) {
                Err(errs) => self.error(
                    errs.iter()
                        .map(|e| e.0.clone())
                        .collect::<Vec<_>>()
                        .join("; "),
                ),
                Ok(()) => {
                    // A new local address restarts the proxy (see SaveConfig);
                    // anything else that needs a restart is said, not hidden.
                    let pending = restart_notice(&h.restart_required());
                    if let Some(notice) = pending {
                        self.error(notice);
                    }
                }
            }
        }
        let _ = self.tx.unbounded_send(Update::Config(Box::new(config)));
        self.publish(false);
    }

    fn error(&self, message: String) {
        let _ = self.tx.unbounded_send(Update::Error(message));
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
            Command::AddAccount(account) => self.save(|s| s.add_account(*account)),
            Command::SaveConfig(config) => {
                let before = self.config_store.as_ref().map(|s| s.config().server.listen);
                let after = config.server.listen;
                self.save(|s| s.update(|c| *c = *config));
                let saved = self.config_store.as_ref().map(|s| s.config().server.listen);
                // The listen address is fixed for a handle's life, so a new
                // address restarts the proxy (draining in-flight requests).
                if before != Some(after) && saved == Some(after) {
                    return self.restart();
                }
            }
            Command::SetAccountPaused { id, paused } => self.save(|s| s.set_paused(&id, paused)),
            Command::ClientSetup(id) => {
                match self.handle.as_ref().and_then(|h| h.client_setup(&id)) {
                    Some(cs) => {
                        let _ = self.tx.unbounded_send(Update::Setup(Setup {
                            account_id: id,
                            lines: setup_lines(&cs.entries),
                            notes: cs.notes,
                        }));
                    }
                    None => self.error(match &self.failure {
                        Some(f) => format!("Setup values need Relay running: {f}"),
                        None => format!("No account named {id}"),
                    }),
                }
            }
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

    /// Loopback-only: a temp data dir and ephemeral ports, no user files.
    #[test]
    fn applying_a_new_port_restarts_the_proxy_there() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(dir.path());
        let mut store = ConfigStore::open(&paths).unwrap();
        store
            .update(|c| c.server.listen = "127.0.0.1:0".parse().unwrap())
            .unwrap();
        let (tx, mut rx) = unbounded();
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
        b.command(Command::SetPaused(true));

        // A free port, released before Relay binds it.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut next = b.config_store.as_ref().unwrap().config().clone();
        next.server.listen.set_port(port);
        assert!(
            b.command(Command::SaveConfig(Box::new(next))),
            "port change restarts"
        );
        let h = b.handle.as_ref().expect("restarted");
        assert_eq!(h.addr().port(), port);
        assert!(h.is_paused(), "routing pause survives the restart");
        // The new address is persisted.
        let saved = relay_proxy::store::load(&dir.path().join("config.toml")).unwrap();
        assert_eq!(saved.server.listen.port(), port);
        // Saving without an address change does not restart.
        let same = b.config_store.as_ref().unwrap().config().clone();
        assert!(!b.command(Command::SaveConfig(Box::new(same))));
        drop(b);
        while rx.try_recv().is_ok() {}
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
