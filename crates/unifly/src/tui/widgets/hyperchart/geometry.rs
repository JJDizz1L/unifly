//! Shared terminal geometry helpers for HyperChart renderers.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::scene::PlotBounds;

pub(super) fn fraction(numerator: usize, denominator: usize) -> f64 {
    #[allow(clippy::cast_precision_loss, clippy::as_conversions)]
    {
        numerator as f64 / denominator as f64
    }
}

pub(super) fn y_to_row(area: Rect, bounds: PlotBounds, y: f64) -> u16 {
    let span = (bounds.y_max - bounds.y_min).max(1.0);
    let rows = area.height.saturating_sub(1);
    let ratio = ((bounds.y_max - y) / span).clamp(0.0, 1.0);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::as_conversions
    )]
    {
        area.y + (f64::from(rows) * ratio).round() as u16
    }
}

pub(super) fn x_to_col(area: Rect, bounds: PlotBounds, x: f64) -> u16 {
    let span = (bounds.x_max - bounds.x_min).max(1.0);
    let columns = area.width.saturating_sub(1);
    let ratio = ((x - bounds.x_min) / span).clamp(0.0, 1.0);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::as_conversions
    )]
    {
        area.x + (f64::from(columns) * ratio).round() as u16
    }
}

pub(super) fn render_text(buf: &mut Buffer, area: Rect, text: &str, style: Style) {
    Paragraph::new(Line::from(Span::styled(text.to_owned(), style))).render(area, buf);
}
