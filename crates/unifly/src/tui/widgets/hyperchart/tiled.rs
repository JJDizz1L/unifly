//! Ratatui `Chart` renderer for compact HyperChart panels.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Widget};

use super::axis;
use super::time_series::HyperChart;
use crate::tui::theme;

pub(super) fn render(chart: &HyperChart<'_>, area: Rect, buf: &mut Buffer) {
    let scene = chart.scene();
    let bounds = scene.bounds;
    let axis_style = Style::default().fg(theme::border_unfocused());
    let fill_density = (usize::from(area.width.saturating_sub(8)) * 3).max(120);
    let caps = chart.caps();

    let line_buffers: Vec<Vec<(f64, f64)>> = scene
        .series
        .iter()
        .map(|series| {
            series
                .data
                .visible_segments()
                .into_iter()
                .flatten()
                .collect()
        })
        .collect();
    let fill_buffers: Vec<Vec<(f64, f64)>> = line_buffers
        .iter()
        .map(|points| axis::interpolate_fill(points, fill_density))
        .collect();

    let mut datasets: Vec<Dataset> = Vec::new();
    for (series, fill_buf) in scene.series.iter().zip(fill_buffers.iter()) {
        let Some(color) = series.fill.chart_color(caps) else {
            continue;
        };
        datasets.push(
            Dataset::default()
                .marker(Marker::HalfBlock)
                .graph_type(GraphType::Bar)
                .style(Style::default().fg(color))
                .data(fill_buf),
        );
    }

    for (series, data) in scene.series.iter().zip(line_buffers.iter()) {
        datasets.push(
            Dataset::default()
                .name(series.name)
                .marker(caps.glyph_tier.marker())
                .graph_type(GraphType::Line)
                .style(Style::default().fg(series.line_color))
                .data(data),
        );
    }

    let y_labels = chart.build_y_labels(bounds.y_max);
    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .bounds([bounds.x_min, bounds.x_max])
                .style(axis_style),
        )
        .y_axis(
            Axis::default()
                .bounds([bounds.y_min, bounds.y_max])
                .labels(y_labels)
                .style(axis_style),
        );

    chart.render(area, buf);
}
