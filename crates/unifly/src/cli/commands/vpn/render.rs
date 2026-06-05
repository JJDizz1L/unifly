//! VPN table rows and detail renderers.

use tabled::Tabled;
use unifly_api::{
    HealthSummary, IpsecSa, MagicSiteToSiteVpnConfig, RemoteAccessVpnServer, SiteToSiteVpn,
    VpnClientConnection, VpnClientProfile, VpnServer, VpnSetting, VpnTunnel, WireGuardPeer,
};

use crate::cli::args::VpnSettingKey;
use crate::cli::output;

#[derive(Tabled)]
pub(super) struct VpnServerRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    server_type: String,
    #[tabled(rename = "Subnet")]
    subnet: String,
    #[tabled(rename = "Port")]
    port: String,
    #[tabled(rename = "Protocol")]
    protocol: String,
    #[tabled(rename = "Enabled")]
    enabled: String,
}

#[derive(Tabled)]
pub(super) struct VpnTunnelRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    tunnel_type: String,
    #[tabled(rename = "Peer")]
    peer: String,
    #[tabled(rename = "IKE")]
    ike: String,
    #[tabled(rename = "Enabled")]
    enabled: String,
}

#[derive(Tabled)]
pub(super) struct IpsecSaRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Remote IP")]
    remote_ip: String,
    #[tabled(rename = "State")]
    state: String,
    #[tabled(rename = "TX Bytes")]
    tx: String,
    #[tabled(rename = "RX Bytes")]
    rx: String,
    #[tabled(rename = "Uptime")]
    uptime: String,
    #[tabled(rename = "IKE")]
    ike: String,
}

pub(super) fn vpn_server_row(server: &VpnServer, painter: &output::Painter) -> VpnServerRow {
    VpnServerRow {
        id: painter.id(&server.id.to_string()),
        name: painter.name(server.name.as_deref().unwrap_or("-")),
        server_type: painter.muted(&server.server_type),
        subnet: painter.ip(server.subnet.as_deref().unwrap_or("-")),
        port: painter.number(&display_optional(server.port)),
        protocol: painter.muted(server.protocol.as_deref().unwrap_or("-")),
        enabled: server
            .enabled
            .map_or_else(|| painter.muted("-"), |enabled| painter.enabled(enabled)),
    }
}

pub(super) fn vpn_tunnel_row(tunnel: &VpnTunnel, painter: &output::Painter) -> VpnTunnelRow {
    VpnTunnelRow {
        id: painter.id(&tunnel.id.to_string()),
        name: painter.name(tunnel.name.as_deref().unwrap_or("-")),
        tunnel_type: painter.muted(&tunnel.tunnel_type),
        peer: painter.ip(tunnel.peer_address.as_deref().unwrap_or("-")),
        ike: painter.muted(tunnel.ike_version.as_deref().unwrap_or("-")),
        enabled: tunnel
            .enabled
            .map_or_else(|| painter.muted("-"), |enabled| painter.enabled(enabled)),
    }
}

pub(super) fn ipsec_sa_row(sa: &IpsecSa, painter: &output::Painter) -> IpsecSaRow {
    let state = sa.state.as_deref().unwrap_or("-");
    IpsecSaRow {
        name: painter.name(sa.name.as_deref().unwrap_or("-")),
        remote_ip: painter.ip(sa.remote_ip.as_deref().unwrap_or("-")),
        state: painter.state(state),
        tx: painter.number(&display_optional(sa.tx_bytes)),
        rx: painter.number(&display_optional(sa.rx_bytes)),
        uptime: painter.muted(&display_optional(
            sa.uptime.map(|value| format!("{value}s")),
        )),
        ike: painter.muted(sa.ike_version.as_deref().unwrap_or("-")),
    }
}

pub(super) fn server_detail(server: &VpnServer, painter: &output::Painter) -> String {
    let mut lines = vec![
        format!("ID:                {}", painter.id(&server.id.to_string())),
        format!(
            "Name:              {}",
            painter.name(server.name.as_deref().unwrap_or("-"))
        ),
        format!("Type:              {}", painter.muted(&server.server_type)),
        format!(
            "Enabled:           {}",
            server
                .enabled
                .map_or_else(|| painter.muted("-"), |enabled| painter.enabled(enabled))
        ),
        format!(
            "Subnet:            {}",
            painter.ip(server.subnet.as_deref().unwrap_or("-"))
        ),
        format!(
            "Port:              {}",
            painter.number(&display_optional(server.port))
        ),
        format!(
            "WAN IP:            {}",
            painter.ip(server.wan_ip.as_deref().unwrap_or("-"))
        ),
        format!(
            "Connected Clients: {}",
            painter.number(&display_optional(server.connected_clients))
        ),
        format!(
            "Protocol:          {}",
            painter.muted(server.protocol.as_deref().unwrap_or("-"))
        ),
    ];
    append_extra(&mut lines, &server.extra);
    lines.join("\n")
}

