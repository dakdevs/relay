//! Shared controls drawn to the approved boards: tactile buttons, tags, quota
//! bars, segmented pills, switches and brand marks. No borders anywhere;
//! focus and selection are filled tints.

use std::{rc::Rc, sync::Arc, time::Instant};

use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, ElementId, FontFeatures, FontWeight, Global,
    IntoElement, MouseButton, ParentElement, RenderOnce, Rgba, SharedString,
    StatefulInteractiveElement, Styled, Window, div, img, linear_color_stop, linear_gradient,
    pattern_slash, point, prelude::*, px, svg,
};

use crate::{
    assets::Mark,
    motion::{self, Tween},
    theme::{Theme, radius},
};

/// App-wide presentation state: palette for the current system appearance and
/// the reduced-motion preference.
#[derive(Clone, Copy)]
pub struct Ui {
    pub theme: Theme,
    pub reduced_motion: bool,
    /// Light/dark preference from settings; `System` follows the OS.
    pub preference: relay_core::config::Theme,
}

impl Ui {
    /// Palette for the preference, using `system` when it follows the OS.
    pub fn resolve(preference: relay_core::config::Theme, system: gpui::WindowAppearance) -> Theme {
        match preference {
            relay_core::config::Theme::System => Theme::for_appearance(system),
            relay_core::config::Theme::Light => Theme::light(),
            relay_core::config::Theme::Dark => Theme::dark(),
        }
    }
}

impl Global for Ui {}

pub fn ui(cx: &App) -> Ui {
    *cx.global::<Ui>()
}

pub type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

pub fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// Numeric text with tabular figures so columns and live values don't jitter.
pub fn num(text: impl Into<SharedString>) -> gpui::Div {
    div().font_features(tabular()).child(text.into())
}

