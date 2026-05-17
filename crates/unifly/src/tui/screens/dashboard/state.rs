use std::sync::Arc;
use std::time::Instant;

use crate::tui::action::Action;
use crate::tui::widgets::hyperchart::axis;
use unifly_api::DeviceType;

use super::{
    BANDWIDTH_SCALE_PERCENTILE, BANDWIDTH_SCALE_WINDOW_SAMPLES, BANDWIDTH_SMOOTHING_ALPHA,
    BANDWIDTH_TICK_COUNT, BandwidthSample, DashboardScreen, LIVE_CHART_SAMPLE_INTERVAL,
    LIVE_CHART_WINDOW_SAMPLES, MIN_BANDWIDTH_SCALE,
};

impl DashboardScreen {
    /// Format the data age as a human-readable string for the title bar.
    pub(super) fn refresh_age_str(&self) -> String {
        match self.last_data_update {
            Some(t) => {
                let secs = t.elapsed().as_secs();
                if secs < 5 {
                    "just now".into()
                } else if secs < 60 {
                    format!("{secs}s ago")
                } else {
                    format!("{}m ago", secs / 60)
                }
            }
            None => "no data".into(),
        }
    }

    /// Record a bandwidth sample into the chart data ring buffer.
    #[allow(clippy::cast_precision_loss, clippy::as_conversions)]
    pub(super) fn push_bandwidth_sample(&mut self, tx_bps: u64, rx_bps: u64) {
        self.sample_counter += 1.0;
        self.bandwidth_tx.push((self.sample_counter, tx_bps as f64));
        self.bandwidth_rx.push((self.sample_counter, rx_bps as f64));
        self.peak_tx = self.peak_tx.max(tx_bps);
        self.peak_rx = self.peak_rx.max(rx_bps);

        if self.bandwidth_tx.len() > LIVE_CHART_WINDOW_SAMPLES {
            self.bandwidth_tx.remove(0);
            self.bandwidth_rx.remove(0);
        }

        let visible_max = self.bandwidth_scale_reference();
        self.chart_y_max = axis::stable_upper_bound(
            self.chart_y_max,
            visible_max,
            BANDWIDTH_TICK_COUNT,
            MIN_BANDWIDTH_SCALE,
        );
    }

    pub(super) fn bandwidth_scale_reference(&self) -> f64 {
        let mut values: Vec<f64> = self
            .bandwidth_tx
            .iter()
            .rev()
            .take(BANDWIDTH_SCALE_WINDOW_SAMPLES)
            .chain(
                self.bandwidth_rx
                    .iter()
                    .rev()
                    .take(BANDWIDTH_SCALE_WINDOW_SAMPLES),
            )
            .map(|&(_, value)| value)
            .filter(|value| *value > 0.0)
            .collect();

        if values.is_empty() {
            return 0.0;
        }

        values.sort_by(f64::total_cmp);
        let percentile_index =
            ((values.len().saturating_sub(1)) * BANDWIDTH_SCALE_PERCENTILE) / 100;
        let percentile_value = values[percentile_index];
        let current_value = self
            .bandwidth_tx
            .last()
            .map_or(0.0, |&(_, value)| value)
            .max(self.bandwidth_rx.last().map_or(0.0, |&(_, value)| value));

        percentile_value.max(current_value)
    }

    pub(super) fn current_bandwidth(&self) -> Option<(u64, u64)> {
        match (self.device_bandwidth, self.health_bandwidth) {
            (Some(device), Some(health)) => {
                if health.captured_at >= device.captured_at {
                    Some((health.tx_bps, health.rx_bps))
                } else {
                    Some((device.tx_bps, device.rx_bps))
                }
            }
            (Some(device), None) => Some((device.tx_bps, device.rx_bps)),
            (None, Some(health)) => Some((health.tx_bps, health.rx_bps)),
            (None, None) => None,
        }
    }

