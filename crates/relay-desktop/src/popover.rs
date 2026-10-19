//! Tray popover (boards 1, 2, 6): provider sections, plan groups with one bar
//! per window, expandable account cards, pinned Through Relay footer with
//! Pause, Settings and Open Relay. Height follows content up to the screen;
//! the list scrolls under the pinned footer.

use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, BoxShadow, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, InteractiveElement, IntoElement, KeyBinding, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Transformation, Window, actions, canvas, div,
    point, prelude::*, px, radians, size, svg,
};

use crate::{
    format,
    model::{Account, Group, GroupWindow, Overview, Section, State, Status},
    motion::{self, Tween},
    store::Store,
    theme::{POPOVER_WIDTH, Theme, radius},
    through,
    widgets::{self, Button, ButtonKind, Tone, caps, ui},
};

actions!(
    popover,
    [
        Close,
        OpenRelay,
        OpenSettings,
        FocusNext,
        FocusPrev,
        TogglePause
    ]
);

/// Transparent margin around the panel so its own soft shadow is visible.
pub const SHADOW_MARGIN: f32 = 14.;

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Popover");
    cx.bind_keys([
        KeyBinding::new("escape", Close, ctx),
        KeyBinding::new("tab", FocusNext, ctx),
        KeyBinding::new("shift-tab", FocusPrev, ctx),
        KeyBinding::new("down", FocusNext, ctx),
        KeyBinding::new("up", FocusPrev, ctx),
        KeyBinding::new("secondary-o", OpenRelay, ctx),
        KeyBinding::new("secondary-,", OpenSettings, ctx),
    ]);
}

#[derive(Clone, Debug, PartialEq)]
pub enum PopoverEvent {
    /// The close animation finished; remove the window.
    Closed,
    OpenRelay,
    OpenSettings,
    /// Content height changed; resize the window to this panel height.
    Resize(f32),
}

pub struct Popover {
    store: Entity<Store>,
    focus: FocusHandle,
    presence: Tween,
    closing: bool,
    /// Expansion progress per group key.
    expand: HashMap<String, Tween>,
    /// Natural heights of each group's account-card block, measured on paint.
    card_heights: Rc<std::cell::RefCell<HashMap<String, f32>>>,
    /// Natural heights of header, list content and footer, measured on paint.
    parts: Rc<[Cell<f32>; 3]>,
    reported_height: f32,
    _subscriptions: [Subscription; 3],
    _tick: gpui::Task<()>,
}

impl EventEmitter<PopoverEvent> for Popover {}

impl Focusable for Popover {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Popover {
    pub fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let reduced = ui(cx).reduced_motion;
        let mut presence = Tween::settled(0.0);
        let open = if reduced {
            motion::REDUCED_FADE
        } else {
            motion::POPOVER_OPEN
        };
        presence.retarget(1.0, open, Instant::now(), false);
        let expanded: Vec<String> = store.read(cx).expanded.iter().cloned().collect();
        let expand = expanded
            .into_iter()
            .map(|k| (k, Tween::settled(1.0)))
            .collect();
        let subscriptions = [
            cx.observe(&store, |_, _, cx| cx.notify()),
            // Click outside (the window loses key status) closes the popover.
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.close(cx);
                }
            }),
            crate::app::watch_appearance(window, cx),
        ];
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let tick = crate::app::keep_times_fresh(&store, cx);
        Self {
            store,
            focus,
            presence,
            closing: false,
            expand,
            card_heights: Rc::default(),
            parts: Rc::default(),
            reported_height: 0.0,
            _tick: tick,
            _subscriptions: subscriptions,
        }
    }

    /// Skips the open animation (offscreen renders and reduced-motion tests).
    pub fn settled(mut self) -> Self {
        self.presence = Tween::settled(1.0);
        self
    }

    /// Natural panel height measured on the last paint.
    pub fn content_height(&self) -> f32 {
        self.parts.iter().map(Cell::get).sum::<f32>() + 16.0
    }

    /// Starts the close animation from the current presentation (interruptible).
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.closing = true;
        let reduced = ui(cx).reduced_motion;
        let d = if reduced {
            motion::REDUCED_FADE
        } else {
            motion::POPOVER_CLOSE
        };
        self.presence.retarget(0.0, d, Instant::now(), false);
        cx.notify();
    }

    pub fn is_closing(&self) -> bool {
        self.closing
    }

    fn toggle_group(&mut self, key: &str, cx: &mut Context<Self>) {
        let expanded = self.store.update(cx, |s, _| s.toggle_expanded(key));
        let reduced = ui(cx).reduced_motion;
        let tw = self
            .expand
            .entry(key.to_string())
            .or_insert(Tween::settled(if expanded { 0.0 } else { 1.0 }));
        tw.retarget(
            if expanded { 1.0 } else { 0.0 },
            motion::EXPAND,
            Instant::now(),
            reduced,
        );
        cx.notify();
    }
}