/// Small uppercase window label ("5-HOUR", "WEEKLY").
pub fn caps(text: impl Into<SharedString>, color: Rgba) -> gpui::Div {
    let text: SharedString = text.into();
    div()
        .text_size(px(10.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(SharedString::from(text.to_uppercase()))
}

// ---------------------------------------------------------------- buttons

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Icon,
}

#[derive(Default)]
struct ButtonMotion {
    hover: Option<Tween>,
    release: Option<Tween>,
    pressed: bool,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    kind: ButtonKind,
    label: Option<SharedString>,
    icon: Option<&'static str>,
    trailing: Option<&'static str>,
    accessible: SharedString,
    disabled: bool,
    on_click: Option<Handler>,
}

impl Button {
    pub fn new(
        id: impl Into<ElementId>,
        kind: ButtonKind,
        accessible: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            label: None,
            icon: None,
            trailing: None,
            accessible: accessible.into(),
            disabled: false,
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn icon(mut self, path: &'static str) -> Self {
        self.icon = Some(path);
        self
    }

    pub fn trailing(mut self, path: &'static str) -> Self {
        self.trailing = Some(path);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Ui {
            theme: t,
            reduced_motion,
            ..
        } = ui(cx);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ButtonMotion::default());
        let now = Instant::now();
        let (hover, press, animating) = {
            let s = state.read(cx);
            let hover = s.hover.map_or(0.0, |tw| tw.value_at(now));
            let press = if s.pressed {
                1.0
            } else {
                s.release.map_or(0.0, |tw| tw.value_at(now))
            };
            let animating = s.hover.is_some_and(|tw| tw.is_animating(now))
                || (!s.pressed && s.release.is_some_and(|tw| tw.is_animating(now)));
            (hover, press, animating)
        };
        if animating {
            window.request_animation_frame();
        }

        let (top, bottom, fg, hover_tint, pressed_bg) = match self.kind {
            ButtonKind::Primary => {
                let (a, b) = t.primary();
                (
                    a,
                    b,
                    t.on_accent,
                    lerp(a, gpui::rgb(0xFFFFFF), 0.08),
                    t.primary_pressed(),
                )
            }
            ButtonKind::Secondary | ButtonKind::Icon => {
                (t.button_top, t.button_bottom, t.text, t.hover, t.pressed)
            }
        };
        let top = lerp(lerp(top, hover_tint, hover), pressed_bg, press);
        let bottom = lerp(lerp(bottom, hover_tint, hover * 0.6), pressed_bg, press);
        let sink = if reduced_motion || self.disabled {
            0.0
        } else {
            press
        };
        let shadow_alpha = t.shadow.a * (1.0 - press);

        let is_icon = self.kind == ButtonKind::Icon;
        let base = div()
            .id(self.id.clone())
            .role(gpui::Role::Button)
            .aria_label(self.accessible.clone())
            .h(px(28.))
            .when(is_icon, |d| d.w(px(28.)).justify_center())
            .when(!is_icon, |d| d.px(px(12.)))
            .flex()
            .items_center()
            .gap(px(5.))
            .rounded(px(radius::BUTTON))
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .relative()
            .top(px(sink));

        let base = if self.disabled {
            base.bg(lerp(t.inset, t.surface, 0.3))
                .text_color(t.faint)
                .aria_description("Unavailable")
        } else {
            let on_click = self.on_click.clone();
            let st_down = state.clone();
            let st_up = state.clone();
            let st_out = state.clone();
            let st_hover = state.clone();
            base.bg(linear_gradient(
                180.,
                linear_color_stop(top, 0.),
                linear_color_stop(bottom, 1.),
            ))
            .text_color(fg)
            .shadow(vec![
                BoxShadow {
                    color: t.shadow.opacity(shadow_alpha / t.shadow.a.max(0.01) * 0.9),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(2.),
                    spread_radius: px(0.),
                    inset: false,
                },
                BoxShadow {
                    color: gpui::hsla(0., 0., 0., 0.14 * press),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(2.),
                    spread_radius: px(0.),
                    inset: true,
                },
            ])
            .cursor_pointer()
            .focusable()
            .tab_stop(true)
            .focus_visible(move |s| {
                s.bg(if is_primary(self.kind) {
                    lerp(t.primary().0, gpui::rgb(0xFFFFFF), 0.14)
                } else {
                    t.focus
                })
            })
            .on_hover(move |hovered, _, cx| {
                st_hover.update(cx, |s, cx| {
                    let now = Instant::now();
                    let mut tw = s.hover.unwrap_or(Tween::settled(0.0));
                    tw.retarget(
                        if *hovered { 1.0 } else { 0.0 },
                        motion::HOVER,
                        now,
                        reduced_motion,
                    );
                    s.hover = Some(tw);
                    cx.notify();
                });
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                // Pressed on the same frame as pointer-down.
                st_down.update(cx, |s, cx| {
                    s.pressed = true;
                    cx.notify();
                });
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                release(&st_up, reduced_motion, cx)
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                release(&st_out, reduced_motion, cx)
            })
            .when_some(on_click, |d, f| d.on_click(move |e, w, cx| f(e, w, cx)))
        };

        base.when_some(self.icon, |d, path| {
            d.child(icon(path, 13., fg_for(self.disabled, fg, &t)))
        })
        .when_some(self.label, |d, l| d.child(l))
        .when_some(self.trailing, |d, path| {
            d.child(icon(path, 11., fg_for(self.disabled, fg, &t)))
        })
    }
}

fn is_primary(kind: ButtonKind) -> bool {
    kind == ButtonKind::Primary
}

fn fg_for(disabled: bool, fg: Rgba, t: &Theme) -> Rgba {
    if disabled { t.faint } else { fg }
}

fn release(state: &gpui::Entity<ButtonMotion>, reduced: bool, cx: &mut App) {
    state.update(cx, |s, cx| {
        if s.pressed {
            s.pressed = false;
            let mut tw = Tween::settled(1.0);
            tw.retarget(0.0, motion::PRESS_RELEASE, Instant::now(), reduced);
            s.release = Some(tw);
            cx.notify();
        }
    });
}

pub fn icon(path: &'static str, size: f32, color: Rgba) -> impl IntoElement {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
}

// ---------------------------------------------------------------- tags

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Green,
    Amber,
    Ember,
    Neutral,
    Strong,
}

