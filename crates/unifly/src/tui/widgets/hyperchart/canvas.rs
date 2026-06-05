//! Canvas renderer for hero HyperChart panels.

use chrono::{DateTime, Utc};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};
use ratatui::widgets::{Paragraph, Widget};

use super::annotations;
use super::axis;
use super::geometry::{fraction, render_text, x_to_col, y_to_row};
use super::model::{Baseline, XAxis};
use super::scene::{ChartScene, PlotBounds};
use super::time_series::HyperChart;
use crate::tui::theme;

const X_AXIS_TICK_COUNT: usize = 4;
const MIN_HEIGHT_FOR_X_AXIS: u16 = 7;

pub(super) fn render(chart: &HyperChart<'_>, area: Rect, buf: &mut Buffer, gutter_width: u16) {
    let has_x_axis =
        !matches!(chart.x_axis_kind(), XAxis::Hidden) && area.height >= MIN_HEIGHT_FOR_X_AXIS;
    let rows = if has_x_axis {
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(area)
    } else {
        Layout::vertical([Constraint::Min(1)]).split(area)
    };
    let chart_area = rows[0];
    let x_axis_area = has_x_axis.then(|| rows[1]);
    let layout = Layout::horizontal([Constraint::Length(gutter_width), Constraint::Min(1)])
        .split(chart_area);
    let gutter_area = layout[0];
    let plot_area = layout[1];
    let scene = chart.scene();
    let bounds = scene.bounds;
    let caps = chart.caps();

    render_y_gutter(chart, gutter_area, plot_area, &scene, buf);

    #[cfg(feature = "tui-graphics")]
    if caps.graphics_protocol.is_pixels()
        && super::time_series::render_graphics_scene(&scene, plot_area, buf)
    {
        annotations::render(chart, &scene, plot_area, buf);
        if let Some(axis_area) = x_axis_area {
            render_x_axis(chart.x_axis_kind(), axis_area, plot_area, bounds, buf);
        }
        return;
    }

    let plot_density = (usize::from(plot_area.width.max(1)) * 4).max(160);
    let paths: Vec<Vec<Vec<(f64, f64)>>> = scene
        .series
        .iter()
        .map(|series| {
            series
                .data
                .visible_segments()
                .into_iter()
                .map(|segment| axis::interpolate_fill(&segment, plot_density))
                .collect()
        })
        .collect();

    let canvas = Canvas::default()
        .background_color(theme::bg_base())
        .marker(caps.glyph_tier.marker())
        .x_bounds([bounds.x_min, bounds.x_max])
        .y_bounds([bounds.y_min, bounds.y_max])
        .paint(|ctx| {
            draw_grid(ctx, &scene);

            for (series, series_paths) in scene.series.iter().zip(paths.iter()) {
                let Some(bands) = series.fill.bands(caps, usize::from(plot_area.height)) else {
                    continue;
                };
                for path in series_paths {
                    for &(x, y) in path {
                        let y = scene.transform_y(series, y);
                        draw_gradient_column(ctx, x, y, &bands);
                    }
                }
            }

            ctx.layer();

            for (series, series_paths) in scene.series.iter().zip(paths.iter()) {
                for path in series_paths {
                    for pair in path.windows(2) {
                        let [(x1, y1), (x2, y2)] = pair else {
                            continue;
                        };
                        ctx.draw(&CanvasLine {
                            x1: *x1,
                            y1: scene.transform_y(series, *y1),
                            x2: *x2,
                            y2: scene.transform_y(series, *y2),
                            color: series.line_color,
                        });
                    }
                }
            }
        });

    canvas.render(plot_area, buf);
    annotations::render(chart, &scene, plot_area, buf);
    if let Some(axis_area) = x_axis_area {
        render_x_axis(chart.x_axis_kind(), axis_area, plot_area, bounds, buf);
    }
}

fn draw_grid(ctx: &mut ratatui::widgets::canvas::Context<'_>, scene: &ChartScene<'_>) {
    let grid_color = theme::border_unfocused();
    let baseline_color = theme::border_unfocused();

    for y in scene.gridlines() {
        ctx.draw(&CanvasLine {
            x1: scene.bounds.x_min,
            y1: y,
            x2: scene.bounds.x_max,
            y2: y,
            color: grid_color,
        });
    }

    ctx.draw(&CanvasLine {
        x1: scene.bounds.x_min,
        y1: 0.0,
        x2: scene.bounds.x_max,
        y2: 0.0,
        color: baseline_color,
    });
}

