//! Cached terminal rendering capabilities for TUI charts.

use std::sync::{OnceLock, RwLock};

use ratatui::symbols::Marker;

static RENDER_CAPS: OnceLock<RwLock<RenderCaps>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderCaps {
    pub color_depth: ColorDepth,
    pub glyph_tier: GlyphTier,
    pub graphics_protocol: GraphicsProtocol,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicsProtocol {
    None,
    Kitty,
    Sixel,
    Iterm2,
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

impl GraphicsProtocol {
    pub fn is_pixels(self) -> bool {
        matches!(self, Self::Kitty | Self::Sixel | Self::Iterm2)
    }
}

pub fn graphics_disabled() -> bool {
    std::env::var_os("UNIFLY_DISABLE_GRAPHICS").is_some()
}

pub fn forced_graphics_protocol() -> Option<GraphicsProtocol> {
    std::env::var("UNIFLY_GRAPHICS_PROTOCOL")
        .ok()
        .and_then(|value| parse_graphics_protocol(&value))
}

pub fn parse_graphics_protocol(value: &str) -> Option<GraphicsProtocol> {
    match value.trim().to_ascii_lowercase().as_str() {
        "kitty" => Some(GraphicsProtocol::Kitty),
        "sixel" | "sixels" => Some(GraphicsProtocol::Sixel),
        "iterm2" | "iterm" => Some(GraphicsProtocol::Iterm2),
        "off" | "none" | "false" | "0" => Some(GraphicsProtocol::None),
        _ => None,
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
            .and_then(parse_glyph_tier)
            .or_else(|| configured_quality.and_then(parse_glyph_tier))
            .unwrap_or(GlyphTier::Octant);

        Self {
            color_depth,
            glyph_tier,
            graphics_protocol: detect_graphics_protocol(&env),
        }
    }
}

pub fn initialize(configured_quality: Option<&str>) -> RenderCaps {
    let caps = RenderCaps::detect(configured_quality);
    store(caps);
    caps
}

pub fn current() -> RenderCaps {
    RENDER_CAPS
        .get()
        .and_then(|lock| lock.read().ok().map(|guard| *guard))
        .unwrap_or_else(|| RenderCaps::detect(None))
}

pub fn set_graphics_protocol(protocol: GraphicsProtocol) -> RenderCaps {
    let mut caps = current();
    caps.graphics_protocol = protocol;
    store(caps);
    caps
}

fn store(caps: RenderCaps) {
    let lock = RENDER_CAPS.get_or_init(|| RwLock::new(caps));
    if let Ok(mut guard) = lock.write() {
        *guard = caps;
    }
}

fn detect_color_depth<F>(env: &F) -> ColorDepth
where
    F: Fn(&str) -> Option<String>,
{
    if env("NO_COLOR").is_some_and(|value| !value.is_empty()) {
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

fn detect_graphics_protocol<F>(env: &F) -> GraphicsProtocol
where
    F: Fn(&str) -> Option<String>,
{
    if graphics_disabled_with(env) {
        return GraphicsProtocol::None;
    }

    if let Some(protocol) = forced_graphics_protocol_with(env) {
        return protocol;
    }

    if env("KITTY_WINDOW_ID").is_some()
        || env("TERM")
            .as_deref()
            .is_some_and(|value| value.contains("xterm-kitty"))
    {
        return GraphicsProtocol::Kitty;
    }

    if env("TERM")
        .as_deref()
        .is_some_and(|value| value.contains("sixel"))
    {
        return GraphicsProtocol::Sixel;
    }

    GraphicsProtocol::None
}

fn graphics_disabled_with<F>(env: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    env("UNIFLY_DISABLE_GRAPHICS").is_some()
}

fn forced_graphics_protocol_with<F>(env: &F) -> Option<GraphicsProtocol>
where
    F: Fn(&str) -> Option<String>,
{
    env("UNIFLY_GRAPHICS_PROTOCOL").and_then(|value| parse_graphics_protocol(&value))
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
        assert_eq!(caps.graphics_protocol, GraphicsProtocol::None);
    }

    #[test]
    fn empty_no_color_does_not_disable_color() {
        let caps =
            RenderCaps::detect_with(None, env(&[("NO_COLOR", ""), ("COLORTERM", "truecolor")]));

        assert_eq!(caps.color_depth, ColorDepth::TrueColor);
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
    fn defaults_to_octant_charts() {
        let caps = RenderCaps::detect_with(None, env(&[]));

        assert_eq!(caps.color_depth, ColorDepth::Ansi16);
        assert_eq!(caps.glyph_tier, GlyphTier::Octant);
    }

    #[test]
    fn env_chart_quality_overrides_config() {
        let caps =
            RenderCaps::detect_with(Some("block"), env(&[("UNIFLY_CHART_QUALITY", "octant")]));

        assert_eq!(caps.glyph_tier, GlyphTier::Octant);
    }

    #[test]
    fn invalid_env_chart_quality_falls_back_to_config() {
        let caps =
            RenderCaps::detect_with(Some("block"), env(&[("UNIFLY_CHART_QUALITY", "crystal")]));

        assert_eq!(caps.glyph_tier, GlyphTier::Block);
    }

    #[test]
    fn config_chart_quality_is_used_without_env() {
        let caps = RenderCaps::detect_with(Some("block"), env(&[]));

        assert_eq!(caps.glyph_tier, GlyphTier::Block);
    }

    #[test]
    fn graphics_protocol_can_be_forced_from_env() {
        let caps = RenderCaps::detect_with(None, env(&[("UNIFLY_GRAPHICS_PROTOCOL", "kitty")]));

        assert_eq!(caps.graphics_protocol, GraphicsProtocol::Kitty);
    }

    #[test]
    fn graphics_protocol_can_be_disabled() {
        let caps = RenderCaps::detect_with(
            None,
            env(&[
                ("UNIFLY_DISABLE_GRAPHICS", "1"),
                ("UNIFLY_GRAPHICS_PROTOCOL", "kitty"),
            ]),
        );

        assert_eq!(caps.graphics_protocol, GraphicsProtocol::None);
    }
}
