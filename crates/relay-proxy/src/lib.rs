//! Relay's loopback proxy.
//!
//! [`start`] binds `127.0.0.1`, spins up a dedicated multi-thread Tokio
//! runtime and returns a [`Handle`]. The UI reads snapshots from the handle
//! and is never on the request path.
#![warn(missing_docs)]

pub mod body;
pub mod forward;
pub mod paths;
pub mod secrets;
pub mod setup;
pub mod state;
pub mod store;

use arc_swap::ArcSwap;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use hyper_util::server::graceful::GracefulShutdown;
use paths::Paths;
use relay_core::config::{Config, ConfigError};
use relay_core::snapshot::Snapshot;
use state::{Registry, Shared, now_ms};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{oneshot, watch};

/// Minimum time between published snapshots.
pub const PUBLISH_INTERVAL: Duration = Duration::from_millis(500);
/// How long in-flight requests may drain at shutdown.
pub const DRAIN: Duration = Duration::from_secs(10);

/// Why the proxy did not start.
#[derive(Debug)]
pub enum StartError {
    /// Something else listens on the configured address.
    AddrInUse(SocketAddr),
    /// Other bind failure.
    Bind(std::io::Error),
    /// Invalid configuration.
    Config(Vec<ConfigError>),
    /// The usage store could not open.
    Usage(String),
    /// Runtime or local-key setup failed.
    Runtime(std::io::Error),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::AddrInUse(a) => write!(f, "{a} is already in use"),
            StartError::Bind(e) => write!(f, "cannot listen: {e}"),
            StartError::Config(errs) => {
                let m: Vec<&str> = errs.iter().map(|e| e.0.as_str()).collect();
                write!(f, "invalid config: {}", m.join("; "))
            }
            StartError::Usage(e) => write!(f, "usage store: {e}"),
            StartError::Runtime(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StartError {}

/// A running proxy.
pub struct Handle {
    shared: Arc<Shared>,
    snapshot: Arc<ArcSwap<Snapshot>>,
    updates: watch::Receiver<Arc<Snapshot>>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    recorder: Option<relay_usage::Recorder>,
    usage_db: std::path::PathBuf,
    started: Config,
}

impl Handle {
    /// Bound address.
    pub fn addr(&self) -> SocketAddr {
        self.shared.addr
    }

    /// Latest snapshot. Lock-free.
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.load_full()
    }

    /// Change notifications. `watch` works with any executor.
    pub fn subscribe(&self) -> watch::Receiver<Arc<Snapshot>> {
        self.updates.clone()
    }

    /// Pauses or resumes router-owned routing. Observed traffic keeps
    /// passing so running CLI sessions don't break.
    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
        self.shared.changed();
    }

    /// Whether routing is paused.
    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::Relaxed)
    }

    /// Applies a new config without restarting: accounts, routing,
    /// upstream allow-list, body limit and queue wait take effect at once.
    /// Settings fixed at start (see [`Handle::restart_required`]) apply on
    /// the next start. Credentials of new or changed accounts load in the
    /// background.
    pub fn apply_config(&self, config: Config) -> Result<(), Vec<ConfigError>> {
        config.validate()?;
        let prev = self.shared.registry.load_full();
        let next = Arc::new(Registry::build(config, Some(&prev)));
        load_credentials(&self.shared, &next);
        self.shared.registry.store(next);
        self.shared.changed();
        Ok(())
    }

    /// Settings in the applied config that differ from the running ones and
    /// only take effect after a restart (display-ready names). Empty when
    /// nothing is pending.
    pub fn restart_required(&self) -> Vec<String> {
        let now = &self.shared.registry.load().config;
        let s = &self.started;
        let mut out = Vec::new();
        if now.server.listen != s.server.listen {
            out.push(format!("Local address ({})", now.server.listen));
        }
        if now.server.local_key != s.server.local_key {
            out.push("Local key source".into());
        }
        if now.server.connect_timeout_ms != s.server.connect_timeout_ms {
            out.push("Upstream connect timeout".into());
        }
        if now.server.response_header_timeout_ms != s.server.response_header_timeout_ms {
            out.push("Upstream response timeout".into());
        }
        if now.usage != s.usage {
            out.push("Usage recording and retention".into());
        }
        out
    }

    /// Copyable T3 settings for an account.
    pub fn client_setup(&self, account: &str) -> Option<setup::ClientSetup> {
        let reg = self.shared.registry.load();
        reg.account(account)
            .map(|a| setup::client_setup(&a.cfg, self.shared.addr))
    }

    /// The local client key. Show only on an explicit user action.
    pub fn reveal_local_key(&self) -> String {
        self.shared.local_key.expose().to_string()
    }

    /// Usage database path, for [`relay_usage::Reader::open`].
    pub fn usage_db(&self) -> &std::path::Path {
        &self.usage_db
    }

    /// Stops accepting, drains in-flight requests for up to [`DRAIN`], then
    /// flushes usage and joins the runtime thread.
    pub fn shutdown(mut self) {
        self.stop_now();
    }

    fn stop_now(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Some(r) = self.recorder.take() {
            r.stop();
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.stop_now();
    }
}

/// Loads credentials for router-owned accounts that lack one, off-thread.
/// Publishes a snapshot after each load, so the UI shows the result.
fn load_credentials(shared: &Arc<Shared>, reg: &Arc<Registry>) {
    let pending: Vec<_> = reg
        .accounts
        .iter()
        .filter(|a| a.cfg.mode.router_owned() && !a.cfg.paused && a.credential.load().is_none())
        .cloned()
        .collect();
    if pending.is_empty() {
        return;
    }
    let shared = shared.clone();
    let _ = std::thread::Builder::new()
        .name("relay-credentials".into())
        .spawn(move || {
            for a in pending {
                a.load_credential();
                shared.changed();
            }
        });
}

