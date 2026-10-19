//! Usage tab (board 5): the Through Relay comparison at full size, then
//! per-account token totals measured by Relay — summed, never averaged.

use gpui::{
    AnyElement, App, Entity, FontWeight, ParentElement, SharedString, Styled, Window, div,
    prelude::*, px, relative,
};

use crate::{
    chart, format,
    model::{Period, Range, UsageRow},
    store::{Command, Store},
    through,
    widgets::{self, caps, ui},
};

pub fn totals(rows: &[UsageRow]) -> UsageRow {
    rows.iter().fold(
        UsageRow {
            label: "Total".into(),
            sublabel: "all accounts".into(),
            ..Default::default()
        },
        |mut acc, r| {
            acc.input += r.input;
            acc.output += r.output;
            acc.cached += r.cached;
            acc.requests += r.requests;
            acc.errors += r.errors;
            acc
        },
    )
}

/// Share of input + output tokens, in percent, per row.
pub fn shares(rows: &[UsageRow]) -> Vec<f32> {
    let total: u64 = rows.iter().map(|r| r.input + r.output).sum();
    rows.iter()
        .map(|r| {
            if total == 0 {
                0.0
            } else {
                (r.input + r.output) as f32 / total as f32 * 100.0
            }
        })
        .collect()
}

