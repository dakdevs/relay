//! Runtime state shared by request tasks and the snapshot publisher.
//!
//! The request path touches atomics, one short per-account mutex for quota
//! merges and one sharded session map. Nothing here does I/O except
//! credential reloads, which run on the blocking pool.

use crate::secrets::{self, Unavailable};
use arc_swap::{ArcSwap, ArcSwapOption};
use relay_core::UnixMs;
use relay_core::config::{AccountConfig, Config};
use relay_core::model::{AccountId, Pool, Provider};
use relay_core::quota::{QuotaState, QuotaUpdate};
use relay_core::select::Candidate;
use relay_core::snapshot::{
    AccountView, Capabilities, RoutingStatus, Snapshot, StateInputs, account_state, group_accounts,
};
use relay_providers::auth::Credential;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{Notify, Semaphore};

/// Current Unix time in ms.
pub fn now_ms() -> UnixMs {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// One account at runtime.
pub struct AccountRt {
    /// Config.
    pub cfg: AccountConfig,
    /// Admission slots.
    pub sem: Arc<Semaphore>,
    /// Requests in flight.
    pub in_flight: AtomicU32,
    /// Quota knowledge.
    pub quota: Mutex<QuotaState>,
    /// Cooling down until (Unix ms, 0 = not).
    pub cooldown_until: AtomicI64,
    /// Consecutive rejections without `retry-after`, for backoff.
    pub backoff_level: AtomicU32,
    /// Last upstream response (Unix ms, 0 = never).
    pub last_seen: AtomicI64,
    /// Resolved credential (router-owned accounts).
    pub credential: ArcSwapOption<Credential>,
    /// Why the credential is unavailable.
    pub credential_error: Mutex<Option<String>>,
    /// Modification time of a file credential when last read.
    pub credential_mtime: Mutex<Option<SystemTime>>,
    /// Single-flight guard for credential reloads.
    pub reloading: tokio::sync::Mutex<()>,
    /// Earliest time the credential source may be read or stat'ed again.
    pub credential_next_check: AtomicI64,
    /// Fingerprint of a token the provider rejected (401), so re-reading the
    /// same token doesn't resurrect it.
    pub rejected_token: Mutex<Option<u64>>,
    /// Models seen, most recent first.
    pub models_seen: Mutex<VecDeque<String>>,
}

const MODELS_SEEN: usize = 8;

/// Retry interval for a missing, expired or rejected credential.
pub const RETRY_BROKEN_MS: i64 = 2_000;
/// Change-check interval for a healthy file credential.
pub const CHECK_HEALTHY_MS: i64 = 30_000;

const REJECTED: &str = "The provider rejected this credential (401). Sign in again with the CLI for this account; Relay picks up the new login automatically.";

/// Non-cryptographic fingerprint used only to recognise a token Relay has
/// already seen rejected; never stored or shown.
fn fingerprint(token: &str) -> u64 {
    token.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

impl AccountRt {
    fn new(cfg: AccountConfig) -> AccountRt {
        AccountRt {
            sem: Arc::new(Semaphore::new(cfg.max_in_flight as usize)),
            cfg,
            in_flight: AtomicU32::new(0),
            quota: Mutex::new(QuotaState::default()),
            cooldown_until: AtomicI64::new(0),
            backoff_level: AtomicU32::new(0),
            last_seen: AtomicI64::new(0),
            credential: ArcSwapOption::empty(),
            credential_error: Mutex::new(None),
            credential_mtime: Mutex::new(None),
            reloading: tokio::sync::Mutex::new(()),
            credential_next_check: AtomicI64::new(0),
            rejected_token: Mutex::new(None),
            models_seen: Mutex::new(VecDeque::new()),
        }
    }

    /// Carries runtime knowledge into a new config for the same account.
    fn carry(prev: &AccountRt, cfg: AccountConfig) -> AccountRt {
        let same_credential =
            prev.cfg.credential == cfg.credential && prev.cfg.provider == cfg.provider;
        let mut next = AccountRt::new(cfg);
        if prev.cfg.max_in_flight == next.cfg.max_in_flight {
            // Keep the semaphore so in-flight permits stay counted.
            next.sem = prev.sem.clone();
        }
        AccountRt::carry_state(prev, next, same_credential)
    }

    fn carry_state(prev: &AccountRt, next: AccountRt, same_credential: bool) -> AccountRt {
        *lock(&next.quota) = lock(&prev.quota).clone();
        next.cooldown_until.store(
            prev.cooldown_until.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        next.last_seen
            .store(prev.last_seen.load(Ordering::Relaxed), Ordering::Relaxed);
        *lock(&next.models_seen) = lock(&prev.models_seen).clone();
        if same_credential {
            next.credential.store(prev.credential.load_full());
            *lock(&next.credential_error) = lock(&prev.credential_error).clone();
            *lock(&next.credential_mtime) = *lock(&prev.credential_mtime);
            *lock(&next.rejected_token) = *lock(&prev.rejected_token);
        }
        next
    }

    /// Account id.
    pub fn id(&self) -> &AccountId {
        &self.cfg.id
    }

    /// Whether the credential is loaded and not expired.
    pub fn credential_ok(&self, now: UnixMs) -> bool {
        if !self.cfg.mode.router_owned() {
            return true;
        }
        match &*self.credential.load() {
            Some(c) => c.expires_at.is_none_or(|e| e > now),
            None => false,
        }
    }

    /// Selector view of this account for a model. With `need_slot`,
    /// saturated accounts are ineligible.
    pub fn candidate(&self, model: Option<&str>, now: UnixMs, need_slot: bool) -> Candidate {
        let q = lock(&self.quota);
        Candidate {
            eligible: self.usable(model, now)
                && (!need_slot || self.sem.available_permits() > 0)
                && !q.exhausted(now),
            headroom: q.headroom(now),
            resets_at: q.binding_reset(now),
        }
    }

    /// Enabled, supports the model, not cooling down, has a free slot, has
    /// a credential.
    pub fn eligible(&self, model: Option<&str>, now: UnixMs) -> bool {
        self.usable(model, now) && self.sem.available_permits() > 0
    }

    /// [`AccountRt::eligible`] without the free-slot condition.
    pub fn usable(&self, model: Option<&str>, now: UnixMs) -> bool {
        !self.cfg.paused
            && model.is_none_or(|m| self.cfg.supports_model(m))
            && self.cooldown_until.load(Ordering::Relaxed) <= now
            && self.credential_ok(now)
    }

    /// Credential usable and not flagged (router-owned), or observed.
    pub fn credential_healthy(&self, now: UnixMs) -> bool {
        self.credential_ok(now)
            && (!self.cfg.mode.router_owned() || lock(&self.credential_error).is_none())
    }

    /// Merges a quota update. Returns true when anything changed.
    pub fn observe(&self, update: &QuotaUpdate, now: UnixMs) -> bool {
        self.last_seen.store(now, Ordering::Relaxed);
        if update.windows.is_empty() && update.plan.is_none() && update.credits.is_none() {
            return false;
        }
        lock(&self.quota).merge(update)
    }

    /// Starts a cooldown after a rejection. Call once per attempt. A
    /// provider-given wait is authoritative (it replaces any backoff and
    /// resets the level); without one, backoff grows 1 s, 2 s, 4 s…
    pub fn cool_down(&self, retry_after_ms: Option<i64>, now: UnixMs) {
        match retry_after_ms.filter(|ms| *ms > 0) {
            Some(ms) => {
                self.backoff_level.store(0, Ordering::Relaxed);
                let until = now + relay_core::attempt::cooldown_ms(Some(ms), 0);
                self.cooldown_until.store(until, Ordering::Relaxed);
            }
            None => {
                let level = self.backoff_level.fetch_add(1, Ordering::Relaxed);
                let until = now + relay_core::attempt::cooldown_ms(None, level);
                self.cooldown_until.fetch_max(until, Ordering::Relaxed);
            }
        }
    }

    /// Clears backoff after a success.
    pub fn succeeded(&self) {
        self.backoff_level.store(0, Ordering::Relaxed);
    }

    /// Records a model name (bounded).
    pub fn saw_model(&self, model: &str) {
        let mut m = lock(&self.models_seen);
        if m.front().is_some_and(|f| f == model) {
            return;
        }
        m.retain(|x| x != model);
        m.push_front(model.to_string());
        m.truncate(MODELS_SEEN);
    }

    /// Re-reads the credential (blocking). Returns true when a usable
    /// credential is now loaded.
    pub fn load_credential(&self) -> bool {
        let Some(r) = &self.cfg.credential else {
            return false;
        };
        let mtime = secrets::modified(r);
        *lock(&self.credential_mtime) = mtime;
        match secrets::resolve(r) {
            Ok(c) => {
                if *lock(&self.rejected_token) == Some(fingerprint(c.token.expose())) {
                    self.credential.store(None);
                    *lock(&self.credential_error) = Some(REJECTED.into());
                    return false;
                }
                *lock(&self.rejected_token) = None;
                let expired = c.expires_at.is_some_and(|e| e <= now_ms());
                self.credential.store(Some(Arc::new(c)));
                *lock(&self.credential_error) = expired.then(|| {
                    "Login expired. Use the CLI for this account once so it refreshes its own login.".to_string()
                });
                !expired
            }
            Err(Unavailable(msg)) => {
                self.credential.store(None);
                *lock(&self.credential_error) = Some(msg);
                false
            }
        }
    }

    /// Whether a file credential changed on disk since it was read.
    pub fn credential_file_changed(&self) -> bool {
        let Some(r) = &self.cfg.credential else {
            return false;
        };
        secrets::modified(r).is_some_and(|m| Some(m) != *lock(&self.credential_mtime))
    }

    /// Marks the current credential rejected by the provider (401). It
    /// stays unusable until its source holds a different token.
    pub fn credential_rejected(&self) {
        if let Some(c) = &*self.credential.load() {
            *lock(&self.rejected_token) = Some(fingerprint(c.token.expose()));
        }
        *lock(&self.credential_error) = Some(REJECTED.into());
        self.credential.store(None);
    }

    /// Whether the credential source should be re-read now, without I/O
    /// except a rate-limited `stat` of file credentials. Broken credentials
    /// (missing, expired, rejected) are retried every [`RETRY_BROKEN_MS`]
    /// or as soon as their file changes; healthy file credentials are
    /// checked for changes every [`CHECK_HEALTHY_MS`]. Other sources have no
    /// change signal, so they are only retried while broken.
    pub fn needs_refresh(&self, now: UnixMs) -> bool {
        let Some(r) = &self.cfg.credential else {
            return false;
        };
        if !self.cfg.mode.router_owned() || self.cfg.paused {
            return false;
        }
        let broken = !self.credential_ok(now) || lock(&self.credential_error).is_some();
        let due = now >= self.credential_next_check.load(Ordering::Relaxed);
        let is_file = matches!(r, relay_core::config::SecretRef::File { .. });
        if broken {
            due || (is_file && self.credential_file_changed())
        } else {
            is_file && due
        }
    }

    /// Re-reads the credential source if [`AccountRt::needs_refresh`] says
    /// so (blocking: file, keychain). Returns true when a usable credential
    /// is loaded afterwards.
    pub fn refresh_credential_blocking(&self) -> bool {
        let now = now_ms();
        if self.needs_refresh(now) {
            let broken = !self.credential_ok(now) || lock(&self.credential_error).is_some();
            if broken || self.credential_file_changed() {
                self.load_credential();
            }
            let still_broken =
                !self.credential_ok(now_ms()) || lock(&self.credential_error).is_some();
            let wait = if still_broken {
                RETRY_BROKEN_MS
            } else {
                CHECK_HEALTHY_MS
            };
            self.credential_next_check
                .store(now_ms() + wait, Ordering::Relaxed);
        }
        self.credential_ok(now_ms()) && lock(&self.credential_error).is_none()
    }

    /// Credential expiry in the future, for snapshot scheduling.
    pub fn credential_expires_at(&self, now: UnixMs) -> Option<UnixMs> {
        self.credential
            .load()
            .as_ref()
            .and_then(|c| c.expires_at)
            .filter(|e| *e > now)
    }

    fn view(&self, sessions: u32, now: UnixMs) -> AccountView {
        let q = lock(&self.quota).clone();
        let cooldown = self.cooldown_until.load(Ordering::Relaxed);
        let in_flight = self.in_flight.load(Ordering::Relaxed);
        let error = lock(&self.credential_error).clone();
        let disconnected =
            self.cfg.mode.router_owned() && (!self.credential_ok(now) || error.is_some());
        let state = account_state(
            &q.windows,
            StateInputs {
                paused: self.cfg.paused,
                disconnected,
                in_flight,
                cooling_down: cooldown > now,
            },
            now,
        );
        let last = self.last_seen.load(Ordering::Relaxed);
        AccountView {
            id: self.cfg.id.clone(),
            provider: self.cfg.provider,
            label: self.cfg.label.clone(),
            identifier: self.cfg.identifier.clone(),
            plan: q.plan.clone().or_else(|| self.cfg.plan.clone()),
            mode: self.cfg.mode,
            credential: self.cfg.credential_kind(),
            state,
            state_detail: if disconnected {
                Some(error.unwrap_or_else(|| "Credential not loaded.".into()))
            } else {
                None
            },
            in_flight,
            sessions,
            last_seen: (last > 0).then_some(last),
            windows: q.windows,
            credits: q.credits,
            cooldown_until: (cooldown > now).then_some(cooldown),
            models_seen: lock(&self.models_seen).iter().cloned().collect(),
            models_allowed: self.cfg.models.clone(),
            capabilities: Capabilities::for_account(self.cfg.provider, self.cfg.credential_kind()),
        }
    }
}

/// Accounts and config, swapped atomically on reload.
pub struct Registry {
    /// Config in effect.
    pub config: Arc<Config>,
    /// Accounts in config order.
    pub accounts: Vec<Arc<AccountRt>>,
}

impl Registry {
    /// Builds from config, reusing runtime state of `prev` by account id.
    pub fn build(config: Config, prev: Option<&Registry>) -> Registry {
        let accounts = config
            .accounts
            .iter()
            .map(|a| {
                let old = prev.and_then(|p| p.account(a.id.as_str()));
                Arc::new(match old {
                    Some(o) => AccountRt::carry(o, a.clone()),
                    None => AccountRt::new(a.clone()),
                })
            })
            .collect();
        Registry {
            config: Arc::new(config),
            accounts,
        }
    }

    /// Account by id.
    pub fn account(&self, id: &str) -> Option<&Arc<AccountRt>> {
        self.accounts.iter().find(|a| a.cfg.id.as_str() == id)
    }

    /// Router-owned accounts of a provider in a pool.
    pub fn pool(&self, provider: Provider, pool: Pool) -> impl Iterator<Item = &Arc<AccountRt>> {
        self.accounts
            .iter()
            .filter(move |a| a.cfg.provider == provider && a.cfg.mode == pool.mode())
    }
}

/// Session → account binding.
#[derive(Debug, Clone)]
struct Binding {
    account: AccountId,
    last: Instant,
}

const SHARDS: usize = 16;
/// Upper bound on remembered sessions.
pub const MAX_SESSIONS: usize = 50_000;

/// Sharded session map with TTL.
pub struct Sessions {
    shards: [Mutex<HashMap<(Provider, String), Binding>>; SHARDS],
    len: AtomicUsize,
}

impl Default for Sessions {
    fn default() -> Self {
        Sessions {
            shards: std::array::from_fn(|_| Mutex::new(HashMap::new())),
            len: AtomicUsize::new(0),
        }
    }
}

impl Sessions {
    fn shard(&self, id: &str) -> &Mutex<HashMap<(Provider, String), Binding>> {
        // FNV-1a; cheap and stable.
        let h = id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        });
        &self.shards[(h as usize) % SHARDS]
    }

    /// Bound account, refreshing its last-use time, if within `ttl`.
    pub fn get(&self, provider: Provider, id: &str, ttl: Duration) -> Option<AccountId> {
        let mut m = lock(self.shard(id));
        let key = (provider, id.to_string());
        match m.get_mut(&key) {
            Some(b) if b.last.elapsed() <= ttl => {
                b.last = Instant::now();
                Some(b.account.clone())
            }
            Some(_) => {
                m.remove(&key);
                self.len.fetch_sub(1, Ordering::Relaxed);
                None
            }
            None => None,
        }
    }

    /// Binds a session. Ignored when the map is full of live sessions.
    pub fn bind(&self, provider: Provider, id: &str, account: &AccountId) {
        let mut m = lock(self.shard(id));
        let key = (provider, id.to_string());
        if let Some(b) = m.get_mut(&key) {
            b.account = account.clone();
            b.last = Instant::now();
            return;
        }
        if self.len.load(Ordering::Relaxed) >= MAX_SESSIONS {
            return;
        }
        m.insert(
            key,
            Binding {
                account: account.clone(),
                last: Instant::now(),
            },
        );
        self.len.fetch_add(1, Ordering::Relaxed);
    }

    /// Drops expired bindings and counts live ones per account.
    pub fn sweep(&self, ttl: Duration) -> HashMap<AccountId, u32> {
        let mut counts = HashMap::new();
        for s in &self.shards {
            let mut m = lock(s);
            let before = m.len();
            m.retain(|_, b| b.last.elapsed() <= ttl);
            self.len.fetch_sub(before - m.len(), Ordering::Relaxed);
            for b in m.values() {
                *counts.entry(b.account.clone()).or_insert(0) += 1;
            }
        }
        counts
    }
}

/// Everything request tasks share.
pub struct Shared {
    /// Accounts and config.
    pub registry: ArcSwap<Registry>,
    /// Session bindings.
    pub sessions: Sessions,
    /// Router-owned routing paused.
    pub paused: AtomicBool,
    /// Something changed since the last snapshot.
    pub dirty: AtomicBool,
    /// Wakes the publisher.
    pub notify: Notify,
    /// Round-robin cursor.
    pub cursor: AtomicUsize,
    /// Usage sink.
    pub usage: relay_usage::Sink,
    /// Local client key.
    pub local_key: relay_providers::auth::Secret,
    /// Snapshot counter.
    pub version: AtomicU64,
    /// Bound address.
    pub addr: std::net::SocketAddr,
}

impl Shared {
    /// Flags a change and wakes the publisher (which coalesces).
    pub fn changed(&self) {
        if !self.dirty.swap(true, Ordering::AcqRel) {
            self.notify.notify_one();
        }
    }

    /// Builds a snapshot. Returns it with the earliest time-driven change.
    pub fn build_snapshot(&self, now: UnixMs) -> (Snapshot, Option<UnixMs>) {
        let reg = self.registry.load();
        let ttl = Duration::from_secs(u64::from(reg.config.routing.session_ttl_minutes) * 60);
        let counts = self.sessions.sweep(ttl);
        let mut next: Option<UnixMs> = None;
        let mut in_flight = 0;
        let views: Vec<AccountView> = reg
            .accounts
            .iter()
            .map(|a| {
                let q = lock(&a.quota).next_transition(now);
                let cd = a.cooldown_until.load(Ordering::Relaxed);
                let exp = a.credential_expires_at(now);
                for t in [q, (cd > now).then_some(cd), exp].into_iter().flatten() {
                    next = Some(next.map_or(t, |n| n.min(t)));
                }
                in_flight += a.in_flight.load(Ordering::Relaxed);
                a.view(counts.get(a.id()).copied().unwrap_or(0), now)
            })
            .collect();
        let routing = if self.paused.load(Ordering::Relaxed) {
            RoutingStatus::Paused { addr: self.addr }
        } else {
            RoutingStatus::Running { addr: self.addr }
        };
        (
            Snapshot {
                version: 0,
                generated_at: now,
                routing,
                providers: group_accounts(views, now),
                telemetry_dropped: self.usage.dropped(),
                in_flight,
            },
            next,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_core::model::AccountMode;

    fn cfg(id: &str, max: u32) -> AccountConfig {
        AccountConfig {
            id: AccountId(id.into()),
            provider: Provider::Anthropic,
            label: id.into(),
            mode: AccountMode::Observed,
            plan: None,
            identifier: None,
            credential_kind: None,
            upstream: None,
            credential: None,
            paused: false,
            max_in_flight: max,
            models: vec![],
        }
    }

    #[test]
    fn sessions_bind_expire_and_count() {
        let s = Sessions::default();
        let a = AccountId("a".into());
        s.bind(Provider::Anthropic, "s1", &a);
        s.bind(Provider::Anthropic, "s2", &a);
        assert_eq!(
            s.get(Provider::Anthropic, "s1", Duration::from_secs(60)),
            Some(a.clone())
        );
        assert_eq!(s.get(Provider::Openai, "s1", Duration::from_secs(60)), None);
        assert_eq!(s.sweep(Duration::from_secs(60)).get(&a), Some(&2));
        assert_eq!(s.sweep(Duration::ZERO).get(&a), None);
        assert_eq!(s.len.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn reload_keeps_quota_and_cooldown() {
        let mut c = Config {
            accounts: vec![cfg("a", 2)],
            ..Default::default()
        };
        let r1 = Registry::build(c.clone(), None);
        r1.accounts[0].cooldown_until.store(99, Ordering::Relaxed);
        r1.accounts[0].saw_model("m");
        c.accounts[0].label = "Renamed".into();
        let r2 = Registry::build(c, Some(&r1));
        assert_eq!(r2.accounts[0].cooldown_until.load(Ordering::Relaxed), 99);
        assert_eq!(r2.accounts[0].cfg.label, "Renamed");
        assert!(Arc::ptr_eq(&r1.accounts[0].sem, &r2.accounts[0].sem));
        assert_eq!(
            lock(&r2.accounts[0].models_seen)
                .front()
                .map(String::as_str),
            Some("m")
        );
    }

    #[test]
    fn cooldown_backoff_and_retry_after() {
        let a = AccountRt::new(cfg("a", 1));
        a.cool_down(None, 0);
        assert_eq!(a.cooldown_until.load(Ordering::Relaxed), 1_000);
        a.cool_down(None, 0);
        assert_eq!(a.cooldown_until.load(Ordering::Relaxed), 2_000);
        a.cool_down(Some(30_000), 0);
        assert_eq!(a.cooldown_until.load(Ordering::Relaxed), 30_000);
        assert!(!a.eligible(None, 10_000));
        assert!(a.eligible(None, 30_000));
        // A provider-given wait is authoritative even when shorter, and
        // resets the backoff level.
        a.cool_down(None, 40_000);
        a.cool_down(Some(1_000), 40_000);
        assert_eq!(a.cooldown_until.load(Ordering::Relaxed), 41_000);
        a.cool_down(None, 50_000);
        assert_eq!(a.cooldown_until.load(Ordering::Relaxed), 51_000);
    }
}