fn render_y_gutter(
    chart: &HyperChart<'_>,
    gutter_area: Rect,
    plot_area: Rect,
    scene: &ChartScene<'_>,
    buf: &mut Buffer,
) {
    match scene.baseline {
        Baseline::Zero { y_max } => {
            let labels = chart.build_y_labels(y_max);
            let divisions = chart.tick_count_value().saturating_sub(1).max(1);
            for (idx, label) in labels.iter().enumerate() {
                let y = y_max * fraction(idx, divisions);
                let row = y_to_row(plot_area, scene.bounds, y);
                Paragraph::new(ratatui::text::Line::from(label.clone())).render(
                    Rect {
                        x: gutter_area.x,
                        y: row,
                        width: gutter_area.width,
                        height: 1,
                    },
                    buf,
                );
            }
        }
        Baseline::Mirror {
            upper_max,
            lower_max,
            upper_label,
            lower_label,
        } => {
            render_mirror_labels(
                chart,
                gutter_area,
                plot_area,
                scene.bounds,
                upper_max,
                true,
                buf,
            );
            render_mirror_labels(
                chart,
                gutter_area,
                plot_area,
                scene.bounds,
                lower_max,
                false,
                buf,
            );

            let baseline_row = y_to_row(plot_area, scene.bounds, 0.0);
            let label_style = Style::default().fg(theme::border_unfocused());
            if baseline_row > plot_area.y {
                render_text(
                    buf,
                    Rect {
                        x: gutter_area.x,
                        y: baseline_row - 1,
                        width: gutter_area.width,
                        height: 1,
                    },
                    upper_label,
                    label_style,
                );
            }
            if baseline_row + 1 < plot_area.y + plot_area.height {
                render_text(
                    buf,
                    Rect {
                        x: gutter_area.x,
                        y: baseline_row + 1,
                        width: gutter_area.width,
                        height: 1,
                    },
                    lower_label,
                    label_style,
                );
            }
        }
    }
}

fn render_mirror_labels(
    chart: &HyperChart<'_>,
    gutter_area: Rect,
    plot_area: Rect,
    bounds: PlotBounds,
    max_value: f64,
    upper: bool,
    buf: &mut Buffer,
) {
    let labels = chart.build_y_labels(max_value);
    let divisions = chart.tick_count_value().saturating_sub(1).max(1);
    for (idx, label) in labels.iter().enumerate().skip(1) {
        let value = max_value * fraction(idx, divisions);
        let signed_value = if upper { value } else { -value };
        let row = y_to_row(plot_area, bounds, signed_value);
        Paragraph::new(ratatui::text::Line::from(label.clone())).render(
            Rect {
                x: gutter_area.x,
                y: row,
                width: gutter_area.width,
                height: 1,
            },
            buf,
        );
    }
}

fn render_x_axis(
    x_axis: XAxis,
    axis_area: Rect,
    plot_area: Rect,
    bounds: PlotBounds,
    buf: &mut Buffer,
) {
    let axis_style = Style::default().fg(theme::border_unfocused());
    let mut occupied_until = axis_area.x;
    for (idx, (x, label)) in x_labels(x_axis, bounds).iter().enumerate() {
        let column = x_to_col(plot_area, bounds, *x);
        let width = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
        let mut start = column.saturating_sub(width / 2);
        if idx + 1 == X_AXIS_TICK_COUNT {
            start = column.saturating_sub(width.saturating_sub(1));
        }
        start = start.max(plot_area.x);
        let end = start
            .saturating_add(width)
            .min(plot_area.x + plot_area.width);
        if end <= occupied_until || start >= plot_area.x + plot_area.width {
            continue;
        }

        render_text(
            buf,
            Rect {
                x: start,
                y: axis_area.y,
                width: end - start,
                height: 1,
            },
            label,
            axis_style,
        );
        occupied_until = end.saturating_add(1);
    }
}

fn x_labels(x_axis: XAxis, bounds: PlotBounds) -> Vec<(f64, String)> {
    let divisions = X_AXIS_TICK_COUNT.saturating_sub(1).max(1);
    (0..X_AXIS_TICK_COUNT)
        .map(|idx| {
            let t = fraction(idx, divisions);
            let x = bounds.x_min + (bounds.x_max - bounds.x_min) * t;
            (
                x,
                format_x_label(x_axis, x, bounds.x_max, idx + 1 == X_AXIS_TICK_COUNT),
            )
        })
        .collect()
}

fn format_x_label(x_axis: XAxis, x: f64, x_max: f64, is_last: bool) -> String {
    match x_axis {
        XAxis::Hidden => String::new(),
        XAxis::Relative { sample_interval } => {
            if is_last {
                return "now".into();
            }
            let seconds = ((x_max - x) * sample_interval.as_secs_f64()).round();
            format_relative_offset(seconds)
        }
        XAxis::Epoch => {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::as_conversions
            )]
            let epoch = x.round() as i64;
            DateTime::<Utc>::from_timestamp(epoch, 0).map_or_else(
                || format!("{x:.0}"),
                |time| time.format("%H:%M").to_string(),
            )
        }
    }
}

fn draw_gradient_column(
    ctx: &mut ratatui::widgets::canvas::Context<'_>,
    x: f64,
    y: f64,
    bands: &[ratatui::style::Color],
) {
    if y.abs() < f64::EPSILON || bands.is_empty() {
        return;
    }

    let divisions = bands.len();
    for (idx, color) in bands.iter().enumerate() {
        let start = fraction(idx, divisions);
        let end = fraction(idx + 1, divisions);
        ctx.draw(&CanvasLine {
            x1: x,
            y1: y * start,
            x2: x,
            y2: y * end,
            color: *color,
        });
    }
}

fn format_relative_offset(seconds: f64) -> String {
    let seconds = seconds.max(0.0).round();
    if seconds < 60.0 {
        format!("-{seconds:.0}s")
    } else if seconds < 3_600.0 {
        format!("-{:.0}m", seconds / 60.0)
    } else {
        format!("-{:.0}h", seconds / 3_600.0)
    }
}