pub(super) fn tunnel_detail(tunnel: &VpnTunnel, painter: &output::Painter) -> String {
    let mut lines = vec![
        format!("ID:             {}", painter.id(&tunnel.id.to_string())),
        format!(
            "Name:           {}",
            painter.name(tunnel.name.as_deref().unwrap_or("-"))
        ),
        format!("Type:           {}", painter.muted(&tunnel.tunnel_type)),
        format!(
            "Enabled:        {}",
            tunnel
                .enabled
                .map_or_else(|| painter.muted("-"), |enabled| painter.enabled(enabled))
        ),
        format!(
            "Peer Address:   {}",
            painter.ip(tunnel.peer_address.as_deref().unwrap_or("-"))
        ),
        format!(
            "Local Subnets:  {}",
            painter.ip(&display_list(&tunnel.local_subnets))
        ),
        format!(
            "Remote Subnets: {}",
            painter.ip(&display_list(&tunnel.remote_subnets))
        ),
        format!(
            "Has PSK:        {}",
            if tunnel.has_psk {
                painter.success("yes")
            } else {
                painter.error("no")
            }
        ),
        format!(
            "IKE Version:    {}",
            painter.muted(tunnel.ike_version.as_deref().unwrap_or("-"))
        ),
    ];
    append_extra(&mut lines, &tunnel.extra);
    lines.join("\n")
}

pub(super) fn vpn_health_detail(health: &HealthSummary, painter: &output::Painter) -> String {
    let mut lines = vec![
        format!("Subsystem: {}", painter.name(&health.subsystem)),
        format!("Status:    {}", painter.health(&health.status)),
        format!(
            "Devices:   {}",
            painter.number(&display_optional(health.num_adopted))
        ),
        format!(
            "Clients:   {}",
            painter.number(&display_optional(health.num_sta))
        ),
        format!(
            "TX/s:      {}",
            painter.number(&display_optional(health.tx_bytes_r))
        ),
        format!(
            "RX/s:      {}",
            painter.number(&display_optional(health.rx_bytes_r))
        ),
        format!(
            "Latency:   {}",
            health.latency.map_or_else(
                || painter.muted("-"),
                |latency| painter.number(&format!("{latency:.1}"))
            )
        ),
        format!(
            "WAN IP:    {}",
            painter.ip(health.wan_ip.as_deref().unwrap_or("-"))
        ),
        format!(
            "Gateways:  {}",
            painter.ip(&health
                .gateways
                .as_ref()
                .map_or_else(|| "-".into(), |gateways| gateways.join(", ")))
        ),
    ];
    if !health.extra.is_null() {
        lines.push(String::new());
        lines.push("Raw:".into());
        lines.push(serde_json::to_string_pretty(&health.extra).unwrap_or_else(|_| "{}".into()));
    }
    lines.join("\n")
}

// ── Session API row structs and helpers ─────────────────────────────

#[derive(Tabled)]
pub(super) struct VpnSettingRow {
    #[tabled(rename = "Key")]
    key: String,
    #[tabled(rename = "Enabled")]
    enabled: String,
    #[tabled(rename = "Fields")]
    fields: String,
}

pub(super) fn vpn_setting_row(setting: &VpnSetting, p: &output::Painter) -> VpnSettingRow {
    let mut field_names = setting
        .fields
        .keys()
        .filter(|key| key.as_str() != "enabled")
        .cloned()
        .collect::<Vec<_>>();
    field_names.sort();

    VpnSettingRow {
        key: p.name(&setting.key),
        enabled: setting
            .enabled
            .map_or_else(|| p.muted("-"), |enabled| p.enabled(enabled)),
        fields: p.muted(&field_names.join(", ")),
    }
}

pub(super) fn vpn_setting_detail(setting: &VpnSetting) -> String {
    serde_json::to_string_pretty(setting).unwrap_or_default()
}

pub(super) fn vpn_setting_key_name(key: VpnSettingKey) -> &'static str {
    match key {
        VpnSettingKey::Teleport => "teleport",
        VpnSettingKey::MagicSiteToSiteVpn => "magic_site_to_site_vpn",
        VpnSettingKey::Openvpn => "openvpn",
        VpnSettingKey::PeerToPeer => "peer_to_peer",
    }
}

