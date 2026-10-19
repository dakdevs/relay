//! Maps core data (`relay_core::snapshot`, `relay_core::period`) to the
//! presentation model. Pure functions, run once per published snapshot.

use chrono::{Datelike, Local, NaiveDate, Timelike, Weekday};
use relay_core::{
    aggregate::WindowAggregate,
    config::PeriodKind,
    model::{AccountMode, CredentialKind, Provider},
    period::{self, ThroughRelay},
    quota::{Freshness, QuotaWindow},
    snapshot::{AccountState, AccountView, Capability, GroupView, RoutingStatus, Snapshot},
};

use crate::{
    assets::Mark,
    format,
    model::{
        Account, AccountWindow, Comparison, Group, GroupWindow, ModeTag, Overview, Section, Series,
        State, Status, Through, UnixMs,
    },
};

/// Window keys that are a plan's own limits; anything else (a per-model
/// limit such as "Fable 7d") nests under them in the summary.
const PLAN_WINDOWS: &[&str] = &[
    "5h",
    "7d",
    "primary",
    "secondary",
    "weekly",
    "tpm",
    "rpm",
    "itpm",
    "otpm",
];

pub fn overview(snapshot: &Snapshot, through: Through, configured_address: &str) -> Overview {
    let now = snapshot.generated_at;
    let (status, address) = match &snapshot.routing {
        RoutingStatus::Stopped => (Status::Starting, configured_address.to_string()),
        RoutingStatus::Running { addr } => (Status::Running, addr.to_string()),
        RoutingStatus::Paused { addr } => (Status::Paused, addr.to_string()),
        RoutingStatus::Failed { reason } => (
            failure(reason, configured_address),
            configured_address.to_string(),
        ),
    };
    let sections = snapshot
        .providers
        .iter()
        .map(|p| Section {
            provider: p.provider.label(),
            mark: provider_mark(p.provider),
            groups: p.groups.iter().map(|g| group(p.provider, g, now)).collect(),
        })
        .collect();
    Overview {
        status,
        address,
        sections,
        through,
        now,
    }
}

fn failure(reason: &str, address: &str) -> Status {
    let lower = reason.to_lowercase();
    let port = address.rsplit(':').next().unwrap_or(address);
    if lower.contains("in use") || lower.contains("addrinuse") {
        Status::NotRouting {
            title: format!("Port {port} is in use"),
            detail: format!(
                "Another process holds {address}. Quota appears again once Relay is routing."
            ),
        }
    } else {
        Status::NotRouting {
            title: "Relay stopped".into(),
            detail: reason.to_string(),
        }
    }
}

pub fn provider_mark(p: Provider) -> Mark {
    match p {
        Provider::Anthropic => Mark::Anthropic,
        Provider::Openai => Mark::OpenAi,
    }
}

/// "Max 20×" → "Max"; the plan family names the group, the full plan shows per account.
fn plan_family(plan: Option<&str>) -> Option<&str> {
    plan.and_then(|p| p.split_whitespace().next())
}

