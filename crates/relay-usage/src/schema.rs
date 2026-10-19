//! SQLite schema and local-time bucketing.

use chrono::{Local, TimeZone, Timelike};
use relay_core::UnixMs;
use rusqlite::Connection;

/// Schema version stored in `meta`.
pub const VERSION: i64 = 1;

/// Creates tables if missing and sets WAL mode.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY,
            at_ms INTEGER NOT NULL,
            day TEXT NOT NULL,
            hour INTEGER NOT NULL,
            account_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            model TEXT,
            mode TEXT NOT NULL,
            outcome TEXT NOT NULL,
            status INTEGER NOT NULL,
            attempt INTEGER NOT NULL,
            input INTEGER NOT NULL,
            cache_read INTEGER NOT NULL,
            cache_write INTEGER NOT NULL,
            output INTEGER NOT NULL,
            reasoning INTEGER NOT NULL,
            usage_known INTEGER NOT NULL,
            latency_ms INTEGER NOT NULL,
            ttft_ms INTEGER
         );
         CREATE INDEX IF NOT EXISTS events_at ON events(at_ms);
         CREATE TABLE IF NOT EXISTS hourly (
            day TEXT NOT NULL,
            hour INTEGER NOT NULL,
            account_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            model TEXT NOT NULL,
            requests INTEGER NOT NULL DEFAULT 0,
            errors INTEGER NOT NULL DEFAULT 0,
            retries INTEGER NOT NULL DEFAULT 0,
            ambiguous INTEGER NOT NULL DEFAULT 0,
            input INTEGER NOT NULL DEFAULT 0,
            cache_read INTEGER NOT NULL DEFAULT 0,
            cache_write INTEGER NOT NULL DEFAULT 0,
            output INTEGER NOT NULL DEFAULT 0,
            reasoning INTEGER NOT NULL DEFAULT 0,
            ttft_sum INTEGER NOT NULL DEFAULT 0,
            ttft_n INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (day, hour, account_id, model)
         );
         CREATE TABLE IF NOT EXISTS coverage (
            day TEXT NOT NULL,
            hour INTEGER NOT NULL,
            PRIMARY KEY (day, hour)
         );",
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO meta(key, value) VALUES ('schema', ?1)",
        [VERSION.to_string()],
    )?;
    Ok(())
}

/// Local calendar day (`YYYY-MM-DD`) and hour for a timestamp.
pub fn local_bucket(at: UnixMs) -> (String, u32) {
    match Local.timestamp_millis_opt(at).single() {
        Some(t) => (t.format("%Y-%m-%d").to_string(), t.hour()),
        None => ("1970-01-01".into(), 0),
    }
}
