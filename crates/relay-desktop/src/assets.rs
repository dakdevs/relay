//! Compiled-in SVG assets: official brand marks (see design/assets/logo-sources.md)
//! and Relay's own monochrome UI icons.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

macro_rules! embed {
    ($($path:literal),* $(,)?) => {
        &[$(($path, include_bytes!(concat!("../assets/", $path)) as &[u8])),*]
    };
}

static FILES: &[(&str, &[u8])] = embed![
    "logos/anthropic-symbol-slate.svg",
    "logos/anthropic-symbol-ivory.svg",
    "logos/claude-spark-clay.svg",
    "logos/openai-symbol.svg",
    "logos/chatgpt-logo-black.svg",
    "logos/chatgpt-logo-white.svg",
    "icons/arrow-up-right.svg",
    "icons/check.svg",
    "icons/chevron-right.svg",
    "icons/close.svg",
    "icons/copy.svg",
    "icons/gear.svg",
    "icons/pause.svg",
    "icons/play.svg",
    "icons/plus.svg",
    "icons/refresh.svg",
    "icons/search.svg",
    "icons/tray.svg",
];

pub struct Assets;

impl Assets {
    pub fn get(path: &str) -> Option<&'static [u8]> {
        FILES.iter().find(|(p, _)| *p == path).map(|(_, b)| *b)
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}

/// Brand marks. The provider symbol identifies the provider; the product mark
/// identifies the plan (Claude spark, ChatGPT logo). There is no Codex mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Anthropic,
    Claude,
    OpenAi,
    ChatGpt,
}

impl Mark {
    /// Asset path for the mark on a light or dark surface.
    pub fn path(self, dark: bool) -> &'static str {
        match (self, dark) {
            (Mark::Anthropic, false) => "logos/anthropic-symbol-slate.svg",
            (Mark::Anthropic, true) => "logos/anthropic-symbol-ivory.svg",
            (Mark::Claude, _) => "logos/claude-spark-clay.svg",
            (Mark::OpenAi, _) => "logos/openai-symbol.svg",
            (Mark::ChatGpt, false) => "logos/chatgpt-logo-black.svg",
            (Mark::ChatGpt, true) => "logos/chatgpt-logo-white.svg",
        }
    }

    /// Marks with no fill (the OpenAI symbol) render as a tinted mask so they
    /// follow the text colour in dark mode; the rest keep their own colours.
    pub fn is_mask(self) -> bool {
        matches!(self, Mark::OpenAi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mark_and_icon_is_embedded_and_parses_as_svg() {
        for mark in [Mark::Anthropic, Mark::Claude, Mark::OpenAi, Mark::ChatGpt] {
            for dark in [false, true] {
                let bytes = Assets::get(mark.path(dark)).expect("mark embedded");
                assert!(std::str::from_utf8(bytes).unwrap().contains("<svg"));
            }
        }
        for (path, bytes) in FILES {
            assert!(
                std::str::from_utf8(bytes).unwrap().contains("<svg"),
                "{path} is not SVG"
            );
        }
    }

    #[test]
    fn embedded_brand_marks_match_recorded_provenance_hashes() {
        // Hashes from design/assets/logo-sources.md: the marks are unmodified.
        let expected = [
            (
                "logos/anthropic-symbol-slate.svg",
                "d55918884053460f24c958fca3fba8d10b48fdd86e73ba3cd1237713bc0a03bb",
            ),
            (
                "logos/claude-spark-clay.svg",
                "6d53db4be375e899c937c26cf16684a80d6e869b1928d72b37748bef2560e219",
            ),
            (
                "logos/openai-symbol.svg",
                "0dd4dd71846aeb7a484acdc59eb08eac2b3c1264a143d11bca9b73e4a8cacfbf",
            ),
            (
                "logos/chatgpt-logo-black.svg",
                "b8b9aba7ba591f54bf1805f422bc23f32c44ee993643cc378cc77f766061f7a1",
            ),
        ];
        for (path, hash) in expected {
            assert_eq!(sha256_hex(Assets::get(path).unwrap()), hash, "{path}");
        }
    }

    #[test]
    fn list_filters_by_prefix() {
        let logos = Assets.list("logos/").unwrap();
        assert_eq!(logos.len(), 6);
    }

    fn sha256_hex(data: &[u8]) -> String {
        use sha2::Digest;
        sha2::Sha256::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}
