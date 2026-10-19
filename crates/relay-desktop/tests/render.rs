//! Offscreen render check. Uses GPUI's `HeadlessAppContext` with the real
//! Metal renderer: no window is shown, nothing takes focus, no Dock icon,
//! no network, no user files. Fixture snapshots go through the same adapter
//! the app uses. PNGs land in `target/relay-snapshots/` for comparison with
//! the approved boards (`design/screenshots/final/`).
//!
//! Runs on the main thread (`harness = false`), as AppKit requires. On
//! platforms without a headless renderer it reports SKIP and exits 0.

use std::{path::PathBuf, sync::Arc};

use gpui::{AppContext, HeadlessAppContext, Pixels, Size, px, size};
use relay_core::{
    config::PeriodKind,
    model::{AccountId, AccountMode, CredentialKind, Provider},
    period::{DayHours, through_relay},
    quota::{Credits, QuotaWindow},
    snapshot::{AccountState, AccountView, Capabilities, RoutingStatus, Snapshot, group_accounts},
};
use relay_desktop::{
    adapter,
    assets::Assets,
    dashboard::Dashboard,
    model::Through,
    popover::{Popover, SHADOW_MARGIN},
    store::{Store, Tab},
    theme::Theme,
    widgets::Ui,
};

const NOW: i64 = 1_792_323_660_000; // fixed instant; times below are relative

fn min(m: i64) -> i64 {
    m * 60_000
}

fn w(
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
        resets_at: reset_in_min.map(|m| NOW + min(m)),
        window_minutes: None,
        status: None,
        observed_at: NOW - min(age_min),
    }
}

#[allow(clippy::too_many_arguments)]
fn acc(
    id: &str,
    label: &str,
    who: &str,
    provider: Provider,
    plan: &str,
    mode: AccountMode,
    state: AccountState,
    sessions: u32,
    seen_min: i64,
    windows: Vec<QuotaWindow>,
) -> AccountView {
    let credential = CredentialKind::for_mode(mode);
    AccountView {
        id: AccountId(id.into()),
        provider,
        label: label.into(),
        identifier: Some(who.into()),
        plan: Some(plan.into()),
        mode,
        credential,
        state,
        state_detail: None,
        in_flight: 0,
        sessions,
        last_seen: Some(NOW - min(seen_min)),
        windows,
        credits: None,
        cooldown_until: None,
        models_seen: vec![],
        models_allowed: vec![],
        capabilities: Capabilities::for_account(provider, credential),
    }
}

