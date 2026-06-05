//! Time-series chart widget with terminal-aware rendering.
//!
//! [`HyperChart`] accepts one or more [`Series`], a [`Domain`] for y-axis
//! label formatting, an [`XAxis`] descriptor, and a [`Baseline`] layout. The
//! canvas renderer is the primary path for portfolio-grade WAN charts: it
//! supports theme-native gradients, mirror TX/RX baselines, gridlines, x-axis
//! labels, gap-aware series, and terminal capability fallbacks.

use chrono::{DateTime, Utc};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Widget};

use super::model::{Baseline, Domain, FillStyle, Series, SeriesData, SeriesDirection, XAxis};
use super::scene::{Annotation, AnnotationKind, ChartScene, GridSpec, PlotBounds, SceneSeries};
use super::{axis, block, empty};
use crate::tui::render_caps::{self, RenderCaps};
use crate::tui::theme;

const X_AXIS_TICK_COUNT: usize = 4;
const MIN_HEIGHT_FOR_X_AXIS: u16 = 7;
const MARKER_VALUE_WIDTH: u16 = 9;

/// Rendering back-end for [`HyperChart`].
#[derive(Debug, Clone, Copy)]
pub enum Renderer {
    /// Canvas renderer with a manual y-axis gutter.
    Canvas {
        /// Width reserved for y-axis labels on the left of the plot area.
        gutter_width: u16,
    },
    /// Ratatui `Chart` widget with built-in axes. Kept for compact fallback
    /// panels that do not need mirror baselines.
    Tiled,
}

/// Unified time-series chart widget.
pub struct HyperChart<'a> {
    title: Line<'a>,
    series: &'a [Series<'a>],
    domain: Domain,
    x_axis: XAxis,
    x_bounds: (f64, f64),
    baseline: Baseline<'a>,
    renderer: Renderer,
    render_caps: Option<RenderCaps>,
    tick_count: usize,
    label_width: usize,
    empty_message: &'a str,
    focused: bool,
}