pub fn view(store: &Entity<Store>, window: &mut Window, cx: &mut App) -> AnyElement {
    let t = ui(cx).theme;
    let (series, period, rows, range) = {
        let s = store.read(cx);
        (
            s.overview.through.get(s.period).clone(),
            s.period,
            s.usage_rows.clone(),
            s.usage_range,
        )
    };
    let st = store.clone();
    let period_toggle = widgets::segmented(
        "usage-period",
        &["Week", "Month"],
        if period == Period::Week { 0 } else { 1 },
        58.,
        move |i, _, cx| {
            st.update(cx, |s, cx| {
                s.period = if i == 0 { Period::Week } else { Period::Month };
                cx.notify();
            })
        },
        window,
        cx,
    );
    let st = store.clone();
    let range_toggle = widgets::segmented(
        "usage-range",
        &["24 hours", "7 days", "30 days"],
        match range {
            Range::Day => 0,
            Range::Week => 1,
            Range::Month => 2,
        },
        72.,
        move |i, _, cx| {
            st.update(cx, |s, cx| {
                s.usage_range = [Range::Day, Range::Week, Range::Month][i];
                let range = s.usage_range;
                s.send_or_report(Command::UsageRange(range), crate::store::Scope::App);
                cx.notify();
            })
        },
        window,
        cx,
    );

    let chart_w = 560.;
    let top = widgets::surface(&t)
        .p(px(22.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .id("usage-through")
        .role(gpui::Role::Group)
        .aria_label(SharedString::from(through::accessible_summary(&series, period)))
        .child(
            div()
                .flex()
                .items_center()
                .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child("Through Relay"))
                .child(div().ml(px(8.)).text_size(px(13.)).text_color(t.muted).child("input + output tokens"))
                .child(div().flex_1())
                .child(period_toggle),
        )
        .child(
            div()
                .flex()
                .gap(px(28.))
                .child(
                    div()
                        .w(px(260.))
                        .flex_none()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    widgets::num(series.total.map_or("—".to_string(), format::tokens))
                                        .text_size(px(40.))
                                        .font_weight(FontWeight::SEMIBOLD),
                                )
                                .children(through::delta_tag(&series, &t)),
                        )
                        .child(div().text_size(px(13.)).text_color(t.muted).child(series.caption.clone()))
                        .child(div().text_size(px(13.)).text_color(t.muted).child(series.span.clone()))
                        .when(series.has_traffic(), |d| {
                            d.child(
                                div()
                                    .mt(px(14.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .text_size(px(12.5))
                                    .text_color(t.muted)
                                    .child(div().flex().items_center().gap(px(8.)).child(div().w(px(14.)).h(px(2.)).bg(t.evergreen)).child(series.legend_current.clone()))
                                    .child(div().flex().items_center().gap(px(8.)).child(div().text_color(t.faint).child("╌╌")).child(series.legend_prior.clone())),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(interactive_chart(store, &series, chart_w, &t, window, cx))
                        .child(
                            div()
                                .w(px(chart_w))
                                .flex()
                                .justify_between()
                                .text_size(px(11.5))
                                .text_color(t.muted)
                                .children(axis_labels(&series.day_labels).into_iter().map(|l| div().child(l))),
                        ),
                )
                .child(
                    div()
                        .w(px(200.))
                        .flex_none()
                        .flex()
                        .items_end()
                        .text_size(px(12.5))
                        .text_color(t.muted)
                        .child("Missing days break the line; zero days sit on the baseline. Counts only requests that passed through Relay."),
                ),
        );

    let share = shares(&rows);
    let total = totals(&rows);
    let num_col = |s: String| widgets::num(s).w(px(96.)).text_right().text_size(px(13.5));
    let head = div()
        .flex()
        .items_center()
        .h(px(32.))
        .px(px(14.))
        .child(caps("Account", t.muted).w(px(280.)))
        .children(
            ["Input", "Output", "Cached", "Requests", "Errors"].map(|h| {
                div()
                    .w(px(96.))
                    .flex()
                    .justify_end()
                    .child(caps(h, t.muted))
            }),
        )
        .child(caps("Share of tokens", t.muted).ml(px(32.)));
    let body_rows = rows.iter().zip(share).enumerate().map(|(i, (r, sh))| {
        div()
            .flex()
            .items_center()
            .h(px(48.))
            .px(px(14.))
            .rounded(px(9.))
            .when(i % 2 == 0, |d| d.bg(t.inset))
            .child(
                div()
                    .w(px(280.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .children(r.mark.map(|m| widgets::mark(m, 15., &t)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(r.label.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(t.muted)
                                    .child(r.sublabel.clone()),
                            ),
                    ),
            )
            .child(num_col(format::tokens(r.input)))
            .child(num_col(format::tokens(r.output)))
            .child(num_col(format::tokens(r.cached)))
            .child(num_col(format::grouped(r.requests)))
            .child(num_col(format::grouped(r.errors)))
            .child(
                div()
                    .ml(px(32.))
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(
                        div().flex_1().h(px(4.)).rounded_full().bg(t.track).child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(t.evergreen)
                                .w(relative(sh / 100.0)),
                        ),
                    )
                    .child(
                        widgets::num(format!("{sh:.0}%"))
                            .w(px(40.))
                            .text_right()
                            .text_color(t.muted),
                    ),
            )
    });
    let table = widgets::surface(&t)
        .p(px(20.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .mb(px(8.))
                .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child("By account"))
                .child(div().ml(px(8.)).text_size(px(13.)).text_color(t.muted).child("measured by Relay · summed, never averaged"))
                .child(div().flex_1())
                .child(range_toggle),
        )
        .child(head)
        .when(rows.is_empty(), |d| {
            d.child(div().px(px(14.)).py(px(16.)).text_size(px(13.)).text_color(t.muted).child("No requests through Relay in this range."))
        })
        .children(body_rows)
        .when(!rows.is_empty(), |d| {
            d.child(
                div()
                    .mt(px(4.))
                    .flex()
                    .items_center()
                    .h(px(48.))
                    .px(px(14.))
                    .rounded(px(9.))
                    .bg(t.inset)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(div().w(px(280.)).flex().flex_col().child("Total").child(div().text_size(px(11.5)).font_weight(FontWeight::NORMAL).text_color(t.muted).child("all accounts")))
                    .child(num_col(format::tokens(total.input)))
                    .child(num_col(format::tokens(total.output)))
                    .child(num_col(format::tokens(total.cached)))
                    .child(num_col(format::grouped(total.requests)))
                    .child(num_col(format::grouped(total.errors))),
            )
        })
        .child(div().mt(px(10.)).text_size(px(12.)).text_color(t.muted).child(
            "Counts only requests that passed through Relay, so totals will not match provider dashboards.",
        ));

    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(top)
        .child(table)
        .into_any_element()
}

/// Slot index for a pointer x within the chart (nearest day).
pub fn day_at(x: f32, left: f32, width: f32, slots: usize) -> usize {
    let slots = slots.max(2);
    let step = (width - 6.0) / (slots - 1) as f32;
    (((x - left - 3.0) / step).round().max(0.0) as usize).min(slots - 1)
}

fn interactive_chart(
    store: &Entity<Store>,
    series: &crate::model::Series,
    width: f32,
    t: &crate::theme::Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (hover, bounds_cell, period) = {
        let s = store.read(cx);
        (
            s.hover_day.filter(|i| *i < series.current.len()),
            s.chart_bounds.clone(),
            s.period,
        )
    };
    let slots = series.slots.max(series.current.len());
    let last = series.current.len().saturating_sub(1);
    let detail = hover.and_then(|i| through::day_detail(series, i, period));
    let (st_move, st_hover, st_key) = (store.clone(), store.clone(), store.clone());
    let cell = bounds_cell.clone();
    let reduced = crate::widgets::ui(cx).reduced_motion;
    let spark = chart::crossfade(
        "usage-spark",
        series,
        period,
        width,
        110.,
        hover,
        reduced,
        t,
        window,
        cx,
    );
    div()
        .id("usage-chart")
        .role(gpui::Role::Group)
        .aria_label(SharedString::from(detail.as_ref().map_or(
            "Daily tokens. Use left and right arrows to read each day.".to_string(),
            |(a, b, c)| format!("{a}. {b}. {c}."),
        )))
        .focusable()
        .tab_stop(true)
        .relative()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div().relative().child(spark).child(
                gpui::canvas(move |b, _, _| cell.set(b), |_, _, _, _| {})
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            ),
        )
        .on_mouse_move(move |e, _, cx| {
            let b = bounds_cell.get();
            let i = day_at(
                f32::from(e.position.x),
                f32::from(b.origin.x),
                f32::from(b.size.width),
                slots,
            )
            .min(last);
            st_move.update(cx, |s, cx| {
                if s.hover_day != Some(i) {
                    s.hover_day = Some(i);
                    cx.notify();
                }
            });
        })
        .on_hover(move |hovered, _, cx| {
            if !hovered {
                st_hover.update(cx, |s, cx| {
                    s.hover_day = None;
                    cx.notify();
                });
            }
        })
        .on_key_down(move |e, _, cx| {
            let step: isize = match e.keystroke.key.as_str() {
                "left" => -1,
                "right" => 1,
                _ => return,
            };
            st_key.update(cx, |s, cx| {
                // The marker jumps to the next day; it does not chase.
                let cur = s.hover_day.unwrap_or(last) as isize;
                s.hover_day = Some((cur + step).clamp(0, last as isize) as usize);
                cx.notify();
            });
        })
        .children(detail.map(|(title, cur, prior)| {
            div()
                .absolute()
                .top(px(116.))
                .left_0()
                .bg(t.inset)
                .rounded(px(9.))
                .px(px(12.))
                .py(px(8.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .text_size(px(12.5))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                .child(
                    div()
                        .flex()
                        .gap(px(14.))
                        .text_color(t.muted)
                        .child(format!("— {cur}"))
                        .child(format!("╌ {prior}")),
                )
        }))
        .into_any_element()
}

/// At most 7 evenly spaced labels so a month axis stays readable.
pub fn axis_labels(labels: &[String]) -> Vec<String> {
    if labels.len() <= 7 {
        return labels.to_vec();
    }
    let step = (labels.len() - 1) as f32 / 6.0;
    (0..7)
        .map(|i| labels[((i as f32 * step).round() as usize).min(labels.len() - 1)].clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(input: u64, output: u64) -> UsageRow {
        UsageRow {
            input,
            output,
            cached: 10,
            requests: 2,
            errors: 1,
            ..Default::default()
        }
    }

    #[test]
    fn totals_are_sums_and_shares_add_to_100() {
        let rows = [row(3_100_000, 412_000), row(900_000, 160_000)];
        let total = totals(&rows);
        assert_eq!(total.input, 4_000_000);
        assert_eq!(total.output, 572_000);
        assert_eq!(total.requests, 4);
        let s = shares(&rows);
        assert!((s.iter().sum::<f32>() - 100.0).abs() < 0.01);
        assert!(s[0] > s[1]);
        assert_eq!(shares(&[row(0, 0)]), vec![0.0]);
    }

    #[test]
    fn pointer_maps_to_nearest_day() {
        // 7 slots over 606 px of plot (3 px padding each side): step 100.
        assert_eq!(day_at(3.0, 0.0, 606.0, 7), 0);
        assert_eq!(day_at(152.0, 0.0, 606.0, 7), 1);
        assert_eq!(day_at(10_000.0, 0.0, 606.0, 7), 6);
        assert_eq!(day_at(-50.0, 0.0, 606.0, 7), 0);
    }

    #[test]
    fn month_axis_is_thinned() {
        let labels: Vec<String> = (1..=31).map(|d| d.to_string()).collect();
        let a = axis_labels(&labels);
        assert_eq!(a.len(), 7);
        assert_eq!(a.first().unwrap(), "1");
        assert_eq!(a.last().unwrap(), "31");
    }
}