/// Mirrors the design-sample accounts of boards 1–3 (fixture data).
fn fixture_snapshot(routing: RoutingStatus) -> Snapshot {
    let a = Provider::Anthropic;
    let o = Provider::Openai;
    let mut personal = acc(
        "claude-personal",
        "Personal",
        "d••@gmail.com",
        a,
        "Max 20×",
        AccountMode::Pooled,
        AccountState::Healthy,
        0,
        2,
        vec![
            w("5h", "5-hour", Some(19.0), 2, Some(130)),
            w("7d", "Weekly", Some(12.0), 2, Some(5760)),
            w("fable-7d", "Fable 7d", Some(0.0), 2, Some(5760)),
        ],
    );
    personal.credits = Some(Credits {
        has_credits: Some(true),
        unlimited: None,
        balance: Some("1 · expire Oct 22, 09:00".into()),
        observed_at: NOW,
    });
    let accounts = vec![
        personal,
        acc(
            "claude-work",
            "Work",
            "d••@acme.co",
            a,
            "Max 20×",
            AccountMode::Pooled,
            AccountState::InUse,
            2,
            4,
            vec![
                w("5h", "5-hour", Some(36.0), 4, Some(180)),
                w("7d", "Weekly", Some(30.0), 4, Some(7200)),
                w("fable-7d", "Fable 7d", Some(8.0), 4, Some(7200)),
            ],
        ),
        acc(
            "claude-secondary",
            "Secondary",
            "d••@proton.me",
            a,
            "Max 20×",
            AccountMode::Pooled,
            AccountState::Exhausted,
            0,
            1,
            vec![
                w("5h", "5-hour", Some(100.0), 1, Some(38)),
                w("7d", "Weekly", Some(46.0), 1, Some(8640)),
                w("fable-7d", "Fable 7d", Some(0.0), 1, Some(8640)),
            ],
        ),
        acc(
            "key-a",
            "Key A",
            "sk-…7f2c",
            a,
            "API",
            AccountMode::Key,
            AccountState::Healthy,
            0,
            0,
            vec![w("tpm", "Tokens/min", Some(20.0), 0, Some(1))],
        ),
        acc(
            "key-b",
            "Key B",
            "sk-…91ab",
            a,
            "API",
            AccountMode::Key,
            AccountState::Healthy,
            0,
            0,
            vec![w("tpm", "Tokens/min", Some(28.0), 0, Some(1))],
        ),
        acc(
            "codex-dev",
            "Dev",
            "d••@acme.co",
            o,
            "Plus",
            AccountMode::Observed,
            AccountState::Low,
            1,
            3,
            vec![
                w("primary", "5-hour", Some(78.0), 3, Some(65)),
                w("secondary", "Weekly", Some(36.0), 3, Some(8640)),
            ],
        ),
        acc(
            "codex-team",
            "Team",
            "t••@acme.co",
            o,
            "Plus",
            AccountMode::Observed,
            AccountState::Stale,
            0,
            18,
            vec![
                w("primary", "5-hour", Some(42.0), 18, Some(300)),
                w("secondary", "Weekly", None, 18, None),
            ],
        ),
    ];
    Snapshot {
        version: 1,
        generated_at: NOW,
        routing,
        providers: group_accounts(accounts, NOW),
        telemetry_dropped: 0,
        in_flight: 3,
    }
}

fn fixture_through() -> Through {
    use chrono::NaiveDate;
    let mut days = std::collections::BTreeMap::new();
    let start = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    for i in 0..48u64 {
        let d = start + chrono::Days::new(i);
        let mut h = DayHours {
            covered: 0x00FF_FFFF,
            tokens: [0; 24],
        };
        let base = [
            1_000_000u64,
            1_150_000,
            820_000,
            1_300_000,
            1_100_000,
            900_000,
            600_000,
        ];
        h.tokens[10] = base[(i % 7) as usize] + (i * 9_000) % 140_000;
        days.insert(d, h);
    }
    let today = NaiveDate::from_ymd_opt(2026, 10, 18).unwrap();
    let week = through_relay(PeriodKind::Week, today, 9, chrono::Weekday::Mon, &days);
    let month = through_relay(PeriodKind::Month, today, 9, chrono::Weekday::Mon, &days);
    Through {
        week: adapter::series(&week, "09:41"),
        month: adapter::series(&month, "09:41"),
    }
}

fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/relay-snapshots");
    std::fs::create_dir_all(&dir).expect("snapshot dir");
    dir
}

struct Checked {
    name: &'static str,
    width: u32,
    height: u32,
}

fn save(img: &image::RgbaImage, name: &str) {
    img.save(out_dir().join(format!("{name}.png")))
        .expect("write png");
}

/// The panel background must be the theme's tonal popover colour (no border).
fn assert_pixel_near(img: &image::RgbaImage, x: u32, y: u32, expected: gpui::Rgba, what: &str) {
    let p = img.get_pixel(x, y).0;
    let e = [
        (expected.r * 255.0) as i32,
        (expected.g * 255.0) as i32,
        (expected.b * 255.0) as i32,
    ];
    let d = (0..3).map(|i| (p[i] as i32 - e[i]).abs()).max().unwrap();
    assert!(d <= 3, "{what}: pixel {:?} at ({x},{y}) is not {:?}", p, e);
}

fn not_blank(img: &image::RgbaImage, what: &str) {
    let first = img.get_pixel(img.width() / 2, img.height() / 2).0;
    let distinct = img.pixels().filter(|p| p.0 != first).count();
    assert!(
        distinct > (img.width() * img.height() / 50) as usize,
        "{what} looks blank"
    );
}

