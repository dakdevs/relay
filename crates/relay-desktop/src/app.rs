//! Application lifecycle: status item, popover window and main window.
//! GPUI owns the main thread; the proxy runs on its own runtime (bridge.rs)
//! and reaches the UI only as published snapshots.

use futures::StreamExt;
use gpui::{
    App, AppContext, BorrowAppContext, Bounds, Entity, Pixels, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions, point, px,
    size,
};

use crate::{
    assets::Assets,
    bridge,
    dashboard::{self, Dashboard},
    popover::{self, Popover, PopoverEvent},
    store::{Store, Tab},
    theme::{POPOVER_WIDTH, Theme},
    tray::{self, Tray, TrayAction},
    widgets::Ui,
};

/// Window handles and the status item, kept on the main thread.
pub struct Shell {
    store: Entity<Store>,
    popover: Option<WindowHandle<Popover>>,
    main: Option<WindowHandle<Dashboard>>,
    tray: Option<Tray>,
}

impl gpui::Global for Shell {}

pub fn run() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            cx.set_global(Ui {
                theme: Theme::for_appearance(cx.window_appearance()),
                reduced_motion: false,
                preference: relay_core::config::Theme::System,
            });
            // Read the OS reduced-motion preference off the UI thread.
            cx.spawn(async |cx| {
                let reduced = cx
                    .background_executor()
                    .spawn(async { crate::motion::system_prefers_reduced_motion() })
                    .await;
                cx.update(|cx| cx.update_global::<Ui, _>(|ui, _| ui.reduced_motion = reduced));
            })
            .detach();
            #[cfg(target_os = "macos")]
            cx.set_activation_policy(gpui::ActivationPolicy::Accessory);
            cx.set_quit_mode(gpui::QuitMode::Explicit);
            popover::bind_keys(cx);
            dashboard::bind_keys(cx);

            let store = bridge::start(cx);
            let menu = { store.read(cx).overview.menu(store.read(cx).paused) };
            let tray = match Tray::create(menu) {
                Ok((tray, mut rx)) => {
                    cx.spawn(async move |cx| {
                        while let Some(action) = rx.next().await {
                            cx.update(|cx| handle(action, cx));
                        }
                    })
                    .detach();
                    Some(tray)
                }
                Err(error) => {
                    // No status item host (e.g. GNOME without the AppIndicator
                    // extension): keep running and show the main window instead.
                    eprintln!("relay: status item unavailable ({error}); opening the main window");
                    None
                }
            };
            let no_tray = tray.is_none();
            cx.set_global(Shell {
                store: store.clone(),
                popover: None,
                main: None,
                tray,
            });
            // Keep the native menu text in step with snapshots (only when it changes).
            cx.observe(&store, |store, cx| {
                let (menu, tooltip) = {
                    let s = store.read(cx);
                    (s.overview.menu(s.paused), s.overview.menu(s.paused).status)
                };
                cx.update_global::<Shell, _>(|shell, _| {
                    if let Some(tray) = shell.tray.as_mut() {
                        tray.update_menu(menu);
                        tray.set_tooltip(&tooltip);
                    }
                });
            })
            .detach();
            if no_tray {
                open_main(Tab::Quotas, cx);
            }
        });
}

