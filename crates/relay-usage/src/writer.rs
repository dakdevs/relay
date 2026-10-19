//! Bounded ingest and the batched SQLite writer thread.

use crate::schema::{local_bucket, migrate};
use relay_core::UnixMs;
use relay_core::attempt::Outcome;
use relay_core::model::{AccountId, AccountMode, Provider};
use relay_core::usage::TokenUsage;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Default queue capacity.
pub const QUEUE: usize = 8192;
/// Flush after this many events.
const BATCH: usize = 256;
/// Flush pending events after this long.
const FLUSH_EVERY: Duration = Duration::from_secs(1);

/// One upstream attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageEvent {
    /// When the attempt ended.
    pub at_ms: UnixMs,
    /// Account.
    pub account_id: AccountId,
    /// Provider.
    pub provider: Provider,
    /// Requested model.
    pub model: Option<String>,
    /// Account mode at the time.
    pub mode: AccountMode,
    /// Outcome.
    pub outcome: Outcome,
    /// Upstream HTTP status (0 when none).
    pub status: u16,
    /// 0 for the first attempt of a request.
    pub attempt: u8,
    /// Tokens.
    pub usage: TokenUsage,
    /// Whether the provider reported usage.
    pub usage_known: bool,
    /// Total attempt time.
    pub latency_ms: u32,
    /// Time to first byte forwarded downstream.
    pub ttft_ms: Option<u32>,
}

enum Msg {
    Event(Box<UsageEvent>),
    Stop,
}

/// Cheap clonable handle used on the request path.
#[derive(Clone)]
pub struct Sink {
    tx: Option<SyncSender<Msg>>,
    dropped: Arc<AtomicU64>,
}

