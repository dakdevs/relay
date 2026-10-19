//! Quotas tab (board 3): provider filters with counts, search, sort, a shared
//! summary with one column per comparable group, and the grouped ledger with
//! every window, reset, credit and capability-gated action per account.

use gpui::{
    AnyElement, App, ElementId, Entity, FontWeight, ParentElement, SharedString, Styled, div,
    prelude::*, px, relative,
};

use crate::{
    assets::Mark,
    format,
    model::{Account, Group, GroupWindow, ModeTag, Overview, State},
    store::Store,
    theme::{Theme, radius},
    widgets::{self, Button, ButtonKind, Tone, caps, ui},
};

/// Ledger ordering (Sort chip cycles through these).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Default,
    LeastLeft,
    Label,
}

impl Sort {
    pub fn next(self) -> Sort {
        match self {
            Sort::Default => Sort::LeastLeft,
            Sort::LeastLeft => Sort::Label,
            Sort::Label => Sort::Default,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Sort::Default => "Default",
            Sort::LeastLeft => "Least left",
            Sort::Label => "Label",
        }
    }
}

/// The window a summary column leads with: the weekly limit when present,
/// otherwise the first plan window.
pub fn lead_window(g: &Group) -> Option<&GroupWindow> {
    let plan: Vec<&GroupWindow> = g.windows.iter().filter(|w| !w.model_limit).collect();
    plan.iter()
        .find(|w| w.label.to_lowercase().contains("week"))
        .copied()
        .or_else(|| plan.first().copied())
}

/// Column order for a group's ledger: plan windows (weekly first), then model limits.
pub fn ledger_windows(g: &Group) -> Vec<&GroupWindow> {
    let mut w: Vec<&GroupWindow> = g.windows.iter().collect();
    w.sort_by_key(|w| (w.model_limit, !w.label.to_lowercase().contains("week")));
    w
}

/// "212%" and "of 300% left" for pooled capacity over fresh accounts.
pub fn pooled_text(w: &GroupWindow) -> (String, String) {
    (
        format!("{:.0}%", w.pooled_left.floor()),
        format!("of {}% left", w.fresh * 100),
    )
}

/// "avg 71% · 3 fresh · 1 stale excluded".
pub fn avg_text(w: &GroupWindow) -> String {
    let mut s = match w.avg_left {
        Some(a) => format!("avg {} · {} fresh", format::percent(a), w.fresh),
        None => "no fresh values".to_string(),
    };
    if w.stale > 0 {
        s.push_str(&format!(" · {} stale excluded", w.stale));
    }
    s
}

/// "Resets in 4d · Oct 22, 09:00 · others by Oct 24, 18:00".
pub fn reset_text(w: &GroupWindow, now: i64) -> Option<String> {
    let first = w.first_reset?;
    let mut s = format!(
        "Resets in {} · {}",
        format::duration_short(crate::adapter::ms(first - now)),
        format::clock(first)
    );
    if let Some(last) = w.last_reset.filter(|l| *l > first + 60_000) {
        s.push_str(&format!(" · others by {}", format::clock(last)));
    }
    Some(s)
}

pub fn matches_search(a: &Account, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty()
        || a.label.to_lowercase().contains(&q)
        || a.masked.to_lowercase().contains(&q)
        || a.id.contains(&q)
}

pub fn sorted_accounts<'a>(g: &'a Group, sort: Sort, query: &str) -> Vec<&'a Account> {
    let mut v: Vec<&Account> = g
        .accounts
        .iter()
        .filter(|a| matches_search(a, query))
        .collect();
    match sort {
        Sort::Default => {}
        Sort::Label => v.sort_by_key(|a| a.label.to_lowercase()),
        Sort::LeastLeft => v.sort_by(|a, b| {
            let min = |x: &Account| {
                x.windows
                    .iter()
                    .filter_map(|w| w.left)
                    .fold(f32::INFINITY, f32::min)
            };
            min(a).total_cmp(&min(b))
        }),
    }
    v
}