pub fn group_title(
    provider: Provider,
    credential: CredentialKind,
    plan: Option<&str>,
) -> (String, Option<Mark>) {
    match (provider, credential) {
        (_, CredentialKind::ApiKey) => ("API keys".into(), None),
        (Provider::Anthropic, CredentialKind::Subscription) => (
            plan_family(plan).map_or("Claude subscription".into(), |f| format!("Claude {f}")),
            Some(Mark::Claude),
        ),
        (Provider::Openai, CredentialKind::Subscription) => (
            plan_family(plan).map_or("ChatGPT · Codex".into(), |f| {
                format!("ChatGPT {f} · Codex")
            }),
            Some(Mark::ChatGpt),
        ),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn group(provider: Provider, g: &GroupView, now: UnixMs) -> Group {
    let (title, mark) = group_title(provider, g.credential, g.plan.as_deref());
    let n = g.accounts.len();
    let count_label = match g.credential {
        CredentialKind::ApiKey => plural(n, "key", "keys"),
        CredentialKind::Subscription => plural(n, "subscription", "subscriptions"),
    };
    let modes: Vec<AccountMode> = g.accounts.iter().map(|a| a.mode).collect();
    let mode = if modes.iter().all(|m| *m == AccountMode::Observed) {
        ModeTag::Observed
    } else if modes.iter().all(|m| *m == AccountMode::Key) {
        ModeTag::Key
    } else if modes.iter().all(|m| *m == AccountMode::Pooled) {
        ModeTag::Pooled
    } else {
        ModeTag::Mixed
    };
    let accounts: Vec<Account> = g.accounts.iter().map(|a| account(a, now)).collect();
    let windows = g
        .windows
        .iter()
        .map(|w| group_window(w, &g.accounts, now))
        .collect();
    Group {
        key: g.key.clone(),
        title,
        mark,
        count_label,
        mode,
        windows,
        accounts,
    }
}

fn group_window(w: &WindowAggregate, accounts: &[AccountView], now: UnixMs) -> GroupWindow {
    let segments = accounts
        .iter()
        .map(|a| match a.windows.iter().find(|x| x.key == w.key) {
            Some(x) => window_left(x, now),
            None => (None, false),
        })
        .collect();
    GroupWindow {
        label: w.label.clone(),
        avg_left: w.avg_left_pct,
        fresh: w.fresh as usize,
        stale: w.stale as usize,
        accounts: w.accounts as usize,
        pooled_left: w.pooled_left_pct,
        first_reset: w.first_reset,
        last_reset: w.last_reset,
        segments,
        model_limit: !PLAN_WINDOWS.contains(&w.key.as_str()),
    }
}

/// Percent left to draw, and whether it is stale. A stale value keeps its
/// last observation (dimmed, "not in avg"); an elapsed reset reads as full.
fn window_left(w: &QuotaWindow, now: UnixMs) -> (Option<f32>, bool) {
    match w.freshness(now) {
        Freshness::Fresh | Freshness::Reset => (w.effective_left(now), false),
        Freshness::Stale => (w.used_pct.map(|u| (100.0 - u).clamp(0.0, 100.0)), true),
        Freshness::Unknown => (None, false),
    }
}

fn account(a: &AccountView, now: UnixMs) -> Account {
    let state = match a.state {
        AccountState::InUse | AccountState::Healthy => State::Healthy,
        AccountState::Low => State::Low,
        AccountState::Exhausted => State::Exhausted,
        AccountState::Stale => State::Stale,
        AccountState::Unknown => State::Unknown,
        AccountState::Disconnected => State::Disconnected,
        AccountState::Paused => State::Paused,
    };
    let in_use = if a.sessions > 0 {
        a.sessions
    } else {
        a.in_flight
    };
    let credential = match a.credential {
        CredentialKind::Subscription => "subscription",
        CredentialKind::ApiKey => "API key",
    };
    let credits = a.credits.as_ref().and_then(|c| {
        if c.unlimited == Some(true) {
            Some("Reset credits: unlimited".to_string())
        } else {
            c.balance.as_ref().map(|b| format!("Reset credits: {b}"))
        }
    });
    let note = a.state_detail.clone().or_else(|| match a.state {
        AccountState::Exhausted => a
            .windows
            .iter()
            .filter(|w| w.is_exhausted(now))
            .filter_map(|w| w.resets_at)
            .min()
            .map(|r| format!("Back in {}.", format::duration_short(ms(r - now)))),
        AccountState::Stale => Some("Updates on this instance's next request.".into()),
        AccountState::Paused => Some("Paused by you · not routed".into()),
        _ => None,
    });
    Account {
        id: a.id.0.clone(),
        provider: a.provider,
        label: a.label.clone(),
        masked: a.identifier.clone().unwrap_or_default(),
        plan: a.plan.clone().unwrap_or_else(|| "—".into()),
        credential: credential.into(),
        state,
        in_use: if matches!(a.state, AccountState::Paused | AccountState::Disconnected) {
            0
        } else {
            in_use
        },
        seen_at: a.last_seen,
        windows: a
            .windows
            .iter()
            .map(|w| {
                let (left, stale) = window_left(w, now);
                AccountWindow {
                    label: w.label.clone(),
                    left,
                    resets_at: w.resets_at.filter(|r| *r > now),
                    stale,
                }
            })
            .collect(),
        credits,
        note,
        reset_unsupported: unsupported(&a.capabilities.reset_limits),
        refresh_unsupported: unsupported(&a.capabilities.refresh_quota),
        mode: match a.mode {
            AccountMode::Observed => ModeTag::Observed,
            AccountMode::Pooled => ModeTag::Pooled,
            AccountMode::Key => ModeTag::Key,
        },
    }
}

fn unsupported(c: &Capability) -> Option<String> {
    match c {
        Capability::Supported => None,
        Capability::Unsupported(reason) => Some(reason.clone()),
    }
}

pub fn ms(d: i64) -> std::time::Duration {
    std::time::Duration::from_millis(d.max(0) as u64)
}

// ---------------------------------------------------------------- Through Relay

fn month_name(d: NaiveDate) -> &'static str {
    const M: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    M[d.month0() as usize]
}

fn short_month(d: NaiveDate) -> &'static str {
    &month_name(d)[..3]
}

