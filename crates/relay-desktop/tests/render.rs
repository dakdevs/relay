//! Offscreen render check. Uses GPUI's `HeadlessAppContext` with the real
//! Metal renderer: no window is shown, nothing takes focus, no Dock icon,
//! no network, no user files. Fixture snapshots go through the same adapter
//! the app uses. PNGs land in `target/relay-snapshots/` for comparison with
//! the approved boards (`design/screenshots/final/`).
//!
//! Runs on the main thread (`harness = false`), as AppKit requires. On
//! platforms without a headless renderer it reports SKIP and exits 0.

use std::{path::PathBuf, sync::Arc};

use gpui::{AppContext, EntityInputHandler, Focusable, HeadlessAppContext, Pixels, Size, px, size};
use relay_core::{
    config::PeriodKind,
    model::{AccountId, AccountMode, CredentialKind, Provider},
    period::{DayHours, through_relay},
    quota::{Credits, QuotaWindow},
    snapshot::{AccountState, AccountView, Capabilities, RoutingStatus, Snapshot, group_accounts},
};
use relay_desktop::{
    accounts::{Choice, ConnectFlow, ConnectSheet, SHEET_MAX_WIDTH},
    adapter,
    assets::Assets,
    dashboard::Dashboard,
    model::Through,
    popover::{Popover, SHADOW_MARGIN},
    store::{ConnectEvent, Setup, Store, Tab},
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

/// A pixel column 3 pt inside the sections' right edge must be tonal
/// surface only (popover, section, or a blend at rounded corners): any text,
/// tag or bar crossing the inset shows up as an off-line colour.
fn assert_inset_clear(img: &image::RgbaImage, scale: f32, t: &Theme) {
    let x = ((SHADOW_MARGIN + 360. - 8. - 3.) * scale) as u32;
    let top = ((SHADOW_MARGIN + 48.) * scale) as u32;
    // Stop above the panel's rounded bottom edge and outer shadow.
    let bottom = img.height() - ((SHADOW_MARGIN + 40.) * scale) as u32;
    let rgb = |c: gpui::Rgba| [c.r * 255.0, c.g * 255.0, c.b * 255.0];
    let (a, b) = (rgb(t.popover), rgb(t.surface));
    for y in top..bottom {
        let p = img.get_pixel(x, y).0;
        let on_line = (0..=20).any(|i| {
            let k = i as f32 / 20.0;
            (0..3).all(|c| ((a[c] + (b[c] - a[c]) * k) - p[c] as f32).abs() <= 8.0)
        });
        assert!(on_line, "content crosses the right inset at y={y}: {p:?}");
    }
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

        let cases: [(&'static str, Option<RoutingStatus>, bool); 5] = [
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
            // Long plan title plus "n stale": nothing may cross the inset.
            (
                "popover-stress",
                Some(RoutingStatus::Running {
                    addr: "127.0.0.1:8787".parse().unwrap(),
                }),
                false,
            ),
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
                Some(r) if name == "popover-stress" => {
                    let mut snap = fixture_snapshot(r.clone());
                    let accounts: Vec<AccountView> = snap
                        .providers
                        .drain(..)
                        .flat_map(|p| p.groups)
                        .flat_map(|g| g.accounts)
                        .map(|mut a| {
                            if a.provider == Provider::Openai {
                                a.plan = Some("EnterpriseEducationWorkspaceAnnual".into());
                            }
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
            if name == "popover-stress" {
                assert_inset_clear(&img, scale, &theme);
            }
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

        connect_sheets(&mut cx, &theme, suffix, &mut results);
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

// ---------------------------------------------------------------- connect sheet

const LONG_LABEL: &str = "Personal research workstation for the evaluation team, second floor";
const LONG_ID: &str =
    "dakota.washbrook-longname+relay-evaluation@extremely-long-subdomain.example.com";
const LONG_CODEX_PATH: &str = "/Users/someone/Library/Application Support/codex-homes/personal-research-workstation/auth.json";
const LONG_CLAUDE_PATH: &str = "/Users/someone/Library/Application Support/claude-configs/personal-research-workstation/.credentials.json";
const LONG_ERROR: &str = "Couldn't save the account: the configuration file /Users/someone/Library/Application Support/Relay/config.toml is read-only (permission denied).";

struct SheetCase {
    name: &'static str,
    window: (f32, f32),
    provider: Provider,
    choice: Choice,
    flow: ConnectFlow,
    fields: [&'static str; 3],
    /// Content must be taller than the space above the footer.
    expect_scroll: bool,
}

fn sheet_cases() -> Vec<SheetCase> {
    let step_two = |op, reconnect: bool| {
        if reconnect {
            ConnectFlow::reconnecting("openai-dev".into(), op, None)
        } else {
            let mut f = ConnectFlow::fresh();
            f.submitted(op);
            f.on_event(&ConnectEvent::Saved {
                op,
                id: "openai-dev".into(),
            });
            f
        }
    };
    let failed = |message: &str| {
        let mut f = ConnectFlow::fresh();
        f.send_failed(message.into());
        f
    };
    vec![
        SheetCase {
            name: "sheet-anthropic-observe",
            window: (1100., 860.),
            provider: Provider::Anthropic,
            choice: Choice::Observe,
            flow: ConnectFlow::fresh(),
            fields: ["", "", ""],
            expect_scroll: false,
        },
        SheetCase {
            name: "sheet-openai-pool-long",
            window: (1100., 860.),
            provider: Provider::Openai,
            choice: Choice::Pool,
            flow: failed(LONG_ERROR),
            fields: [LONG_LABEL, LONG_ID, LONG_CODEX_PATH],
            expect_scroll: false,
        },
        SheetCase {
            name: "sheet-openai-observe",
            window: (1100., 860.),
            provider: Provider::Openai,
            choice: Choice::Observe,
            flow: ConnectFlow::fresh(),
            fields: ["Dev", "dev@acme.co", ""],
            expect_scroll: false,
        },
        SheetCase {
            name: "sheet-openai-key",
            window: (1100., 860.),
            provider: Provider::Openai,
            choice: Choice::Key,
            flow: ConnectFlow::fresh(),
            fields: ["Key A", "", ""],
            expect_scroll: false,
        },
        SheetCase {
            name: "sheet-anthropic-pool-long",
            window: (1100., 860.),
            provider: Provider::Anthropic,
            choice: Choice::Pool,
            flow: ConnectFlow::fresh(),
            fields: [LONG_LABEL, LONG_ID, LONG_CLAUDE_PATH],
            expect_scroll: false,
        },
        SheetCase {
            name: "sheet-anthropic-key-narrow",
            window: (420., 560.),
            provider: Provider::Anthropic,
            choice: Choice::Key,
            flow: failed(LONG_ERROR),
            fields: [
                LONG_LABEL,
                LONG_ID,
                "ANTHROPIC_API_KEY_FOR_THE_RESEARCH_WORKSTATION_SECOND_FLOOR",
            ],
            expect_scroll: true,
        },
        SheetCase {
            name: "sheet-openai-setup",
            window: (1100., 860.),
            provider: Provider::Openai,
            choice: Choice::Observe,
            flow: step_two(1, false),
            fields: [LONG_LABEL, "", ""],
            expect_scroll: false,
        },
        SheetCase {
            name: "sheet-anthropic-observe-setup",
            window: (1100., 860.),
            provider: Provider::Anthropic,
            choice: Choice::Observe,
            flow: step_two(3, false),
            fields: ["Personal", "", ""],
            expect_scroll: false,
        },
        SheetCase {
            name: "sheet-anthropic-pool-setup",
            window: (1100., 860.),
            provider: Provider::Anthropic,
            choice: Choice::Pool,
            flow: step_two(3, false),
            fields: ["Work", "", ""],
            expect_scroll: false,
        },
        SheetCase {
            name: "sheet-openai-key-setup",
            window: (1100., 860.),
            provider: Provider::Openai,
            choice: Choice::Key,
            flow: step_two(4, false),
            fields: ["Key A", "", ""],
            expect_scroll: false,
        },
        // Same width as the reconnect case below, which must lack Back.
        SheetCase {
            name: "sheet-openai-setup-narrow",
            window: (420., 520.),
            provider: Provider::Openai,
            choice: Choice::Observe,
            flow: step_two(1, false),
            fields: [LONG_LABEL, "", ""],
            expect_scroll: true,
        },
        SheetCase {
            name: "sheet-reconnect-error-narrow",
            window: (420., 520.),
            provider: Provider::Openai,
            choice: Choice::Observe,
            flow: {
                let mut f = step_two(2, true);
                f.on_event(&ConnectEvent::Failed {
                    op: 2,
                    message: "Couldn't prepare setup values: account openai-personal-research-workstation-second-floor isn't in the running configuration yet.".into(),
                });
                f
            },
            fields: [LONG_LABEL, "", ""],
            expect_scroll: true,
        },
    ]
}

fn rgb255(c: gpui::Rgba) -> [f32; 3] {
    [c.r * 255.0, c.g * 255.0, c.b * 255.0]
}

fn near(p: [u8; 4], c: gpui::Rgba, tol: f32) -> bool {
    let c = rgb255(c);
    (0..3).all(|i| (p[i] as f32 - c[i]).abs() <= tol)
}

/// True when `p` lies on the straight colour line from `a` to `b`.
fn on_line(p: [u8; 4], a: gpui::Rgba, b: gpui::Rgba, tol: f32) -> bool {
    let (a, b) = (rgb255(a), rgb255(b));
    (0..=20).any(|i| {
        let k = i as f32 / 20.0;
        (0..3).all(|c| ((a[c] + (b[c] - a[c]) * k) - p[c] as f32).abs() <= tol)
    })
}

/// Renders the real sheet inside the main window's overlay and checks it
/// against the window: it fits, its footer is pinned and shows the primary
/// action, nothing paints into its side padding, and short windows scroll
/// the body instead of pushing the footer out.
fn connect_sheets(
    cx: &mut HeadlessAppContext,
    t: &Theme,
    suffix: &str,
    results: &mut Vec<Checked>,
) {
    // Left edge of the footer's buttons, per case.
    let mut actions_left = std::collections::HashMap::new();
    for case in sheet_cases() {
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
                s.tab = Tab::Accounts;
                s.push_connect_event(ConnectEvent::Setup {
                    op: 1,
                    setup: Setup {
                        account_id: "openai-dev".into(),
                        lines: vec![
                            "-c model_provider=\"relay\"".into(),
                            "-c model_providers.relay.base_url=\"http://127.0.0.1:8787/v1\"".into(),
                            "-c model_providers.relay.wire_api=\"responses\"".into(),
                            "-c model_providers.relay.env_http_headers={\"X-Relay-Account\"=\"RELAY_ACCOUNT_PERSONAL_RESEARCH_WORKSTATION\"}".into(),
                        ],
                        notes: vec!["Leave OPENAI_API_KEY unset in that instance; Relay supplies the account.".into()],
                    },
                });
                s.push_connect_event(ConnectEvent::Setup {
                    op: 3,
                    setup: Setup {
                        account_id: "anthropic-personal".into(),
                        lines: vec![
                            "ANTHROPIC_BASE_URL=http://127.0.0.1:8787".into(),
                            "ANTHROPIC_CUSTOM_HEADERS=X-Relay-Account: anthropic-personal".into(),
                        ],
                        notes: vec![],
                    },
                });
                s.push_connect_event(ConnectEvent::Setup {
                    op: 4,
                    setup: Setup {
                        account_id: "openai-key-a".into(),
                        lines: vec!["OPENAI_BASE_URL=http://127.0.0.1:8787/v1".into()],
                        notes: vec![],
                    },
                });
                s
            })
        });
        let (w, h) = case.window;
        let sheet = std::rc::Rc::new(std::cell::RefCell::new(None));
        let slot = sheet.clone();
        let handle = cx
            .open_window(size(px(w), px(h)), |window, cx| {
                cx.new(|cx| {
                    let mut d = Dashboard::new(store.clone(), window, cx);
                    let s = cx.new(|cx| {
                        ConnectSheet::fixture(
                            store.clone(),
                            case.provider,
                            case.choice,
                            case.flow.clone(),
                            case.fields,
                            window,
                            cx,
                        )
                    });
                    *slot.borrow_mut() = Some(s.clone());
                    d.connect = Some(s);
                    d
                })
            })
            .expect("open sheet");
        let sheet = sheet.borrow().clone().expect("sheet");
        // Focus the credential field, as when typing a long path: its end
        // must scroll into view. Focus stays inside this offscreen window.
        let focus = cx.update(|cx| sheet.read(cx).credential_input().focus_handle(cx));
        let _ = cx.update_window(handle.into(), |_, window, cx| window.focus(&focus, cx));
        cx.run_until_parked();
        // Twice: scroll and caret offsets settle from the first frame's layout.
        for _ in 0..2 {
            let _ = cx.update_window(handle.into(), |_, window, _| window.refresh());
            cx.run_until_parked();
        }
        let img = cx.capture_screenshot(handle.into()).expect("capture sheet");
        let name = case.name;
        not_blank(&img, name);
        save(&img, &format!("{name}-{suffix}"));

        let scale = img.width() as f32 / w;
        let sw = (w - 48.).min(SHEET_MAX_WIDTH);
        let x0 = (w - sw) / 2.;
        let at = |v: f32| (v * scale) as u32;
        // Sheet extent from a column inside its left padding.
        let col = at(x0 + 10.);
        let top = (0..img.height())
            .find(|&y| near(img.get_pixel(col, y).0, t.surface, 2.))
            .unwrap_or_else(|| panic!("{name}: sheet surface not found"));
        let bottom = (0..img.height())
            .rev()
            .find(|&y| near(img.get_pixel(col, y).0, t.inset, 2.))
            .unwrap_or_else(|| panic!("{name}: pinned footer not found"));
        assert!(
            bottom < img.height() - at(16.),
            "{name}: footer runs off the window ({bottom} of {})",
            img.height()
        );
        // The primary action is drawn inside the footer, at its right end.
        let band = bottom.saturating_sub(at(48.))..bottom;
        let (pa, pb) = t.primary();
        let primary = band
            .clone()
            .flat_map(|y| (at(x0 + sw / 2.)..at(x0 + sw)).map(move |x| (x, y)))
            .filter(|&(x, y)| on_line(img.get_pixel(x, y).0, pa, pb, 6.))
            .count();
        assert!(
            primary > (40. * scale * scale) as usize,
            "{name}: primary button not visible in the footer"
        );
        // Footer buttons start where the first non-footer pixel appears on
        // the footer's middle row.
        let footer_top = (0..bottom)
            .rev()
            .find(|&y| !near(img.get_pixel(col, y).0, t.inset, 2.))
            .expect("footer top");
        let mid = (footer_top + bottom) / 2;
        let left = (col..at(x0 + sw))
            .find(|&x| !near(img.get_pixel(x, mid).0, t.inset, 3.))
            .expect("footer buttons");
        actions_left.insert(name, left as f32 / scale);
        // A step 1 error shows on arrival, pinned above the footer, without
        // scrolling.
        if case.flow.step == 1 && case.flow.error.is_some() {
            let band = footer_top.saturating_sub(at(90.))..footer_top;
            let inner = at(x0 + 30.)..at(x0 + sw - 30.);
            let error = band
                .flat_map(|y| inner.clone().map(move |x| (x, y)))
                .filter(|&(x, y)| near(img.get_pixel(x, y).0, t.tag_ember, 2.))
                .count();
            assert!(
                error > (400. * scale * scale) as usize,
                "{name}: error not visible on arrival ({error} px)"
            );
        }
        // Side padding holds only the sheet's two tonal fills: any text,
        // field or card crossing it shows up as another colour.
        for x in [at(x0 + 10.), at(x0 + sw - 10.)] {
            for y in top + at(18.)..bottom - at(18.) {
                let p = img.get_pixel(x, y).0;
                assert!(
                    on_line(p, t.surface, t.inset, 3.),
                    "{name}: content crosses the sheet padding at ({x},{y}): {p:?}"
                );
            }
        }
        let (overflow, cred_scroll, cred_width) = cx.update(|cx| {
            let s = sheet.read(cx);
            let input = s.credential_input().read(cx);
            (s.body_overflow(), input.scroll_x(), input.text_width())
        });
        assert_eq!(
            overflow > px(0.),
            case.expect_scroll,
            "{name}: body overflow {overflow:?}"
        );
        if case.expect_scroll {
            check_scrolled_to_end(cx, handle.into(), &sheet, t, (x0, sw, scale), name, suffix);
        }
        // A long credential scrolls so its end (and the caret) is visible.
        if case.flow.step == 1 && case.fields[2].len() > 40 {
            let width = cred_width.expect("credential laid out");
            assert!(
                cred_scroll > px(0.),
                "{name}: long credential didn't scroll"
            );
            assert!(
                width - cred_scroll <= px(sw - 48. - 24.),
                "{name}: credential end is out of view"
            );
            // IME geometry for the last character must land inside the
            // field's text area (sheet padding 24 + field padding 12), where
            // it is drawn, not where it would be unscrolled.
            let len = case.fields[2].len();
            let input = cx.update(|cx| sheet.read(cx).credential_input().clone());
            let ime = cx
                .update_window(handle.into(), |_, window, cx| {
                    input.update(cx, |i, cx| {
                        i.bounds_for_range(len - 1..len, gpui::Bounds::default(), window, cx)
                    })
                })
                .expect("window")
                .expect("ime bounds");
            let (lo, hi) = (px(x0 + 36. - 1.), px(x0 + sw - 36. + 1.));
            assert!(
                ime.left() >= lo && ime.right() <= hi,
                "{name}: IME bounds {ime:?} outside the field text area {lo:?}..{hi:?}"
            );
        }
        results.push(Checked {
            name,
            width: img.width(),
            height: img.height(),
        });
        let _ = cx.update_window(handle.into(), |_, window, _| window.remove_window());
    }
    // Reconnect isn't an edit flow: same width, same step, but no Back, so
    // its buttons start at least a Back button's width further right.
    let (edit, reconnect) = (
        actions_left["sheet-openai-setup-narrow"],
        actions_left["sheet-reconnect-error-narrow"],
    );
    assert!(
        reconnect - edit >= 50.,
        "reconnect footer still shows Back ({edit} vs {reconnect})"
    );
}

/// Scrolls a short window's sheet body to its end and checks that the last
/// content (fields, plus the pinned step 1 error) is in view above the pinned
/// footer, whose primary action stays in the window.
fn check_scrolled_to_end(
    cx: &mut HeadlessAppContext,
    window: gpui::AnyWindowHandle,
    sheet: &gpui::Entity<ConnectSheet>,
    t: &Theme,
    (x0, sw, scale): (f32, f32, f32),
    name: &str,
    suffix: &str,
) {
    cx.update(|cx| sheet.read(cx).scroll_body_to_end());
    let _ = cx.update_window(window, |_, window, _| window.refresh());
    cx.run_until_parked();
    let img = cx
        .capture_screenshot(window)
        .expect("capture scrolled sheet");
    save(&img, &format!("{name}-scrolled-{suffix}"));
    let at = |v: f32| (v * scale) as u32;
    let col = at(x0 + 10.);
    let footer_bottom = (0..img.height())
        .rev()
        .find(|&y| near(img.get_pixel(col, y).0, t.inset, 2.))
        .expect("footer after scrolling");
    let footer_top = (0..footer_bottom)
        .rev()
        .find(|&y| !near(img.get_pixel(col, y).0, t.inset, 2.))
        .expect("footer top");
    assert!(
        footer_bottom < img.height() - at(16.),
        "{name}: footer left the window"
    );
    let (pa, pb) = t.primary();
    let xs = at(x0 + sw / 2.)..at(x0 + sw);
    let primary = (footer_top..footer_bottom)
        .flat_map(|y| xs.clone().map(move |x| (x, y)))
        .filter(|&(x, y)| on_line(img.get_pixel(x, y).0, pa, pb, 6.))
        .count();
    assert!(
        primary > (40. * scale * scale) as usize,
        "{name}: primary action hidden after scrolling"
    );
    // In the visible body just above the footer, count the fills that only
    // the last content uses: field wells (track) and the error note.
    let body = footer_top.saturating_sub(at(220.))..footer_top;
    let inner = at(x0 + 30.)..at(x0 + sw - 30.);
    let count = |c: gpui::Rgba| {
        body.clone()
            .flat_map(|y| inner.clone().map(move |x| (x, y)))
            .filter(|&(x, y)| near(img.get_pixel(x, y).0, c, 2.))
            .count()
    };
    let min = (400. * scale * scale) as usize;
    if name.contains("key") {
        // Step 1 ends with the credential field; the save error is pinned
        // just above the footer.
        let error = count(t.tag_ember);
        assert!(
            error > min,
            "{name}: scoped error not reachable ({error} px)"
        );
        let wells = count(t.track);
        assert!(wells > min, "{name}: fields not reachable ({wells} px)");
    } else {
        // Step 2 ends with the amber "Not yet tested live" note.
        let amber = body
            .clone()
            .flat_map(|y| inner.clone().map(move |x| (x, y)))
            .filter(|&(x, y)| near(img.get_pixel(x, y).0, t.amber, 40.))
            .count();
        assert!(
            amber > (20. * scale * scale) as usize,
            "{name}: last note not reachable"
        );
    }
}
