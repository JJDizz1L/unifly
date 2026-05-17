//! Semantic theme adapter for unifly-tui.
//!
//! Bridges Opaline's token-based theme engine to unifly-specific UI concepts.
//! All colors resolve at runtime from the active theme — swap themes and every
//! widget follows.

use std::sync::Arc;

use opaline::names::tokens;
use ratatui::style::{Color, Modifier, Style};

// ── Token derivation ─────────────────────────────────────────────────

/// Register unifly-specific derived tokens on a theme.
///
/// Called automatically during [`initialize`]. Tokens are registered with
/// `register_default_token`, so TOML overrides take priority.
pub fn derive_tokens(theme: &mut opaline::Theme) {
    let accent_primary = theme.color(tokens::ACCENT_PRIMARY);
    let accent_secondary = theme.color(tokens::ACCENT_SECONDARY);
    let accent_tertiary = theme.color(tokens::ACCENT_TERTIARY);

    // Area fill colors for traffic charts — heavily darkened accents
    theme.register_default_token("unifly.tx_fill", accent_secondary.darken(0.85));
    theme.register_default_token("unifly.rx_fill", accent_tertiary.darken(0.85));
    theme.register_default_token("unifly.tx_gradient.start", accent_secondary.darken(0.93));
    theme.register_default_token("unifly.tx_gradient.end", accent_secondary.darken(0.45));
    theme.register_default_token("unifly.rx_gradient.start", accent_tertiary.darken(0.93));
    theme.register_default_token("unifly.rx_gradient.end", accent_tertiary.darken(0.45));
    theme.register_default_token("unifly.client_gradient.start", accent_primary.darken(0.93));
    theme.register_default_token("unifly.client_gradient.end", accent_primary.darken(0.45));

    // Chart series — ordered standard tokens for multi-line graphs
    theme.register_default_token("unifly.chart.0", theme.color(tokens::ACCENT_SECONDARY));
    theme.register_default_token("unifly.chart.1", theme.color(tokens::ACCENT_TERTIARY));
    theme.register_default_token("unifly.chart.2", theme.color(tokens::ACCENT_PRIMARY));
    theme.register_default_token("unifly.chart.3", theme.color(tokens::SUCCESS));
    theme.register_default_token("unifly.chart.4", theme.color(tokens::WARNING));
    theme.register_default_token("unifly.chart.5", theme.color(tokens::INFO));
    theme.register_default_token("unifly.wifi.your_ap", theme.color(tokens::ACCENT_PRIMARY));
    theme.register_default_token("unifly.wifi.neighbor", theme.color(tokens::TEXT_MUTED));
    theme.register_default_token("unifly.wifi.conflict", theme.color(tokens::WARNING));
}