impl Sink {
    /// Queues an event without blocking. Returns false (and counts a drop)
    /// when the queue is full or the writer is gone.
    pub fn record(&self, event: UsageEvent) -> bool {
        let Some(tx) = &self.tx else {
            // Recording is off: nothing is dropped, nothing is counted.
            return false;
        };
        match tx.try_send(Msg::Event(Box::new(event))) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// Events dropped so far.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// A sink for disabled recording: ignores events and reports no drops.
    pub fn disabled() -> Sink {
        Sink {
            tx: None,
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// Owns the writer thread.
pub struct Recorder {
    sink: Sink,
    thread: Option<JoinHandle<()>>,
}

impl Recorder {
    /// Opens (creating) the database, applies retention and starts the writer.
    pub fn start(
        path: &Path,
        raw_retention_days: u32,
        hourly_retention_days: u32,
        capacity: usize,
    ) -> rusqlite::Result<Recorder> {
        if let Some(dir) = path.parent() {
            private_dir(dir);
        }
        let conn = Connection::open(path)?;
        // Owner-only before WAL mode creates -wal/-shm; SQLite gives those
        // the database file's permissions.
        private_file(path);
        migrate(&conn)?;
        prune(&conn, now_ms(), raw_retention_days, hourly_retention_days)?;
        let (tx, rx) = sync_channel(capacity.max(1));
        let thread = std::thread::Builder::new()
            .name("relay-usage-writer".into())
            .spawn(move || run(conn, rx, raw_retention_days, hourly_retention_days))
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok(Recorder {
            sink: Sink {
                tx: Some(tx),
                dropped: Arc::new(AtomicU64::new(0)),
            },
            thread: Some(thread),
        })
    }

    /// Handle for the request path.
    pub fn sink(&self) -> Sink {
        self.sink.clone()
    }

    /// Flushes pending events and stops the thread.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        // A blocking send is fine here: shutdown is off the request path and
        // the writer drains continuously.
        if let Some(tx) = &self.sink.tx {
            let _ = tx.send(Msg::Stop);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Creates `dir` (and parents) and restricts it to the owner.
fn private_dir(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
}

fn private_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn now_ms() -> UnixMs {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn prune(conn: &Connection, now: UnixMs, raw_days: u32, hourly_days: u32) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM events WHERE at_ms < ?1",
        [now - i64::from(raw_days) * 86_400_000],
    )?;
    let (cutoff, _) = local_bucket(now - i64::from(hourly_days) * 86_400_000);
    conn.execute("DELETE FROM hourly WHERE day < ?1", [&cutoff])?;
    conn.execute("DELETE FROM coverage WHERE day < ?1", [&cutoff])?;
    Ok(())
}

fn mark_coverage(conn: &Connection, at: UnixMs) -> rusqlite::Result<(String, u32)> {
    let (day, hour) = local_bucket(at);
    conn.execute(
        "INSERT OR IGNORE INTO coverage(day, hour) VALUES (?1, ?2)",
        params![day, hour],
    )?;
    Ok((day, hour))
}

fn write_batch(conn: &mut Connection, batch: &[UsageEvent]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut ins = tx.prepare_cached(
            "INSERT INTO events(at_ms, day, hour, account_id, provider, model, mode, outcome, status,
                attempt, input, cache_read, cache_write, output, reasoning, usage_known, latency_ms, ttft_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        )?;
        let mut up = tx.prepare_cached(
            "INSERT INTO hourly(day, hour, account_id, provider, model, requests, errors, retries, ambiguous,
                input, cache_read, cache_write, output, reasoning, ttft_sum, ttft_n)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(day, hour, account_id, model) DO UPDATE SET
                requests = requests + 1,
                errors = errors + excluded.errors,
                retries = retries + excluded.retries,
                ambiguous = ambiguous + excluded.ambiguous,
                input = input + excluded.input,
                cache_read = cache_read + excluded.cache_read,
                cache_write = cache_write + excluded.cache_write,
                output = output + excluded.output,
                reasoning = reasoning + excluded.reasoning,
                ttft_sum = ttft_sum + excluded.ttft_sum,
                ttft_n = ttft_n + excluded.ttft_n",
        )?;
        let mut cov =
            tx.prepare_cached("INSERT OR IGNORE INTO coverage(day, hour) VALUES (?1, ?2)")?;
        for e in batch {
            let (day, hour) = local_bucket(e.at_ms);
            let u = &e.usage;
            ins.execute(params![
                e.at_ms,
                day,
                hour,
                e.account_id.as_str(),
                e.provider.key(),
                e.model,
                format!("{:?}", e.mode).to_lowercase(),
                e.outcome.key(),
                e.status,
                e.attempt,
                u.input as i64,
                u.cache_read as i64,
                u.cache_write as i64,
                u.output as i64,
                u.reasoning as i64,
                e.usage_known,
                e.latency_ms,
                e.ttft_ms,
            ])?;
            up.execute(params![
                day,
                hour,
                e.account_id.as_str(),
                e.provider.key(),
                e.model.as_deref().unwrap_or(""),
                i64::from(e.outcome.is_error()),
                i64::from(e.attempt > 0),
                i64::from(e.outcome == Outcome::Ambiguous),
                u.input as i64,
                u.cache_read as i64,
                u.cache_write as i64,
                u.output as i64,
                u.reasoning as i64,
                e.ttft_ms.map_or(0, i64::from),
                i64::from(e.ttft_ms.is_some()),
            ])?;
            cov.execute(params![day, hour])?;
        }
    }
    tx.commit()
}

fn run(mut conn: Connection, rx: Receiver<Msg>, raw_days: u32, hourly_days: u32) {
    let mut batch: Vec<UsageEvent> = Vec::with_capacity(BATCH);
    let mut covered = mark_coverage(&conn, now_ms()).ok();
    let mut first_pending: Option<Instant> = None;
    let mut last_prune = Instant::now();
    loop {
        // Idle: sleep until the next local hour so coverage is marked once
        // per hour while running. Pending: wake for the flush deadline.
        let wait = match first_pending {
            Some(t) => FLUSH_EVERY.saturating_sub(t.elapsed()),
            None => {
                let now = now_ms();
                let ms_into_hour = now.rem_euclid(3_600_000);
                Duration::from_millis((3_600_000 - ms_into_hour) as u64 + 50)
            }
        };
        let msg = rx.recv_timeout(wait);
        let stop = matches!(msg, Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected));
        if let Ok(Msg::Event(e)) = msg {
            batch.push(*e);
            first_pending.get_or_insert_with(Instant::now);
            // Drain what is already queued without waiting.
            while batch.len() < BATCH {
                match rx.try_recv() {
                    Ok(Msg::Event(e)) => batch.push(*e),
                    Ok(Msg::Stop) => {
                        flush(&mut conn, &mut batch);
                        return;
                    }
                    Err(_) => break,
                }
            }
        }
        let due = first_pending.is_some_and(|t| t.elapsed() >= FLUSH_EVERY);
        if !batch.is_empty() && (stop || due || batch.len() >= BATCH) {
            flush(&mut conn, &mut batch);
            first_pending = None;
        }
        let bucket = local_bucket(now_ms());
        if covered.as_ref() != Some(&bucket) {
            covered = mark_coverage(&conn, now_ms()).ok();
        }
        if last_prune.elapsed() > Duration::from_secs(3_600) {
            let _ = prune(&conn, now_ms(), raw_days, hourly_days);
            last_prune = Instant::now();
        }
        if stop {
            return;
        }
    }
}

fn flush(conn: &mut Connection, batch: &mut Vec<UsageEvent>) {
    if batch.is_empty() {
        return;
    }
    if let Err(e) = write_batch(conn, batch) {
        // Telemetry is best-effort; the request path is unaffected.
        eprintln!("relay-usage: dropped {} events: {e}", batch.len());
    }
    batch.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn event(at: UnixMs, account: &str, input: u64, output: u64) -> UsageEvent {
        UsageEvent {
            at_ms: at,
            account_id: AccountId(account.into()),
            provider: Provider::Anthropic,
            model: Some("claude-sonnet-5-5".into()),
            mode: AccountMode::Observed,
            outcome: Outcome::Ok,
            status: 200,
            attempt: 0,
            usage: TokenUsage {
                input,
                output,
                ..Default::default()
            },
            usage_known: true,
            latency_ms: 10,
            ttft_ms: Some(5),
        }
    }

    #[test]
    fn full_queue_drops_without_blocking() {
        // A sink whose receiver never drains: capacity 2.
        let (tx, _rx) = sync_channel(2);
        let sink = Sink {
            tx: Some(tx),
            dropped: Arc::new(AtomicU64::new(0)),
        };
        let start = Instant::now();
        let accepted = (0..10).filter(|_| sink.record(event(0, "a", 1, 1))).count();
        assert_eq!(accepted, 2);
        assert_eq!(sink.dropped(), 8);
        assert!(start.elapsed() < Duration::from_millis(50));
        let off = Sink::disabled();
        assert!(!off.record(event(0, "a", 1, 1)));
        assert_eq!(off.dropped(), 0, "disabled recording is not a drop");
    }

    #[test]
    fn writes_events_hourly_and_coverage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.db");
        let rec = Recorder::start(&path, 7, 400, 64).unwrap();
        let now = now_ms();
        let sink = rec.sink();
        assert!(sink.record(event(now, "a", 10, 5)));
        assert!(sink.record(event(now, "a", 1, 2)));
        rec.stop();
        let conn = Connection::open(&path).unwrap();
        let (n, input, output, requests): (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM events), SUM(input), SUM(output), SUM(requests) FROM hourly",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((n, input, output, requests), (2, 11, 7, 2));
        let (day, hour) = local_bucket(now);
        let covered: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM coverage WHERE day = ?1 AND hour = ?2",
                params![day, hour],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(covered, 1);
    }

    #[cfg(unix)]
    #[test]
    fn database_and_directory_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("relay");
        let path = home.join("usage.db");
        let rec = Recorder::start(&path, 7, 400, 8).unwrap();
        rec.sink().record(event(now_ms(), "a", 1, 1));
        rec.stop();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&home), 0o700);
        assert_eq!(mode(&path), 0o600);
        let wal = home.join("usage.db-wal");
        if wal.exists() {
            assert_eq!(mode(&wal), 0o600);
        }
    }

    #[test]
    fn retention_prunes_old_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.db");
        let old = now_ms() - 10 * 86_400_000;
        {
            let rec = Recorder::start(&path, 7, 400, 8).unwrap();
            rec.sink().record(event(old, "a", 1, 1));
            rec.stop();
        }
        // Reopening applies retention: raw rows older than 7 days go, hourly stays.
        Recorder::start(&path, 7, 400, 8).unwrap().stop();
        let conn = Connection::open(&path).unwrap();
        let raw: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap();
        let hourly: i64 = conn
            .query_row("SELECT COUNT(*) FROM hourly", [], |r| r.get(0))
            .unwrap();
        assert_eq!((raw, hourly), (0, 1));
    }
}