fn main() {
    if !cfg!(target_os = "macos") {
        println!(
            "SKIP render: headless Metal renderer is macOS-only; Linux renders in CI via the app smoke test."
        );
        return;
    }
    let text_system = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        Arc::new(Assets),
        gpui_platform::current_headless_renderer,
    );
    let mut results: Vec<Checked> = Vec::new();

    for (dark, suffix) in [(false, "light"), (true, "dark")] {
        let theme = if dark { Theme::dark() } else { Theme::light() };
        cx.update(|cx| {
            cx.set_global(Ui {
                theme,
                reduced_motion: true,
                preference: if dark {
                    relay_core::config::Theme::Dark
                } else {
                    relay_core::config::Theme::Light
                },
            });
            relay_desktop::popover::bind_keys(cx);
            relay_desktop::dashboard::bind_keys(cx);
        });

        let cases: [(&'static str, Option<RoutingStatus>, bool); 4] = [
            (
                "popover-collapsed",
                Some(RoutingStatus::Running {
                    addr: "127.0.0.1:8787".parse().unwrap(),
                }),
                false,
            ),
            (
                "popover-expanded",
                Some(RoutingStatus::Running {
                    addr: "127.0.0.1:8787".parse().unwrap(),
                }),
                true,
            ),
            (
                "popover-not-routing",
                Some(RoutingStatus::Failed {
                    reason: "127.0.0.1:8787 is already in use".into(),
                }),
                false,
            ),
            ("popover-empty", None, false),
        ];
        for (name, routing, expanded) in cases {
            let overview = match &routing {
                // Not routing: the bridge lists configured accounts with unknown quota.
                Some(r @ RoutingStatus::Failed { .. }) => {
                    let mut snap = fixture_snapshot(r.clone());
                    let accounts: Vec<AccountView> = snap
                        .providers
                        .drain(..)
                        .flat_map(|p| p.groups)
                        .flat_map(|g| g.accounts)
                        .map(|mut a| {
                            a.windows.clear();
                            a.state = AccountState::Unknown;
                            a.sessions = 0;
                            a.last_seen = None;
                            a
                        })
                        .collect();
                    snap.providers = group_accounts(accounts, NOW);
                    adapter::overview(&snap, fixture_through(), "127.0.0.1:8787")
                }
                Some(r) => adapter::overview(
                    &fixture_snapshot(r.clone()),
                    fixture_through(),
                    "127.0.0.1:8787",
                ),
                None => adapter::overview(
                    &Snapshot {
                        version: 0,
                        generated_at: NOW,
                        routing: RoutingStatus::Running {
                            addr: "127.0.0.1:8787".parse().unwrap(),
                        },
                        providers: vec![],
                        telemetry_dropped: 0,
                        in_flight: 0,
                    },
                    Through::default(),
                    "127.0.0.1:8787",
                ),
            };
            let keys: Vec<String> = overview
                .sections
                .iter()
                .flat_map(|s| s.groups.iter().map(|g| g.key.clone()))
                .collect();
            let store = cx.update(|cx| cx.new(|_| Store::new(overview, None)));
            if expanded {
                cx.update(|cx| {
                    store.update(cx, |s, _| {
                        s.expanded
                            .extend(keys.iter().filter(|k| !k.contains("api-key")).cloned())
                    })
                });
            }
            let tall: Size<Pixels> = size(px(360. + SHADOW_MARGIN * 2.), px(2000.));
            let handle = cx
                .open_window(tall, |window, cx| {
                    cx.new(|cx| Popover::new(store.clone(), window, cx).settled())
                })
                .expect("open popover");
            cx.run_until_parked();
            // First frame measures content; resize to it like the app does.
            let _ = cx.update_window(handle.into(), |_, window, _| window.refresh());
            let content =
                cx.update(|cx| handle.read(cx).map(|p| p.content_height()).unwrap_or(600.));
            let h = relay_desktop::popover::window_height(content, 2000.);
            let _ = cx.update_window(handle.into(), |_, window, _| {
                window.resize(size(px(360. + SHADOW_MARGIN * 2.), px(h)))
            });
            cx.run_until_parked();
            let _ = cx.update_window(handle.into(), |_, window, _| window.refresh());
            let img = match cx.capture_screenshot(handle.into()) {
                Ok(img) => img,
                Err(e) => {
                    println!("SKIP render: no headless renderer ({e})");
                    return;
                }
            };
            let scale = img.width() as f32 / (360. + SHADOW_MARGIN * 2.);
            not_blank(&img, name);
            // Header area just inside the panel: popover surface colour.
            assert_pixel_near(
                &img,
                ((SHADOW_MARGIN + 6.) * scale) as u32,
                ((SHADOW_MARGIN + 30.) * scale) as u32,
                theme.popover,
                name,
            );
            let file = format!("{name}-{suffix}");
            save(&img, &file);
            results.push(Checked {
                name,
                width: img.width(),
                height: img.height(),
            });
            let _ = cx.update_window(handle.into(), |_, window, _| window.remove_window());
        }

        for tab in [Tab::Quotas, Tab::Accounts, Tab::Usage, Tab::Routing] {
            let overview = adapter::overview(
                &fixture_snapshot(RoutingStatus::Running {
                    addr: "127.0.0.1:8787".parse().unwrap(),
                }),
                fixture_through(),
                "127.0.0.1:8787",
            );
            let store = cx.update(|cx| {
                cx.new(|_| {
                    let mut s = Store::new(overview, None);
                    s.tab = tab;
                    s.usage_rows = usage_fixture();
                    s
                })
            });
            let handle = cx
                .open_window(size(px(1340.), px(1500.)), |window, cx| {
                    cx.new(|cx| Dashboard::new(store.clone(), window, cx))
                })
                .expect("open main");
            cx.run_until_parked();
            let _ = cx.update_window(handle.into(), |_, window, _| window.refresh());
            let img = cx.capture_screenshot(handle.into()).expect("capture main");
            not_blank(&img, tab.label());
            let file = format!("main-{}-{suffix}", tab.label().to_lowercase());
            save(&img, &file);
            results.push(Checked {
                name: tab.label(),
                width: img.width(),
                height: img.height(),
            });
            let _ = cx.update_window(handle.into(), |_, window, _| window.remove_window());
        }
    }
    for r in &results {
        println!("rendered {} ({}×{})", r.name, r.width, r.height);
    }
    println!(
        "ok: {} offscreen renders in {}",
        results.len(),
        out_dir().display()
    );
}