/// Initialize the theme subsystem. Call early in `main()`, before any rendering.
///
/// Resolution priority: explicit name > default (`silkcircuit-neon`).
pub fn initialize(theme_name: Option<&str>) {
    let name = theme_name.unwrap_or("silkcircuit-neon");
    if let Err(e) = opaline::load_theme_by_name_with(name, derive_tokens) {
        tracing::warn!("failed to load theme '{name}': {e}, falling back to default");
        let _ = opaline::load_theme_by_name_with("silkcircuit-neon", derive_tokens);
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Snapshot of the active theme. Cheap (`Arc` clone).
#[inline]
fn t() -> Arc<opaline::Theme> {
    opaline::current()
}

// ── Semantic color accessors ─────────────────────────────────────────

pub fn accent_primary() -> Color {
    t().color(tokens::ACCENT_PRIMARY).into()
}
pub fn accent_secondary() -> Color {
    t().color(tokens::ACCENT_SECONDARY).into()
}
pub fn accent_tertiary() -> Color {
    t().color(tokens::ACCENT_TERTIARY).into()
}
pub fn text_primary() -> Color {
    t().color(tokens::TEXT_PRIMARY).into()
}
pub fn text_secondary() -> Color {
    t().color(tokens::TEXT_SECONDARY).into()
}
pub fn text_muted() -> Color {
    t().color(tokens::TEXT_MUTED).into()
}
pub fn success() -> Color {
    t().color(tokens::SUCCESS).into()
}
pub fn error() -> Color {
    t().color(tokens::ERROR).into()
}
pub fn warning() -> Color {
    t().color(tokens::WARNING).into()
}
pub fn info() -> Color {
    t().color(tokens::INFO).into()
}
pub fn bg_base() -> Color {
    t().color(tokens::BG_BASE).into()
}
pub fn bg_highlight() -> Color {
    t().color(tokens::BG_HIGHLIGHT).into()
}
pub fn border_focused_color() -> Color {
    t().color(tokens::BORDER_FOCUSED).into()
}
pub fn border_unfocused() -> Color {
    t().color(tokens::BORDER_UNFOCUSED).into()
}

// ── unifly-derived color accessors ───────────────────────────────────

pub fn tx_fill() -> Color {
    t().color("unifly.tx_fill").into()
}
pub fn rx_fill() -> Color {
    t().color("unifly.rx_fill").into()
}

pub fn tx_gradient_endpoints() -> (Color, Color) {
    let th = t();
    (
        th.color("unifly.tx_gradient.start").into(),
        th.color("unifly.tx_gradient.end").into(),
    )
}

pub fn rx_gradient_endpoints() -> (Color, Color) {
    let th = t();
    (
        th.color("unifly.rx_gradient.start").into(),
        th.color("unifly.rx_gradient.end").into(),
    )
}

pub fn client_gradient_endpoints() -> (Color, Color) {
    let th = t();
    (
        th.color("unifly.client_gradient.start").into(),
        th.color("unifly.client_gradient.end").into(),
    )
}

pub fn chart_series() -> [Color; 6] {
    let th = t();
    [
        th.color("unifly.chart.0").into(),
        th.color("unifly.chart.1").into(),
        th.color("unifly.chart.2").into(),
        th.color("unifly.chart.3").into(),
        th.color("unifly.chart.4").into(),
        th.color("unifly.chart.5").into(),
    ]
}

pub fn wifi_your_ap() -> Color {
    t().color("unifly.wifi.your_ap").into()
}

pub fn wifi_neighbor() -> Color {
    t().color("unifly.wifi.neighbor").into()
}

pub fn wifi_conflict() -> Color {
    t().color("unifly.wifi.conflict").into()
}

// ── Semantic style accessors ─────────────────────────────────────────

/// Title text for blocks/panels.
pub fn title_style() -> Style {
    Style::default()
        .fg(accent_secondary())
        .add_modifier(Modifier::BOLD)
}

/// Border for a focused panel.
pub fn border_focused() -> Style {
    Style::default().fg(accent_primary())
}

/// Border for an unfocused panel.
pub fn border_default() -> Style {
    Style::default().fg(border_unfocused())
}

/// Table header row.
pub fn table_header() -> Style {
    Style::default()
        .fg(accent_secondary())
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

/// Normal table row text.
pub fn table_row() -> Style {
    Style::default().fg(text_secondary())
}

/// Selected / highlighted table row.
pub fn table_selected() -> Style {
    Style::default()
        .fg(accent_primary())
        .bg(bg_highlight())
        .add_modifier(Modifier::BOLD)
}

/// Active tab in the tab bar.
pub fn tab_active() -> Style {
    Style::default()
        .fg(accent_primary())
        .add_modifier(Modifier::UNDERLINED)
}

/// Inactive tab in the tab bar.
pub fn tab_inactive() -> Style {
    Style::default().fg(text_secondary())
}

/// Status bar text.
#[allow(dead_code)]
pub fn status_bar() -> Style {
    Style::default().fg(text_secondary())
}

/// Key hint text (e.g., "q quit  ? help").
pub fn key_hint() -> Style {
    Style::default().fg(border_unfocused())
}

/// Key hint key character.
pub fn key_hint_key() -> Style {
    Style::default()
        .fg(accent_secondary())
        .add_modifier(Modifier::BOLD)
}