/// Starts the proxy. Binds synchronously, so a port conflict is reported
/// here rather than later.
pub fn start(paths: &Paths, config: Config) -> Result<Handle, StartError> {
    config.validate().map_err(StartError::Config)?;
    let listen = config.server.listen;
    let std_listener = std::net::TcpListener::bind(listen).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            StartError::AddrInUse(listen)
        } else {
            StartError::Bind(e)
        }
    })?;
    std_listener
        .set_nonblocking(true)
        .map_err(StartError::Bind)?;
    let addr = std_listener.local_addr().map_err(StartError::Bind)?;

    let local_key = secrets::local_key(config.server.local_key.as_ref(), &paths.local_key_file)
        .map_err(|e| StartError::Runtime(std::io::Error::other(e.0)))?;
    let recorder = if config.usage.enabled {
        Some(
            relay_usage::Recorder::start(
                &paths.usage_db,
                config.usage.raw_retention_days,
                config.usage.hourly_retention_days,
                relay_usage::writer::QUEUE,
            )
            .map_err(|e| StartError::Usage(e.to_string()))?,
        )
    } else {
        None
    };
    let sink = recorder
        .as_ref()
        .map(relay_usage::Recorder::sink)
        .unwrap_or_else(relay_usage::Sink::disabled);
    let connect_timeout = Duration::from_millis(config.server.connect_timeout_ms);
    let header_timeout = Duration::from_millis(config.server.response_header_timeout_ms);
    let started = config.clone();
    let registry = Arc::new(Registry::build(config, None));
    let shared = Arc::new(Shared {
        registry: ArcSwap::new(registry.clone()),
        sessions: Default::default(),
        paused: AtomicBool::new(false),
        dirty: AtomicBool::new(true),
        notify: Default::default(),
        cursor: AtomicUsize::new(0),
        usage: sink,
        local_key,
        version: AtomicU64::new(0),
        addr,
    });
    load_credentials(&shared, &registry);
    let (first, _) = shared.build_snapshot(now_ms());
    let first = Arc::new(first);
    let snapshot = Arc::new(ArcSwap::new(first.clone()));
    let (tx, rx) = watch::channel(first);
    let (stop_tx, stop_rx) = oneshot::channel();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(std::thread::available_parallelism().map_or(2, |n| n.get().clamp(2, 4)))
        .thread_name("relay-proxy")
        .enable_all()
        .build()
        .map_err(StartError::Runtime)?;
    let sh = shared.clone();
    let snap = snapshot.clone();
    let thread = std::thread::Builder::new()
        .name("relay-proxy-main".into())
        .spawn(move || {
            runtime.block_on(async move {
                let listener = match tokio::net::TcpListener::from_std(std_listener) {
                    Ok(l) => l,
                    Err(_) => return,
                };
                let publisher = tokio::spawn(publish(sh.clone(), snap, tx));
                serve(
                    listener,
                    sh,
                    forward::client(connect_timeout, header_timeout),
                    stop_rx,
                )
                .await;
                publisher.abort();
            });
            runtime.shutdown_timeout(Duration::from_secs(1));
        })
        .map_err(StartError::Runtime)?;
    Ok(Handle {
        shared,
        snapshot,
        updates: rx,
        stop: Some(stop_tx),
        thread: Some(thread),
        recorder,
        usage_db: paths.usage_db.clone(),
        started,
    })
}

async fn serve(
    listener: tokio::net::TcpListener,
    sh: Arc<Shared>,
    client: forward::Client,
    mut stop: oneshot::Receiver<()>,
) {
    let graceful = GracefulShutdown::new();
    let builder = auto::Builder::new(TokioExecutor::new());
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { continue };
                let _ = stream.set_nodelay(true);
                let sh = sh.clone();
                let client = client.clone();
                let svc = service_fn(move |req| {
                    let sh = sh.clone();
                    let client = client.clone();
                    async move { Ok::<_, std::convert::Infallible>(forward::handle(sh, client, req).await) }
                });
                let conn = builder.serve_connection(TokioIo::new(stream), svc).into_owned();
                let conn = graceful.watch(conn);
                tokio::spawn(async move {
                    let _ = conn.await;
                });
            }
            _ = &mut stop => break,
        }
    }
    drop(listener);
    let _ = tokio::time::timeout(DRAIN, graceful.shutdown()).await;
}

/// Publishes snapshots on change, at most every [`PUBLISH_INTERVAL`], and
/// otherwise sleeps until the next time-driven change.
async fn publish(sh: Arc<Shared>, snap: Arc<ArcSwap<Snapshot>>, tx: watch::Sender<Arc<Snapshot>>) {
    let mut last = snap.load_full();
    loop {
        // Clear before building: a change during the build sets it again
        // (and stores a Notify permit), so it is never lost.
        sh.dirty.store(false, Ordering::Release);
        let (mut next, wake_at) = sh.build_snapshot(now_ms());
        next.version = last.version;
        next.generated_at = last.generated_at;
        if next != *last {
            next.version = sh.version.fetch_add(1, Ordering::Relaxed) + 1;
            next.generated_at = now_ms();
            let next = Arc::new(next);
            snap.store(next.clone());
            tx.send_replace(next.clone());
            last = next;
        }
        tokio::time::sleep(PUBLISH_INTERVAL).await;
        if sh.dirty.load(Ordering::Acquire) {
            continue;
        }
        match wake_at {
            Some(t) => {
                let ms = (t - now_ms()).max(0) as u64 + 5;
                let _ = tokio::time::timeout(Duration::from_millis(ms), sh.notify.notified()).await;
            }
            None => sh.notify.notified().await,
        }
    }
}
