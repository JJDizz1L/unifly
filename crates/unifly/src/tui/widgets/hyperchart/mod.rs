//! HyperChart — unified time-series and ranked-bar widgets for the unifly TUI.
//!
//! Two widgets live here:
//!
//! - [`HyperChart`] for time-series visualisations (WAN bandwidth, client
//!   counts, anything with an x/y axis). Two back-ends: [`Renderer::Canvas`]
//!   for hero panels (Octant marker, manual gutter) and [`Renderer::Tiled`]
//!   for dense grid cells (Ratatui `Chart` widget, built-in axes).
//! - [`HyperBars`] for ranked horizontal bar lists (top apps, traffic
//!   categories). Denominator is configurable (max-observed or total).
//!
//! Both widgets share axis math ([`axis`]), empty-state rendering
//! ([`empty`]), and block styling ([`block`]), so any visual refinement
//! lands in one place.

pub mod axis;
pub mod bars;
pub mod block;
pub mod color;
pub mod empty;
pub mod time_series;

pub use bars::{Denominator, HyperBars, Row, ValueFormat};
pub use color::ChartGradient;
pub use time_series::{Domain, HyperChart, Renderer, Series};
