//! Read-only queries for the UI. Call from a background thread; each query
//! is a few indexed SQLite reads.

use chrono::{Days, Local, NaiveDate, Timelike, Weekday};
use relay_core::config::PeriodKind;
use relay_core::model::{AccountId, Provider};
use relay_core::period::{DayHours, ThroughRelay, period_start, through_relay};
use relay_core::usage::TokenUsage;
use rusqlite::{Connection, OpenFlags, params};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Trailing range for token tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    /// Last 24 hours.
    Day,
    /// Last 7 days.
    Week,
    /// Last 30 days.
    Month,
}

impl Range {
    fn hours(self) -> i64 {
        match self {
            Range::Day => 24,
            Range::Week => 7 * 24,
            Range::Month => 30 * 24,
        }
    }
}

/// Summed usage for one account (and optionally one model).
#[derive(Debug, Clone, PartialEq)]
pub struct AccountTotals {
    /// Account.
    pub account_id: AccountId,
    /// Provider.
    pub provider: Option<Provider>,
    /// Model, for per-model queries.
    pub model: Option<String>,
    /// Tokens (summed, never averaged).
    pub usage: TokenUsage,
    /// Attempts.
    pub requests: u64,
    /// Attempts that ended in an error.
    pub errors: u64,
    /// Attempts after the first for a request.
    pub retries: u64,
    /// Attempts with an ambiguous outcome.
    pub ambiguous: u64,
    /// Mean time to first byte, when measured.
    pub mean_ttft_ms: Option<u64>,
}

/// Read-only access. A missing database reads as "never recorded".
pub struct Reader {
    path: PathBuf,
    conn: Option<Connection>,
}

impl Reader {
    /// Opens `path` read-only. Succeeds when the file does not exist yet.
    pub fn open(path: &Path) -> rusqlite::Result<Reader> {
        let mut r = Reader {
            path: path.to_path_buf(),
            conn: None,
        };
        r.connect()?;
        Ok(r)
    }

    fn connect(&mut self) -> rusqlite::Result<()> {
        if self.conn.is_none() && self.path.exists() {
            self.conn = Some(Connection::open_with_flags(
                &self.path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?);
        }
        Ok(())
    }

    /// Through Relay for the current local time.
    pub fn through_relay(
        &mut self,
        kind: PeriodKind,
        week_start: Weekday,
    ) -> rusqlite::Result<ThroughRelay> {
        let now = Local::now();
        self.through_relay_at(kind, week_start, now.date_naive(), now.hour())
    }

    /// Through Relay for an explicit local date and hour.
    pub fn through_relay_at(
        &mut self,
        kind: PeriodKind,
        week_start: Weekday,
        today: NaiveDate,
        hour: u32,
    ) -> rusqlite::Result<ThroughRelay> {
        self.connect()?;
        let start = period_start(kind, today, week_start);
        let prior_start = match kind {
            PeriodKind::Week => start - Days::new(7),
            PeriodKind::Month => start
                .checked_sub_months(chrono::Months::new(1))
                .unwrap_or(start),
        };
        let mut days: BTreeMap<NaiveDate, DayHours> = BTreeMap::new();
        if let Some(conn) = &self.conn {
            let from = prior_start.format("%Y-%m-%d").to_string();
            let to = today.format("%Y-%m-%d").to_string();
            let mut cov = conn
                .prepare_cached("SELECT day, hour FROM coverage WHERE day >= ?1 AND day <= ?2")?;
            for row in cov.query_map(params![from, to], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?))
            })? {
                let (day, hour) = row?;
                if let (Ok(d), true) = (NaiveDate::parse_from_str(&day, "%Y-%m-%d"), hour < 24) {
                    days.entry(d).or_default().covered |= 1 << hour;
                }
            }
            let mut tok = conn.prepare_cached(
                "SELECT day, hour, SUM(input + cache_read + cache_write + output)
                 FROM hourly WHERE day >= ?1 AND day <= ?2 GROUP BY day, hour",
            )?;
            for row in tok.query_map(params![from, to], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, u32>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })? {
                let (day, hour, t) = row?;
                if let (Ok(d), true) = (NaiveDate::parse_from_str(&day, "%Y-%m-%d"), hour < 24) {
                    let e = days.entry(d).or_default();
                    // Tokens imply Relay was running that hour.
                    e.covered |= 1 << hour;
                    e.tokens[hour as usize] += t.max(0) as u64;
                }
            }
        }
        Ok(through_relay(kind, today, hour, week_start, &days))
    }

    fn totals(&mut self, range: Range, by_model: bool) -> rusqlite::Result<Vec<AccountTotals>> {
        self.connect()?;
        let Some(conn) = &self.conn else {
            return Ok(Vec::new());
        };
        let now = Local::now();
        let cutoff = now - chrono::Duration::hours(range.hours());
        let (cd, ch) = (cutoff.format("%Y-%m-%d").to_string(), cutoff.hour());
        let group = if by_model {
            "account_id, provider, model"
        } else {
            "account_id, provider"
        };
        let model_col = if by_model { "model" } else { "NULL" };
        let sql = format!(
            "SELECT account_id, provider, {model_col}, SUM(input), SUM(cache_read), SUM(cache_write),
                    SUM(output), SUM(reasoning), SUM(requests), SUM(errors), SUM(retries), SUM(ambiguous),
                    SUM(ttft_sum), SUM(ttft_n)
             FROM hourly WHERE day > ?1 OR (day = ?1 AND hour >= ?2)
             GROUP BY {group} ORDER BY account_id, {model_col}"
        );
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![cd, ch], |r| {
            let g = |i: usize| -> rusqlite::Result<u64> { Ok(r.get::<_, i64>(i)?.max(0) as u64) };
            let ttft_n = g(13)?;
            Ok(AccountTotals {
                account_id: AccountId(r.get(0)?),
                provider: Provider::from_key(&r.get::<_, String>(1)?),
                model: r.get::<_, Option<String>>(2)?.filter(|m| !m.is_empty()),
                usage: TokenUsage {
                    input: g(3)?,
                    cache_read: g(4)?,
                    cache_write: g(5)?,
                    output: g(6)?,
                    reasoning: g(7)?,
                },
                requests: g(8)?,
                errors: g(9)?,
                retries: g(10)?,
                ambiguous: g(11)?,
                mean_ttft_ms: (ttft_n > 0).then(|| g(12).unwrap_or(0) / ttft_n),
            })
        })?;
        rows.collect()
    }

    /// Per-account totals over a trailing range (hour granularity).
    pub fn account_totals(&mut self, range: Range) -> rusqlite::Result<Vec<AccountTotals>> {
        self.totals(range, false)
    }

    /// Per-account, per-model totals over a trailing range.
    pub fn model_totals(&mut self, range: Range) -> rusqlite::Result<Vec<AccountTotals>> {
        self.totals(range, true)
    }
}

