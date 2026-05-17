use std::sync::Arc;

use super::{TopoNode, TopologyScreen};

use unifly_api::DeviceType;

use crate::tui::action::Action;

impl TopologyScreen {
    pub fn new() -> Self {
        Self {
            focused: false,
            devices: Arc::new(Vec::new()),
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }

    #[allow(clippy::cast_precision_loss, clippy::as_conversions)]
    pub(super) fn build_nodes(&self) -> Vec<TopoNode> {
        let mut nodes = Vec::new();

        let gateways: Vec<_> = self
            .devices
            .iter()
            .filter(|device| device.device_type == DeviceType::Gateway)
            .collect();
        let switches: Vec<_> = self
            .devices
            .iter()
            .filter(|device| device.device_type == DeviceType::Switch)
            .collect();
        let aps: Vec<_> = self
            .devices
            .iter()
            .filter(|device| device.device_type == DeviceType::AccessPoint)
            .collect();
        let others: Vec<_> = self
            .devices
            .iter()
            .filter(|device| {
                device.device_type != DeviceType::Gateway
                    && device.device_type != DeviceType::Switch
                    && device.device_type != DeviceType::AccessPoint
            })
            .collect();

        let gateway_total = gateways.len().max(1);
        let gateway_spacing = 90.0 / gateway_total as f64;
        for (index, gateway) in gateways.iter().enumerate() {
            let x = 50.0 - (gateway_total as f64 * gateway_spacing) / 2.0
                + index as f64 * gateway_spacing;
            nodes.push(TopoNode {
                label: gateway.name.clone().unwrap_or_else(|| "Gateway".into()),
                ip: gateway.ip.map(|ip| ip.to_string()).unwrap_or_default(),
                device_type: gateway.device_type,
                state: gateway.state,
                client_count: gateway.client_count.unwrap_or(0),
                traffic_bytes_per_sec: device_traffic_bytes_per_sec(gateway),
                x,
                y: 80.0,
                width: 16.0,
                height: 8.0,
            });
        }

        let switch_total = switches.len().max(1);
        let switch_spacing = 90.0 / switch_total as f64;
        for (index, switch) in switches.iter().enumerate() {
            let x = 5.0 + switch_spacing * index as f64;
            nodes.push(TopoNode {
                label: switch.name.clone().unwrap_or_else(|| "Switch".into()),
                ip: switch.ip.map(|ip| ip.to_string()).unwrap_or_default(),
                device_type: switch.device_type,
                state: switch.state,
                client_count: switch.client_count.unwrap_or(0),
                traffic_bytes_per_sec: device_traffic_bytes_per_sec(switch),
                x,
                y: 52.0,
                width: 14.0,
                height: 7.0,
            });
        }

        let ap_total = aps.len().max(1);
        let ap_spacing = 90.0 / ap_total as f64;
        for (index, ap) in aps.iter().enumerate() {
            let x = 5.0 + ap_spacing * index as f64;
            nodes.push(TopoNode {
                label: ap.name.clone().unwrap_or_else(|| "AP".into()),
                ip: ap.ip.map(|ip| ip.to_string()).unwrap_or_default(),
                device_type: ap.device_type,
                state: ap.state,
                client_count: ap.client_count.unwrap_or(0),
                traffic_bytes_per_sec: device_traffic_bytes_per_sec(ap),
                x,
                y: 24.0,
                width: 12.0,
                height: 7.0,
            });
        }

        let other_total = others.len().max(1);
        let other_spacing = 90.0 / other_total as f64;
        for (index, device) in others.iter().enumerate() {
            let x = 5.0 + index as f64 * other_spacing;
            nodes.push(TopoNode {
                label: device.name.clone().unwrap_or_else(|| "Device".into()),
                ip: device.ip.map(|ip| ip.to_string()).unwrap_or_default(),
                device_type: device.device_type,
                state: device.state,
                client_count: device.client_count.unwrap_or(0),
                traffic_bytes_per_sec: device_traffic_bytes_per_sec(device),
                x,
                y: 2.0,
                width: 12.0,
                height: 6.0,
            });
        }

        nodes
    }

    pub(super) fn apply_action(&mut self, action: &Action) {
        if let Action::DevicesUpdated(devices) = action {
            self.devices = Arc::clone(devices);
        }
    }
}

fn device_traffic_bytes_per_sec(device: &unifly_api::Device) -> u64 {
    device
        .stats
        .uplink_bandwidth
        .as_ref()
        .map_or(0, |bandwidth| {
            bandwidth
                .tx_bytes_per_sec
                .saturating_add(bandwidth.rx_bytes_per_sec)
        })
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use std::sync::Arc;

    use super::TopologyScreen;

    use serde_json::json;
    use unifly_api::{Device, DeviceType};

    use crate::tui::action::Action;

    fn test_device(id: &str, name: &str, device_type: DeviceType) -> Arc<Device> {
        let device_type_name = match device_type {
            DeviceType::Gateway => "Gateway",
            DeviceType::Switch => "Switch",
            DeviceType::AccessPoint => "AccessPoint",
            _ => "Other",
        };

        Arc::new(
            serde_json::from_value(json!({
                "id": id,
                "mac": format!("aa:bb:cc:dd:ee:{:0>2}", id.chars().last().unwrap_or('0')),
                "ip": "192.168.1.10",
                "wan_ipv6": null,
                "name": name,
                "model": "UniFi",
                "device_type": device_type_name,
                "state": "Online",
                "firmware_version": null,
                "firmware_updatable": false,
                "adopted_at": null,
                "provisioned_at": null,
                "last_seen": null,
                "serial": null,
                "supported": true,
                "ports": [],
                "radios": [],
                "uplink_device_id": null,
                "uplink_device_mac": null,
                "has_switching": device_type == DeviceType::Switch || device_type == DeviceType::Gateway,
                "has_access_point": device_type == DeviceType::AccessPoint,
                "stats": {
                    "uptime_secs": null,
                    "cpu_utilization_pct": null,
                    "memory_utilization_pct": null,
                    "load_average_1m": null,
                    "load_average_5m": null,
                    "load_average_15m": null,
                    "uplink_bandwidth": null,
                    "last_heartbeat": null,
                    "next_heartbeat": null
                },
                "client_count": 3,
                "origin": null
            }))
            .expect("device fixture should deserialize"),
        )
    }

    #[test]
    fn build_nodes_places_devices_by_topology_level() {
        let mut screen = TopologyScreen::new();
        screen.devices = Arc::new(vec![
            test_device("gateway-1", "Gateway", DeviceType::Gateway),
            test_device("switch-1", "Switch", DeviceType::Switch),
            test_device("ap-1", "AP", DeviceType::AccessPoint),
            test_device("other-1", "Camera", DeviceType::Other),
        ]);

        let nodes = screen.build_nodes();

        assert_eq!(nodes.len(), 4);
        assert_eq!(
            nodes
                .iter()
                .find(|node| node.device_type == DeviceType::Gateway)
                .expect("gateway node")
                .y,
            80.0
        );
        assert_eq!(
            nodes
                .iter()
                .find(|node| node.device_type == DeviceType::Switch)
                .expect("switch node")
                .y,
            52.0
        );
        assert_eq!(
            nodes
                .iter()
                .find(|node| node.device_type == DeviceType::AccessPoint)
                .expect("ap node")
                .y,
            24.0
        );
        assert_eq!(
            nodes
                .iter()
                .find(|node| node.device_type == DeviceType::Other)
                .expect("other node")
                .y,
            2.0
        );
    }

    #[test]
    fn devices_update_replaces_visible_topology_devices() {
        let mut screen = TopologyScreen::new();
        let devices = Arc::new(vec![
            test_device("gateway-1", "Gateway", DeviceType::Gateway),
            test_device("switch-1", "Switch", DeviceType::Switch),
        ]);

        screen.apply_action(&Action::DevicesUpdated(Arc::clone(&devices)));

        assert_eq!(screen.devices.len(), 2);
        assert_eq!(screen.devices[0].name.as_deref(), Some("Gateway"));
        assert_eq!(screen.devices[1].name.as_deref(), Some("Switch"));
    }
}
