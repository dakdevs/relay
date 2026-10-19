//! Status item: template icon, native fallback menu (board 7) and the anchor
//! point for the rich popover. Events reach the UI through an async channel,
//! so nothing polls while Relay is idle.

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use gpui::{Bounds, Pixels, Point, point, px};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{
        Menu, MenuEvent, MenuItem, PredefinedMenuItem,
        accelerator::{Accelerator, Code, Modifiers},
    },
};

use crate::assets::Assets;

/// What the status item asks the app to do.
#[derive(Clone, Debug, PartialEq)]
pub enum TrayAction {
    /// Primary click: toggle the popover, anchored below this point (logical, global).
    Toggle {
        anchor: Option<Point<Pixels>>,
        item_height: f32,
    },
    OpenRelay,
    TogglePause,
    Settings,
    Quit,
}

pub const ID_OPEN: &str = "relay.open";
pub const ID_PAUSE: &str = "relay.pause";
pub const ID_SETTINGS: &str = "relay.settings";
pub const ID_QUIT: &str = "relay.quit";

/// Rasterises the tray glyph to straight RGBA at `size` × `size` pixels.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let svg = Assets::get("icons/tray.svg").expect("tray glyph embedded");
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default())
        .expect("tray glyph parses");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("non-zero icon size");
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia stores premultiplied alpha; the tray APIs expect straight alpha.
    pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

/// One line per menu row, in board 7 order. Status rows are disabled text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuModel {
    pub status: String,
    /// (text, indented) — group summaries and the accounts in use under them.
    pub rows: Vec<(String, bool)>,
    pub paused: bool,
}

pub struct Tray {
    icon: TrayIcon,
    menu_model: MenuModel,
    pause_item: MenuItem,
}

impl Tray {
    /// Must run on the main thread after the platform event loop has started
    /// (inside GPUI's `Application::run` callback).
    pub fn create(model: MenuModel) -> anyhow::Result<(Self, UnboundedReceiver<TrayAction>)> {
        let (tx, rx) = unbounded();
        let (menu, pause_item) = build_menu(&model);
        let size = if cfg!(target_os = "macos") { 36 } else { 44 };
        let icon = Icon::from_rgba(icon_rgba(size), size, size)?;
        let builder = TrayIconBuilder::new()
            .with_tooltip("Relay")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false);
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);
        let icon = builder.build()?;

        install_handlers(tx);
        Ok((
            Self {
                icon,
                menu_model: model,
                pause_item,
            },
            rx,
        ))
    }

    /// Rebuilds the native menu only when its text changed.
    pub fn update_menu(&mut self, model: MenuModel) {
        if model == self.menu_model {
            return;
        }
        if model.rows == self.menu_model.rows && model.status == self.menu_model.status {
            self.pause_item.set_text(pause_label(model.paused));
        } else {
            let (menu, pause_item) = build_menu(&model);
            self.icon.set_menu(Some(Box::new(menu)));
            self.pause_item = pause_item;
        }
        self.menu_model = model;
    }

    pub fn set_tooltip(&self, text: &str) {
        let _ = self.icon.set_tooltip(Some(text));
    }
}

fn pause_label(paused: bool) -> &'static str {
    if paused {
        "Resume routing"
    } else {
        "Pause routing"
    }
}

fn build_menu(model: &MenuModel) -> (Menu, MenuItem) {
    let primary = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    let menu = Menu::new();
    let status = MenuItem::new(&model.status, false, None);
    let _ = menu.append(&status);
    for (text, indented) in &model.rows {
        let text = if *indented {
            format!("    {text}")
        } else {
            text.clone()
        };
        let _ = menu.append(&MenuItem::new(text, false, None));
    }
    let _ = menu.append(&PredefinedMenuItem::separator());
    let open = MenuItem::with_id(
        ID_OPEN,
        "Open Relay",
        true,
        Some(Accelerator::new(primary, Code::KeyO)),
    );
    let pause = MenuItem::with_id(ID_PAUSE, pause_label(model.paused), true, None);
    let settings = MenuItem::with_id(
        ID_SETTINGS,
        "Settings…",
        true,
        Some(Accelerator::new(primary, Code::Comma)),
    );
    let quit = MenuItem::with_id(
        ID_QUIT,
        "Quit Relay",
        true,
        Some(Accelerator::new(primary, Code::KeyQ)),
    );
    let _ = menu.append(&open);
    let _ = menu.append(&pause);
    let _ = menu.append(&settings);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);
    (menu, pause)
}

