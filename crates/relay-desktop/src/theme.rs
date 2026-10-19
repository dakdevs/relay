//! Visual tokens from the approved design (design-brief.md, "Visual tokens").
//! Separation comes from tonal surfaces only: there are no border tokens.

use gpui::{Hsla, Rgba, WindowAppearance, rgb};

/// Corner radii in logical pixels.
pub mod radius {
    pub const POPOVER: f32 = 16.;
    pub const SECTION: f32 = 13.;
    pub const ROW: f32 = 9.;
    pub const BUTTON: f32 = 7.;
    pub const TAG: f32 = 5.;
}

/// Popover width from the design.
pub const POPOVER_WIDTH: f32 = 360.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub canvas: Rgba,
    pub popover: Rgba,
    pub surface: Rgba,
    pub inset: Rgba,
    pub track: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
    pub faint: Rgba,
    pub evergreen: Rgba,
    pub amber: Rgba,
    pub ember: Rgba,
    /// Text drawn on an evergreen fill.
    pub on_accent: Rgba,
    /// Filled tint for focus and selection (never an outline).
    pub focus: Rgba,
    /// Hover tint on rows and secondary buttons.
    pub hover: Rgba,
    /// Pressed shade.
    pub pressed: Rgba,
    /// Soft tag fills.
    pub tag_green: Rgba,
    pub tag_amber: Rgba,
    pub tag_ember: Rgba,
    pub tag_neutral: Rgba,
    /// Secondary button rest gradient, top → bottom.
    pub button_top: Rgba,
    pub button_bottom: Rgba,
    pub shadow: Hsla,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            dark: false,
            canvas: rgb(0xEEF0EF),
            popover: rgb(0xF2F3F2),
            surface: rgb(0xFFFFFF),
            inset: rgb(0xF4F5F4),
            track: rgb(0xE6E9E7),
            text: rgb(0x1C1F1E),
            muted: rgb(0x646A67),
            faint: rgb(0x9AA09D),
            evergreen: rgb(0x1F6B4F),
            amber: rgb(0x9A6614),
            ember: rgb(0xB4432F),
            on_accent: rgb(0xFFFFFF),
            focus: rgb(0xDCEBE3),
            hover: rgb(0xEBEDEC),
            pressed: rgb(0xE1E4E2),
            tag_green: rgb(0xE3EFE8),
            tag_amber: rgb(0xF5EBD8),
            tag_ember: rgb(0xF7E2DD),
            tag_neutral: rgb(0xE8EAE9),
            button_top: rgb(0xFFFFFF),
            button_bottom: rgb(0xF3F4F3),
            shadow: gpui::hsla(150. / 360., 0.08, 0.1, 0.16),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            canvas: rgb(0x121413),
            popover: rgb(0x1B1D1C),
            surface: rgb(0x262927),
            inset: rgb(0x2E3230),
            track: rgb(0x383C3A),
            text: rgb(0xECEFED),
            muted: rgb(0xA3AAA6),
            faint: rgb(0x6F7672),
            evergreen: rgb(0x4DB389),
            amber: rgb(0xD9A140),
            ember: rgb(0xE26A52),
            on_accent: rgb(0x0F2219),
            focus: rgb(0x2C4237),
            hover: rgb(0x303432),
            pressed: rgb(0x232624),
            tag_green: rgb(0x23392E),
            tag_amber: rgb(0x3D3220),
            tag_ember: rgb(0x43271F),
            tag_neutral: rgb(0x333735),
            button_top: rgb(0x343836),
            button_bottom: rgb(0x2C302E),
            shadow: gpui::hsla(0., 0., 0., 0.45),
        }
    }

    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
        }
    }

    /// Bar fill for a quota with `left` percent remaining (board 6: low below 40%).
    pub fn bar_color(&self, left: f32, stale: bool) -> Rgba {
        if stale {
            self.faint
        } else if left <= 0.0 {
            self.ember
        } else if left < 40.0 {
            self.amber
        } else {
            self.evergreen
        }
    }

    /// Primary button gradient (top, bottom) derived from evergreen.
    pub fn primary(&self) -> (Rgba, Rgba) {
        if self.dark {
            (rgb(0x6CC79F), rgb(0x4DB389))
        } else {
            (rgb(0x2A7C5D), rgb(0x1F6B4F))
        }
    }

    pub fn primary_pressed(&self) -> Rgba {
        if self.dark {
            rgb(0x3F9C76)
        } else {
            rgb(0x195A42)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_color_follows_state_thresholds() {
        let t = Theme::light();
        assert_eq!(t.bar_color(72.0, false), rgb(0x1F6B4F));
        assert_eq!(t.bar_color(40.0, false), rgb(0x1F6B4F));
        assert_eq!(t.bar_color(39.9, false), rgb(0x9A6614));
        assert_eq!(t.bar_color(0.0, false), rgb(0xB4432F));
        assert_eq!(t.bar_color(90.0, true), t.faint);
    }

    #[test]
    fn appearance_selects_palette() {
        assert!(Theme::for_appearance(WindowAppearance::VibrantDark).dark);
        assert!(!Theme::for_appearance(WindowAppearance::Light).dark);
    }
}