impl Render for Popover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = ui(cx).theme;
        let reduced = ui(cx).reduced_motion;
        let now = Instant::now();
        let p = self.presence.value_at(now);
        let mut animating =
            self.presence.is_animating(now) || self.expand.values().any(|tw| tw.is_animating(now));
        if self.closing && !self.presence.is_animating(now) {
            cx.emit(PopoverEvent::Closed);
            animating = false;
        }
        if animating {
            window.request_animation_frame();
        }
        // Report content height changes after layout (measured last frame).
        let measured = self.content_height();
        if measured > 0.0 && (measured - self.reported_height).abs() >= 1.0 {
            self.reported_height = measured;
            cx.emit(PopoverEvent::Resize(measured));
        }

        let store = self.store.read(cx);
        let overview = store.overview.clone();
        let paused = store.paused;
        let app_error = store.error(crate::store::Scope::App).map(str::to_owned);
        let expanded = store.expanded.clone();
        let drop = if reduced { 0.0 } else { -4.0 * (1.0 - p) };

        let entity = cx.entity();
        let list = div()
            .relative()
            .pb(px(8.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(match &overview.status {
                Status::NotRouting { title, detail } => {
                    Some(not_routing(title, detail, &self.store, &entity, &t))
                }
                _ => None,
            })
            .children(app_error.map(|e| {
                div()
                    .px(px(8.))
                    .text_size(px(12.5))
                    .text_color(t.ember)
                    .child(e)
            }))
            .when(overview.is_empty(), |d| d.child(empty_state(&entity, &t)))
            .children(
                overview
                    .sections
                    .iter()
                    .map(|section| self.section(section, &overview, &expanded, now, &entity, &t)),
            )
            .child(measured_by(&self.parts, 1));

        let panel = div()
            .id("popover-panel")
            .w(px(POPOVER_WIDTH))
            .max_h_full()
            .relative()
            .top(px(drop))
            .flex()
            .flex_col()
            .bg(t.popover)
            .rounded(px(radius::POPOVER))
            .shadow(vec![
                BoxShadow {
                    color: t.shadow.opacity(t.shadow.a * 1.4),
                    offset: point(px(0.), px(8.)),
                    blur_radius: px(12.),
                    spread_radius: px(-4.),
                    inset: false,
                },
                BoxShadow {
                    color: t.shadow.opacity(t.shadow.a * 0.6),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(1.),
                    spread_radius: px(0.),
                    inset: false,
                },
            ])
            .overflow_hidden()
            .child(
                div()
                    .flex_none()
                    .relative()
                    .child(header(&overview, &t))
                    .child(measured_by(&self.parts, 0)),
            )
            .child(
                div()
                    .id("popover-list")
                    .role(gpui::Role::List)
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(8.))
                    .child(list),
            )
            .child(
                div()
                    .flex_none()
                    .relative()
                    .px(px(8.))
                    .pb(px(8.))
                    .child(self.footer(paused, window, cx))
                    .child(measured_by(&self.parts, 2)),
            );

        div()
            .id("popover-root")
            .key_context("Popover")
            .track_focus(&self.focus)
            .role(gpui::Role::Dialog)
            .aria_label("Relay")
            .on_action(cx.listener(|this, _: &Close, _, cx| this.close(cx)))
            .on_action(cx.listener(|_, _: &FocusNext, w, cx| w.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrev, w, cx| w.focus_prev(cx)))
            .on_action(cx.listener(|_, _: &OpenRelay, _, cx| cx.emit(PopoverEvent::OpenRelay)))
            .on_action(
                cx.listener(|_, _: &OpenSettings, _, cx| cx.emit(PopoverEvent::OpenSettings)),
            )
            .size_full()
            .p(px(SHADOW_MARGIN))
            .opacity(p)
            .text_color(t.text)
            .child(panel)
    }
}