fn install_handlers(tx: UnboundedSender<TrayAction>) {
    let menu_tx = tx.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let action = match event.id().0.as_str() {
            ID_OPEN => TrayAction::OpenRelay,
            ID_PAUSE => TrayAction::TogglePause,
            ID_SETTINGS => TrayAction::Settings,
            ID_QUIT => TrayAction::Quit,
            _ => return,
        };
        let _ = menu_tx.unbounded_send(action);
    }));
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let Some(action) = click_action(&event) {
            let _ = tx.unbounded_send(action);
        }
    }));
}

/// Maps a raw tray event to an action. Only a completed primary click toggles
/// the popover; moves, enters and right-clicks (native menu) are ignored here.
pub fn click_action(event: &TrayIconEvent) -> Option<TrayAction> {
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            rect,
            position,
            ..
        } => Some(TrayAction::Toggle {
            item_height: rect.size.height as f32,
            anchor: (rect.size.width > 0)
                .then(|| {
                    point(
                        px(rect.position.x as f32 + rect.size.width as f32 / 2.0),
                        px(rect.position.y as f32 + rect.size.height as f32),
                    )
                })
                .or_else(|| {
                    (position.x != 0.0 || position.y != 0.0)
                        .then(|| point(px(position.x as f32), px(position.y as f32)))
                }),
        }),
        _ => None,
    }
}

