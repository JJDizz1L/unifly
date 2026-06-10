//! Curve geometry for the pixel rasterizer: monotone cubic splines and M4
//! downsampling, all in pixel space.

use tiny_skia::{Path, PathBuilder};

/// Average sample spacing in pixels above which the curve is splined.
/// Denser data is already smooth at pixel scale and splining duplicate-x
/// M4 output would be wasted work.
const SPLINE_MIN_SPACING: f32 = 3.0;

/// Build the stroked curve through the points: monotone cubic when samples
/// are sparse enough to benefit, polyline otherwise.
pub(super) fn curve_path(points: &[(f32, f32)]) -> Option<Path> {
    let mut builder = PathBuilder::new();
    append_curve(&mut builder, points)?;
    builder.finish()
}

/// Build the closed area between the curve and the baseline row.
pub(super) fn fill_path(points: &[(f32, f32)], baseline: f32) -> Option<Path> {
    let first = points.first()?;
    let last = points.last()?;
    let mut builder = PathBuilder::new();
    append_curve(&mut builder, points)?;
    builder.line_to(last.0, baseline);
    builder.line_to(first.0, baseline);
    builder.close();
    builder.finish()
}

/// M4 reduction: keep first, vertical extremes, and last sample per pixel
/// column so peaks survive any data-to-pixel density ratio.
pub(super) fn downsample_m4(points: Vec<(f32, f32)>, width: u32) -> Vec<(f32, f32)> {
    let budget = usize::try_from(width)
        .unwrap_or(usize::MAX)
        .saturating_mul(2);
    if points.len() <= budget {
        return points;
    }

    let mut out: Vec<(f32, f32)> = Vec::with_capacity(budget.saturating_mul(2));
    let mut bucket: Option<(i64, [(f32, f32); 4])> = None;

    #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
    let column_of = |x: f32| x.floor() as i64;

    for point in points {
        let column = column_of(point.0);
        match bucket.as_mut() {
            Some((current, [_, low, high, last])) if *current == column => {
                if point.1 < low.1 {
                    *low = point;
                }
                if point.1 > high.1 {
                    *high = point;
                }
                *last = point;
            }
            _ => {
                if let Some((_, cells)) = bucket.take() {
                    flush_bucket(&mut out, cells);
                }
                bucket = Some((column, [point; 4]));
            }
        }
    }
    if let Some((_, cells)) = bucket.take() {
        flush_bucket(&mut out, cells);
    }

    out
}

fn flush_bucket(out: &mut Vec<(f32, f32)>, [first, low, high, last]: [(f32, f32); 4]) {
    let mut ordered = [first, low, high, last];
    ordered.sort_by(|left, right| left.0.total_cmp(&right.0));
    for point in ordered {
        if out.last() != Some(&point) {
            out.push(point);
        }
    }
}

fn append_curve(builder: &mut PathBuilder, points: &[(f32, f32)]) -> Option<()> {
    let (&(first_x, first_y), rest) = points.split_first()?;
    if rest.is_empty() {
        return None;
    }
    builder.move_to(first_x, first_y);

    #[allow(clippy::cast_precision_loss, clippy::as_conversions)]
    let spacing = (points.last()?.0 - first_x) / (points.len() - 1) as f32;
    if spacing >= SPLINE_MIN_SPACING {
        let tangents = monotone_tangents(points);
        for (window, tangent_pair) in points.windows(2).zip(tangents.windows(2)) {
            let [(x0, y0), (x1, y1)] = [window[0], window[1]];
            let step = (x1 - x0) / 3.0;
            builder.cubic_to(
                x0 + step,
                tangent_pair[0].mul_add(step, y0),
                x1 - step,
                (-tangent_pair[1]).mul_add(step, y1),
                x1,
                y1,
            );
        }
    } else {
        for &(x, y) in rest {
            builder.line_to(x, y);
        }
    }
    Some(())
}

/// Fritsch-Carlson monotone tangents: flat at local extrema, secant-limited
/// elsewhere, so the spline never overshoots the data.
fn monotone_tangents(points: &[(f32, f32)]) -> Vec<f32> {
    let count = points.len();
    let secants: Vec<f32> = points
        .windows(2)
        .map(|pair| {
            let [(x0, y0), (x1, y1)] = [pair[0], pair[1]];
            (y1 - y0) / (x1 - x0).max(f32::EPSILON)
        })
        .collect();

    let mut tangents = vec![0.0_f32; count];
    if let (Some(first), Some(last)) = (secants.first(), secants.last()) {
        tangents[0] = *first;
        tangents[count - 1] = *last;
    }
    for index in 1..count.saturating_sub(1) {
        let before = secants[index - 1];
        let after = secants[index];
        tangents[index] = if before * after <= 0.0 {
            0.0
        } else {
            f32::midpoint(before, after)
        };
    }

    for (index, &secant) in secants.iter().enumerate() {
        if secant.abs() < f32::EPSILON {
            tangents[index] = 0.0;
            tangents[index + 1] = 0.0;
            continue;
        }
        let alpha = tangents[index] / secant;
        let beta = tangents[index + 1] / secant;
        let magnitude = alpha.mul_add(alpha, beta * beta);
        if magnitude > 9.0 {
            let scale = 3.0 / magnitude.sqrt();
            tangents[index] = scale * alpha * secant;
            tangents[index + 1] = scale * beta * secant;
        }
    }

    tangents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotone_tangents_flatten_local_extrema() {
        let points = [(0.0, 0.0), (10.0, 4.0), (20.0, 0.0)];
        let tangents = monotone_tangents(&points);

        assert!(tangents[1].abs() < f32::EPSILON, "peak must be flat");
        assert!(tangents[0] > 0.0);
        assert!(tangents[2] < 0.0);
    }

    #[test]
    fn m4_downsampling_preserves_spikes() {
        let mut points: Vec<(f32, f32)> = (0..1_000)
            .map(|index| {
                #[allow(clippy::cast_precision_loss, clippy::as_conversions)]
                let x = index as f32 / 50.0;
                (x, 5.0)
            })
            .collect();
        points[500].1 = 95.0;

        let reduced = downsample_m4(points, 20);

        assert!(reduced.len() < 200);
        assert!(
            reduced
                .iter()
                .any(|&(_, y)| (y - 95.0).abs() < f32::EPSILON),
            "spike lost in reduction"
        );
    }
}