fn weekday_short(d: NaiveDate) -> &'static str {
    match d.weekday() {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

/// Converts one period's comparison into the sparkline series.
/// `now_hm` is the local time of day ("09:41") for span captions.
pub fn series(tr: &ThroughRelay, now_hm: &str) -> Series {
    let current: Vec<Option<u64>> = tr.current.iter().map(|b| b.tokens).collect();
    let prior: Vec<Option<u64>> = tr.prior.iter().map(|b| b.tokens).collect();
    let any = |v: &[Option<u64>]| v.iter().flatten().any(|t| *t > 0);
    let has_traffic = any(&current) || any(&prior);
    let start = tr.current.first().map(|b| b.date);
    let today = tr.current.last().map(|b| b.date);
    let week = tr.kind == PeriodKind::Week;
    let (legend_current, legend_prior) = match (week, today) {
        (true, _) => (
            "This week (to now)".to_string(),
            "Last week, same span".to_string(),
        ),
        (false, Some(t)) => {
            let prev = t.checked_sub_months(chrono::Months::new(1)).unwrap_or(t);
            (
                format!("{} (to now)", month_name(t)),
                format!("{}, same span", month_name(prev)),
            )
        }
        (false, None) => (String::new(), String::new()),
    };
    let prior_total = tr.prior_equivalent_total;
    let vs = prior_total.map(|p| format!("vs {}", format::tokens(p)));
    let comparison = match tr.comparison {
        period::Comparison::Delta(d) => Some(Comparison::Delta(d)),
        period::Comparison::TooEarly => Some(Comparison::TooEarly),
        period::Comparison::NoComparison => Some(Comparison::NoComparison),
        period::Comparison::PriorIncomplete => Some(Comparison::PriorIncomplete),
        period::Comparison::Incomplete => Some(Comparison::Incomplete),
        period::Comparison::NoTraffic => None,
    };
    let caption = if !has_traffic {
        "No requests through Relay yet".to_string()
    } else {
        match (&tr.comparison, start, today) {
            (period::Comparison::NoComparison, Some(s), _) => {
                let next = period::next_period_start(tr.kind, s);
                format!(
                    "Comparison starts {} {} {}",
                    weekday_short(next),
                    short_month(next),
                    next.day()
                )
            }
            (period::Comparison::Incomplete, ..) => {
                let missing: Vec<&str> = tr
                    .current
                    .iter()
                    .filter(|b| b.tokens.is_none())
                    .map(|b| weekday_short(b.date))
                    .collect();
                format!("{} missing", missing.join(", "))
            }
            (period::Comparison::PriorIncomplete, ..) => vs.clone().map_or(String::new(), |v| {
                format!("{v} · prior span partly recorded")
            }),
            (period::Comparison::TooEarly, _, Some(t)) => format!(
                "{} · {} 00:00–{now_hm} vs last {}",
                vs.clone().unwrap_or_default(),
                weekday_short(t),
                weekday_short(t)
            ),
            (_, Some(s), Some(t)) if !week => {
                let p = s.checked_sub_months(chrono::Months::new(1)).unwrap_or(s);
                let end = tr.prior_truncated_to.map_or(format!("{}", t.day()), |d| {
                    format!("{} (last day)", d.day())
                });
                format!(
                    "{} · {} 1–{} vs {} 1–{end}",
                    vs.clone().unwrap_or_default(),
                    short_month(t),
                    t.day(),
                    short_month(p)
                )
            }
            _ => format!("{} · same span last week", vs.clone().unwrap_or_default()),
        }
    };
    let span = match (start, today) {
        (Some(s), Some(t)) if week => format!(
            "{} 00:00 → {} {now_hm}, local time",
            weekday_short(s),
            weekday_short(t)
        ),
        (Some(s), Some(t)) => format!(
            "{} 1 00:00 → {} {} {now_hm}, local time",
            short_month(s),
            short_month(t),
            t.day()
        ),
        _ => String::new(),
    };
    let day_labels = (0..tr.period_days as usize)
        .map(|i| start.map(|s| s + chrono::Days::new(i as u64)))
        .map(|d| {
            d.map_or(String::new(), |d| {
                if week {
                    weekday_short(d).to_string()
                } else {
                    d.day().to_string()
                }
            })
        })
        .collect();
    let long = |d: NaiveDate| format!("{} {} {}", weekday_short(d), short_month(d), d.day());
    Series {
        current_dates: tr.current.iter().map(|b| long(b.date)).collect(),
        prior_dates: tr.prior.iter().map(|b| long(b.date)).collect(),
        current,
        prior,
        slots: tr.period_days as usize,
        day_labels,
        total: has_traffic.then_some(tr.current_total),
        prior_total,
        caption,
        legend_current,
        legend_prior,
        comparison: if has_traffic { comparison } else { None },
        span,
    }
}

pub fn through(week: &ThroughRelay, month: &ThroughRelay) -> Through {
    let now = Local::now();
    let hm = format!("{:02}:{:02}", now.hour(), now.minute());
    Through {
        week: series(week, &hm),
        month: series(month, &hm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_core::{
        model::AccountId,
        period::{DayBucket, through_relay},
        snapshot::{Capabilities, group_accounts},
    };
    use std::collections::BTreeMap;

    const NOW: UnixMs = 1_760_780_460_000; // fixed instant

    fn qw(
        key: &str,
        label: &str,
        used: Option<f32>,
        age_min: i64,
        reset_in_min: Option<i64>,
    ) -> QuotaWindow {
        QuotaWindow {
            key: key.into(),
            label: label.into(),
            used_pct: used,
            resets_at: reset_in_min.map(|m| NOW + m * 60_000),
            window_minutes: None,
            status: None,
            observed_at: NOW - age_min * 60_000,
        }
    }

    fn acc(
        id: &str,
        provider: Provider,
        plan: &str,
        mode: AccountMode,
        state: AccountState,
        windows: Vec<QuotaWindow>,
    ) -> AccountView {
        AccountView {
            id: AccountId(id.into()),
            provider,
            label: id.to_string(),
            identifier: Some("d••@acme.co".into()),
            plan: Some(plan.into()),
            mode,
            credential: CredentialKind::for_mode(mode),
            state,
            state_detail: None,
            in_flight: 0,
            sessions: if state == AccountState::InUse { 2 } else { 0 },
            last_seen: Some(NOW - 120_000),
            windows,
            credits: None,
            cooldown_until: None,
            models_seen: vec![],
            models_allowed: vec![],
            capabilities: Capabilities::for_account(provider, CredentialKind::for_mode(mode)),
        }
    }

    fn snapshot() -> Snapshot {
        let accounts = vec![
            acc(
                "Personal",
                Provider::Anthropic,
                "Max 20×",
                AccountMode::Pooled,
                AccountState::Healthy,
                vec![
                    qw("5h", "5-hour", Some(19.0), 2, Some(130)),
                    qw("fable-7d", "Fable 7d", Some(0.0), 2, Some(5760)),
                ],
            ),
            acc(
                "Work",
                Provider::Anthropic,
                "Max 20×",
                AccountMode::Pooled,
                AccountState::InUse,
                vec![qw("5h", "5-hour", Some(36.0), 4, Some(180))],
            ),
            acc(
                "Secondary",
                Provider::Anthropic,
                "Max 20×",
                AccountMode::Pooled,
                AccountState::Exhausted,
                vec![qw("5h", "5-hour", Some(100.0), 1, Some(38))],
            ),
            acc(
                "Team",
                Provider::Openai,
                "Plus",
                AccountMode::Observed,
                AccountState::Stale,
                vec![
                    qw("primary", "5-hour", Some(42.0), 18, Some(300)),
                    qw("secondary", "Weekly", None, 18, None),
                ],
            ),
        ];
        Snapshot {
            version: 1,
            generated_at: NOW,
            routing: RoutingStatus::Running {
                addr: "127.0.0.1:8787".parse().unwrap(),
            },
            providers: group_accounts(accounts, NOW),
            telemetry_dropped: 0,
            in_flight: 0,
        }
    }

    #[test]
    fn groups_are_titled_by_product_with_counts_and_marks() {
        let o = overview(&snapshot(), Through::default(), "127.0.0.1:8787");
        assert_eq!(o.status, Status::Running);
        assert_eq!(o.address, "127.0.0.1:8787");
        let claude = &o.sections[0].groups[0];
        assert_eq!(o.sections[0].provider, "Anthropic");
        assert_eq!(
            (claude.title.as_str(), claude.mark),
            ("Claude Max", Some(Mark::Claude))
        );
        assert_eq!(claude.count_label, "3 subscriptions");
        assert_eq!(claude.mode, ModeTag::Pooled);
        let codex = &o.sections[1].groups[0];
        assert_eq!(
            (codex.title.as_str(), codex.mark),
            ("ChatGPT Plus · Codex", Some(Mark::ChatGpt))
        );
        assert_eq!(codex.mode, ModeTag::Observed);
        assert_eq!(
            group_title(Provider::Anthropic, CredentialKind::ApiKey, None).0,
            "API keys"
        );
    }

    #[test]
    fn group_average_counts_fresh_only_and_model_limits_nest() {
        let o = overview(&snapshot(), Through::default(), "");
        let claude = &o.sections[0].groups[0];
        let five = claude.windows.iter().find(|w| w.label == "5-hour").unwrap();
        // (81 + 64 + 0) / 3, all fresh.
        assert_eq!(five.fresh, 3);
        assert!((five.avg_left.unwrap() - 48.333).abs() < 0.01);
        assert!(!five.model_limit);
        assert_eq!(five.segments.len(), 3);
        let fable = claude
            .windows
            .iter()
            .find(|w| w.label == "Fable 7d")
            .unwrap();
        assert!(fable.model_limit);
        // The Codex weekly window is unknown: never 0%.
        let codex = &o.sections[1].groups[0];
        let weekly = codex.windows.iter().find(|w| w.label == "Weekly").unwrap();
        assert_eq!(weekly.avg_left, None);
    }

    #[test]
    fn accounts_keep_stale_values_dimmed_and_explain_states() {
        let o = overview(&snapshot(), Through::default(), "");
        let claude = &o.sections[0].groups[0];
        let work = &claude.accounts[1];
        assert_eq!(work.in_use, 2);
        let secondary = &claude.accounts[2];
        assert_eq!(secondary.state, State::Exhausted);
        assert_eq!(secondary.note.as_deref(), Some("Back in 38m."));
        assert_eq!(secondary.windows[0].left, Some(0.0));
        let team = &o.sections[1].groups[0].accounts[0];
        assert_eq!(team.state, State::Stale);
        assert_eq!(team.windows[0].left, Some(58.0));
        assert!(team.windows[0].stale);
        assert_eq!(team.windows[1].left, None);
    }

    #[test]
    fn port_conflict_becomes_not_routing_with_the_port() {
        let mut s = snapshot();
        s.routing = RoutingStatus::Failed {
            reason: "address 127.0.0.1:8787 already in use".into(),
        };
        let o = overview(&s, Through::default(), "127.0.0.1:8787");
        match o.status {
            Status::NotRouting { title, .. } => assert_eq!(title, "Port 8787 is in use"),
            other => panic!("unexpected {other:?}"),
        }
    }

    fn day(d: &str) -> NaiveDate {
        NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn week_series_from_recorded_days() {
        // Sun 2026-10-18 09:00, week starts Monday 2026-10-12, every day recorded.
        let mut days = BTreeMap::new();
        for i in 0..14u64 {
            let mut h = period::DayHours {
                covered: 0x00FF_FFFF,
                tokens: [0; 24],
            };
            h.tokens[8] = 1_000_000;
            days.insert(day("2026-10-05") + chrono::Days::new(i), h);
        }
        let tr = through_relay(PeriodKind::Week, day("2026-10-18"), 9, Weekday::Mon, &days);
        let s = series(&tr, "09:41");
        assert_eq!(s.slots, 7);
        assert_eq!(s.current.len(), 7);
        assert_eq!(s.total, Some(7_000_000));
        assert_eq!(s.prior_total, Some(7_000_000));
        assert_eq!(s.comparison, Some(Comparison::Delta(0.0)));
        assert_eq!(s.caption, "vs 7M · same span last week");
        assert_eq!(s.legend_current, "This week (to now)");
        assert_eq!(s.day_labels[0], "Mon");
        assert_eq!(s.span, "Mon 00:00 → Sun 09:41, local time");
    }

    #[test]
    fn no_traffic_shows_dash_not_zero() {
        let tr = through_relay(
            PeriodKind::Week,
            day("2026-10-18"),
            9,
            Weekday::Mon,
            &BTreeMap::new(),
        );
        let s = series(&tr, "09:41");
        assert_eq!(s.total, None);
        assert_eq!(s.comparison, None);
        assert_eq!(s.caption, "No requests through Relay yet");
        assert!(s.current.iter().all(Option::is_none));
    }

    #[test]
    fn month_caption_names_both_spans() {
        let tr = ThroughRelay {
            kind: PeriodKind::Month,
            period_days: 31,
            current: (0..18)
                .map(|i| DayBucket {
                    date: day("2026-10-01") + chrono::Days::new(i),
                    tokens: Some(1_000_000),
                    partial: i == 17,
                })
                .collect(),
            prior: vec![],
            current_total: 18_800_000,
            prior_equivalent_total: Some(15_900_000),
            comparison: period::Comparison::Delta(18.2),
            prior_truncated_to: None,
        };
        let s = series(&tr, "09:41");
        assert_eq!(s.caption, "vs 15.9M · Oct 1–18 vs Sep 1–18");
        assert_eq!(s.legend_current, "October (to now)");
        assert_eq!(s.legend_prior, "September, same span");
    }
}