/// Records the natural height of its parent (which must be `relative`) into
/// `parts[i]` on each paint. The list content lives inside the scroller, so
/// its height does not depend on the window's current height.
fn measured_by(parts: &Rc<[Cell<f32>; 3]>, i: usize) -> impl IntoElement {
    let parts = parts.clone();
    canvas(
        move |bounds, window, _| {
            let h = f32::from(bounds.size.height);
            // Content changed size: draw again so the window follows on the
            // next frame. Sub-pixel jitter is ignored, so this settles.
            if (h - parts[i].get()).abs() >= 1.0 {
                parts[i].set(h);
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

impl Popover {
    fn section(
        &self,
        section: &Section,
        overview: &Overview,
        expanded: &std::collections::HashSet<String>,
        now: Instant,
        entity: &Entity<Self>,
        t: &Theme,
    ) -> AnyElement {
        widgets::surface(t)
            .px(px(12.))
            .pt(px(12.))
            .pb(px(10.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .mb(px(6.))
                    .child(widgets::mark(section.mark, 14., t))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.muted)
                            .child(section.provider),
                    ),
            )
            .children(section.groups.iter().map(|g| {
                let progress = self
                    .expand
                    .get(&g.key)
                    .map_or(if expanded.contains(&g.key) { 1.0 } else { 0.0 }, |tw| {
                        tw.value_at(now)
                    });
                self.group(g, overview.now, progress, entity, t)
            }))
            .into_any_element()
    }

    fn group(
        &self,
        g: &Group,
        now_ms: i64,
        progress: f32,
        entity: &Entity<Self>,
        t: &Theme,
    ) -> AnyElement {
        let key = g.key.clone();
        let header_entity = entity.clone();
        let expanded = progress > 0.5;
        let stale = g.stale_count();
        let plan_windows: Vec<&GroupWindow> = g.windows.iter().filter(|w| !w.model_limit).collect();
        let shown_windows: Vec<&GroupWindow> = if progress > 0.0 {
            g.windows.iter().collect()
        } else {
            plan_windows.clone()
        };
        let summary = group_accessible_label(g, expanded);
        let count = if g.title.len() > 16 {
            g.accounts.len().to_string()
        } else {
            g.count_label.clone()
        };

        let header = div()
            .id(ElementId::Name(SharedString::from(format!("group-{key}"))))
            .role(gpui::Role::Button)
            .aria_label(SharedString::from(summary))
            .aria_expanded(expanded)
            .focusable()
            .tab_stop(true)
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(30.))
            .mx(px(-6.))
            .px(px(6.))
            .rounded(px(radius::ROW))
            .cursor_pointer()
            .hover(|s| s.bg(t.inset))
            .active(|s| s.bg(t.pressed))
            .focus_visible(|s| s.bg(t.focus))
            // → expands and ← collapses the focused group (board 8).
            .on_key_down({
                let entity = entity.clone();
                let key = g.key.clone();
                move |e, _, cx| {
                    let want = match e.keystroke.key.as_str() {
                        "right" => true,
                        "left" => false,
                        _ => return,
                    };
                    if want != expanded {
                        entity.update(cx, |this, cx| this.toggle_group(&key, cx));
                    }
                }
            })
            .on_click(move |_, _, cx| {
                header_entity.update(cx, |this, cx| this.toggle_group(&key, cx))
            })
            .child(
                svg()
                    .path("icons/chevron-right.svg")
                    .size(px(11.))
                    .flex_none()
                    .text_color(t.muted)
                    .with_transformation(Transformation::rotate(radians(
                        progress * std::f32::consts::FRAC_PI_2,
                    ))),
            )
            .children(g.mark.map(|m| widgets::mark(m, 16., t)))
            .child(
                // Long plan names truncate; counts, tags and "n stale" keep
                // their size inside the section's right inset.
                div()
                    .min_w_0()
                    .flex_shrink(1.)
                    .truncate()
                    .text_size(px(15.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(g.title.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(12.5))
                    .text_color(t.muted)
                    .child(count),
            )
            .child(widgets::tag(
                g.mode.text(),
                if g.mode == crate::model::ModeTag::Observed {
                    Tone::Neutral
                } else {
                    Tone::Green
                },
                t,
            ))
            .child(div().flex_1())
            .when(stale > 0, |d| {
                d.child(
                    div()
                        .flex_none()
                        .text_size(px(12.5))
                        .text_color(t.amber)
                        .child(format!("{stale} stale")),
                )
            });

        let windows = div()
            .flex()
            .flex_col()
            .children(shown_windows.iter().map(|w| group_window_row(w, t)));

        let in_use = g.in_use();
        let in_use_line = (!in_use.is_empty() && progress < 1.0).then(|| {
            div()
                .flex()
                .flex_col()
                .opacity(1.0 - progress)
                .children(in_use.iter().map(|(label, n)| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .pl(px(24.))
                        .h(px(22.))
                        .child(div().size(px(8.)).rounded_full().bg(t.text))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(label.to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(t.muted)
                                .child(format!("{n} in use")),
                        )
                }))
        });

        let heights = self.card_heights.clone();
        let natural = heights.borrow().get(&g.key).copied().unwrap_or(0.0);
        let cards_key = g.key.clone();
        let cards = (progress > 0.0).then(|| {
            div()
                .overflow_hidden()
                .when(progress < 1.0, |d| d.h(px(natural * progress)))
                .child(
                    div()
                        .relative()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .pt(px(6.))
                        .pl(px(18.))
                        .children(g.accounts.iter().map(|a| account_card(a, now_ms, t)))
                        .child(
                            canvas(
                                move |bounds, _, _| {
                                    heights
                                        .borrow_mut()
                                        .insert(cards_key, f32::from(bounds.size.height));
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full(),
                        ),
                )
        });

        div()
            .flex()
            .flex_col()
            .pb(px(4.))
            .child(header)
            .child(windows)
            .children(in_use_line)
            .children(cards)
            .into_any_element()
    }

    fn footer(&self, paused: bool, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = ui(cx).theme;
        let store = self.store.clone();
        let through = through::compact(&self.store, window, cx);
        let pause_store = store.clone();
        widgets::surface(&t)
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(through)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new(
                            "pause",
                            ButtonKind::Icon,
                            if paused {
                                "Resume routing"
                            } else {
                                "Pause routing"
                            },
                        )
                        .icon(if paused {
                            "icons/play.svg"
                        } else {
                            "icons/pause.svg"
                        })
                        .on_click(move |_, _, cx| {
                            pause_store.update(cx, |s, cx| {
                                s.toggle_paused();
                                cx.notify();
                            })
                        }),
                    )
                    .child(
                        Button::new("settings", ButtonKind::Icon, "Settings")
                            .icon("icons/gear.svg")
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(PopoverEvent::OpenSettings)),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new(
                            "open-relay",
                            ButtonKind::Primary,
                            "Open Relay, opens the main window on Quotas",
                        )
                        .label("Open Relay")
                        .trailing("icons/arrow-up-right.svg")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(PopoverEvent::OpenRelay))),
                    ),
            )
            .into_any_element()
    }
}