impl<'a> HyperChart<'a> {
    /// Construct a new `HyperChart` with sensible defaults.
    pub fn new(
        title: Line<'a>,
        series: &'a [Series<'a>],
        x_bounds: (f64, f64),
        y_max: f64,
    ) -> Self {
        Self {
            title,
            series,
            domain: Domain::Rate,
            x_axis: XAxis::Hidden,
            x_bounds,
            baseline: Baseline::Zero { y_max },
            renderer: Renderer::Tiled,
            render_caps: None,
            tick_count: 4,
            label_width: 6,
            empty_message: "No data",
            focused: false,
        }
    }

    #[must_use]
    pub fn domain(mut self, domain: Domain) -> Self {
        self.domain = domain;
        self
    }

    #[must_use]
    pub fn x_axis(mut self, x_axis: XAxis) -> Self {
        self.x_axis = x_axis;
        self
    }

    #[must_use]
    pub fn baseline(mut self, baseline: Baseline<'a>) -> Self {
        self.baseline = baseline;
        self
    }

    #[must_use]
    pub fn renderer(mut self, renderer: Renderer) -> Self {
        self.renderer = renderer;
        self
    }

    #[must_use]
    pub fn render_caps(mut self, caps: RenderCaps) -> Self {
        self.render_caps = Some(caps);
        self
    }

    #[must_use]
    pub fn tick_count(mut self, tick_count: usize) -> Self {
        self.tick_count = tick_count;
        self
    }

    #[must_use]
    pub fn label_width(mut self, label_width: usize) -> Self {
        self.label_width = label_width;
        self
    }

    #[must_use]
    pub fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
    }

    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    fn caps(&self) -> RenderCaps {
        self.render_caps.unwrap_or_else(render_caps::current)
    }

    fn is_empty(&self) -> bool {
        self.scene().is_empty()
    }

    fn resolved_x_bounds(&self) -> (f64, f64) {
        let (min, max) = self.x_bounds;
        if (max - min).abs() < f64::EPSILON {
            (min - 0.5, max + 0.5)
        } else {
            (min, max)
        }
    }

    fn plot_bounds(&self) -> PlotBounds {
        let (x_min, x_max) = self.resolved_x_bounds();
        match self.baseline {
            Baseline::Zero { y_max } => PlotBounds {
                x_min,
                x_max,
                y_min: 0.0,
                y_max: y_max.max(1.0),
            },
            Baseline::Mirror {
                upper_max,
                lower_max,
                ..
            } => PlotBounds {
                x_min,
                x_max,
                y_min: -lower_max.max(1.0),
                y_max: upper_max.max(1.0),
            },
        }
    }

    fn scene(&self) -> ChartScene<'_> {
        let bounds = self.plot_bounds();
        ChartScene {
            x_axis: self.x_axis,
            bounds,
            baseline: self.baseline,
            series: self
                .series
                .iter()
                .map(|series| SceneSeries {
                    name: series.name,
                    data: series.data,
                    line_color: series.line_color,
                    fill: series.fill,
                    direction: series.direction,
                })
                .collect(),
            grid: GridSpec {
                tick_count: self.tick_count,
            },
            annotations: self.build_annotations(bounds),
        }
    }

    fn build_y_labels(&self, max_value: f64) -> Vec<Span<'static>> {
        let axis_style = Style::default().fg(theme::border_unfocused());
        match self.domain {
            Domain::Rate => {
                axis::rate_axis_labels(max_value, self.tick_count, self.label_width, axis_style)
            }
            Domain::Count => {
                axis::count_axis_labels(max_value, self.tick_count, self.label_width, axis_style)
            }
        }
    }

    fn format_value(&self, value: f64) -> String {
        match self.domain {
            Domain::Rate => crate::tui::widgets::bytes_fmt::fmt_rate_axis(value),
            Domain::Count => format!("{value:.0}"),
        }
    }

    fn render_tiled(self, area: Rect, buf: &mut Buffer) {
        let scene = self.scene();
        let bounds = scene.bounds;
        let axis_style = Style::default().fg(theme::border_unfocused());
        let fill_density = (usize::from(area.width.saturating_sub(8)) * 3).max(120);
        let caps = self.caps();

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

        let y_labels = self.build_y_labels(bounds.y_max);
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

    fn render_canvas(self, area: Rect, buf: &mut Buffer, gutter_width: u16) {
        let has_x_axis =
            !matches!(self.x_axis, XAxis::Hidden) && area.height >= MIN_HEIGHT_FOR_X_AXIS;
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
        let scene = self.scene();
        let bounds = scene.bounds;
        let caps = self.caps();

        self.render_y_gutter(gutter_area, plot_area, &scene, buf);

        #[cfg(feature = "tui-graphics")]
        if caps.graphics_protocol.is_pixels() && render_graphics_scene(&scene, plot_area, buf) {
            self.render_annotations(&scene, plot_area, buf);
            if let Some(axis_area) = x_axis_area {
                self.render_x_axis(axis_area, plot_area, bounds, buf);
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
                Self::draw_grid(ctx, &scene);

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
        self.render_annotations(&scene, plot_area, buf);
        if let Some(axis_area) = x_axis_area {
            self.render_x_axis(axis_area, plot_area, bounds, buf);
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
        &self,
        gutter_area: Rect,
        plot_area: Rect,
        scene: &ChartScene<'_>,
        buf: &mut Buffer,
    ) {
        match scene.baseline {
            Baseline::Zero { y_max } => {
                let labels = self.build_y_labels(y_max);
                let divisions = self.tick_count.saturating_sub(1).max(1);
                for (idx, label) in labels.iter().enumerate() {
                    let y = y_max * fraction(idx, divisions);
                    let row = y_to_row(plot_area, scene.bounds, y);
                    Paragraph::new(Line::from(label.clone())).render(
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
                self.render_mirror_labels(
                    gutter_area,
                    plot_area,
                    scene.bounds,
                    upper_max,
                    true,
                    buf,
                );
                self.render_mirror_labels(
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
        &self,
        gutter_area: Rect,
        plot_area: Rect,
        bounds: PlotBounds,
        max_value: f64,
        upper: bool,
        buf: &mut Buffer,
    ) {
        let labels = self.build_y_labels(max_value);
        let divisions = self.tick_count.saturating_sub(1).max(1);
        for (idx, label) in labels.iter().enumerate().skip(1) {
            let value = max_value * fraction(idx, divisions);
            let signed_value = if upper { value } else { -value };
            let row = y_to_row(plot_area, bounds, signed_value);
            Paragraph::new(Line::from(label.clone())).render(
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
        &self,
        axis_area: Rect,
        plot_area: Rect,
        bounds: PlotBounds,
        buf: &mut Buffer,
    ) {
        let axis_style = Style::default().fg(theme::border_unfocused());
        let mut occupied_until = axis_area.x;
        for (idx, (x, label)) in self.x_labels(bounds).iter().enumerate() {
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

    fn x_labels(&self, bounds: PlotBounds) -> Vec<(f64, String)> {
        let divisions = X_AXIS_TICK_COUNT.saturating_sub(1).max(1);
        (0..X_AXIS_TICK_COUNT)
            .map(|idx| {
                let t = fraction(idx, divisions);
                let x = bounds.x_min + (bounds.x_max - bounds.x_min) * t;
                (
                    x,
                    self.format_x_label(x, bounds.x_max, idx + 1 == X_AXIS_TICK_COUNT),
                )
            })
            .collect()
    }

    fn format_x_label(&self, x: f64, x_max: f64, is_last: bool) -> String {
        match self.x_axis {
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

    fn render_annotations(&self, scene: &ChartScene<'_>, plot_area: Rect, buf: &mut Buffer) {
        for annotation in &scene.annotations {
            let symbol = match annotation.kind {
                AnnotationKind::Now => "●",
                AnnotationKind::Peak => "◆",
            };
            Self::render_point_marker(symbol, *annotation, plot_area, scene.bounds, buf);
            if annotation.kind == AnnotationKind::Now {
                self.render_marker_value(*annotation, plot_area, scene.bounds, buf);
            }
        }
    }

    fn build_annotations(&self, bounds: PlotBounds) -> Vec<Annotation> {
        let mut annotations = Vec::new();
        for series in self.series {
            if let Some(annotation) = self.visible_annotation(series, bounds, AnnotationKind::Now) {
                annotations.push(annotation);
            }
            if let Some(annotation) = self.visible_annotation(series, bounds, AnnotationKind::Peak)
            {
                annotations.push(annotation);
            }
        }
        annotations
    }

    fn visible_annotation(
        &self,
        series: &Series<'_>,
        bounds: PlotBounds,
        kind: AnnotationKind,
    ) -> Option<Annotation> {
        let mut points = series
            .data
            .points()
            .into_iter()
            .filter(|&(x, _)| x >= bounds.x_min && x <= bounds.x_max);

        let point = match kind {
            AnnotationKind::Now => points.next_back(),
            AnnotationKind::Peak => points.max_by(|left, right| left.1.total_cmp(&right.1)),
        };

        point.map(|(x, y)| Annotation {
            kind,
            x,
            y,
            transformed_y: transform_value(self.baseline, series.direction, y),
            color: series.line_color,
        })
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
        &self,
        point: Annotation,
        plot_area: Rect,
        bounds: PlotBounds,
        buf: &mut Buffer,
    ) {
        let column = x_to_col(plot_area, bounds, point.x);
        let row = y_to_row(plot_area, bounds, point.transformed_y);
        let label = format!("─ {}", self.format_value(point.y));
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
}

impl Widget for HyperChart<'_> {
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::as_conversions,
        clippy::too_many_lines
    )]
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = block::standard(self.title.clone(), self.focused);
        let inner = block.inner(area);
        block.render(area, buf);

        if self.is_empty() {
            empty::render(inner, buf, self.empty_message);
            return;
        }

        match self.renderer {
            Renderer::Tiled => self.render_tiled(inner, buf),
            Renderer::Canvas { gutter_width } => self.render_canvas(inner, buf, gutter_width),
        }
    }
}

impl FillStyle {
    fn chart_color(self, caps: RenderCaps) -> Option<Color> {
        match self {
            Self::None => None,
            Self::Solid(color) => Some(color),
            Self::Gradient(gradient) => gradient.bands(caps, 3).into_iter().next(),
        }
    }

    fn bands(self, caps: RenderCaps, height: usize) -> Option<Vec<Color>> {
        match self {
            Self::None => None,
            Self::Solid(color) => Some(vec![color]),
            Self::Gradient(gradient) => Some(gradient.bands(caps, height.clamp(2, 10))),
        }
    }
}

fn draw_gradient_column(
    ctx: &mut ratatui::widgets::canvas::Context<'_>,
    x: f64,
    y: f64,
    bands: &[Color],
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

#[cfg(feature = "tui-graphics")]
fn render_graphics_scene(scene: &ChartScene<'_>, area: Rect, buf: &mut Buffer) -> bool {
    use ratatui::layout::Size;

    use crate::tui::graphics::CachedChart;

    let Some(picker) = crate::tui::graphics::current_picker() else {
        return false;
    };
    let font_size = picker.font_size();
    let target = Size::new(area.width.max(1), area.height.max(1));
    let slot = graphics_chart_slot_key(scene, area, font_size);
    let key = graphics_chart_key(scene, area, font_size);

    match crate::tui::graphics::render_cached_chart(slot, key, area, buf) {
        CachedChart::Rendered => return true,
        CachedChart::Stale(current) => {
            if matches!(current, crate::tui::graphics::CachedChartStatus::Missing) {
                queue_graphics_scene(slot, key, scene, area, target, font_size);
            }
            return true;
        }
        CachedChart::Pending | CachedChart::Failed => return false,
        CachedChart::Missing => {}
    }

    queue_graphics_scene(slot, key, scene, area, target, font_size);
    false
}

#[cfg(feature = "tui-graphics")]
fn queue_graphics_scene(
    slot: crate::tui::graphics::ChartSlotKey,
    key: crate::tui::graphics::ChartImageKey,
    scene: &ChartScene<'_>,
    area: Rect,
    target: ratatui::layout::Size,
    font_size: ratatui_image::FontSize,
) {
    use image::DynamicImage;

    use super::raster::rasterize_scene;

    let raster_size = super::raster::RasterSize {
        width: u32::from(area.width.max(1)).saturating_mul(u32::from(font_size.width.max(1))),
        height: u32::from(area.height.max(1)).saturating_mul(u32::from(font_size.height.max(1))),
    };
    crate::tui::graphics::queue_chart(slot, key, target, || {
        DynamicImage::ImageRgba8(rasterize_scene(scene, raster_size))
    });
}

#[cfg(feature = "tui-graphics")]
fn graphics_chart_slot_key(
    scene: &ChartScene<'_>,
    area: Rect,
    font_size: ratatui_image::FontSize,
) -> crate::tui::graphics::ChartSlotKey {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut state = DefaultHasher::new();
    area.width.hash(&mut state);
    area.height.hash(&mut state);
    font_size.width.hash(&mut state);
    font_size.height.hash(&mut state);
    render_caps::current().graphics_protocol.hash(&mut state);
    hash_x_axis(scene.x_axis, &mut state);
    hash_baseline_shape(scene.baseline, &mut state);
    scene.grid.tick_count.hash(&mut state);
    for series in &scene.series {
        series.name.hash(&mut state);
        hash_color(series.line_color, &mut state);
        hash_fill(series.fill, &mut state);
        match series.direction {
            SeriesDirection::Up => 1u8.hash(&mut state),
            SeriesDirection::Down => 2u8.hash(&mut state),
        }
    }
    crate::tui::graphics::ChartSlotKey(state.finish())
}

#[cfg(feature = "tui-graphics")]
fn graphics_chart_key(
    scene: &ChartScene<'_>,
    area: Rect,
    font_size: ratatui_image::FontSize,
) -> crate::tui::graphics::ChartImageKey {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut state = DefaultHasher::new();
    area.width.hash(&mut state);
    area.height.hash(&mut state);
    font_size.width.hash(&mut state);
    font_size.height.hash(&mut state);
    render_caps::current().graphics_protocol.hash(&mut state);
    hash_x_axis(scene.x_axis, &mut state);
    hash_bounds(scene.bounds, &mut state);
    hash_baseline(scene.baseline, &mut state);
    scene.grid.tick_count.hash(&mut state);
    for series in &scene.series {
        series.name.hash(&mut state);
        hash_series_data(series.data, &mut state);
        hash_color(series.line_color, &mut state);
        hash_fill(series.fill, &mut state);
        match series.direction {
            SeriesDirection::Up => 1u8.hash(&mut state),
            SeriesDirection::Down => 2u8.hash(&mut state),
        }
    }
    for annotation in &scene.annotations {
        match annotation.kind {
            AnnotationKind::Now => 1u8.hash(&mut state),
            AnnotationKind::Peak => 2u8.hash(&mut state),
        }
        hash_f64(annotation.x, &mut state);
        hash_f64(annotation.y, &mut state);
        hash_f64(annotation.transformed_y, &mut state);
        hash_color(annotation.color, &mut state);
    }
    crate::tui::graphics::ChartImageKey(state.finish())
}

#[cfg(feature = "tui-graphics")]
fn hash_x_axis(axis: XAxis, state: &mut impl std::hash::Hasher) {
    use std::hash::Hash;

    match axis {
        XAxis::Hidden => 0u8.hash(state),
        XAxis::Relative { sample_interval } => {
            1u8.hash(state);
            sample_interval.as_nanos().hash(state);
        }
        XAxis::Epoch => 2u8.hash(state),
    }
}

#[cfg(feature = "tui-graphics")]
fn hash_bounds(bounds: PlotBounds, state: &mut impl std::hash::Hasher) {
    hash_f64(bounds.x_min, state);
    hash_f64(bounds.x_max, state);
    hash_f64(bounds.y_min, state);
    hash_f64(bounds.y_max, state);
}

#[cfg(feature = "tui-graphics")]
fn hash_baseline(baseline: Baseline<'_>, state: &mut impl std::hash::Hasher) {
    use std::hash::Hash;

    match baseline {
        Baseline::Zero { y_max } => {
            0u8.hash(state);
            hash_f64(y_max, state);
        }
        Baseline::Mirror {
            upper_max,
            lower_max,
            upper_label,
            lower_label,
        } => {
            1u8.hash(state);
            hash_f64(upper_max, state);
            hash_f64(lower_max, state);
            upper_label.hash(state);
            lower_label.hash(state);
        }
    }
}

#[cfg(feature = "tui-graphics")]
fn hash_baseline_shape(baseline: Baseline<'_>, state: &mut impl std::hash::Hasher) {
    use std::hash::Hash;

    match baseline {
        Baseline::Zero { .. } => 0u8.hash(state),
        Baseline::Mirror {
            upper_label,
            lower_label,
            ..
        } => {
            1u8.hash(state);
            upper_label.hash(state);
            lower_label.hash(state);
        }
    }
}

#[cfg(feature = "tui-graphics")]
fn hash_series_data(data: SeriesData<'_>, state: &mut impl std::hash::Hasher) {
    use std::hash::Hash;

    match data {
        SeriesData::Dense(points) => {
            0u8.hash(state);
            points.len().hash(state);
            for (x, y) in points {
                hash_f64(*x, state);
                hash_f64(*y, state);
            }
        }
        SeriesData::Gapped(points) => {
            1u8.hash(state);
            points.len().hash(state);
            for point in points {
                hash_f64(point.x, state);
                match point.y {
                    Some(y) => {
                        1u8.hash(state);
                        hash_f64(y, state);
                    }
                    None => 0u8.hash(state),
                }
            }
        }
    }
}

#[cfg(feature = "tui-graphics")]
fn hash_fill(fill: FillStyle, state: &mut impl std::hash::Hasher) {
    use std::hash::Hash;

    match fill {
        FillStyle::None => 0u8.hash(state),
        FillStyle::Solid(color) => {
            1u8.hash(state);
            hash_color(color, state);
        }
        FillStyle::Gradient(gradient) => {
            2u8.hash(state);
            let (start, end) = gradient.endpoints();
            hash_color(start, state);
            hash_color(end, state);
        }
    }
}

#[cfg(feature = "tui-graphics")]
fn hash_color(color: Color, state: &mut impl std::hash::Hasher) {
    use std::hash::Hash;

    match color {
        Color::Reset => 0u8.hash(state),
        Color::Black => 1u8.hash(state),
        Color::Red => 2u8.hash(state),
        Color::Green => 3u8.hash(state),
        Color::Yellow => 4u8.hash(state),
        Color::Blue => 5u8.hash(state),
        Color::Magenta => 6u8.hash(state),
        Color::Cyan => 7u8.hash(state),
        Color::Gray => 8u8.hash(state),
        Color::DarkGray => 9u8.hash(state),
        Color::LightRed => 10u8.hash(state),
        Color::LightGreen => 11u8.hash(state),
        Color::LightYellow => 12u8.hash(state),
        Color::LightBlue => 13u8.hash(state),
        Color::LightMagenta => 14u8.hash(state),
        Color::LightCyan => 15u8.hash(state),
        Color::White => 16u8.hash(state),
        Color::Rgb(r, g, b) => {
            17u8.hash(state);
            r.hash(state);
            g.hash(state);
            b.hash(state);
        }
        Color::Indexed(index) => {
            18u8.hash(state);
            index.hash(state);
        }
    }
}

#[cfg(feature = "tui-graphics")]
fn hash_f64(value: f64, state: &mut impl std::hash::Hasher) {
    use std::hash::Hash;

    value.to_bits().hash(state);
}

fn transform_value(baseline: Baseline<'_>, direction: SeriesDirection, value: f64) -> f64 {
    match (baseline, direction) {
        (Baseline::Mirror { lower_max, .. }, SeriesDirection::Down) => -value.min(lower_max),
        (Baseline::Mirror { upper_max, .. }, SeriesDirection::Up) => value.min(upper_max),
        _ => value,
    }
}

fn fraction(numerator: usize, denominator: usize) -> f64 {
    #[allow(clippy::cast_precision_loss, clippy::as_conversions)]
    {
        numerator as f64 / denominator as f64
    }
}

fn y_to_row(area: Rect, bounds: PlotBounds, y: f64) -> u16 {
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

fn x_to_col(area: Rect, bounds: PlotBounds, x: f64) -> u16 {
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

fn render_text(buf: &mut Buffer, area: Rect, text: &str, style: Style) {
    Paragraph::new(Line::from(Span::styled(text.to_owned(), style))).render(area, buf);
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;

    use crate::tui::render_caps::{ColorDepth, GlyphTier};
    use crate::tui::widgets::hyperchart::{ChartGradient, ChartPoint};

    fn render_chart(widget: HyperChart<'_>, width: u16, height: u16) -> Buffer {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
        buf
    }

    fn buffer_text(buf: &Buffer) -> String {
        (0..buf.area().height)
            .map(|y| {
                (0..buf.area().width)
                    .filter_map(|x| buf.cell((x, y)).map(|cell| cell.symbol().to_string()))
                    .collect::<String>()
            })
            .collect()
    }

    fn test_caps(glyph_tier: GlyphTier) -> RenderCaps {
        RenderCaps {
            color_depth: ColorDepth::TrueColor,
            glyph_tier,
            graphics_protocol: crate::tui::render_caps::GraphicsProtocol::None,
        }
    }

    #[test]
    fn empty_series_renders_empty_message() {
        let series: &[Series<'_>] = &[];
        let widget = HyperChart::new(Line::from(" Bandwidth "), series, (0.0, 10.0), 1_000.0)
            .empty_message("No bandwidth data yet");
        let buf = render_chart(widget, 60, 12);

        assert!(buffer_text(&buf).contains("No bandwidth data yet"));
    }

    #[test]
    fn gapped_series_splits_visible_segments() {
        let points = [
            ChartPoint {
                x: 0.0,
                y: Some(1.0),
            },
            ChartPoint {
                x: 1.0,
                y: Some(2.0),
            },
            ChartPoint { x: 2.0, y: None },
            ChartPoint {
                x: 3.0,
                y: Some(4.0),
            },
        ];

        let segments = SeriesData::Gapped(&points).visible_segments();

        assert_eq!(
            segments,
            vec![vec![(0.0, 1.0), (1.0, 2.0)], vec![(3.0, 4.0)]]
        );
    }

    #[test]
    fn relative_x_axis_labels_end_with_now() {
        let data = [(0.0, 1.0), (120.0, 2.0)];
        let series = [Series {
            name: "TX",
            data: SeriesData::Dense(&data),
            line_color: Color::Cyan,
            fill: FillStyle::None,
            direction: SeriesDirection::Up,
        }];
        let widget = HyperChart::new(Line::from(" Bandwidth "), &series, (0.0, 120.0), 2_000.0)
            .x_axis(XAxis::Relative {
                sample_interval: Duration::from_millis(250),
            })
            .renderer(Renderer::Canvas { gutter_width: 7 })
            .render_caps(test_caps(GlyphTier::Braille));
        let buf = render_chart(widget, 80, 16);
        let text = buffer_text(&buf);

        assert!(text.contains("-30s"));
        assert!(text.contains("now"));
    }

    #[test]
    fn epoch_x_axis_labels_render_clock_times() {
        let data = [(3_600.0, 1.0), (7_200.0, 2.0)];
        let series = [Series {
            name: "Clients",
            data: SeriesData::Dense(&data),
            line_color: Color::Cyan,
            fill: FillStyle::None,
            direction: SeriesDirection::Up,
        }];
        let widget = HyperChart::new(Line::from(" Clients "), &series, (3_600.0, 7_200.0), 3.0)
            .domain(Domain::Count)
            .x_axis(XAxis::Epoch)
            .renderer(Renderer::Canvas { gutter_width: 6 })
            .render_caps(test_caps(GlyphTier::Braille));
        let buf = render_chart(widget, 80, 16);
        let text = buffer_text(&buf);

        assert!(text.contains("01:00"));
        assert!(text.contains("02:00"));
    }

    #[test]
    fn tiled_renderer_draws_single_series_without_panic() {
        let data: Vec<(f64, f64)> = (0..20)
            .map(|i| (f64::from(i), f64::from(i * 100)))
            .collect();
        let series = [Series {
            name: "TX",
            data: SeriesData::Dense(&data),
            line_color: Color::Cyan,
            fill: FillStyle::Solid(Color::Blue),
            direction: SeriesDirection::Up,
        }];
        let widget = HyperChart::new(Line::from(" Bandwidth "), &series, (0.0, 20.0), 2_000.0);
        let _ = render_chart(widget, 60, 12);
    }

    #[test]
    fn canvas_renderer_draws_mirror_series_without_panic() {
        let tx: Vec<(f64, f64)> = (0..30)
            .map(|i| (f64::from(i), f64::from(i) * 123.0))
            .collect();
        let rx: Vec<(f64, f64)> = (0..30)
            .map(|i| (f64::from(i), f64::from(i) * 456.0))
            .collect();
        let series = [
            Series {
                name: "RX",
                data: SeriesData::Dense(&rx),
                line_color: Color::Magenta,
                fill: FillStyle::Gradient(ChartGradient::new(Color::Black, Color::Red)),
                direction: SeriesDirection::Up,
            },
            Series {
                name: "TX",
                data: SeriesData::Dense(&tx),
                line_color: Color::Cyan,
                fill: FillStyle::Gradient(ChartGradient::new(Color::Black, Color::Blue)),
                direction: SeriesDirection::Down,
            },
        ];
        let widget = HyperChart::new(Line::from(" WAN Traffic "), &series, (0.0, 30.0), 20_000.0)
            .baseline(Baseline::Mirror {
                upper_max: 20_000.0,
                lower_max: 4_000.0,
                upper_label: "RX",
                lower_label: "TX",
            })
            .renderer(Renderer::Canvas { gutter_width: 7 })
            .render_caps(test_caps(GlyphTier::Octant));
        let _ = render_chart(widget, 80, 16);
    }

    #[test]
    fn block_tier_renders_without_braille_assumptions() {
        let data: Vec<(f64, f64)> = (0..12)
            .map(|i| (f64::from(i), f64::from(i * 100)))
            .collect();
        let series = [Series {
            name: "Clients",
            data: SeriesData::Dense(&data),
            line_color: Color::Green,
            fill: FillStyle::None,
            direction: SeriesDirection::Up,
        }];
        let widget = HyperChart::new(Line::from(" Clients "), &series, (0.0, 12.0), 1_500.0)
            .renderer(Renderer::Canvas { gutter_width: 6 })
            .render_caps(test_caps(GlyphTier::Block));
        let _ = render_chart(widget, 64, 12);
    }
}