pub(super) fn vpn_setting_patch_body(body: serde_json::Value) -> serde_json::Value {
    body.get("fields")
        .and_then(serde_json::Value::as_object)
        .map(|fields| serde_json::Value::Object(fields.clone()))
        .unwrap_or(body)
}

#[derive(Tabled)]
pub(super) struct SiteToSiteVpnRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    vpn_type: String,
    #[tabled(rename = "Remote")]
    remote_host: String,
    #[tabled(rename = "Subnets")]
    remote_vpn_subnets: String,
    #[tabled(rename = "Enabled")]
    enabled: String,
}

pub(super) fn site_to_site_vpn_row(vpn: &SiteToSiteVpn, p: &output::Painter) -> SiteToSiteVpnRow {
    SiteToSiteVpnRow {
        id: p.id(&vpn.id.to_string()),
        name: p.name(&vpn.name),
        vpn_type: p.muted(&vpn.vpn_type),
        remote_host: vpn
            .remote_host
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.muted(value)),
        remote_vpn_subnets: if vpn.remote_vpn_subnets.is_empty() {
            p.muted("-")
        } else {
            p.muted(&vpn.remote_vpn_subnets.join(", "))
        },
        enabled: p.enabled(vpn.enabled),
    }
}

pub(super) fn site_to_site_vpn_detail(vpn: &SiteToSiteVpn) -> String {
    serde_json::to_string_pretty(vpn).unwrap_or_default()
}

#[derive(Tabled)]
pub(super) struct RemoteAccessVpnServerRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    vpn_type: String,
    #[tabled(rename = "Interface")]
    interface: String,
    #[tabled(rename = "WAN IP")]
    local_wan_ip: String,
    #[tabled(rename = "Port")]
    local_port: String,
    #[tabled(rename = "Enabled")]
    enabled: String,
}

pub(super) fn remote_access_vpn_server_row(
    server: &RemoteAccessVpnServer,
    p: &output::Painter,
) -> RemoteAccessVpnServerRow {
    RemoteAccessVpnServerRow {
        id: p.id(&server.id.to_string()),
        name: p.name(&server.name),
        vpn_type: p.muted(&server.vpn_type),
        interface: server
            .interface
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.muted(value)),
        local_wan_ip: server
            .local_wan_ip
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.muted(value)),
        local_port: server
            .local_port
            .map_or_else(|| p.muted("-"), |value| p.muted(&value.to_string())),
        enabled: p.enabled(server.enabled),
    }
}

pub(super) fn remote_access_vpn_server_detail(server: &RemoteAccessVpnServer) -> String {
    serde_json::to_string_pretty(server).unwrap_or_default()
}

#[derive(Tabled)]
pub(super) struct OpenVpnPortRow {
    #[tabled(rename = "Port")]
    port: String,
}

pub(super) fn openvpn_port_row(port: u16, p: &output::Painter) -> OpenVpnPortRow {
    OpenVpnPortRow {
        port: p.number(&port.to_string()),
    }
}

#[derive(Tabled)]
pub(super) struct VpnClientProfileRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    vpn_type: String,
    #[tabled(rename = "Server")]
    server_address: String,
    #[tabled(rename = "Port")]
    server_port: String,
    #[tabled(rename = "Local")]
    local_address: String,
    #[tabled(rename = "Enabled")]
    enabled: String,
}

pub(super) fn vpn_client_profile_row(
    client: &VpnClientProfile,
    p: &output::Painter,
) -> VpnClientProfileRow {
    VpnClientProfileRow {
        id: p.id(&client.id.to_string()),
        name: p.name(&client.name),
        vpn_type: p.muted(&client.vpn_type),
        server_address: client
            .server_address
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.ip(value)),
        server_port: client
            .server_port
            .map_or_else(|| p.muted("-"), |value| p.muted(&value.to_string())),
        local_address: client
            .local_address
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.ip(value)),
        enabled: p.enabled(client.enabled),
    }
}

pub(super) fn vpn_client_profile_detail(client: &VpnClientProfile) -> String {
    serde_json::to_string_pretty(client).unwrap_or_default()
}

#[derive(Tabled)]
pub(super) struct VpnClientConnectionRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    connection_type: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Local")]
    local_address: String,
    #[tabled(rename = "Remote")]
    remote_address: String,
}

