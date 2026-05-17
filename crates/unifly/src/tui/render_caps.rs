//! Cached terminal rendering capabilities for TUI charts.

use std::sync::OnceLock;

use ratatui::symbols::Marker;

static RENDER_CAPS: OnceLock<RenderCaps> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderCaps {
    pub color_depth: ColorDepth,
    pub glyph_tier: GlyphTier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    NoColor,
    Ansi16,
    Ansi256,
    TrueColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphTier {
    Block,
    Braille,
    Octant,
}

impl GlyphTier {
    pub fn marker(self) -> Marker {
        match self {
            Self::Block => Marker::HalfBlock,
            Self::Braille => Marker::Braille,
            Self::Octant => Marker::Octant,
        }
    }
}

impl RenderCaps {
    pub fn detect(configured_quality: Option<&str>) -> Self {
        Self::detect_with(configured_quality, |key| std::env::var(key).ok())
    }

    pub fn detect_with<F>(configured_quality: Option<&str>, env: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let color_depth = detect_color_depth(&env);
        let glyph_tier = env("UNIFLY_CHART_QUALITY")
            .as_deref()
            .or(configured_quality)
            .and_then(parse_glyph_tier)
            .unwrap_or(GlyphTier::Braille);

        Self {
            color_depth,
            glyph_tier,
        }
    }
}

pub fn initialize(configured_quality: Option<&str>) -> RenderCaps {
    let caps = RenderCaps::detect(configured_quality);
    let _ = RENDER_CAPS.set(caps);
    caps
}

pub fn current() -> RenderCaps {
    RENDER_CAPS
        .get()
        .copied()
        .unwrap_or_else(|| RenderCaps::detect(None))
}

fn detect_color_depth<F>(env: &F) -> ColorDepth
where
    F: Fn(&str) -> Option<String>,
{
    if env("NO_COLOR").is_some() {
        return ColorDepth::NoColor;
    }

    if env("COLORTERM")
        .as_deref()
        .is_some_and(|value| matches!(value, "truecolor" | "24bit"))
    {
        return ColorDepth::TrueColor;
    }

    if env("TERM")
        .as_deref()
        .is_some_and(|value| value.contains("256color"))
    {
        return ColorDepth::Ansi256;
    }

    ColorDepth::Ansi16
}

fn parse_glyph_tier(value: &str) -> Option<GlyphTier> {
    match value.trim().to_ascii_lowercase().as_str() {
        "block" | "blocks" | "minimal" => Some(GlyphTier::Block),
        "braille" | "default" => Some(GlyphTier::Braille),
        "octant" | "octants" | "ultra" => Some(GlyphTier::Octant),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        |key| {
            pairs
                .iter()
                .find_map(|(name, value)| (*name == key).then(|| (*value).to_owned()))
        }
    }

    #[test]
    fn no_color_wins_over_truecolor() {
        let caps =
            RenderCaps::detect_with(None, env(&[("NO_COLOR", "1"), ("COLORTERM", "truecolor")]));

        assert_eq!(caps.color_depth, ColorDepth::NoColor);
    }

    #[test]
    fn detects_truecolor_before_ansi256() {
        let caps = RenderCaps::detect_with(
            None,
            env(&[("COLORTERM", "24bit"), ("TERM", "xterm-256color")]),
        );

        assert_eq!(caps.color_depth, ColorDepth::TrueColor);
    }

    #[test]
    fn detects_ansi256_from_term() {
        let caps = RenderCaps::detect_with(None, env(&[("TERM", "screen-256color")]));

        assert_eq!(caps.color_depth, ColorDepth::Ansi256);
    }

    #[test]
    fn defaults_to_ansi16_and_braille() {
        let caps = RenderCaps::detect_with(None, env(&[]));

        assert_eq!(caps.color_depth, ColorDepth::Ansi16);
        assert_eq!(caps.glyph_tier, GlyphTier::Braille);
    }

    #[test]
    fn env_chart_quality_overrides_config() {
        let caps =
            RenderCaps::detect_with(Some("block"), env(&[("UNIFLY_CHART_QUALITY", "octant")]));

        assert_eq!(caps.glyph_tier, GlyphTier::Octant);
    }

    #[test]
    fn config_chart_quality_is_used_without_env() {
        let caps = RenderCaps::detect_with(Some("block"), env(&[]));

        assert_eq!(caps.glyph_tier, GlyphTier::Block);
    }
}
