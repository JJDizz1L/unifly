//! Marker and value label overlay rendering for HyperChart.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::geometry::{render_text, x_to_col, y_to_row};
use super::scene::{Annotation, AnnotationKind, ChartScene, PlotBounds};
use super::time_series::HyperChart;

const MARKER_VALUE_WIDTH: u16 = 9;

pub(super) fn render(
    chart: &HyperChart<'_>,
    scene: &ChartScene<'_>,
    plot_area: Rect,
    buf: &mut Buffer,
) {
    for annotation in &scene.annotations {
        let symbol = match annotation.kind {
            AnnotationKind::Now => "●",
            AnnotationKind::Peak => "◆",
        };
        render_point_marker(symbol, *annotation, plot_area, scene.bounds, buf);
        if annotation.kind == AnnotationKind::Now {
            render_marker_value(chart, *annotation, plot_area, scene.bounds, buf);
        }
    }
}

fn render_point_marker(
    symbol: &str,
    point: Annotation,
    plot_area: Rect,
    bounds: PlotBounds,
    buf: &mut Buffer,
) {
    let column = x_to_col(plot_area, bounds, point.x);
    let row = y_to_row(plot_area, bounds, point.transformed_y);
    render_text(
        buf,
        Rect {
            x: column,
            y: row,
            width: 1,
            height: 1,
        },
        symbol,
        Style::default().fg(point.color),
    );
}

fn render_marker_value(
    chart: &HyperChart<'_>,
    point: Annotation,
    plot_area: Rect,
    bounds: PlotBounds,
    buf: &mut Buffer,
) {
    let column = x_to_col(plot_area, bounds, point.x);
    let row = y_to_row(plot_area, bounds, point.transformed_y);
    let label = format!("─ {}", chart.format_value(point.y));
    let style = Style::default().fg(point.color);
    let right_x = column.saturating_add(1);
    if right_x + MARKER_VALUE_WIDTH <= plot_area.x + plot_area.width {
        render_text(
            buf,
            Rect {
                x: right_x,
                y: row,
                width: MARKER_VALUE_WIDTH,
                height: 1,
            },
            &label,
            style,
        );
        return;
    }

    if column >= plot_area.x + MARKER_VALUE_WIDTH {
        render_text(
            buf,
            Rect {
                x: column - MARKER_VALUE_WIDTH,
                y: row,
                width: MARKER_VALUE_WIDTH,
                height: 1,
            },
            &label,
            style,
        );
    }
}