fn header(o: &Overview, t: &Theme) -> impl IntoElement {
    let (label, tone) = match &o.status {
        Status::Starting => ("Starting", Tone::Neutral),
        Status::Running => ("Running", Tone::Green),
        Status::Paused => ("Paused", Tone::Amber),
        Status::NotRouting { .. } => ("Not routing", Tone::Ember),
    };
    let (bg, fg) = match tone {
        Tone::Green => (t.tag_green, t.evergreen),
        Tone::Amber => (t.tag_amber, t.amber),
        Tone::Ember => (t.tag_ember, t.ember),
        _ => (t.tag_neutral, t.muted),
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(16.))
        .pt(px(13.))
        .pb(px(9.))
        .child(
            div()
                .text_size(px(17.))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Relay"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .px(px(8.))
                .h(px(22.))
                .rounded_full()
                .bg(bg)
                .text_color(fg)
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .child(div().size(px(6.)).rounded_full().bg(fg))
                .child(label),
        )
        .child(div().flex_1())
        .child(
            widgets::num(o.address.clone())
                .text_size(px(12.))
                .text_color(t.muted),
        )
}

fn group_window_row(w: &GroupWindow, t: &Theme) -> impl IntoElement {
    let stale_only = w.avg_left.is_none() && w.stale > 0;
    let detail = match w.avg_left {
        Some(_) => format!("avg · {} fresh", w.fresh),
        None if w.stale > 0 => format!("{} stale", w.stale),
        None => "not reported".into(),
    };
    div()
        .flex()
        .items_center()
        .h(px(24.))
        .pl(px(18.))
        .child(caps(w.label.clone(), t.muted).w(px(72.)).flex_none())
        .child(widgets::bar(w.avg_left, stale_only, 92., t))
        .child(
            widgets::num(w.avg_left.map_or("—".to_string(), format::percent))
                .w(px(47.))
                .flex_none()
                .text_right()
                .text_size(px(15.))
                .font_weight(FontWeight::MEDIUM),
        )
        .child(
            div()
                .ml(px(11.))
                .text_size(px(12.5))
                .text_color(t.muted)
                .truncate()
                .child(detail),
        )
}