/// Converts a physical anchor (status item bottom-centre, or the click point on
/// Linux) to the popover's logical origin. The scale is the one for which the
/// status item is menu-bar height and the point lies inside a display
/// (global logical bounds). The popover
/// is horizontally centred on the anchor and clamped 8 px inside that display.
/// Without an anchor (hosts that send zero coordinates) it goes top-right of
/// the first display.
pub fn popover_origin(
    anchor_physical: Option<Point<Pixels>>,
    item_height: f32,
    displays: &[Bounds<Pixels>],
    width: Pixels,
) -> Point<Pixels> {
    let margin = px(8.);
    let gap = px(6.);
    let first = displays.first().copied().unwrap_or_default();
    let found = anchor_physical.and_then(|a| {
        // A macOS status item is one menu bar tall (about 22–44 pt), which
        // disambiguates scales across mixed-density displays.
        let scales: &[f32] = if item_height > 0.0 {
            &[2.0, 1.0, 3.0, 1.5]
        } else {
            &[1.0, 2.0]
        };
        scales
            .iter()
            .filter(|s| item_height == 0.0 || (18.0..=48.0).contains(&(item_height / **s)))
            .find_map(|s| {
                let p = point(a.x / *s, a.y / *s);
                displays
                    .iter()
                    .find(|d| {
                        d.contains(&p) || (p.y == d.bottom() && p.x >= d.left() && p.x < d.right())
                    })
                    .map(|d| (p, *d))
            })
    });
    match found {
        Some((p, d)) => {
            let x = (p.x - width / 2.0)
                .max(d.left() + margin)
                .min(d.right() - width - margin);
            point(x, p.y + gap)
        }
        None => point(first.right() - width - margin, first.top() + px(32.)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Bounds, size};
    use tray_icon::{
        Rect, TrayIconId,
        dpi::{PhysicalPosition, PhysicalSize},
    };

    fn display(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(w), px(h)),
        }
    }

    #[test]
    fn icon_is_opaque_glyph_on_transparent_background() {
        let rgba = icon_rgba(36);
        assert_eq!(rgba.len(), 36 * 36 * 4);
        let alpha = |x: usize, y: usize| rgba[(y * 36 + x) * 4 + 3];
        assert_eq!(alpha(0, 0), 0, "corner must be transparent");
        // Source node at (3.6, 9) of an 18 viewBox → (7, 18) at 2×.
        assert!(alpha(7, 18) > 200, "source node must be drawn");
        // Template icons must be monochrome black.
        assert!(
            rgba.chunks(4)
                .filter(|p| p[3] > 0)
                .all(|p| p[0] < 10 && p[1] < 10 && p[2] < 10)
        );
    }

    #[test]
    fn retina_status_item_anchor_centres_and_clamps_to_screen() {
        // Observed on a 2× MacBook: status item at physical x 2632, 76 wide, 78 tall.
        let displays = [display(0., 0., 1512., 982.)];
        // Mid-bar item: centred, below the 39 pt menu bar plus a 6 pt gap.
        let o = popover_origin(Some(point(px(1600.), px(78.))), 78., &displays, px(360.));
        assert_eq!(o, point(px(800. - 180.), px(45.)));
        // That item (centre 1335 pt) is near the right edge: clamps 8 pt inside.
        let o = popover_origin(
            Some(point(px(2632. + 38.), px(78.))),
            78.,
            &displays,
            px(360.),
        );
        assert_eq!(o.x, px(1512. - 360. - 8.));
    }

    #[test]
    fn second_display_and_missing_coordinates() {
        let displays = [
            display(0., 0., 1512., 982.),
            display(1512., 0., 1920., 1080.),
        ];
        // 1× external display to the right: physical == logical. At 2× the
        // point would land on the laptop screen, but a 12 pt status item is
        // impossible, so 1× wins.
        let o = popover_origin(Some(point(px(3000.), px(24.))), 24., &displays, px(360.));
        assert_eq!(o, point(px(3000. - 180.), px(30.)));
        let o = popover_origin(None, 0., &displays, px(360.));
        assert_eq!(o, point(px(1512. - 368.), px(32.)));
    }

    fn click(
        button: MouseButton,
        state: MouseButtonState,
        rect: Rect,
        pos: (f64, f64),
    ) -> TrayIconEvent {
        TrayIconEvent::Click {
            id: TrayIconId::new("t"),
            position: PhysicalPosition::new(pos.0, pos.1),
            rect,
            button,
            button_state: state,
        }
    }

    #[test]
    fn only_primary_release_toggles() {
        let rect = Rect {
            size: PhysicalSize::new(76, 78),
            position: PhysicalPosition::new(2632., 0.),
        };
        assert_eq!(
            click_action(&click(
                MouseButton::Left,
                MouseButtonState::Up,
                rect,
                (2640., 20.)
            )),
            Some(TrayAction::Toggle {
                anchor: Some(point(px(2670.), px(78.))),
                item_height: 78.
            })
        );
        assert_eq!(
            click_action(&click(
                MouseButton::Left,
                MouseButtonState::Down,
                rect,
                (1., 1.)
            )),
            None
        );
        assert_eq!(
            click_action(&click(
                MouseButton::Right,
                MouseButtonState::Up,
                rect,
                (1., 1.)
            )),
            None
        );
        // Linux SNI: zero-size rect, click point is the anchor; zero coordinates mean unknown.
        let none = Rect::default();
        assert_eq!(
            click_action(&click(
                MouseButton::Left,
                MouseButtonState::Up,
                none,
                (1800., 30.)
            )),
            Some(TrayAction::Toggle {
                anchor: Some(point(px(1800.), px(30.))),
                item_height: 0.
            })
        );
        assert_eq!(
            click_action(&click(
                MouseButton::Left,
                MouseButtonState::Up,
                none,
                (0., 0.)
            )),
            Some(TrayAction::Toggle {
                anchor: None,
                item_height: 0.
            })
        );
    }
}