/// State tag: a glyph plus a word; colour only reinforces it (board 6).
pub fn tag(text: impl Into<SharedString>, tone: Tone, t: &Theme) -> gpui::Div {
    let (bg, fg) = match tone {
        Tone::Green => (t.tag_green, t.evergreen),
        Tone::Amber => (t.tag_amber, t.amber),
        Tone::Ember => (t.tag_ember, t.ember),
        Tone::Neutral => (t.tag_neutral, t.muted),
        Tone::Strong => (t.tag_neutral, t.text),
    };
    let text: SharedString = text.into();
    div()
        .flex_none()
        .px(px(6.))
        .py(px(1.5))
        .rounded(px(radius::TAG))
        .bg(bg)
        .text_color(fg)
        .text_size(px(9.5))
        .font_weight(FontWeight::SEMIBOLD)
        .font_features(tabular())
        .child(SharedString::from(text.to_uppercase()))
}

// ---------------------------------------------------------------- bars

/// Quota-left bar. `None` is unknown and drawn hatched, never as 0%.
pub fn bar(left: Option<f32>, stale: bool, width: f32, t: &Theme) -> gpui::Div {
    let track = div()
        .w(px(width))
        .h(px(4.))
        .flex_none()
        .rounded_full()
        .bg(t.track)
        .overflow_hidden();
    match left {
        None => track.bg(pattern_slash(t.faint.opacity(0.55), 1.5, 2.5)),
        Some(left) => {
            let frac = (left / 100.0).clamp(0.0, 1.0);
            track.child(
                div()
                    .h_full()
                    .w(px(width * frac))
                    .rounded_full()
                    .bg(t.bar_color(left, stale)),
            )
        }
    }
}

/// One pooled bar split into per-account segments (board 3 summary).
pub fn segments(values: &[Option<f32>], stale: &[bool], t: &Theme) -> gpui::Div {
    div()
        .flex()
        .gap(px(6.))
        .w_full()
        .children(values.iter().zip(stale).map(|(v, s)| {
            let frac = v.map_or(0.0, |v| (v / 100.0).clamp(0.0, 1.0));
            let fill = div()
                .h_full()
                .rounded_full()
                .bg(t.bar_color(v.unwrap_or(0.0), *s));
            div()
                .flex_1()
                .h(px(5.))
                .rounded_full()
                .bg(t.track)
                .overflow_hidden()
                .when(v.is_none(), |d| {
                    d.bg(pattern_slash(t.faint.opacity(0.55), 1.5, 2.5))
                })
                .child(fill.w(gpui::relative(frac)))
        }))
}

// ---------------------------------------------------------------- segmented

struct PillMotion {
    x: Tween,
}