pub fn filter_chips(
    o: &Overview,
    selected: Option<&'static str>,
    store: &Entity<Store>,
    t: &Theme,
) -> AnyElement {
    let all = o.account_count();
    let mut chips: Vec<(Option<&'static str>, Option<Mark>, String, usize)> =
        vec![(None, None, "All".into(), all)];
    for s in &o.sections {
        chips.push((
            Some(s.provider),
            Some(s.mark),
            s.provider.into(),
            s.account_count(),
        ));
    }
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .children(chips.into_iter().map(|(key, mark, label, n)| {
            let on = key == selected;
            let store = store.clone();
            div()
                .id(ElementId::Name(SharedString::from(format!(
                    "filter-{label}"
                ))))
                .role(gpui::Role::Tab)
                .aria_selected(on)
                .aria_label(SharedString::from(format!("{label}, {n} accounts")))
                .focusable()
                .tab_stop(true)
                .flex()
                .items_center()
                .gap(px(7.))
                .h(px(32.))
                .px(px(12.))
                .rounded(px(radius::ROW))
                .when(on, |d| d.bg(t.surface))
                .hover(|s| s.bg(if on { t.surface } else { t.hover }))
                .focus_visible(|s| s.bg(t.focus))
                .cursor_pointer()
                .children(mark.map(|m| widgets::mark(m, 14., t)))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(if on {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .child(label),
                )
                .child(
                    widgets::num(n.to_string())
                        .text_size(px(13.))
                        .text_color(t.muted),
                )
                .on_click(move |_, _, cx| {
                    store.update(cx, |s, cx| {
                        s.provider_filter = key;
                        cx.notify();
                    })
                })
        }))
        .child(
            div()
                .ml(px(10.))
                .text_size(px(12.5))
                .text_color(t.muted)
                .child("Configured providers only · logos do not imply other integrations"),
        )
        .into_any_element()
}

fn summary_column(
    g: &Group,
    provider_mark: Mark,
    now: i64,
    hidden_model: bool,
    store: &Entity<Store>,
    t: &Theme,
) -> AnyElement {
    let Some(lead) = lead_window(g) else {
        return div().into_any_element();
    };
    let (big, of) = pooled_text(lead);
    let others: Vec<&GroupWindow> = g
        .windows
        .iter()
        .filter(|w| !w.model_limit && w.label != lead.label)
        .collect();
    let model: Vec<&GroupWindow> = g.windows.iter().filter(|w| w.model_limit).collect();
    let key = g.key.clone();
    let store = store.clone();
    div()
        .flex_1()
        .min_w(px(260.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(widgets::mark(g.mark.unwrap_or(provider_mark), 17., t))
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(g.title.clone()),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(t.muted)
                        .child(g.count_label.clone()),
                ),
        )
        .child(
            div()
                .mt(px(10.))
                .flex()
                .child(div().text_size(px(13.5)).child(window_title(lead)))
                .child(div().flex_1())
                .child(
                    widgets::num(avg_text(lead))
                        .text_size(px(12.5))
                        .text_color(t.muted),
                ),
        )
        .child(
            div()
                .flex()
                .items_end()
                .gap(px(6.))
                .child(
                    widgets::num(big)
                        .text_size(px(34.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(40.)),
                )
                .child(
                    div()
                        .pb(px(5.))
                        .text_size(px(14.))
                        .text_color(t.muted)
                        .child(of),
                ),
        )
        .child(div().mt(px(4.)).child(widgets::segments(
            &lead.segments.iter().map(|s| s.0).collect::<Vec<_>>(),
            &lead.segments.iter().map(|s| s.1).collect::<Vec<_>>(),
            t,
        )))
        .children(reset_text(lead, now).map(|r| {
            div()
                .mt(px(6.))
                .text_size(px(12.5))
                .text_color(t.muted)
                .child(r)
        }))
        .children(others.iter().map(|w| {
            let mut line = format!(
                "{} · {}% of {}%",
                w.label,
                w.pooled_left.floor(),
                w.fresh * 100
            );
            if let Some(a) = w.avg_left {
                line.push_str(&format!(" · avg {}", format::percent(a)));
            }
            if let Some(r) = w.first_reset {
                line.push_str(&format!(
                    " · first reset in {}",
                    format::duration_short(crate::adapter::ms(r - now))
                ));
            }
            div()
                .mt(px(8.))
                .text_size(px(12.5))
                .text_color(t.muted)
                .child(line)
        }))
        .children((!model.is_empty()).then(|| {
            let w = model[0];
            let (mbig, mof) = pooled_text(w);
            div()
                .mt(px(12.))
                .bg(t.inset)
                .rounded(px(radius::ROW + 1.))
                .p(px(14.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .id(ElementId::Name(SharedString::from(format!("model-{key}"))))
                        .role(gpui::Role::Button)
                        .aria_expanded(!hidden_model)
                        .focusable()
                        .tab_stop(true)
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .cursor_pointer()
                        .on_click(move |_, _, cx| {
                            store.update(cx, |s, cx| {
                                if !s.collapsed.remove(&format!("model:{key}")) {
                                    s.collapsed.insert(format!("model:{key}"));
                                }
                                cx.notify();
                            })
                        })
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("Model limit · {}", w.label)),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_size(px(12.5))
                                .text_color(t.muted)
                                .child(if hidden_model { "Show" } else { "Hide" }),
                        ),
                )
                .when(!hidden_model, |d| {
                    d.child(
                        div()
                            .flex()
                            .child(div().text_size(px(12.5)).child(w.label.clone()))
                            .child(div().flex_1())
                            .child(
                                widgets::num(avg_text(w))
                                    .text_size(px(12.5))
                                    .text_color(t.muted),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap(px(6.))
                            .child(
                                widgets::num(mbig)
                                    .text_size(px(26.))
                                    .font_weight(FontWeight::SEMIBOLD),
                            )
                            .child(
                                div()
                                    .pb(px(4.))
                                    .text_size(px(13.))
                                    .text_color(t.muted)
                                    .child(mof),
                            ),
                    )
                    .child(widgets::segments(
                        &w.segments.iter().map(|s| s.0).collect::<Vec<_>>(),
                        &w.segments.iter().map(|s| s.1).collect::<Vec<_>>(),
                        t,
                    ))
                    .children(reset_text(w, now).map(|r| {
                        div()
                            .mt(px(4.))
                            .text_size(px(12.5))
                            .text_color(t.muted)
                            .child(r)
                    }))
                })
        }))
        .into_any_element()
}

fn window_title(w: &GroupWindow) -> String {
    let l = w.label.to_lowercase();
    if l.contains("week") {
        "Weekly limit".into()
    } else if l.contains("token") {
        "Tokens per minute".into()
    } else {
        format!("{} limit", w.label)
    }
}

pub fn summary(o: &Overview, store: &Entity<Store>, cx: &App) -> AnyElement {
    let t = ui(cx).theme;
    let s = store.read(cx);
    let cols: Vec<AnyElement> = o
        .sections
        .iter()
        .filter(|sec| s.provider_filter.is_none_or(|f| f == sec.provider))
        .flat_map(|sec| sec.groups.iter().map(move |g| (sec.mark, g)))
        .map(|(mark, g)| {
            summary_column(
                g,
                mark,
                o.now,
                s.collapsed.contains(&format!("model:{}", g.key)),
                store,
                &t,
            )
        })
        .collect();
    widgets::surface(&t)
        .p(px(28.))
        .flex()
        .flex_wrap()
        .gap_x(px(48.))
        .gap_y(px(28.))
        .children(cols)
        .into_any_element()
}

fn state_tag(a: &Account, t: &Theme) -> Option<gpui::Div> {
    Some(match a.state {
        _ if a.in_use > 0 => widgets::tag(format!("● In use · {}", a.in_use), Tone::Strong, t),
        State::Exhausted => widgets::tag(
            a.note
                .as_deref()
                .and_then(|n| n.strip_prefix("Back in "))
                .map_or("! Exhausted".to_string(), |r| {
                    format!("! Back in {}", r.trim_end_matches('.'))
                }),
            Tone::Ember,
            t,
        ),
        State::Stale => widgets::tag("◷ Stale", Tone::Amber, t),
        State::Disconnected => widgets::tag("○ Disconnected", Tone::Neutral, t),
        State::Paused => widgets::tag("Ⅱ Paused", Tone::Neutral, t),
        _ => return None,
    })
}

// Account, credits and actions are fixed; window columns share the rest
// with the same flex rules in the header and rows, so they align at any width.
const ACCOUNT_COL: f32 = 220.;
const CREDIT_COL: f32 = 150.;
const ACTIONS_COL: f32 = 270.;

fn window_col() -> gpui::Div {
    div()
        .flex_1()
        .flex_basis(px(0.))
        .min_w(px(110.))
        .pr(px(24.))
}

pub fn ledger(o: &Overview, store: &Entity<Store>, sort: Sort, cx: &App) -> Vec<AnyElement> {
    let t = ui(cx).theme;
    let s = store.read(cx);
    let query = s.search.clone();
    o.sections
        .iter()
        .filter(|sec| s.provider_filter.is_none_or(|f| f == sec.provider))
        .flat_map(|sec| sec.groups.iter().map(move |g| (sec, g)))
        .map(|(sec, g)| {
            let collapsed = s.collapsed.contains(&g.key);
            let windows = ledger_windows(g);
            let accounts = sorted_accounts(g, sort, &query);
            let key = g.key.clone();
            let toggle_store = store.clone();
            let header = div()
                .id(ElementId::Name(SharedString::from(format!(
                    "ledger-{}",
                    g.key
                ))))
                .role(gpui::Role::Button)
                .aria_expanded(!collapsed)
                .aria_label(SharedString::from(format!(
                    "{}, {}, {}",
                    g.title, sec.provider, g.count_label
                )))
                .focusable()
                .tab_stop(true)
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(34.))
                .mx(px(-8.))
                .px(px(8.))
                .rounded(px(radius::ROW))
                .cursor_pointer()
                .hover(|s| s.bg(t.inset))
                .focus_visible(|s| s.bg(t.focus))
                .on_click(move |_, _, cx| {
                    toggle_store.update(cx, |s, cx| {
                        if !s.collapsed.remove(&key) {
                            s.collapsed.insert(key.clone());
                        }
                        cx.notify();
                    })
                })
                .child(
                    div()
                        .text_size(px(9.))
                        .text_color(t.muted)
                        .child(if collapsed { "▸" } else { "▾" }),
                )
                .child(widgets::mark(g.mark.unwrap_or(sec.mark), 17., &t))
                .child(
                    div()
                        .text_size(px(15.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(g.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(t.muted)
                        .child(format!("{} · {}", sec.provider, g.count_label)),
                )
                .child(widgets::tag(
                    g.mode.text(),
                    if g.mode == ModeTag::Observed {
                        Tone::Neutral
                    } else {
                        Tone::Green
                    },
                    &t,
                ));
            let columns = div()
                .flex()
                .items_center()
                .h(px(32.))
                .px(px(16.))
                .child(caps("Account", t.muted).w(px(ACCOUNT_COL)).flex_none())
                .children(
                    windows
                        .iter()
                        .map(|w| window_col().child(caps(w.label.clone(), t.muted))),
                )
                .child(caps("Reset credits", t.muted).w(px(CREDIT_COL)).flex_none())
                .child(div().w(px(ACTIONS_COL)).flex_none());
            let rows = accounts
                .iter()
                .enumerate()
                .map(|(i, a)| ledger_row(a, &windows, i % 2 == 0, o.now, store, &t));
            widgets::surface(&t)
                .px(px(20.))
                .py(px(14.))
                .flex()
                .flex_col()
                .child(header)
                .when(!collapsed, |d| d.child(columns).children(rows))
                .into_any_element()
        })
        .collect()
}

fn ledger_row(
    a: &Account,
    windows: &[&GroupWindow],
    tinted: bool,
    now: i64,
    store: &Entity<Store>,
    t: &Theme,
) -> AnyElement {
    let sub = [
        Some(a.masked.clone()).filter(|m| !m.is_empty()),
        Some(a.plan.clone()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let cells = windows.iter().map(|gw| {
        let w = a.windows.iter().find(|w| w.label == gw.label);
        let (left, stale, reset) =
            w.map_or((None, false, None), |w| (w.left, w.stale, w.resets_at));
        let exhausted = left == Some(0.0);
        let detail = match (w, left, stale, reset) {
            (None, ..) => "— not reported".to_string(),
            (_, None, ..) => "— not reported".to_string(),
            (_, Some(_), true, _) => a.seen_at.map_or("stale · not in avg".into(), |s| {
                format!(
                    "seen {} ago · not in avg",
                    format::duration_short(crate::adapter::ms(now - s))
                )
            }),
            (_, Some(_), false, Some(r)) => format::reset_cell(r, now),
            (_, Some(_), false, None) => "reset not reported".into(),
        };
        window_col()
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(
                div().flex().justify_end().child(
                    widgets::num(left.map_or("—".to_string(), format::percent))
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if exhausted { t.ember } else { t.text }),
                ),
            )
            .child(div().w_full().child(bar_full(left, stale, t)))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(if stale { t.amber } else { t.muted })
                    .truncate()
                    .child(detail),
            )
    });
    let (credit_top, credit_sub) = match &a.credits {
        Some(c) => (c.clone(), String::new()),
        None => ("Not reported".to_string(), "—".to_string()),
    };
    let reset_disabled = a.reset_unsupported.is_some();
    let refresh_disabled = a.refresh_unsupported.is_some();
    // One reason line under the actions; when both are unavailable the
    // refresh reason (what the user can expect instead) is the useful one.
    let reason = a
        .refresh_unsupported
        .clone()
        .or_else(|| a.reset_unsupported.clone())
        .unwrap_or_else(|| "Provider action".into());
    let id = a.id.clone();
    let refresh_store = store.clone();
    div()
        .id(ElementId::Name(SharedString::from(format!("row-{}", a.id))))
        .role(gpui::Role::Row)
        .aria_label(SharedString::from(
            crate::popover::account_accessible_label(a, now),
        ))
        .flex()
        .items_start()
        .px(px(16.))
        .py(px(14.))
        .rounded(px(radius::ROW + 1.))
        .when(tinted, |d| d.bg(t.inset))
        .child(
            div()
                .w(px(ACCOUNT_COL))
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(a.label.clone()),
                )
                .child(div().text_size(px(13.)).text_color(t.muted).child(sub))
                .children(state_tag(a, t).map(|tag| div().mt(px(3.)).flex().child(tag))),
        )
        .children(cells)
        .child(
            div()
                .w(px(CREDIT_COL))
                .flex_none()
                .pr(px(12.))
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(div().text_size(px(14.)).child(credit_top))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(credit_sub),
                ),
        )
        .child(
            div()
                .flex_none()
                .w(px(ACTIONS_COL))
                .flex()
                .flex_col()
                .items_end()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(
                            Button::new(
                                ElementId::Name(format!("reset-{}", a.id).into()),
                                ButtonKind::Secondary,
                                "Reset limits",
                            )
                            .icon("icons/refresh.svg")
                            .label("Reset limits")
                            .disabled(reset_disabled),
                        )
                        .child(
                            Button::new(
                                ElementId::Name(format!("refresh-{}", a.id).into()),
                                ButtonKind::Secondary,
                                "Refresh quota",
                            )
                            .icon("icons/refresh.svg")
                            .label("Refresh quota")
                            .disabled(refresh_disabled)
                            .on_click(move |_, _, cx| {
                                refresh_store.update(cx, |s, cx| {
                                    s.send_or_report(
                                        crate::store::Command::RefreshQuota(id.clone()),
                                        crate::store::Scope::App,
                                    );
                                    cx.notify();
                                })
                            }),
                        ),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(t.muted)
                        .text_right()
                        .child(short_reason(&reason)),
                ),
        )
        .into_any_element()
}

/// Board 3 prints a short reason under disabled actions.
fn short_reason(reason: &str) -> String {
    if reason.contains("values update with traffic") || reason.contains("arrive with each response")
    {
        "Updates on next request · no supported refresh".into()
    } else {
        reason.to_string()
    }
}

fn bar_full(left: Option<f32>, stale: bool, t: &Theme) -> gpui::Div {
    let track = div()
        .w_full()
        .h(px(5.))
        .rounded_full()
        .bg(t.track)
        .overflow_hidden();
    match left {
        None => track.bg(gpui::pattern_slash(t.faint.opacity(0.55), 1.5, 2.5)),
        Some(l) => track.child(
            div()
                .h_full()
                .w(relative((l / 100.0).clamp(0.0, 1.0)))
                .rounded_full()
                .bg(t.bar_color(l, stale)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fixtures::*;

    fn group() -> Group {
        let mut model = group_window("Fable 7d", Some(97.0), 3);
        model.model_limit = true;
        let mut weekly = group_window("Weekly", Some(70.67), 3);
        weekly.pooled_left = 212.0;
        Group {
            key: "k".into(),
            title: "Claude Max".into(),
            mark: Some(Mark::Claude),
            count_label: "3 subscriptions".into(),
            mode: ModeTag::Pooled,
            windows: vec![group_window("5-hour", Some(48.0), 3), weekly, model],
            accounts: vec![
                account("Work", 2, State::Healthy),
                account("Personal", 0, State::Healthy),
            ],
        }
    }

    #[test]
    fn summary_leads_with_weekly_and_prints_pooled_capacity() {
        let g = group();
        let lead = lead_window(&g).unwrap();
        assert_eq!(lead.label, "Weekly");
        assert_eq!(
            pooled_text(lead),
            ("212%".to_string(), "of 300% left".to_string())
        );
        assert_eq!(avg_text(lead), "avg 70% · 3 fresh");
        let labels: Vec<&str> = ledger_windows(&g)
            .iter()
            .map(|w| w.label.as_str())
            .collect();
        assert_eq!(labels, ["Weekly", "5-hour", "Fable 7d"]);
    }

    #[test]
    fn stale_values_are_counted_as_excluded() {
        let mut w = group_window("Weekly", Some(64.0), 1);
        w.stale = 1;
        assert_eq!(avg_text(&w), "avg 64% · 1 fresh · 1 stale excluded");
        assert_eq!(
            avg_text(&group_window("Weekly", None, 0)),
            "no fresh values"
        );
    }

    #[test]
    fn reset_range_only_when_resets_differ() {
        let mut w = group_window("Weekly", Some(70.0), 3);
        w.first_reset = Some(4 * 86_400_000);
        w.last_reset = Some(4 * 86_400_000);
        let now = 0;
        assert!(!reset_text(&w, now).unwrap().contains("others by"));
        w.last_reset = Some(6 * 86_400_000);
        assert!(reset_text(&w, now).unwrap().contains("others by"));
        w.first_reset = None;
        assert_eq!(reset_text(&w, now), None);
    }

    #[test]
    fn search_and_sort() {
        let mut g = group();
        g.accounts[0].windows[0].left = Some(10.0);
        assert_eq!(sorted_accounts(&g, Sort::Label, "")[0].label, "Personal");
        assert_eq!(sorted_accounts(&g, Sort::LeastLeft, "")[0].label, "Work");
        assert_eq!(sorted_accounts(&g, Sort::Default, "pers").len(), 1);
        assert_eq!(sorted_accounts(&g, Sort::Default, "acme").len(), 0);
        assert_eq!(Sort::Label.next(), Sort::Default);
    }
}