    #[allow(clippy::cast_precision_loss, clippy::as_conversions)]
    pub(super) fn sample_bandwidth_if_due(&mut self, now: Instant) -> bool {
        if self.last_chart_sample_at.is_some_and(|last_sample_at| {
            now.duration_since(last_sample_at) < LIVE_CHART_SAMPLE_INTERVAL
        }) {
            return false;
        }

        let Some((tx_bps, rx_bps)) = self.current_bandwidth() else {
            return false;
        };

        let target_upload_bps = tx_bps as f64;
        let target_download_bps = rx_bps as f64;

        let next_upload_bps = self
            .display_tx_bps
            .map_or(target_upload_bps, |current_tx_bps| {
                let smoothed = current_tx_bps
                    + (target_upload_bps - current_tx_bps) * BANDWIDTH_SMOOTHING_ALPHA;
                if (target_upload_bps - smoothed).abs() < 1.0 {
                    target_upload_bps
                } else {
                    smoothed
                }
            });
        let next_download_bps = self
            .display_rx_bps
            .map_or(target_download_bps, |current_rx_bps| {
                let smoothed = current_rx_bps
                    + (target_download_bps - current_rx_bps) * BANDWIDTH_SMOOTHING_ALPHA;
                if (target_download_bps - smoothed).abs() < 1.0 {
                    target_download_bps
                } else {
                    smoothed
                }
            });

        self.display_tx_bps = Some(next_upload_bps);
        self.display_rx_bps = Some(next_download_bps);
        #[allow(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            clippy::as_conversions
        )]
        {
            self.push_bandwidth_sample(
                next_upload_bps.round() as u64,
                next_download_bps.round() as u64,
            );
        }
        self.last_chart_sample_at = Some(now);
        true
    }

    pub(super) fn apply_action(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::Tick => {
                if self.sample_bandwidth_if_due(Instant::now()) && self.focused {
                    return Some(Action::Invalidate);
                }
            }
            Action::DevicesUpdated(devices) => {
                self.devices = Arc::clone(devices);
                let now = Instant::now();
                self.last_data_update = Some(now);
                self.device_bandwidth = self
                    .devices
                    .iter()
                    .find(|d| d.device_type == DeviceType::Gateway)
                    .and_then(|gw| {
                        gw.stats
                            .uplink_bandwidth
                            .as_ref()
                            .map(|bw| BandwidthSample {
                                tx_bps: bw.tx_bytes_per_sec,
                                rx_bps: bw.rx_bytes_per_sec,
                                captured_at: now,
                            })
                    });
                if let Some((tx_bps, rx_bps)) = self.current_bandwidth() {
                    self.peak_tx = self.peak_tx.max(tx_bps);
                    self.peak_rx = self.peak_rx.max(rx_bps);
                }
            }
            Action::ClientsUpdated(clients) => {
                self.clients = Arc::clone(clients);
            }
            Action::NetworksUpdated(networks) => {
                self.networks = Arc::clone(networks);
            }
            Action::EventReceived(event) => {
                self.events.push(Arc::clone(event));
                if self.events.len() > 100 {
                    self.events.remove(0);
                }
            }
            Action::HealthUpdated(health) => {
                self.health = Arc::clone(health);
                let now = Instant::now();
                self.last_data_update = Some(now);
                self.health_bandwidth = self
                    .health
                    .iter()
                    .find(|health| health.subsystem == "wan")
                    .map(|wan| BandwidthSample {
                        tx_bps: wan.tx_bytes_r.unwrap_or(0),
                        rx_bps: wan.rx_bytes_r.unwrap_or(0),
                        captured_at: now,
                    });
                if let Some((tx_bps, rx_bps)) = self.current_bandwidth() {
                    self.peak_tx = self.peak_tx.max(tx_bps);
                    self.peak_rx = self.peak_rx.max(rx_bps);
                }
            }
            _ => {}
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn current_bandwidth_prefers_most_recent_sample() {
        let mut screen = DashboardScreen::new();
        let base = Instant::now();
        screen.device_bandwidth = Some(BandwidthSample {
            tx_bps: 10,
            rx_bps: 20,
            captured_at: base,
        });
        screen.health_bandwidth = Some(BandwidthSample {
            tx_bps: 30,
            rx_bps: 40,
            captured_at: base + Duration::from_secs(1),
        });

        assert_eq!(screen.current_bandwidth(), Some((30, 40)));
    }

    #[test]
    fn bandwidth_scaling_uses_recent_visible_samples() {
        let mut screen = DashboardScreen::new();
        for idx in 1..=4 {
            screen
                .bandwidth_tx
                .push((f64::from(idx), f64::from(idx * 1000)));
            screen
                .bandwidth_rx
                .push((f64::from(idx), f64::from(idx * 500)));
        }

        let reference = screen.bandwidth_scale_reference();
        assert!(reference >= 4_000.0);
    }

    #[test]
    fn focused_tick_invalidates_after_sampling() {
        let mut screen = DashboardScreen::new();
        screen.focused = true;
        let now = Instant::now();
        screen.device_bandwidth = Some(BandwidthSample {
            tx_bps: 10_000,
            rx_bps: 40_000,
            captured_at: now,
        });

        let action = screen.apply_action(&Action::Tick);

        assert!(matches!(action, Some(Action::Invalidate)));
        assert_eq!(screen.bandwidth_tx.len(), 1);
        assert_eq!(screen.bandwidth_rx.len(), 1);
    }
}