/// Segmented control with a sliding selected pill (Week / Month, View, Strategy).
pub fn segmented(
    id: impl Into<ElementId>,
    options: &[&'static str],
    selected: usize,
    segment_width: f32,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Ui {
        theme: t,
        reduced_motion,
        ..
    } = ui(cx);
    let id: ElementId = id.into();
    let target = selected as f32 * segment_width;
    let state = window.use_keyed_state(id.clone(), cx, |_, _| PillMotion {
        x: Tween::settled(target),
    });
    let now = Instant::now();
    let x = state.update(cx, |s, _| {
        s.x.retarget(target, motion::PILL, now, reduced_motion);
        if s.x.is_animating(now) {
            window.request_animation_frame();
        }
        s.x.value_at(now)
    });
    let on_select = Rc::new(on_select);
    div()
        .id(id)
        .role(gpui::Role::TabList)
        .relative()
        .flex()
        .p(px(2.))
        .rounded(px(8.))
        .bg(t.inset)
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(px(2. + x))
                .w(px(segment_width))
                .h(px(22.))
                .rounded(px(6.))
                .bg(t.surface)
                .shadow(vec![BoxShadow {
                    color: t.shadow.opacity(0.10),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(2.),
                    spread_radius: px(0.),
                    inset: false,
                }]),
        )
        .children(options.iter().enumerate().map(|(i, label)| {
            let on_select = on_select.clone();
            div()
                .id(ElementId::Name(SharedString::from(format!("seg-{label}"))))
                .role(gpui::Role::Tab)
                .aria_label(SharedString::from(*label))
                .aria_selected(i == selected)
                .focusable()
                .tab_stop(true)
                .focus_visible(move |s| s.bg(t.focus))
                .relative()
                .w(px(segment_width))
                .h(px(22.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .font_weight(if i == selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if i == selected { t.text } else { t.muted })
                .cursor_pointer()
                .child(*label)
                .on_click(move |_, w, cx| on_select(i, w, cx))
        }))
        .into_any_element()
}

// ---------------------------------------------------------------- switch

struct KnobMotion {
    x: Tween,
}

pub fn switch(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let Ui {
        theme: t,
        reduced_motion,
        ..
    } = ui(cx);
    let id: ElementId = id.into();
    let target = if on { 1.0 } else { 0.0 };
    let state = window.use_keyed_state(id.clone(), cx, |_, _| KnobMotion {
        x: Tween::settled(target),
    });
    let now = Instant::now();
    let p = state.update(cx, |s, _| {
        s.x.retarget(target, motion::HOVER, now, reduced_motion);
        if s.x.is_animating(now) {
            window.request_animation_frame();
        }
        s.x.value_at(now)
    });
    div()
        .id(id)
        .role(gpui::Role::Switch)
        .aria_label(label.into())
        .aria_toggled(if on {
            gpui::Toggled::True
        } else {
            gpui::Toggled::False
        })
        .focusable()
        .tab_stop(true)
        .flex_none()
        .w(px(38.))
        .h(px(22.))
        .rounded_full()
        .bg(lerp(t.track, t.evergreen, p))
        .focus_visible(move |s| s.bg(lerp(t.focus, t.evergreen, p)))
        .cursor_pointer()
        .relative()
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(px(2. + 16. * p))
                .size(px(18.))
                .rounded_full()
                .bg(gpui::rgb(0xFFFFFF))
                .shadow(vec![BoxShadow {
                    color: gpui::hsla(0., 0., 0., 0.2),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(2.),
                    spread_radius: px(0.),
                    inset: false,
                }]),
        )
        .on_click(move |_, w, cx| on_toggle(!on, w, cx))
        .into_any_element()
}

// ---------------------------------------------------------------- marks

pub fn mark(mark: Mark, size: f32, t: &Theme) -> AnyElement {
    let path = mark.path(t.dark);
    if mark.is_mask() {
        svg()
            .path(path)
            .size(px(size))
            .flex_none()
            .text_color(t.text)
            .into_any_element()
    } else {
        img(path).size(px(size)).flex_none().into_any_element()
    }
}

/// A rounded tonal surface (section, card). No border.
pub fn surface(t: &Theme) -> gpui::Div {
    div().bg(t.surface).rounded(px(radius::SECTION))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_preference_overrides_system_appearance() {
        use relay_core::config::Theme as Pref;
        assert!(Ui::resolve(Pref::Dark, gpui::WindowAppearance::Light).dark);
        assert!(!Ui::resolve(Pref::Light, gpui::WindowAppearance::Dark).dark);
        assert!(Ui::resolve(Pref::System, gpui::WindowAppearance::Dark).dark);
    }

    #[test]
    fn lerp_endpoints_and_midpoint() {
        let a = gpui::rgb(0x000000);
        let b = gpui::rgb(0xFFFFFF);
        assert_eq!(lerp(a, b, 0.0), a);
        assert_eq!(lerp(a, b, 1.0), b);
        assert!((lerp(a, b, 0.5).r - 0.5).abs() < 1e-6);
        // Clamped: no overshoot past either end.
        assert_eq!(lerp(a, b, 2.0), b);
    }
}
