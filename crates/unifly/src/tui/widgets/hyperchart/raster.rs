//! Pixel rasterizer for optional graphics-protocol chart rendering.

use image::{Rgba, RgbaImage};
use ratatui::style::Color;

use super::axis;
use super::color::color_to_rgb;
use super::model::FillStyle;
use super::scene::{AnnotationKind, ChartScene, PlotBounds, SceneSeries};
use crate::tui::render_caps::{ColorDepth, GlyphTier, GraphicsProtocol, RenderCaps};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterSize {
    pub width: u32,
    pub height: u32,
}

pub fn rasterize_scene(scene: &ChartScene<'_>, size: RasterSize) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(size.width.max(1), size.height.max(1), transparent());

    draw_grid(&mut image, scene, size);
    for series in &scene.series {
        draw_series_fill(&mut image, scene, series, size);
    }
    for series in &scene.series {
        draw_series_line(&mut image, scene, series, size);
    }
    draw_annotations(&mut image, scene, size);

    image
}

fn draw_grid(image: &mut RgbaImage, scene: &ChartScene<'_>, size: RasterSize) {
    let grid = rgba(Color::DarkGray, 80);
    for y in scene.gridlines() {
        draw_horizontal(image, y_to_pixel(size, scene.bounds, y), grid);
    }
    draw_horizontal(
        image,
        y_to_pixel(size, scene.bounds, 0.0),
        rgba(Color::Gray, 120),
    );
}

fn draw_series_fill(
    image: &mut RgbaImage,
    scene: &ChartScene<'_>,
    series: &SceneSeries<'_>,
    size: RasterSize,
) {
    let Some(bands) = fill_bands(series.fill, size.height) else {
        return;
    };
    let density = usize::try_from(size.width.max(1)).unwrap_or(usize::MAX);
    for segment in series.data.visible_segments() {
        for (x, y) in axis::interpolate_fill(&segment, density) {
            let transformed_y = scene.transform_y(series, y);
            draw_fill_column(image, size, scene.bounds, x, transformed_y, &bands);
        }
    }
}

fn draw_series_line(
    image: &mut RgbaImage,
    scene: &ChartScene<'_>,
    series: &SceneSeries<'_>,
    size: RasterSize,
) {
    let color = rgba(series.line_color, 245);
    for segment in series.data.visible_segments() {
        let points = axis::interpolate_fill(&segment, usize::try_from(size.width).unwrap_or(1));
        for pair in points.windows(2) {
            let [(x1, y1), (x2, y2)] = pair else {
                continue;
            };
            draw_line(
                image,
                (
                    x_to_pixel(size, scene.bounds, *x1),
                    y_to_pixel(size, scene.bounds, scene.transform_y(series, *y1)),
                ),
                (
                    x_to_pixel(size, scene.bounds, *x2),
                    y_to_pixel(size, scene.bounds, scene.transform_y(series, *y2)),
                ),
                color,
            );
        }
    }
}

fn draw_annotations(image: &mut RgbaImage, scene: &ChartScene<'_>, size: RasterSize) {
    for annotation in &scene.annotations {
        let x = x_to_pixel(size, scene.bounds, annotation.x);
        let y = y_to_pixel(size, scene.bounds, annotation.transformed_y);
        let color = rgba(annotation.color, 255);
        match annotation.kind {
            AnnotationKind::Now => draw_disc(image, x, y, 3, color),
            AnnotationKind::Peak => draw_diamond(image, x, y, 4, color),
        }
    }
}

fn draw_fill_column(
    image: &mut RgbaImage,
    size: RasterSize,
    bounds: PlotBounds,
    x: f64,
    y: f64,
    bands: &[Color],
) {
    if y.abs() < f64::EPSILON {
        return;
    }

    let x = x_to_pixel(size, bounds, x);
    let baseline = y_to_pixel(size, bounds, 0.0);
    let value = y_to_pixel(size, bounds, y);
    let start = baseline.min(value);
    let end = baseline.max(value);
    let span = end.saturating_sub(start).max(1);

    for row in start..=end {
        let distance = row.abs_diff(baseline);
        let band = band_at(bands, distance, span);
        blend_pixel(image, x, row, rgba(band, 130));
    }
}

fn fill_bands(fill: FillStyle, height: u32) -> Option<Vec<Color>> {
    let caps = RenderCaps {
        color_depth: ColorDepth::TrueColor,
        glyph_tier: GlyphTier::Braille,
        graphics_protocol: GraphicsProtocol::None,
    };
    match fill {
        FillStyle::None => None,
        FillStyle::Solid(color) => Some(vec![color]),
        FillStyle::Gradient(gradient) => {
            Some(gradient.bands(caps, usize::try_from(height.max(2)).unwrap_or(usize::MAX)))
        }
    }
}

fn band_at(bands: &[Color], distance: u32, span: u32) -> Color {
    let last = bands.len().saturating_sub(1);
    if last == 0 {
        return bands[0];
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::as_conversions
    )]
    let index = ((f64::from(distance) / f64::from(span)) * last as f64).round() as usize;
    bands[index.min(last)]
}

