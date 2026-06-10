//! Environment-based detection heuristics for terminal render capabilities.

use super::{ColorDepth, GlyphTier, GraphicsProtocol};

pub(super) fn detect_color_depth<F>(env: &F) -> ColorDepth
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

pub(super) fn detect_graphics_protocol<F>(env: &F) -> GraphicsProtocol
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

pub(super) fn graphics_disabled_with<F>(env: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    env("UNIFLY_DISABLE_GRAPHICS").is_some()
}

pub(super) fn forced_graphics_protocol_with<F>(env: &F) -> Option<GraphicsProtocol>
where
    F: Fn(&str) -> Option<String>,
{
    env("UNIFLY_GRAPHICS_PROTOCOL").and_then(|value| parse_graphics_protocol(&value))
}

/// Octant glyphs live in the Unicode 16 Symbols for Legacy Computing
/// Supplement, which most installed fonts do not cover. Default to Octant
/// only on terminals that rasterize block glyphs internally instead of
/// relying on the font; everything else gets Braille, which has been safe
/// since long before Unicode 16.
pub(super) fn default_glyph_tier<F>(env: &F) -> GlyphTier
where
    F: Fn(&str) -> Option<String>,
{
    if octant_capable_terminal(env) {
        GlyphTier::Octant
    } else {
        GlyphTier::Braille
    }
}

fn octant_capable_terminal<F>(env: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    if env("KITTY_WINDOW_ID").is_some()
        || env("GHOSTTY_RESOURCES_DIR").is_some()
        || env("WEZTERM_EXECUTABLE").is_some()
    {
        return true;
    }

    if env("TERM").as_deref().is_some_and(|term| {
        term.contains("kitty")
            || term.contains("ghostty")
            || term.contains("wezterm")
            || term.contains("foot")
    }) {
        return true;
    }

    env("TERM_PROGRAM").is_some_and(|program| {
        let program = program.to_ascii_lowercase();
        program.contains("kitty") || program.contains("ghostty") || program.contains("wezterm")
    })
}

pub(super) fn parse_glyph_tier(value: &str) -> Option<GlyphTier> {
    match value.trim().to_ascii_lowercase().as_str() {
        "block" | "blocks" | "minimal" => Some(GlyphTier::Block),
        "braille" | "default" => Some(GlyphTier::Braille),
        "octant" | "octants" | "ultra" => Some(GlyphTier::Octant),
        _ => None,
    }
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
