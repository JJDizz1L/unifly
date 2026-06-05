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

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::text::Line;

    use super::super::scene::GridSpec;
    use super::*;
    use crate::tui::widgets::hyperchart::{Baseline, Domain, Series, XAxis};

    fn buffer_text(buf: &Buffer) -> String {
        (0..buf.area().height)
            .map(|y| {
                (0..buf.area().width)
                    .filter_map(|x| buf.cell((x, y)).map(|cell| cell.symbol().to_string()))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn now_annotation_renders_value_when_there_is_room_on_the_right() {
        let series: [Series<'_>; 0] = [];
        let chart = HyperChart::new(Line::raw(""), &series, (0.0, 10.0), 1_024.0)
            .domain(Domain::Rate)
            .baseline(Baseline::Zero { y_max: 1_024.0 });
        let plot_area = Rect::new(0, 0, 24, 4);
        let mut buf = Buffer::empty(plot_area);
        let scene = ChartScene {
            x_axis: XAxis::Hidden,
            bounds: PlotBounds {
                x_min: 0.0,
                x_max: 10.0,
                y_min: 0.0,
                y_max: 1_024.0,
            },
            baseline: Baseline::Zero { y_max: 1_024.0 },
            series: Vec::new(),
            grid: GridSpec { tick_count: 4 },
            annotations: vec![Annotation {
                kind: AnnotationKind::Now,
                x: 2.0,
                y: 1_024.0,
                transformed_y: 1_024.0,
                color: Color::Cyan,
            }],
        };

        render(&chart, &scene, plot_area, &mut buf);

        let text = buffer_text(&buf);
        assert!(text.contains("●"));
        assert!(text.contains("─"));
    }
}
