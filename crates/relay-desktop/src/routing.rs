//! Routing tab (board 5): pooled-group placement, quota data, endpoint, and
//! what this build can do per credential. Settings persist through the bridge.

use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, Styled, Window, div,
    prelude::*, px,
};
use relay_core::{quota::FRESH_FOR_MS, select::Strategy};

use crate::{
    assets::Mark,
    input::TextInput,
    login,
    store::Store,
    theme::Theme,
    widgets::{self, Tone, caps, ui},
};

pub const STRATEGIES: [(Strategy, &str); 3] = [
    (Strategy::QuotaAware, "Quota-aware"),
    (Strategy::RoundRobin, "Round robin"),
    (Strategy::FillFirst, "Priority"),
];

pub fn strategy_help(s: Strategy) -> &'static str {
    match s {
        Strategy::QuotaAware => {
            "Most headroom first; when two are close, the one that resets sooner."
        }
        Strategy::RoundRobin => "New sessions rotate through eligible accounts.",
        Strategy::FillFirst => "Accounts in their configured order, each until it runs low.",
    }
}

/// Skip-below choices cycle on click.
pub const SKIP_STEPS: [f32; 4] = [0.0, 5.0, 10.0, 20.0];

pub fn next_skip(current: f32) -> f32 {
    let i = SKIP_STEPS
        .iter()
        .position(|s| (s - current).abs() < 0.01)
        .unwrap_or(0);
    SKIP_STEPS[(i + 1) % SKIP_STEPS.len()]
}

fn card(title: &'static str, t: &Theme) -> gpui::Div {
    widgets::surface(t)
        .p(px(20.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(caps(title, t.muted).mb(px(8.)))
}

fn setting(
    title: &'static str,
    help: Option<String>,
    control: AnyElement,
    t: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(20.))
        .py(px(8.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_size(px(14.)).child(title))
                .children(help.map(|h| div().text_size(px(12.5)).text_color(t.muted).child(h))),
        )
        .child(div().flex_none().child(control))
}

/// Parses a local port. Below 1024 needs privileges, so it is refused.
pub fn parse_port(text: &str) -> Result<u16, String> {
    let text = text.trim();
    match text.parse::<u16>() {
        Ok(p) if p >= 1024 => Ok(p),
        Ok(_) => Err("Use a port from 1024 to 65535.".into()),
        Err(_) => Err(format!("“{text}” is not a port number.")),
    }
}

fn port_editor(store: &Entity<Store>, window: &mut Window, cx: &mut App) -> AnyElement {
    let t = ui(cx).theme;
    let listen = store.read(cx).config.server.listen;
    let input = window.use_keyed_state("port-input", cx, |_, cx| {
        let mut i = TextInput::new("8787", cx);
        i.set_text(listen.port().to_string(), cx);
        i
    });
    let st = store.clone();
    let field = input.clone();
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(13.5))
                .text_color(t.muted)
                .child(format!("{}:", listen.ip())),
        )
        .child(
            div()
                .w(px(76.))
                .h(px(32.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(8.))
                .bg(t.inset)
                .child(input),
        )
        .child(
            widgets::Button::new(
                "apply-port",
                widgets::ButtonKind::Secondary,
                "Apply port and restart routing",
            )
            .label("Apply")
            .on_click(move |_, _, cx| {
                let text = field.read(cx).content.to_string();
                st.update(cx, |s, cx| {
                    match parse_port(&text) {
                        Ok(port) => {
                            s.error = None;
                            s.edit_config(|c| c.server.listen.set_port(port));
                        }
                        Err(e) => s.error = Some(e),
                    }
                    cx.notify();
                });
            }),
        )
        .into_any_element()
}

fn value_box(text: String, t: &Theme) -> gpui::Div {
    div()
        .h(px(32.))
        .px(px(12.))
        .flex()
        .items_center()
        .rounded(px(8.))
        .bg(t.inset)
        .font_features(widgets::tabular())
        .text_size(px(13.5))
        .child(text)
}

