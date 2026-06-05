//! Shared data model for HyperChart scenes and renderers.

use std::time::Duration;

use ratatui::style::Color;

use super::ChartGradient;

/// A gap-aware time-series point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartPoint {
    pub x: f64,
    pub y: Option<f64>,
}

/// Data backing a chart series.
#[derive(Debug, Clone, Copy)]
pub enum SeriesData<'a> {
    /// Dense data where every point has a value.
    Dense(&'a [(f64, f64)]),
    /// Gap-aware data where `None` breaks lines and fills.
    Gapped(&'a [ChartPoint]),
}

/// Fill style for a chart series.
#[derive(Debug, Clone, Copy)]
pub enum FillStyle {
    None,
    Solid(Color),
    Gradient(ChartGradient),
}

/// Direction relative to the chart baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesDirection {
    Up,
    Down,
}

/// One time-series dataset rendered by `HyperChart`.
#[derive(Debug, Clone, Copy)]
pub struct Series<'a> {
    pub name: &'a str,
    pub data: SeriesData<'a>,
    pub line_color: Color,
    pub fill: FillStyle,
    pub direction: SeriesDirection,
}

/// Y-axis label formatting domain.
#[derive(Debug, Clone, Copy)]
pub enum Domain {
    /// Bytes-per-second rate (labels look like `"1.2G"`, `"500K"`).
    Rate,
    /// Integer counts (labels look like `"42"`).
    Count,
}

/// X-axis label model.
#[derive(Debug, Clone, Copy)]
pub enum XAxis {
    Hidden,
    /// x is a monotonic sample index. Labels render relative to the newest
    /// visible sample: `-30s`, `-20s`, `now`.
    Relative {
        sample_interval: Duration,
    },
    /// x is epoch seconds. Labels render as UTC clock times.
    Epoch,
}

/// Baseline layout for a time-series chart.
#[derive(Debug, Clone, Copy)]
pub enum Baseline<'a> {
    Zero {
        y_max: f64,
    },
    Mirror {
        upper_max: f64,
        lower_max: f64,
        upper_label: &'a str,
        lower_label: &'a str,
    },
}

impl SeriesData<'_> {
    pub(super) fn points(self) -> Vec<(f64, f64)> {
        match self {
            Self::Dense(data) => data.to_vec(),
            Self::Gapped(points) => points
                .iter()
                .filter_map(|point| point.y.map(|y| (point.x, y)))
                .collect(),
        }
    }

    pub(super) fn visible_segments(self) -> Vec<Vec<(f64, f64)>> {
        match self {
            Self::Dense(data) => {
                if data.is_empty() {
                    Vec::new()
                } else {
                    vec![data.to_vec()]
                }
            }
            Self::Gapped(points) => {
                let mut segments = Vec::new();
                let mut current = Vec::new();
                for point in points {
                    if let Some(y) = point.y {
                        current.push((point.x, y));
                    } else if !current.is_empty() {
                        segments.push(std::mem::take(&mut current));
                    }
                }
                if !current.is_empty() {
                    segments.push(current);
                }
                segments
            }
        }
    }
}
