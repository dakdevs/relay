//! Through Relay sparkline (board 9), shared by the tray footer and the Usage
//! tab: current span solid, same-elapsed prior span dashed, a hollow end dot on
//! the partial bucket, gaps over a tinted band for missing days, and a
//! baseline dot for real zeros. Telemetry updates in place: no draw-on.

use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, ElementId, IntoElement, ParentElement, PathBuilder, Pixels, Point,
    Rgba, Styled, Window, canvas, div, point, px,
};

use crate::{
    model::{Period, Series},
    motion::{self, Tween},
    theme::Theme,
};

struct Fade {
    period: Period,
    shown: Series,
    previous: Option<Series>,
    tween: Tween,
}

/// Sparkline that crossfades (opacity only, 120 ms) when the Week/Month
/// period changes. Values are never interpolated between periods; reduced
/// motion swaps instantly. New data within a period updates in place.
#[allow(clippy::too_many_arguments)]
pub fn crossfade(
    id: impl Into<ElementId>,
    series: &Series,
    period: Period,
    width: f32,
    height: f32,
    highlight: Option<usize>,
    reduced: bool,
    t: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let state = window.use_keyed_state(id.into(), cx, |_, _| Fade {
        period,
        shown: series.clone(),
        previous: None,
        tween: Tween::settled(1.0),
    });
    let now = Instant::now();
    let (previous, p) = state.update(cx, |f, _| {
        if f.period != period {
            f.previous = Some(std::mem::replace(&mut f.shown, series.clone()));
            f.period = period;
            f.tween = Tween::settled(0.0);
            f.tween.retarget(1.0, motion::CROSSFADE, now, reduced);
        } else {
            f.shown = series.clone();
        }
        if !f.tween.is_animating(now) {
            f.previous = None;
        }
        (f.previous.clone(), f.tween.value_at(now))
    });
    if previous.is_some() {
        window.request_animation_frame();
    }
    div()
        .relative()
        .w(px(width))
        .h(px(height))
        .flex_none()
        .children(previous.map(|prev| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .opacity(1.0 - p)
                .child(sparkline(&prev, width, height, None, t))
        }))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .opacity(p)
                .child(sparkline(series, width, height, highlight, t)),
        )
        .into_any_element()
}

/// Normalised geometry for one line: `None` where the bucket is missing.
pub fn line_points(
    values: &[Option<u64>],
    slots: usize,
    max: u64,
    width: f32,
    height: f32,
) -> Vec<Option<(f32, f32)>> {
    let slots = slots.max(2);
    let step = width / (slots - 1) as f32;
    let max = max.max(1) as f32;
    values
        .iter()
        .enumerate()
        .map(|(i, v)| v.map(|v| (i as f32 * step, height - (v as f32 / max) * height)))
        .collect()
}