pub fn view(store: &Entity<Store>, window: &mut Window, cx: &mut App) -> AnyElement {
    let t = ui(cx).theme;
    let config = store.read(cx).config.clone();
    let r = &config.routing;
    let st = store.clone();
    let strategy = widgets::segmented(
        "strategy",
        &STRATEGIES.map(|(_, l)| l),
        STRATEGIES
            .iter()
            .position(|(s, _)| *s == r.strategy)
            .unwrap_or(0),
        96.,
        move |i, _, cx| {
            st.update(cx, |s, cx| {
                s.edit_config(|c| c.routing.strategy = STRATEGIES[i].0);
                cx.notify();
            })
        },
        window,
        cx,
    );
    let st = store.clone();
    let sticky = widgets::switch(
        "sticky",
        "Running sessions stay on their account",
        r.sticky,
        move |on, _, cx| {
            st.update(cx, |s, cx| {
                s.edit_config(|c| c.routing.sticky = on);
                cx.notify();
            })
        },
        window,
        cx,
    );
    let st = store.clone();
    let skip = r.skip_below_pct;
    let skip_box = value_box(format!("{skip:.0}% left"), &t)
        .id("skip-below")
        .role(gpui::Role::Button)
        .aria_label(gpui::SharedString::from(format!(
            "Skip an account below {skip:.0} percent left"
        )))
        .focusable()
        .tab_stop(true)
        .cursor_pointer()
        .hover(|s| s.bg(t.hover))
        .focus_visible(|s| s.bg(t.focus))
        .on_click(move |_, _, cx| {
            st.update(cx, |s, cx| {
                s.edit_config(|c| c.routing.skip_below_pct = next_skip(c.routing.skip_below_pct));
                cx.notify();
            })
        })
        .into_any_element();
    let idle = widgets::switch(
        "idle-poll",
        "Poll quota while idle (unavailable)",
        false,
        |_, _, _| {},
        window,
        cx,
    );
    let login_path = login::current();
    let login_on = login_path.as_deref().is_some_and(login::is_enabled);
    let st = store.clone();
    let open_at_login = widgets::switch(
        "open-at-login",
        "Open at login",
        login_on,
        move |on, _, cx| {
            let result = match (&login_path, std::env::current_exe()) {
                (Some(path), Ok(exe)) => {
                    login::set_enabled(path, &exe, on).map_err(|e| e.to_string())
                }
                _ => Err("Cannot find the login items folder".into()),
            };
            st.update(cx, |s, cx| {
                s.error = result.err();
                cx.notify();
            });
        },
        window,
        cx,
    );

    let left = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            card("Pooled groups · new sessions", &t)
                .child(setting("Strategy", Some(strategy_help(r.strategy).into()), strategy, &t))
                .child(setting(
                    "Running sessions stay on their account",
                    Some("Only new sessions are placed by headroom. A session moves only if its account is exhausted.".into()),
                    sticky,
                    &t,
                ))
                .child(setting("Skip an account below", None, skip_box, &t)),
        )
        .child(
            card("Quota data", &t)
                .child(setting(
                    "Treat values as stale after",
                    Some("Stale values show their age and are left out of averages.".into()),
                    value_box(format!("{} min", FRESH_FOR_MS / 60_000), &t).into_any_element(),
                    &t,
                ))
                .child(setting(
                    "Poll quota while idle",
                    Some("Off: values update only when requests pass through. No documented, non-billed quota read exists for subscriptions.".into()),
                    div().opacity(0.5).child(idle).into_any_element(),
                    &t,
                )),
        )
        .child(
            card("Endpoint", &t)
                .child(setting(
                    "Local address",
                    Some("Applying a new port restarts routing; requests in flight finish first. Point your T3 instances at the new address.".into()),
                    port_editor(store, window, cx),
                    &t,
                ))
                .child(setting("Open at login", None, open_at_login, &t)),
        );

    type Status = (&'static str, Tone);
    let rows: [(Option<Mark>, &str, Status, Status); 5] = [
        (
            Some(Mark::Claude),
            "Claude subscription",
            ("Documented", Tone::Neutral),
            ("Opt-in · terms unresolved", Tone::Amber),
        ),
        (
            Some(Mark::Anthropic),
            "Anthropic API key",
            ("Yes", Tone::Neutral),
            ("Available", Tone::Green),
        ),
        (
            Some(Mark::ChatGpt),
            "ChatGPT / Codex subscription",
            ("T3 Existing · untested", Tone::Neutral),
            ("Opt-in · terms unresolved", Tone::Amber),
        ),
        (
            Some(Mark::OpenAi),
            "OpenAI API key",
            ("Yes", Tone::Neutral),
            ("Available", Tone::Green),
        ),
        (
            None,
            "Other providers",
            ("Not evaluated", Tone::Neutral),
            ("Not evaluated", Tone::Neutral),
        ),
    ];
    let right = card("Capabilities in this build", &t)
        .flex_1()
        .min_w_0()
        .child(
            div()
                .flex()
                .px(px(12.))
                .child(caps("Credential", t.muted).w(px(200.)))
                .child(caps("Observe", t.muted).w(px(170.)))
                .child(caps("Pool", t.muted)),
        )
        .children(rows.into_iter().enumerate().map(|(i, (mark, name, observe, pool))| {
            div()
                .flex()
                .items_center()
                .min_h(px(44.))
                .px(px(12.))
                .rounded(px(9.))
                .when(i % 2 == 0, |d| d.bg(t.inset))
                .child(
                    div()
                        .w(px(200.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .children(mark.map(|m| widgets::mark(m, 15., &t)))
                        .child(div().text_size(px(13.5)).child(name)),
                )
                .child(div().w(px(170.)).flex_none().flex().child(widgets::tag(observe.0, observe.1, &t)))
                .child(div().flex().child(widgets::tag(pool.0, pool.1, &t)))
        }))
        .child(div().mt(px(8.)).text_size(px(12.)).text_color(t.muted).child(
            "Statuses describe what this build does, not legal advice. Observed groups always follow their T3 instance.",
        ));

    let error = store.read(cx).error.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .children(error.map(|e| {
            div()
                .text_size(px(13.))
                .text_color(t.ember)
                .font_weight(FontWeight::MEDIUM)
                .child(e)
        }))
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(12.))
                .child(left)
                .child(right),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_steps_cycle_and_recover_from_custom_values() {
        assert_eq!(next_skip(0.0), 5.0);
        assert_eq!(next_skip(20.0), 0.0);
        assert_eq!(next_skip(7.5), 5.0);
    }

    #[test]
    fn ports_are_validated() {
        assert_eq!(parse_port(" 8788 "), Ok(8788));
        assert!(parse_port("80").is_err());
        assert!(parse_port("70000").is_err());
        assert!(parse_port("abc").is_err());
    }

    #[test]
    fn priority_is_fill_first() {
        assert_eq!(STRATEGIES[2], (Strategy::FillFirst, "Priority"));
    }
}