fn draw_horizontal(image: &mut RgbaImage, y: u32, color: Rgba<u8>) {
    for x in 0..image.width() {
        blend_pixel(image, x, y, color);
    }
}

fn draw_line(image: &mut RgbaImage, start: (u32, u32), end: (u32, u32), color: Rgba<u8>) {
    let (mut x0, mut y0) = (i64::from(start.0), i64::from(start.1));
    let (x1, y1) = (i64::from(end.0), i64::from(end.1));
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        draw_disc_i64(image, x0, y0, 1, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let doubled = err.saturating_mul(2);
        if doubled >= dy {
            err += dy;
            x0 += sx;
        }
        if doubled <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn draw_disc(image: &mut RgbaImage, x: u32, y: u32, radius: i64, color: Rgba<u8>) {
    draw_disc_i64(image, i64::from(x), i64::from(y), radius, color);
}

fn draw_disc_i64(image: &mut RgbaImage, cx: i64, cy: i64, radius: i64, color: Rgba<u8>) {
    for y in -radius..=radius {
        for x in -radius..=radius {
            if x.saturating_mul(x) + y.saturating_mul(y) <= radius.saturating_mul(radius) {
                blend_pixel_i64(image, cx + x, cy + y, color);
            }
        }
    }
}

fn draw_diamond(image: &mut RgbaImage, cx: u32, cy: u32, radius: i64, color: Rgba<u8>) {
    let (cx, cy) = (i64::from(cx), i64::from(cy));
    for y in -radius..=radius {
        for x in -radius..=radius {
            if x.abs() + y.abs() <= radius {
                blend_pixel_i64(image, cx + x, cy + y, color);
            }
        }
    }
}

fn blend_pixel_i64(image: &mut RgbaImage, x: i64, y: i64, color: Rgba<u8>) {
    let Ok(x) = u32::try_from(x) else {
        return;
    };
    let Ok(y) = u32::try_from(y) else {
        return;
    };
    blend_pixel(image, x, y, color);
}

fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, source: Rgba<u8>) {
    if x >= image.width() || y >= image.height() {
        return;
    }

    let alpha = u16::from(source[3]);
    let inverse = 255u16.saturating_sub(alpha);
    let target = image.get_pixel_mut(x, y);
    for channel in 0..3 {
        let value =
            (u16::from(source[channel]) * alpha + u16::from(target[channel]) * inverse) / 255;
        target[channel] = u8::try_from(value).unwrap_or(u8::MAX);
    }
    target[3] = target[3].saturating_add(source[3]);
}

fn transparent() -> Rgba<u8> {
    Rgba([0, 0, 0, 0])
}

fn rgba(color: Color, alpha: u8) -> Rgba<u8> {
    let (red, green, blue) = color_to_rgb(color);
    Rgba([red, green, blue, alpha])
}

fn x_to_pixel(size: RasterSize, bounds: PlotBounds, x: f64) -> u32 {
    let span = (bounds.x_max - bounds.x_min).max(1.0);
    let ratio = ((x - bounds.x_min) / span).clamp(0.0, 1.0);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::as_conversions
    )]
    {
        (f64::from(size.width.saturating_sub(1)) * ratio).round() as u32
    }
}

fn y_to_pixel(size: RasterSize, bounds: PlotBounds, y: f64) -> u32 {
    let span = (bounds.y_max - bounds.y_min).max(1.0);
    let ratio = ((bounds.y_max - y) / span).clamp(0.0, 1.0);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::as_conversions
    )]
    {
        (f64::from(size.height.saturating_sub(1)) * ratio).round() as u32
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::*;
    use crate::tui::widgets::hyperchart::model::{
        Baseline, FillStyle, SeriesData, SeriesDirection, XAxis,
    };
    use crate::tui::widgets::hyperchart::scene::{Annotation, GridSpec};

    #[test]
    fn rasterizer_draws_non_empty_scene() {
        let data = [(0.0, 1.0), (1.0, 3.0), (2.0, 2.0)];
        let scene = ChartScene {
            x_axis: XAxis::Hidden,
            bounds: PlotBounds {
                x_min: 0.0,
                x_max: 2.0,
                y_min: 0.0,
                y_max: 4.0,
            },
            baseline: Baseline::Zero { y_max: 4.0 },
            series: vec![SceneSeries {
                name: "rx",
                data: SeriesData::Dense(&data),
                line_color: Color::Cyan,
                fill: FillStyle::Solid(Color::Blue),
                direction: SeriesDirection::Up,
            }],
            grid: GridSpec { tick_count: 4 },
            annotations: vec![Annotation {
                kind: AnnotationKind::Now,
                x: 2.0,
                y: 2.0,
                transformed_y: 2.0,
                color: Color::Cyan,
            }],
        };

        let image = rasterize_scene(
            &scene,
            RasterSize {
                width: 80,
                height: 32,
            },
        );

        assert!(image.pixels().any(|pixel| pixel[3] > 0));
    }
}