/// Contiguous runs of present points; a missing bucket breaks the line.
pub fn runs(points: &[Option<(f32, f32)>]) -> Vec<Vec<(f32, f32)>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for p in points {
        match p {
            Some(p) => cur.push(*p),
            None if !cur.is_empty() => out.push(std::mem::take(&mut cur)),
            None => {}
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn series_max(series: &Series) -> u64 {
    series
        .current
        .iter()
        .chain(&series.prior)
        .flatten()
        .copied()
        .max()
        .unwrap_or(0)
}

/// Draws the sparkline into `width` × `height`. `highlight` marks a focused day.
pub fn sparkline(
    series: &Series,
    width: f32,
    height: f32,
    highlight: Option<usize>,
    t: &Theme,
) -> impl IntoElement {
    let series = series.clone();
    let t = *t;
    canvas(
        move |_, _, _| (),
        move |bounds: Bounds<Pixels>, _, window: &mut Window, _: &mut App| {
            paint(&series, bounds, highlight, &t, window);
        },
    )
    .w(px(width))
    .h(px(height))
    .flex_none()
}

fn paint(
    series: &Series,
    bounds: Bounds<Pixels>,
    highlight: Option<usize>,
    t: &Theme,
    window: &mut Window,
) {
    let pad = 3.0;
    let w = f32::from(bounds.size.width) - pad * 2.0;
    let h = f32::from(bounds.size.height) - pad * 2.0;
    let origin = bounds.origin + point(px(pad), px(pad));
    let at = |(x, y): (f32, f32)| -> Point<Pixels> { origin + point(px(x), px(y)) };
    let slots = series
        .slots
        .max(series.current.len())
        .max(series.prior.len());

    // Baseline.
    stroke(window, &[at((0.0, h)), at((w, h))], px(1.0), t.track);
    if !series.has_traffic() {
        return;
    }
    let max = series_max(series);
    let step = w / (slots.max(2) - 1) as f32;

    // Missing current days: tinted band.
    let last = series.current.len().saturating_sub(1);
    for (i, v) in series.current.iter().enumerate() {
        if v.is_none() {
            let x = i as f32 * step;
            let band = gpui::Bounds::new(
                at((x - step * 0.35, 0.0)),
                gpui::size(px(step * 0.7), px(h)),
            );
            window.paint_quad(gpui::fill(band, t.track).corner_radii(px(2.)));
        }
    }

    // Prior span: dashed.
    for run in runs(&line_points(&series.prior, slots, max, w, h)) {
        dashed(
            window,
            &run.iter().map(|p| at(*p)).collect::<Vec<_>>(),
            t.faint,
        );
    }

    // Current span: solid.
    let current = line_points(&series.current, slots, max, w, h);
    for run in runs(&current) {
        if run.len() >= 2 {
            stroke(
                window,
                &run.iter().map(|p| at(*p)).collect::<Vec<_>>(),
                px(1.8),
                t.evergreen,
            );
        }
    }
    // Real zeros get a baseline dot; lone points get a dot so they stay visible.
    for (i, p) in current.iter().enumerate() {
        if let Some(p) = p {
            let zero = series.current[i] == Some(0);
            let alone = (i == 0 || current[i - 1].is_none())
                && current.get(i + 1).is_none_or(|n| n.is_none());
            if (zero || alone) && i != last {
                dot(window, at(*p), 2.2, t.evergreen, None);
            }
        }
    }
    // Partial current bucket: hollow end dot.
    if let Some(Some(p)) = current.get(last) {
        let bg = if t.dark {
            t.surface
        } else {
            gpui::rgb(0xFFFFFF)
        };
        dot(window, at(*p), 3.2, t.evergreen, Some(bg));
    }
    if let Some(i) = highlight.filter(|i| *i < slots) {
        let x = i as f32 * step;
        stroke(window, &[at((x, 0.0)), at((x, h))], px(1.0), t.faint);
        if let Some(Some(p)) = current.get(i) {
            dot(window, at(*p), 3.0, t.evergreen, None);
        }
    }
}

fn stroke(window: &mut Window, pts: &[Point<Pixels>], width: Pixels, color: Rgba) {
    if pts.len() < 2 {
        return;
    }
    let mut b = PathBuilder::stroke(width);
    b.move_to(pts[0]);
    for p in &pts[1..] {
        b.line_to(*p);
    }
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

fn dashed(window: &mut Window, pts: &[Point<Pixels>], color: Rgba) {
    let (dash, gap) = (3.0f32, 2.5f32);
    for seg in pts.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let (dx, dy) = (f32::from(b.x - a.x), f32::from(b.y - a.y));
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            continue;
        }
        let mut s = 0.0;
        while s < len {
            let e = (s + dash).min(len);
            let p0 = a + point(px(dx * s / len), px(dy * s / len));
            let p1 = a + point(px(dx * e / len), px(dy * e / len));
            stroke(window, &[p0, p1], px(1.2), color);
            s += dash + gap;
        }
    }
}

fn dot(window: &mut Window, c: Point<Pixels>, r: f32, color: Rgba, hollow: Option<Rgba>) {
    let b = gpui::Bounds::new(
        c - point(px(r), px(r)),
        gpui::size(px(r * 2.0), px(r * 2.0)),
    );
    window.paint_quad(gpui::fill(b, color).corner_radii(px(r)));
    if let Some(inner) = hollow {
        let ri = r - 1.4;
        let bi = gpui::Bounds::new(
            c - point(px(ri), px(ri)),
            gpui::size(px(ri * 2.0), px(ri * 2.0)),
        );
        window.paint_quad(gpui::fill(bi, inner).corner_radii(px(ri)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_scale_to_max_and_skip_missing() {
        let pts = line_points(&[Some(0), Some(50), None, Some(100)], 7, 100, 60.0, 20.0);
        assert_eq!(pts[0], Some((0.0, 20.0)));
        assert_eq!(pts[1], Some((10.0, 10.0)));
        assert_eq!(pts[2], None);
        assert_eq!(pts[3], Some((30.0, 0.0)));
    }

    #[test]
    fn missing_bucket_breaks_the_line_but_zero_does_not() {
        let gap = line_points(&[Some(1), None, Some(3)], 3, 3, 10.0, 10.0);
        assert_eq!(runs(&gap).len(), 2);
        let zero = line_points(&[Some(1), Some(0), Some(3)], 3, 3, 10.0, 10.0);
        assert_eq!(runs(&zero).len(), 1);
        assert_eq!(runs(&zero)[0].len(), 3);
    }

    #[test]
    fn all_zero_series_does_not_divide_by_zero() {
        let pts = line_points(&[Some(0), Some(0)], 7, 0, 60.0, 20.0);
        assert_eq!(pts[1], Some((10.0, 20.0)));
    }
}