pub fn handle(action: TrayAction, cx: &mut App) {
    match action {
        TrayAction::Toggle {
            anchor,
            item_height,
        } => toggle_popover(anchor, item_height, cx),
        TrayAction::OpenRelay => open_main(Tab::Quotas, cx),
        TrayAction::Settings => open_main(Tab::Routing, cx),
        TrayAction::TogglePause => {
            let store = cx.global::<Shell>().store.clone();
            store.update(cx, |s, cx| {
                s.toggle_paused();
                cx.notify();
            });
        }
        TrayAction::Quit => {
            let store = cx.global::<Shell>().store.clone();
            // The bridge drains in-flight requests (≤10 s), then replies
            // `Stopped`, which quits. The fallback covers a dead bridge.
            if store.read(cx).send(crate::store::Command::Quit).is_err() {
                // No runtime to drain: quit now.
                cx.quit();
                return;
            }
            cx.spawn(async |cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(12))
                    .await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
    }
}

fn displays(cx: &App) -> Vec<Bounds<Pixels>> {
    cx.displays().iter().map(|d| d.bounds()).collect()
}

fn toggle_popover(anchor: Option<gpui::Point<Pixels>>, item_height: f32, cx: &mut App) {
    if let Some(handle) = cx.global::<Shell>().popover {
        // A click on the icon while open closes it. The same click usually
        // deactivates the popover first (closing already), so a click during
        // a close lets it finish instead of reversing it.
        let _ = handle.update(cx, |p, _, cx| p.close(cx));
        return;
    }
    let displays = displays(cx);
    let origin = tray::popover_origin(anchor, item_height, &displays, px(POPOVER_WIDTH));
    let screen = displays
        .iter()
        .find(|d| d.contains(&origin))
        .copied()
        .unwrap_or_else(|| displays.first().copied().unwrap_or_default());
    let available = f32::from(screen.bottom() - origin.y) - 12.0;
    let margin = px(popover::SHADOW_MARGIN);
    let bounds = Bounds {
        origin: point(origin.x - margin, origin.y - margin),
        size: popover::window_size(420.0, available),
    };
    let store = cx.global::<Shell>().store.clone();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        kind: WindowKind::PopUp,
        is_movable: false,
        focus: true,
        show: true,
        window_background: WindowBackgroundAppearance::Transparent,
        app_id: Some("dev.dak.relay".into()),
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| Popover::new(store, window, cx));
        window
            .subscribe(
                &view,
                cx,
                move |_, event: &PopoverEvent, window, cx| match event {
                    PopoverEvent::Closed => {
                        cx.update_global::<Shell, _>(|shell, _| shell.popover = None);
                        window.remove_window();
                        // Return focus to the previous app unless our main window is up.
                        if cx.global::<Shell>().main.is_none() {
                            cx.hide();
                        }
                    }
                    PopoverEvent::OpenRelay => open_main_deferred(Tab::Quotas, window, cx),
                    PopoverEvent::OpenSettings => open_main_deferred(Tab::Routing, window, cx),
                    PopoverEvent::Resize(h) => window.resize(popover::window_size(*h, available)),
                },
            )
            .detach();
        view
    });
    match opened {
        Ok(handle) => {
            cx.update_global::<Shell, _>(|shell, _| shell.popover = Some(handle));
            cx.activate(true);
        }
        Err(error) => eprintln!("relay: could not open the popover: {error}"),
    }
}

fn open_main_deferred(tab: Tab, window: &mut Window, cx: &mut App) {
    if let Some(handle) = cx.global::<Shell>().popover {
        let _ = handle.update(cx, |p, _, cx| p.close(cx));
    }
    let _ = window;
    cx.defer(move |cx| open_main(tab, cx));
}

/// Opens or focuses the main window on `tab`.
pub fn open_main(tab: Tab, cx: &mut App) {
    let store = cx.global::<Shell>().store.clone();
    store.update(cx, |s, cx| {
        s.tab = tab;
        cx.notify();
    });
    if let Some(handle) = cx.global::<Shell>().main
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        cx.activate(true);
        return;
    }
    let displays = displays(cx);
    let screen = displays.first().copied().unwrap_or_default();
    // Board 3 is drawn at 1340 pt; smaller screens get what fits.
    let size = size(px(1340.), px(880.)).min(&(screen.size - size(px(40.), px(80.))));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered_at(
            screen.center(),
            size,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("Relay".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(16.), px(18.))),
        }),
        window_min_size: Some(gpui::size(px(1000.), px(560.))),
        focus: true,
        show: true,
        app_id: Some("dev.dak.relay".into()),
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| Dashboard::new(store, window, cx));
        window.on_window_should_close(cx, |_, cx| {
            cx.update_global::<Shell, _>(|shell, _| shell.main = None);
            true
        });
        view
    });
    match opened {
        Ok(handle) => {
            cx.update_global::<Shell, _>(|shell, _| shell.main = Some(handle));
            cx.activate(true);
        }
        Err(error) => eprintln!("relay: could not open the main window: {error}"),
    }
}

/// Light and dark follow the system; every window re-renders on change.
pub fn watch_appearance<T: 'static>(
    window: &mut Window,
    cx: &mut gpui::Context<T>,
) -> gpui::Subscription {
    cx.observe_window_appearance(window, |_, window, cx| {
        let appearance = window.appearance();
        cx.update_global::<Ui, _>(|ui, _| ui.theme = Ui::resolve(ui.preference, appearance));
        cx.refresh_windows();
    })
}

/// While a window is open, asks the bridge to re-evaluate relative times every
/// 30 s. The returned task is owned by the view, so nothing runs when hidden.
pub fn keep_times_fresh<T: 'static>(
    store: &gpui::Entity<Store>,
    cx: &mut gpui::Context<T>,
) -> gpui::Task<()> {
    let store = store.clone();
    // Best effort: a missing runtime is already reported elsewhere.
    let _ = store.read(cx).send(crate::store::Command::Republish);
    cx.spawn(async move |_, cx| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(30))
                .await;
            // Dropping the owning view drops this task, ending the loop.
            cx.update(|cx| {
                let _ = store.read(cx).send(crate::store::Command::Republish);
            });
        }
    })
}