fn state_tag(a: &Account, t: &Theme) -> Option<gpui::Div> {
    Some(match a.state {
        _ if a.in_use > 0 => widgets::tag(format!("● In use · {}", a.in_use), Tone::Strong, t),
        State::Exhausted => widgets::tag("! Exhausted", Tone::Ember, t),
        State::Stale => widgets::tag("◷ Stale", Tone::Amber, t),
        State::Disconnected => widgets::tag("○ Disconnected", Tone::Neutral, t),
        State::Paused => widgets::tag("Ⅱ Paused", Tone::Neutral, t),
        State::Unknown => widgets::tag("— Unknown", Tone::Neutral, t),
        State::Healthy | State::Low => return None,
    })
}

pub fn account_accessible_label(a: &Account, now_ms: i64) -> String {
    let mut s = a.label.clone();
    if !a.masked.is_empty() {
        s.push_str(&format!(", {}", a.masked));
    }
    if a.in_use > 0 {
        let unit = if a.in_use == 1 { "session" } else { "sessions" };
        s.push_str(&format!(", in use by {} {unit}", a.in_use));
    }
    for w in &a.windows {
        match w.left {
            Some(left) => {
                s.push_str(&format!(". {} {} percent left", w.label, left.floor()));
                if w.stale {
                    s.push_str(", stale, not in average");
                } else if let Some(r) = w.resets_at {
                    s.push_str(&format!(
                        ", resets in {}",
                        spoken_duration(crate::adapter::ms(r - now_ms))
                    ));
                }
            }
            None => s.push_str(&format!(". {}, not reported", w.label)),
        }
    }
    s.push('.');
    s
}