/// Weekday from a `UiPrefs::week_start` index (0 = Monday), defaulting to
/// Monday (ISO 8601) when unset.
pub fn week_start_from_pref(pref: Option<u8>) -> Weekday {
    pref.and_then(|i| Weekday::try_from(i).ok())
        .unwrap_or(Weekday::Mon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::{Recorder, UsageEvent};
    use chrono::TimeZone;
    use relay_core::attempt::Outcome;
    use relay_core::model::AccountMode;
    use relay_core::period::Comparison;

    fn ev(
        at: i64,
        account: &str,
        model: &str,
        input: u64,
        output: u64,
        outcome: Outcome,
        attempt: u8,
    ) -> UsageEvent {
        UsageEvent {
            at_ms: at,
            account_id: AccountId(account.into()),
            provider: Provider::Openai,
            model: Some(model.into()),
            mode: AccountMode::Pooled,
            outcome,
            status: 200,
            attempt,
            usage: TokenUsage {
                input,
                output,
                ..Default::default()
            },
            usage_known: true,
            latency_ms: 1,
            ttft_ms: Some(10),
        }
    }

    fn local_ms(d: NaiveDate, hour: u32) -> i64 {
        Local
            .from_local_datetime(&d.and_hms_opt(hour, 30, 0).unwrap())
            .earliest()
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn missing_database_is_null_not_zero() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = Reader::open(&dir.path().join("none.db")).unwrap();
        let tr = r
            .through_relay_at(
                PeriodKind::Week,
                Weekday::Mon,
                NaiveDate::from_ymd_opt(2026, 10, 14).unwrap(),
                9,
            )
            .unwrap();
        assert!(tr.current.iter().all(|b| b.tokens.is_none()));
        assert_eq!(tr.comparison, Comparison::Incomplete);
        assert!(r.account_totals(Range::Week).unwrap().is_empty());
    }

    #[test]
    fn through_relay_reads_recorded_days() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.db");
        let d = |day| NaiveDate::from_ymd_opt(2026, 10, day).unwrap();
        let rec = Recorder::start(&path, 3650, 3650, 64).unwrap();
        let sink = rec.sink();
        sink.record(ev(local_ms(d(12), 9), "a", "m", 100, 50, Outcome::Ok, 0));
        sink.record(ev(local_ms(d(13), 9), "b", "m", 10, 0, Outcome::Ok, 0));
        rec.stop();
        let mut r = Reader::open(&path).unwrap();
        let tr = r
            .through_relay_at(PeriodKind::Week, Weekday::Mon, d(13), 12)
            .unwrap();
        assert_eq!(tr.current[0].tokens, Some(150));
        assert_eq!(tr.current[1].tokens, Some(10));
        assert_eq!(tr.current_total, 160);
        // Prior week never recorded.
        assert_eq!(tr.comparison, Comparison::NoComparison);
    }

    #[test]
    fn totals_sum_per_account_and_model() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.db");
        let now = Local::now().timestamp_millis();
        let rec = Recorder::start(&path, 7, 400, 64).unwrap();
        let s = rec.sink();
        s.record(ev(now, "a", "gpt-5", 10, 1, Outcome::Ok, 0));
        s.record(ev(now, "a", "gpt-5", 20, 2, Outcome::Rejected, 0));
        s.record(ev(now, "a", "gpt-5-mini", 5, 5, Outcome::Ok, 1));
        s.record(ev(now, "b", "gpt-5", 7, 0, Outcome::Ambiguous, 0));
        s.record(ev(
            now - 3 * 86_400_000,
            "a",
            "gpt-5",
            1_000,
            0,
            Outcome::Ok,
            0,
        ));
        rec.stop();
        let mut r = Reader::open(&path).unwrap();
        let day = r.account_totals(Range::Day).unwrap();
        assert_eq!(day.len(), 2);
        let a = &day[0];
        assert_eq!(a.account_id.as_str(), "a");
        assert_eq!((a.usage.input, a.usage.output), (35, 8));
        assert_eq!((a.requests, a.errors, a.retries), (3, 1, 1));
        assert_eq!(day[1].ambiguous, 1);
        assert_eq!(a.mean_ttft_ms, Some(10));
        let week = r.account_totals(Range::Week).unwrap();
        assert_eq!(week[0].usage.input, 1_035);
        let models = r.model_totals(Range::Day).unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].model.as_deref(), Some("gpt-5"));
    }

    #[test]
    fn week_start_pref() {
        assert_eq!(week_start_from_pref(None), Weekday::Mon);
        assert_eq!(week_start_from_pref(Some(6)), Weekday::Sun);
    }
}