pub(super) fn vpn_client_connection_row(
    connection: &VpnClientConnection,
    p: &output::Painter,
) -> VpnClientConnectionRow {
    VpnClientConnectionRow {
        id: p.id(&connection.id.to_string()),
        name: connection
            .name
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.name(value)),
        connection_type: connection
            .connection_type
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.muted(value)),
        status: connection
            .status
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.state(value)),
        local_address: connection
            .local_address
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.ip(value)),
        remote_address: connection
            .remote_address
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.ip(value)),
    }
}

pub(super) fn vpn_client_connection_detail(connection: &VpnClientConnection) -> String {
    serde_json::to_string_pretty(connection).unwrap_or_default()
}

#[derive(Tabled)]
pub(super) struct WireGuardPeerRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Server")]
    server_id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "IPv4")]
    interface_ip: String,
    #[tabled(rename = "IPv6")]
    interface_ipv6: String,
    #[tabled(rename = "Allowed IPs")]
    allowed_ips: String,
    #[tabled(rename = "PSK")]
    has_preshared_key: String,
}

pub(super) fn wireguard_peer_row(peer: &WireGuardPeer, p: &output::Painter) -> WireGuardPeerRow {
    WireGuardPeerRow {
        id: p.id(&peer.id.to_string()),
        server_id: peer
            .server_id
            .as_ref()
            .map_or_else(|| p.muted("-"), |value| p.id(&value.to_string())),
        name: p.name(&peer.name),
        interface_ip: peer
            .interface_ip
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.ip(value)),
        interface_ipv6: peer
            .interface_ipv6
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.ip(value)),
        allowed_ips: if peer.allowed_ips.is_empty() {
            p.muted("-")
        } else {
            p.muted(&peer.allowed_ips.join(", "))
        },
        has_preshared_key: p.enabled(peer.has_preshared_key),
    }
}

pub(super) fn wireguard_peer_detail(peer: &WireGuardPeer) -> String {
    serde_json::to_string_pretty(peer).unwrap_or_default()
}

#[derive(Tabled)]
pub(super) struct MagicSiteToSiteVpnConfigRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Enabled")]
    enabled: String,
    #[tabled(rename = "Local Site")]
    local_site_name: String,
    #[tabled(rename = "Remote Site")]
    remote_site_name: String,
}

pub(super) fn magic_site_to_site_vpn_config_row(
    config: &MagicSiteToSiteVpnConfig,
    p: &output::Painter,
) -> MagicSiteToSiteVpnConfigRow {
    let name = config.name.clone().or_else(|| {
        match (
            config.local_site_name.as_deref(),
            config.remote_site_name.as_deref(),
        ) {
            (Some(local), Some(remote)) => Some(format!("{local} <-> {remote}")),
            _ => None,
        }
    });

    MagicSiteToSiteVpnConfigRow {
        id: p.id(&config.id.to_string()),
        name: name
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.name(value)),
        status: config
            .status
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.state(value)),
        enabled: config
            .enabled
            .map_or_else(|| p.muted("-"), |value| p.enabled(value)),
        local_site_name: config
            .local_site_name
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.name(value)),
        remote_site_name: config
            .remote_site_name
            .as_deref()
            .map_or_else(|| p.muted("-"), |value| p.name(value)),
    }
}

pub(super) fn magic_site_to_site_vpn_config_detail(config: &MagicSiteToSiteVpnConfig) -> String {
    serde_json::to_string_pretty(config).unwrap_or_default()
}

#[derive(Tabled)]
pub(super) struct WireGuardPeerSubnetRow {
    #[tabled(rename = "Subnet")]
    subnet: String,
}

pub(super) fn wireguard_peer_subnet_row(
    subnet: &str,
    p: &output::Painter,
) -> WireGuardPeerSubnetRow {
    WireGuardPeerSubnetRow {
        subnet: p.ip(subnet),
    }
}

fn display_optional<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(|| "-".into(), |value| value.to_string())
}

fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "-".into()
    } else {
        values.join(", ")
    }
}

fn append_extra(lines: &mut Vec<String>, extra: &serde_json::Map<String, serde_json::Value>) {
    if extra.is_empty() {
        return;
    }

    lines.push(String::new());
    lines.push("Raw:".into());
    lines.push(
        serde_json::to_string_pretty(&serde_json::Value::Object(extra.clone()))
            .unwrap_or_else(|_| "{}".into()),
    );
}

pub(super) fn ipsec_sa_identity(sa: &IpsecSa) -> String {
    sa.name
        .clone()
        .or_else(|| sa.remote_ip.clone())
        .or_else(|| sa.local_ip.clone())
        .unwrap_or_default()
}