fn spoken_duration(d: Duration) -> String {
    format::duration_short(d)
        .replace('m', " minutes")
        .replace('h', " hours")
        .replace('d', " days")
}

pub fn group_accessible_label(g: &Group, expanded: bool) -> String {
    let mut s = format!(
        "{}, {}, {}",
        g.title,
        g.count_label,
        g.mode.text().to_lowercase()
    );
    for w in g.windows.iter().filter(|w| !w.model_limit) {
        match w.avg_left {
            Some(avg) => s.push_str(&format!(
                ". {} {} percent left, average of {} fresh accounts",
                w.label,
                avg.floor(),
                w.fresh
            )),
            None => s.push_str(&format!(". {}, not reported", w.label)),
        }
    }
    s.push_str(if expanded {
        ". Expanded"
    } else {
        ". Collapsed"
    });
    s
}

fn account_card(a: &Account, now_ms: i64, t: &Theme) -> impl IntoElement {
    let seen = a
        .seen_at
        .map(|s| format::seen_ago(crate::adapter::ms(now_ms - s)));
    let sub = [Some(a.masked.clone()).filter(|m| !m.is_empty()), seen]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    div()
        .id(ElementId::Name(SharedString::from(format!(
            "card-{}",
            a.id
        ))))
        .role(gpui::Role::Group)
        .aria_label(SharedString::from(account_accessible_label(a, now_ms)))
        .focusable()
        .tab_stop(true)
        .focus_visible(|s| s.bg(t.focus))
        .bg(t.inset)
        .rounded(px(10.))
        .px(px(12.))
        .pt(px(10.))
        .pb(px(10.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_start()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(a.label.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(t.muted)
                                .truncate()
                                .child(sub),
                        ),
                )
                .children(state_tag(a, t)),
        )
        .child(
            div()
                .mt(px(6.))
                .flex()
                .flex_col()
                .children(a.windows.iter().map(|w| {
                    let exhausted = w.left == Some(0.0);
                    let detail = match (w.left, w.stale, w.resets_at) {
                        (None, _, _) => "not reported".to_string(),
                        (Some(_), true, _) => "not in avg".to_string(),
                        (Some(_), false, Some(r)) => format!(
                            "resets {}",
                            format::duration_short(crate::adapter::ms(r - now_ms))
                        ),
                        (Some(_), false, None) => "reset not reported".to_string(),
                    };
                    div()
                        .flex()
                        .items_center()
                        .h(px(23.))
                        .child(caps(w.label.clone(), t.muted).w(px(66.)).flex_none())
                        .child(widgets::bar(w.left, w.stale, 66., t))
                        .child(
                            widgets::num(w.left.map_or("—".to_string(), format::percent))
                                .w(px(44.))
                                .flex_none()
                                .text_right()
                                .text_size(px(14.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if exhausted { t.ember } else { t.text }),
                        )
                        .child(
                            div()
                                .ml(px(8.))
                                .text_size(px(12.))
                                .text_color(if exhausted { t.ember } else { t.muted })
                                .truncate()
                                .child(detail),
                        )
                })),
        )
        .children(a.credits.clone().map(|c| {
            div()
                .mt(px(4.))
                .text_size(px(12.))
                .text_color(t.muted)
                .child(c)
        }))
        .children(a.note.clone().map(|n| {
            div()
                .mt(px(4.))
                .text_size(px(12.))
                .text_color(t.muted)
                .child(n)
        }))
}

fn empty_state(entity: &Entity<Popover>, t: &Theme) -> AnyElement {
    let entity = entity.clone();
    widgets::surface(t)
        .p(px(16.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child("No accounts yet"))
        .child(div().text_size(px(13.)).text_color(t.muted).child(
            "Connect a subscription or API key, or observe a T3 instance. Relay shows quota as soon as a request passes through.",
        ))
        .child(
            div().mt(px(6.)).child(
                Button::new("connect-empty", ButtonKind::Secondary, "Connect an account")
                    .label("Connect an account…")
                    .on_click(move |_, _, cx| entity.update(cx, |_, cx| cx.emit(PopoverEvent::OpenRelay))),
            ),
        )
        .into_any_element()
}

fn not_routing(
    title: &str,
    detail: &str,
    store: &Entity<Store>,
    entity: &Entity<Popover>,
    t: &Theme,
) -> AnyElement {
    let store = store.clone();
    let entity = entity.clone();
    div()
        .px(px(8.))
        .pb(px(4.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.ember)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(13.))
                .text_color(t.muted)
                .child(detail.to_string()),
        )
        .child(
            div()
                .mt(px(6.))
                .flex()
                .gap(px(8.))
                .child(
                    Button::new("retry", ButtonKind::Secondary, "Try again")
                        .label("Try again")
                        .on_click(move |_, _, cx| {
                            store.update(cx, |s, cx| {
                                s.send_or_report(
                                    crate::store::Command::Retry,
                                    crate::store::Scope::App,
                                );
                                cx.notify();
                            })
                        }),
                )
                .child(
                    Button::new(
                        "change-port",
                        ButtonKind::Secondary,
                        "Change port, opens Routing settings",
                    )
                    .label("Change port…")
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |_, cx| cx.emit(PopoverEvent::OpenSettings))
                    }),
                ),
        )
        .into_any_element()
}

