//! "Through Relay" block: total, delta or muted state label, caption,
//! sparkline and legend. One component for the tray footer and the larger
//! Usage-tab chart (board 5, board 9).

use gpui::{
    AnyElement, App, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px,
};

use crate::{
    chart, format,
    model::{Comparison, Period, Series},
    store::Store,
    widgets::{self, Tone, ui},
};

/// Accessible summary (board 9): "Tokens through Relay, week to date 7.4
/// million, up 17 percent versus 6.3 million over the same span last week."
pub fn accessible_summary(series: &Series, period: Period) -> String {
    let span = match period {
        Period::Week => "week",
        Period::Month => "month",
    };
    let Some(total) = series.total else {
        return "Tokens through Relay: no requests through Relay yet.".into();
    };
    let spoken = |n: u64| {
        format::tokens(n)
            .replace('M', " million")
            .replace('K', " thousand")
            .replace('B', " billion")
    };
    let mut s = format!("Tokens through Relay, {span} to date {}", spoken(total));
    match (&series.comparison, series.prior_total) {
        (Some(Comparison::Delta(d)), Some(p)) => {
            let dir = if *d >= 0.0 { "up" } else { "down" };
            s.push_str(&format!(
                ", {dir} {:.0} percent versus {} over the same span last {span}",
                d.abs().round(),
                spoken(p)
            ));
        }
        (Some(c), _) => {
            if let Some(label) = c.muted_label() {
                s.push_str(&format!(", {}", label.to_lowercase()));
            }
        }
        (None, _) => {}
    }
    s.push('.');
    s
}

pub fn delta_tag(series: &Series, t: &crate::theme::Theme) -> Option<gpui::Div> {
    match series.comparison.as_ref()? {
        Comparison::Delta(d) => {
            let tone = if *d >= 0.0 {
                Tone::Green
            } else {
                Tone::Neutral
            };
            Some(widgets::tag(format::delta(*d), tone, t))
        }
        Comparison::Incomplete => Some(widgets::tag("Incomplete", Tone::Amber, t)),
        other => other
            .muted_label()
            .map(|l| widgets::tag(l, Tone::Neutral, t)),
    }
}

/// Hover/focus detail for day `i` (board 9): title, this-span value and the
/// matching prior day. Missing days read "no data", never 0.
pub fn day_detail(series: &Series, i: usize, period: Period) -> Option<(String, String, String)> {
    let title = series.current_dates.get(i)?.clone();
    let value = |v: Option<&Option<u64>>| match v {
        Some(Some(n)) => format::tokens(*n),
        _ => "no data".to_string(),
    };
    let (this, last) = match period {
        Period::Week => ("This week", "Last week"),
        Period::Month => ("This month", "Last month"),
    };
    let current = format!("{this} {}", value(series.current.get(i)));
    let prior = match series.prior_dates.get(i) {
        Some(d) => format!("{last} ({d}) {}", value(series.prior.get(i))),
        None => format!("{last} — no matching day"),
    };
    Some((title, current, prior))
}

/// Tray footer variant.
pub fn compact(store: &gpui::Entity<Store>, window: &mut Window, cx: &mut App) -> AnyElement {
    let t = ui(cx).theme;
    let (series, period) = {
        let s = store.read(cx);
        (s.overview.through.get(s.period).clone(), s.period)
    };
    let store_for_toggle = store.clone();
    let toggle = widgets::segmented(
        "through-period",
        &["Week", "Month"],
        if period == Period::Week { 0 } else { 1 },
        46.,
        move |i, _, cx| {
            store_for_toggle.update(cx, |s, cx| {
                s.period = if i == 0 { Period::Week } else { Period::Month };
                cx.notify();
            })
        },
        window,
        cx,
    );
    let total = series.total.map_or("—".to_string(), format::tokens);
    div()
        .id("through")
        .role(gpui::Role::Group)
        .aria_label(SharedString::from(accessible_summary(&series, period)))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text)
                        .child("Through Relay"),
                )
                .child(
                    div()
                        .ml(px(6.))
                        .text_size(px(12.5))
                        .text_color(t.muted)
                        .child("tokens"),
                )
                .child(div().flex_1())
                .child(toggle),
        )
        .child(
            div()
                .mt(px(8.))
                .flex()
                .items_end()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    widgets::num(total)
                                        .text_size(px(24.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(t.text),
                                )
                                .children(delta_tag(&series, &t)),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(t.muted)
                                .truncate()
                                .child(series.caption.clone()),
                        ),
                )
                .child(chart::crossfade(
                    "footer-spark",
                    &series,
                    period,
                    118.,
                    34.,
                    None,
                    ui(cx).reduced_motion,
                    &t,
                    window,
                    cx,
                )),
        )
        .when(series.has_traffic(), |d| d.child(legend(&series, &t)))
        .into_any_element()
}

pub fn legend(series: &Series, t: &crate::theme::Theme) -> gpui::Div {
    div()
        .mt(px(6.))
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(11.5))
        .text_color(t.muted)
        .child(div().w(px(12.)).h(px(2.)).rounded_full().bg(t.evergreen))
        .child(series.legend_current.clone())
        .child(div().ml(px(10.)).text_color(t.faint).child("╌╌"))
        .child(series.legend_prior.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_detail_pairs_matching_days_and_never_shows_missing_as_zero() {
        let s = Series {
            current: vec![Some(1_600_000), None],
            prior: vec![Some(1_200_000), Some(0)],
            current_dates: vec!["Thu Oct 15".into(), "Fri Oct 16".into()],
            prior_dates: vec!["Thu Oct 8".into(), "Fri Oct 9".into()],
            ..Default::default()
        };
        assert_eq!(
            day_detail(&s, 0, Period::Week),
            Some((
                "Thu Oct 15".into(),
                "This week 1.6M".into(),
                "Last week (Thu Oct 8) 1.2M".into()
            ))
        );
        let (_, cur, prior) = day_detail(&s, 1, Period::Week).unwrap();
        assert_eq!(cur, "This week no data");
        assert_eq!(prior, "Last week (Fri Oct 9) 0");
        assert_eq!(day_detail(&s, 5, Period::Week), None);
    }

    #[test]
    fn accessible_summary_matches_board_wording() {
        let s = Series {
            total: Some(7_400_000),
            prior_total: Some(6_300_000),
            comparison: Some(Comparison::Delta(17.4)),
            ..Default::default()
        };
        assert_eq!(
            accessible_summary(&s, Period::Week),
            "Tokens through Relay, week to date 7.4 million, up 17 percent versus 6.3 million over the same span last week."
        );
        let early = Series {
            total: Some(200_000),
            comparison: Some(Comparison::TooEarly),
            ..Default::default()
        };
        assert_eq!(
            accessible_summary(&early, Period::Week),
            "Tokens through Relay, week to date 200 thousand, too early."
        );
        assert_eq!(
            accessible_summary(&Series::default(), Period::Month),
            "Tokens through Relay: no requests through Relay yet."
        );
    }
}
