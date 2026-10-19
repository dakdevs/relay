//! Interruptible motion (design-brief.md, "Motion spec").
//!
//! A [`Tween`] retargets from its *current* presented value, so reversing an
//! animation mid-way never jumps. Durations are the design's; every curve is
//! the ease-out `cubic-bezier(0.23, 1, 0.32, 1)`, never ease-in or overshoot.

use std::time::{Duration, Instant};

pub const PRESS_RELEASE: Duration = Duration::from_millis(120);
pub const HOVER: Duration = Duration::from_millis(120);
pub const POPOVER_OPEN: Duration = Duration::from_millis(160);
pub const POPOVER_CLOSE: Duration = Duration::from_millis(120);
pub const REDUCED_FADE: Duration = Duration::from_millis(120);
pub const EXPAND: Duration = Duration::from_millis(180);
pub const PILL: Duration = Duration::from_millis(160);
pub const CROSSFADE: Duration = Duration::from_millis(120);

/// Evaluates `cubic-bezier(x1, y1, x2, y2)` at progress `t` in 0..=1.
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 || t == 1.0 {
        return t;
    }
    let bez = |a: f32, b: f32, s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    // Solve x(s) = t by bisection; x is monotonic for x1, x2 in 0..=1.
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..32 {
        let mid = (lo + hi) / 2.0;
        if bez(x1, x2, mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez(y1, y2, (lo + hi) / 2.0)
}

/// The design's single ease-out curve.
pub fn ease_out(t: f32) -> f32 {
    cubic_bezier(0.23, 1.0, 0.32, 1.0, t)
}

/// A scalar that animates toward a target and can be retargeted at any time.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
}

impl Tween {
    pub fn settled(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            start: Instant::now(),
            duration: Duration::ZERO,
        }
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    pub fn value_at(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return self.to;
        }
        let t =
            now.saturating_duration_since(self.start).as_secs_f32() / self.duration.as_secs_f32();
        self.from + (self.to - self.from) * ease_out(t)
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        !self.duration.is_zero() && now.saturating_duration_since(self.start) < self.duration
    }

    /// Animates toward `to` from wherever the value is presented at `now`.
    /// With reduced motion (or a zero duration) the value jumps immediately.
    pub fn retarget(&mut self, to: f32, duration: Duration, now: Instant, reduced: bool) {
        if (to - self.to).abs() < f32::EPSILON && self.is_animating(now) {
            return;
        }
        let current = self.value_at(now);
        let duration = if reduced { Duration::ZERO } else { duration };
        *self = Self {
            from: current,
            to,
            start: now,
            duration,
        };
    }
}

/// Reads the operating system's reduced-motion preference.
///
/// macOS: `com.apple.universalaccess reduceMotion`. Linux (GNOME/GTK):
/// `org.gnome.desktop.interface enable-animations` false. Missing tools or keys
/// mean "not reduced". This spawns a process, so call it off the UI thread.
pub fn system_prefers_reduced_motion() -> bool {
    #[cfg(target_os = "macos")]
    {
        command_output(
            "defaults",
            &["read", "com.apple.universalaccess", "reduceMotion"],
        )
        .is_some_and(|out| out.trim() == "1")
    }
    #[cfg(target_os = "linux")]
    {
        command_output(
            "gsettings",
            &["get", "org.gnome.desktop.interface", "enable-animations"],
        )
        .is_some_and(|out| out.trim() == "false")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_is_monotonic_bounded_and_front_loaded() {
        let mut prev = 0.0;
        for i in 0..=100 {
            let v = ease_out(i as f32 / 100.0);
            assert!(v >= prev - 1e-4, "not monotonic at {i}");
            assert!((0.0..=1.0 + 1e-4).contains(&v), "overshoot at {i}: {v}");
            prev = v;
        }
        // Ease-out: well past halfway at a quarter of the time.
        assert!(ease_out(0.25) > 0.6);
    }

    #[test]
    fn linear_bezier_is_identity() {
        for t in [0.1, 0.33, 0.5, 0.9] {
            assert!((cubic_bezier(0.25, 0.25, 0.75, 0.75, t) - t).abs() < 1e-3);
        }
    }

    #[test]
    fn retarget_mid_animation_starts_from_presented_value() {
        let t0 = Instant::now();
        let mut tw = Tween::settled(0.0);
        tw.retarget(1.0, Duration::from_millis(180), t0, false);
        let mid = t0 + Duration::from_millis(60);
        let presented = tw.value_at(mid);
        assert!(presented > 0.0 && presented < 1.0);
        tw.retarget(0.0, Duration::from_millis(180), mid, false);
        // No jump on reversal.
        assert!((tw.value_at(mid) - presented).abs() < 1e-5);
        assert_eq!(tw.value_at(mid + Duration::from_millis(400)), 0.0);
    }

    #[test]
    fn reduced_motion_jumps() {
        let now = Instant::now();
        let mut tw = Tween::settled(0.0);
        tw.retarget(1.0, EXPAND, now, true);
        assert_eq!(tw.value_at(now), 1.0);
        assert!(!tw.is_animating(now));
    }

    #[test]
    fn all_durations_within_bounds() {
        for d in [
            PRESS_RELEASE,
            HOVER,
            POPOVER_OPEN,
            POPOVER_CLOSE,
            REDUCED_FADE,
            EXPAND,
            PILL,
        ] {
            assert!(d <= Duration::from_millis(300));
        }
    }
}