/// Panel height for the window: content, clamped to the screen.
pub fn window_height(content: f32, available: f32) -> f32 {
    (content + SHADOW_MARGIN * 2.0).min(available).max(160.0)
}

pub fn window_size(content: f32, available: f32) -> gpui::Size<gpui::Pixels> {
    size(
        px(POPOVER_WIDTH + SHADOW_MARGIN * 2.0),
        px(window_height(content, available)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountWindow, ModeTag, fixtures::*};

    #[test]
    fn account_label_speaks_state_and_resets() {
        let mut a = account("Work", 2, State::Healthy);
        a.masked = "d••@acme.co".into();
        a.windows = vec![AccountWindow {
            label: "5-hour".into(),
            left: Some(64.0),
            resets_at: Some(3 * 3_600_000),
            stale: false,
        }];
        assert_eq!(
            account_accessible_label(&a, 0),
            "Work, d••@acme.co, in use by 2 sessions. 5-hour 64 percent left, resets in 3 hours."
        );
        let mut stale = account("Team", 0, State::Stale);
        stale.masked.clear();
        stale.windows = vec![
            AccountWindow {
                label: "5-hour".into(),
                left: Some(58.0),
                resets_at: None,
                stale: true,
            },
            AccountWindow {
                label: "Weekly".into(),
                left: None,
                resets_at: None,
                stale: false,
            },
        ];
        assert_eq!(
            account_accessible_label(&stale, 0),
            "Team. 5-hour 58 percent left, stale, not in average. Weekly, not reported."
        );
    }

    #[test]
    fn group_label_reads_like_board_eight() {
        let g = Group {
            key: "k".into(),
            title: "Claude Max".into(),
            mark: None,
            count_label: "3 subscriptions".into(),
            mode: ModeTag::Pooled,
            windows: vec![group_window("5-hour", Some(48.0), 3)],
            accounts: vec![],
        };
        assert_eq!(
            group_accessible_label(&g, false),
            "Claude Max, 3 subscriptions, pooled. 5-hour 48 percent left, average of 3 fresh accounts. Collapsed"
        );
    }

    #[test]
    fn window_height_clamps_to_screen() {
        assert_eq!(window_height(500.0, 900.0), 528.0);
        assert_eq!(window_height(2000.0, 900.0), 900.0);
        assert_eq!(window_height(10.0, 900.0), 160.0);
    }
}