fn usage_fixture() -> Vec<relay_desktop::model::UsageRow> {
    use relay_desktop::{assets::Mark, model::UsageRow};
    let row = |label: &str, sub: &str, mark, i, o, c, r, e| UsageRow {
        label: label.into(),
        sublabel: sub.into(),
        mark: Some(mark),
        input: i,
        output: o,
        cached: c,
        requests: r,
        errors: e,
    };
    vec![
        row(
            "Personal",
            "Claude Max · Anthropic",
            Mark::Claude,
            3_100_000,
            412_000,
            18_400_000,
            1204,
            3,
        ),
        row(
            "Work",
            "Claude Max · Anthropic",
            Mark::Claude,
            2_200_000,
            288_000,
            11_000_000,
            806,
            1,
        ),
        row(
            "Secondary",
            "Claude Max · Anthropic",
            Mark::Claude,
            1_400_000,
            190_000,
            6_200_000,
            512,
            14,
        ),
        row(
            "Key A · Key B",
            "API keys · Anthropic",
            Mark::Anthropic,
            400_000,
            52_000,
            1_100_000,
            140,
            0,
        ),
        row(
            "Dev",
            "ChatGPT Plus · Codex · OpenAI",
            Mark::ChatGpt,
            900_000,
            160_000,
            2_800_000,
            331,
            0,
        ),
        row(
            "Team",
            "ChatGPT Plus · Codex · OpenAI",
            Mark::ChatGpt,
            300_000,
            41_000,
            900_000,
            97,
            2,
        ),
    ]
}
