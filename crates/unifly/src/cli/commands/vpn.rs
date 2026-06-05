//! VPN command handlers.

use std::path::PathBuf;
use tabled::Tabled;
use unifly_api::{
    Command as CoreCommand, Controller, EntityId, HealthSummary, IpsecSa, MagicSiteToSiteVpnConfig,
    RemoteAccessVpnServer, SiteToSiteVpn, VpnClientConnection, VpnClientProfile, VpnServer,
    VpnSetting, VpnTunnel, WireGuardPeer,
};

use crate::cli::args::{
    GlobalOpts, MagicSiteToSiteVpnArgs, MagicSiteToSiteVpnCommand, OutputFormat,
    RemoteAccessVpnArgs, RemoteAccessVpnCommand, SiteToSiteVpnArgs, SiteToSiteVpnCommand, VpnArgs,
    VpnClientsArgs, VpnClientsCommand, VpnCommand, VpnConnectionsArgs, VpnConnectionsCommand,
    VpnPeersArgs, VpnPeersCommand, VpnServersArgs, VpnServersCommand, VpnSettingKey,
    VpnSettingsArgs, VpnSettingsCommand, VpnTunnelsArgs, VpnTunnelsCommand,
};
use crate::cli::error::CliError;
use crate::cli::output;

use super::util;

#[derive(Tabled)]
struct VpnServerRow {
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
struct VpnTunnelRow {
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
struct IpsecSaRow {
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

fn vpn_server_row(server: &VpnServer, painter: &output::Painter) -> VpnServerRow {
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

fn vpn_tunnel_row(tunnel: &VpnTunnel, painter: &output::Painter) -> VpnTunnelRow {
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

fn ipsec_sa_row(sa: &IpsecSa, painter: &output::Painter) -> IpsecSaRow {
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

fn server_detail(server: &VpnServer, painter: &output::Painter) -> String {
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

fn tunnel_detail(tunnel: &VpnTunnel, painter: &output::Painter) -> String {
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

fn vpn_health_detail(health: &HealthSummary, painter: &output::Painter) -> String {
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
struct VpnSettingRow {
    #[tabled(rename = "Key")]
    key: String,
    #[tabled(rename = "Enabled")]
    enabled: String,
    #[tabled(rename = "Fields")]
    fields: String,
}

fn vpn_setting_row(setting: &VpnSetting, p: &output::Painter) -> VpnSettingRow {
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

fn vpn_setting_detail(setting: &VpnSetting) -> String {
    serde_json::to_string_pretty(setting).unwrap_or_default()
}

fn vpn_setting_key_name(key: VpnSettingKey) -> &'static str {
    match key {
        VpnSettingKey::Teleport => "teleport",
        VpnSettingKey::MagicSiteToSiteVpn => "magic_site_to_site_vpn",
        VpnSettingKey::Openvpn => "openvpn",
        VpnSettingKey::PeerToPeer => "peer_to_peer",
    }
}

fn vpn_setting_patch_body(body: serde_json::Value) -> serde_json::Value {
    body.get("fields")
        .and_then(serde_json::Value::as_object)
        .map(|fields| serde_json::Value::Object(fields.clone()))
        .unwrap_or(body)
}

#[derive(Tabled)]
struct SiteToSiteVpnRow {
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

fn site_to_site_vpn_row(vpn: &SiteToSiteVpn, p: &output::Painter) -> SiteToSiteVpnRow {
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

fn site_to_site_vpn_detail(vpn: &SiteToSiteVpn) -> String {
    serde_json::to_string_pretty(vpn).unwrap_or_default()
}

#[derive(Tabled)]
struct RemoteAccessVpnServerRow {
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

fn remote_access_vpn_server_row(
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

fn remote_access_vpn_server_detail(server: &RemoteAccessVpnServer) -> String {
    serde_json::to_string_pretty(server).unwrap_or_default()
}

#[derive(Tabled)]
struct OpenVpnPortRow {
    #[tabled(rename = "Port")]
    port: String,
}

fn openvpn_port_row(port: u16, p: &output::Painter) -> OpenVpnPortRow {
    OpenVpnPortRow {
        port: p.number(&port.to_string()),
    }
}

#[derive(Tabled)]
struct VpnClientProfileRow {
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

fn vpn_client_profile_row(client: &VpnClientProfile, p: &output::Painter) -> VpnClientProfileRow {
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

fn vpn_client_profile_detail(client: &VpnClientProfile) -> String {
    serde_json::to_string_pretty(client).unwrap_or_default()
}

#[derive(Tabled)]
struct VpnClientConnectionRow {
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

fn vpn_client_connection_row(
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

fn vpn_client_connection_detail(connection: &VpnClientConnection) -> String {
    serde_json::to_string_pretty(connection).unwrap_or_default()
}

#[derive(Tabled)]
struct WireGuardPeerRow {
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

fn wireguard_peer_row(peer: &WireGuardPeer, p: &output::Painter) -> WireGuardPeerRow {
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

fn wireguard_peer_detail(peer: &WireGuardPeer) -> String {
    serde_json::to_string_pretty(peer).unwrap_or_default()
}

#[derive(Tabled)]
struct MagicSiteToSiteVpnConfigRow {
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

fn magic_site_to_site_vpn_config_row(
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

fn magic_site_to_site_vpn_config_detail(config: &MagicSiteToSiteVpnConfig) -> String {
    serde_json::to_string_pretty(config).unwrap_or_default()
}

#[derive(Tabled)]
struct WireGuardPeerSubnetRow {
    #[tabled(rename = "Subnet")]
    subnet: String,
}

fn wireguard_peer_subnet_row(subnet: &str, p: &output::Painter) -> WireGuardPeerSubnetRow {
    WireGuardPeerSubnetRow {
        subnet: p.ip(subnet),
    }
}

// ── Handler ─────────────────────────────────────────────────────────

pub async fn handle(
    controller: &Controller,
    args: VpnArgs,
    global: &GlobalOpts,
) -> Result<(), CliError> {
    let painter = output::Painter::new(global);

    match args.command {
        VpnCommand::Servers(args) => handle_servers(controller, args, global, &painter).await,
        VpnCommand::Tunnels(args) => handle_tunnels(controller, args, global, &painter).await,
        VpnCommand::Status => {
            util::ensure_session_access(controller, "vpn status").await?;
            let sas = controller.list_ipsec_sa().await?;
            if sas.is_empty() {
                if !global.quiet && matches!(global.output, OutputFormat::Table) {
                    eprintln!("No active IPsec security associations");
                }
                if matches!(global.output, OutputFormat::Table) {
                    return Ok(());
                }
            }
            let out = output::render_list(
                &global.output,
                &sas,
                |sa| ipsec_sa_row(sa, &painter),
                ipsec_sa_identity,
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        VpnCommand::Health => {
            util::ensure_session_access(controller, "vpn health").await?;
            match controller.get_vpn_health() {
                Some(health) => {
                    let out = output::render_single(
                        &global.output,
                        &health,
                        |health| vpn_health_detail(health, &painter),
                        |health| health.subsystem.clone(),
                    );
                    output::print_output(&out, global.quiet);
                    Ok(())
                }
                None => Err(CliError::NotFound {
                    resource_type: "vpn health".into(),
                    identifier: "vpn".into(),
                    list_command: "system health".into(),
                }),
            }
        }
        VpnCommand::SiteToSite(site_to_site) => {
            handle_site_to_site(controller, site_to_site, global, &painter).await
        }
        VpnCommand::RemoteAccess(remote_access) => {
            handle_remote_access(controller, remote_access, global, &painter).await
        }
        VpnCommand::Clients(clients) => handle_clients(controller, clients, global, &painter).await,
        VpnCommand::Connections(connections) => {
            handle_connections(controller, connections, global, &painter).await
        }
        VpnCommand::Peers(peers) => handle_peers(controller, peers, global, &painter).await,
        VpnCommand::MagicSiteToSite(magic_site_to_site) => {
            handle_magic_site_to_site(controller, magic_site_to_site, global, &painter).await
        }
        VpnCommand::Settings(settings) => {
            handle_settings(controller, settings, global, &painter).await
        }
    }
}

async fn handle_servers(
    controller: &Controller,
    args: VpnServersArgs,
    global: &GlobalOpts,
    painter: &output::Painter,
) -> Result<(), CliError> {
    util::ensure_integration_access(controller, "vpn servers").await?;

    match args.command {
        Some(VpnServersCommand::Get { id }) => {
            let servers = controller.list_vpn_servers().await?;
            let target_id = EntityId::from(id.clone());
            let server = servers.iter().find(|server| server.id == target_id);
            match server {
                Some(server) => {
                    let out = output::render_single(
                        &global.output,
                        server,
                        |server| server_detail(server, painter),
                        |server| server.id.to_string(),
                    );
                    output::print_output(&out, global.quiet);
                    Ok(())
                }
                None => Err(CliError::NotFound {
                    resource_type: "vpn server".into(),
                    identifier: id,
                    list_command: "vpn servers".into(),
                }),
            }
        }
        None => {
            let servers = util::apply_list_args(
                controller.list_vpn_servers().await?,
                &args.list,
                util::matches_json_filter,
            );
            let out = output::render_list(
                &global.output,
                &servers,
                |server| vpn_server_row(server, painter),
                |server| server.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
    }
}

async fn handle_tunnels(
    controller: &Controller,
    args: VpnTunnelsArgs,
    global: &GlobalOpts,
    painter: &output::Painter,
) -> Result<(), CliError> {
    util::ensure_integration_access(controller, "vpn tunnels").await?;

    match args.command {
        Some(VpnTunnelsCommand::Get { id }) => {
            let tunnels = controller.list_vpn_tunnels().await?;
            let target_id = EntityId::from(id.clone());
            let tunnel = tunnels.iter().find(|tunnel| tunnel.id == target_id);
            match tunnel {
                Some(tunnel) => {
                    let out = output::render_single(
                        &global.output,
                        tunnel,
                        |tunnel| tunnel_detail(tunnel, painter),
                        |tunnel| tunnel.id.to_string(),
                    );
                    output::print_output(&out, global.quiet);
                    Ok(())
                }
                None => Err(CliError::NotFound {
                    resource_type: "vpn tunnel".into(),
                    identifier: id,
                    list_command: "vpn tunnels".into(),
                }),
            }
        }
        None => {
            let tunnels = util::apply_list_args(
                controller.list_vpn_tunnels().await?,
                &args.list,
                util::matches_json_filter,
            );
            let out = output::render_list(
                &global.output,
                &tunnels,
                |tunnel| vpn_tunnel_row(tunnel, painter),
                |tunnel| tunnel.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
    }
}

async fn handle_site_to_site(
    controller: &Controller,
    args: SiteToSiteVpnArgs,
    global: &GlobalOpts,
    painter: &output::Painter,
) -> Result<(), CliError> {
    util::ensure_session_access(controller, "site-to-site vpn").await?;

    match args.command {
        SiteToSiteVpnCommand::List(list) => {
            let vpns = util::apply_list_args(
                controller.list_site_to_site_vpns().await?,
                &list,
                util::matches_json_filter,
            );
            let out = output::render_list(
                &global.output,
                &vpns,
                |vpn| site_to_site_vpn_row(vpn, painter),
                |vpn| vpn.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        SiteToSiteVpnCommand::Get { id } => {
            let vpn = controller.get_site_to_site_vpn(&id).await?;
            let out = output::render_single(&global.output, &vpn, site_to_site_vpn_detail, |vpn| {
                vpn.id.to_string()
            });
            output::print_output(&out, global.quiet);
            Ok(())
        }
        SiteToSiteVpnCommand::Create { from_file } => {
            let req = serde_json::from_value(util::read_json_file(&from_file)?)?;
            controller
                .execute(CoreCommand::CreateSiteToSiteVpn(req))
                .await?;
            if !global.quiet {
                eprintln!("Site-to-site VPN created");
            }
            Ok(())
        }
        SiteToSiteVpnCommand::Update { id, from_file } => {
            let req = serde_json::from_value(util::read_json_file(&from_file)?)?;
            controller
                .execute(CoreCommand::UpdateSiteToSiteVpn {
                    id: EntityId::Legacy(id),
                    update: req,
                })
                .await?;
            if !global.quiet {
                eprintln!("Site-to-site VPN updated");
            }
            Ok(())
        }
        SiteToSiteVpnCommand::Delete { id } => {
            controller
                .execute(CoreCommand::DeleteSiteToSiteVpn {
                    id: EntityId::Legacy(id),
                })
                .await?;
            if !global.quiet {
                eprintln!("Site-to-site VPN deleted");
            }
            Ok(())
        }
    }
}

async fn handle_remote_access(
    controller: &Controller,
    args: RemoteAccessVpnArgs,
    global: &GlobalOpts,
    painter: &output::Painter,
) -> Result<(), CliError> {
    util::ensure_session_access(controller, "remote-access vpn").await?;

    match args.command {
        RemoteAccessVpnCommand::List(list) => {
            let servers = util::apply_list_args(
                controller.list_remote_access_vpn_servers().await?,
                &list,
                util::matches_json_filter,
            );
            let out = output::render_list(
                &global.output,
                &servers,
                |server| remote_access_vpn_server_row(server, painter),
                |server| server.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        RemoteAccessVpnCommand::Get { id } => {
            let server = controller.get_remote_access_vpn_server(&id).await?;
            let out = output::render_single(
                &global.output,
                &server,
                remote_access_vpn_server_detail,
                |server| server.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        RemoteAccessVpnCommand::Create { from_file } => {
            let req = serde_json::from_value(util::read_json_file(&from_file)?)?;
            controller
                .execute(CoreCommand::CreateRemoteAccessVpnServer(req))
                .await?;
            if !global.quiet {
                eprintln!("Remote-access VPN server created");
            }
            Ok(())
        }
        RemoteAccessVpnCommand::Update { id, from_file } => {
            let req = serde_json::from_value(util::read_json_file(&from_file)?)?;
            controller
                .execute(CoreCommand::UpdateRemoteAccessVpnServer {
                    id: EntityId::Legacy(id),
                    update: req,
                })
                .await?;
            if !global.quiet {
                eprintln!("Remote-access VPN server updated");
            }
            Ok(())
        }
        RemoteAccessVpnCommand::SuggestPort => {
            let ports = controller.list_openvpn_port_suggestions().await?;
            let out = output::render_list(
                &global.output,
                &ports,
                |port| openvpn_port_row(*port, painter),
                ToString::to_string,
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        RemoteAccessVpnCommand::DownloadConfig { id, path } => {
            let bytes = controller.download_openvpn_configuration(&id).await?;
            let default_name = format!("{id}.ovpn");
            let mut target = path.unwrap_or_else(|| PathBuf::from(&default_name));
            if target.is_dir() {
                target = target.join(&default_name);
            }
            std::fs::write(&target, bytes)?;
            if !global.quiet {
                eprintln!("OpenVPN configuration downloaded to {}", target.display());
            }
            Ok(())
        }
        RemoteAccessVpnCommand::Delete { id } => {
            controller
                .execute(CoreCommand::DeleteRemoteAccessVpnServer {
                    id: EntityId::Legacy(id),
                })
                .await?;
            if !global.quiet {
                eprintln!("Remote-access VPN server deleted");
            }
            Ok(())
        }
    }
}

async fn handle_settings(
    controller: &Controller,
    args: VpnSettingsArgs,
    global: &GlobalOpts,
    painter: &output::Painter,
) -> Result<(), CliError> {
    util::ensure_session_access(controller, "vpn settings").await?;

    match args.command {
        VpnSettingsCommand::List(list) => {
            let settings = util::apply_list_args(
                controller.list_vpn_settings().await?,
                &list,
                util::matches_json_filter,
            );
            let out = output::render_list(
                &global.output,
                &settings,
                |setting| vpn_setting_row(setting, painter),
                |setting| setting.key.clone(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        VpnSettingsCommand::Get { key } => {
            let setting = controller
                .get_vpn_setting(vpn_setting_key_name(key))
                .await?;
            let out =
                output::render_single(&global.output, &setting, vpn_setting_detail, |setting| {
                    setting.key.clone()
                });
            output::print_output(&out, global.quiet);
            Ok(())
        }
        VpnSettingsCommand::Set { key, enabled } => {
            controller
                .update_vpn_setting(
                    vpn_setting_key_name(key),
                    &serde_json::json!({ "enabled": enabled }),
                )
                .await?;
            if !global.quiet {
                eprintln!("VPN setting updated");
            }
            Ok(())
        }
        VpnSettingsCommand::Patch { key, from_file } => {
            let body = vpn_setting_patch_body(util::read_json_file(&from_file)?);
            controller
                .update_vpn_setting(vpn_setting_key_name(key), &body)
                .await?;
            if !global.quiet {
                eprintln!("VPN setting patched");
            }
            Ok(())
        }
    }
}

async fn handle_clients(
    controller: &Controller,
    args: VpnClientsArgs,
    global: &GlobalOpts,
    painter: &output::Painter,
) -> Result<(), CliError> {
    util::ensure_session_access(controller, "vpn clients").await?;

    match args.command {
        VpnClientsCommand::List(list) => {
            let clients = util::apply_list_args(
                controller.list_vpn_client_profiles().await?,
                &list,
                util::matches_json_filter,
            );
            let out = output::render_list(
                &global.output,
                &clients,
                |client| vpn_client_profile_row(client, painter),
                |client| client.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        VpnClientsCommand::Get { id } => {
            let client = controller.get_vpn_client_profile(&id).await?;
            let out = output::render_single(
                &global.output,
                &client,
                vpn_client_profile_detail,
                |client| client.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        VpnClientsCommand::Create { from_file } => {
            let req = serde_json::from_value(util::read_json_file(&from_file)?)?;
            controller
                .execute(CoreCommand::CreateVpnClientProfile(req))
                .await?;
            if !global.quiet {
                eprintln!("VPN client created");
            }
            Ok(())
        }
        VpnClientsCommand::Update { id, from_file } => {
            let req = serde_json::from_value(util::read_json_file(&from_file)?)?;
            controller
                .execute(CoreCommand::UpdateVpnClientProfile {
                    id: EntityId::Legacy(id),
                    update: req,
                })
                .await?;
            if !global.quiet {
                eprintln!("VPN client updated");
            }
            Ok(())
        }
        VpnClientsCommand::Delete { id } => {
            controller
                .execute(CoreCommand::DeleteVpnClientProfile {
                    id: EntityId::Legacy(id),
                })
                .await?;
            if !global.quiet {
                eprintln!("VPN client deleted");
            }
            Ok(())
        }
    }
}

async fn handle_connections(
    controller: &Controller,
    args: VpnConnectionsArgs,
    global: &GlobalOpts,
    painter: &output::Painter,
) -> Result<(), CliError> {
    util::ensure_session_access(controller, "vpn connections").await?;

    match args.command {
        VpnConnectionsCommand::List(list) => {
            let connections = util::apply_list_args(
                controller.list_vpn_client_connections().await?,
                &list,
                util::matches_json_filter,
            );
            let out = output::render_list(
                &global.output,
                &connections,
                |connection| vpn_client_connection_row(connection, painter),
                |connection| connection.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        VpnConnectionsCommand::Get { id } => {
            let connection = controller.get_vpn_client_connection(&id).await?;
            let out = output::render_single(
                &global.output,
                &connection,
                vpn_client_connection_detail,
                |connection| connection.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        VpnConnectionsCommand::Restart { id } => {
            controller
                .execute(CoreCommand::RestartVpnClientConnection {
                    id: EntityId::Legacy(id),
                })
                .await?;
            if !global.quiet {
                eprintln!("VPN client connection restarted");
            }
            Ok(())
        }
    }
}

async fn handle_peers(
    controller: &Controller,
    args: VpnPeersArgs,
    global: &GlobalOpts,
    painter: &output::Painter,
) -> Result<(), CliError> {
    util::ensure_session_access(controller, "vpn peers").await?;

    match args.command {
        VpnPeersCommand::List { server_id, list } => {
            let peers = util::apply_list_args(
                controller
                    .list_wireguard_peers(server_id.as_deref())
                    .await?,
                &list,
                util::matches_json_filter,
            );
            let out = output::render_list(
                &global.output,
                &peers,
                |peer| wireguard_peer_row(peer, painter),
                |peer| peer.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        VpnPeersCommand::Get { server_id, id } => {
            let peer = controller.get_wireguard_peer(&server_id, &id).await?;
            let out = output::render_single(&global.output, &peer, wireguard_peer_detail, |peer| {
                peer.id.to_string()
            });
            output::print_output(&out, global.quiet);
            Ok(())
        }
        VpnPeersCommand::Create {
            server_id,
            from_file,
        } => {
            let req = serde_json::from_value(util::read_json_file(&from_file)?)?;
            controller
                .execute(CoreCommand::CreateWireGuardPeer {
                    server_id: EntityId::Legacy(server_id),
                    peer: req,
                })
                .await?;
            if !global.quiet {
                eprintln!("WireGuard peer created");
            }
            Ok(())
        }
        VpnPeersCommand::Update {
            server_id,
            id,
            from_file,
        } => {
            let req = serde_json::from_value(util::read_json_file(&from_file)?)?;
            controller
                .execute(CoreCommand::UpdateWireGuardPeer {
                    server_id: EntityId::Legacy(server_id),
                    peer_id: EntityId::Legacy(id),
                    update: req,
                })
                .await?;
            if !global.quiet {
                eprintln!("WireGuard peer updated");
            }
            Ok(())
        }
        VpnPeersCommand::Delete { server_id, id } => {
            controller
                .execute(CoreCommand::DeleteWireGuardPeer {
                    server_id: EntityId::Legacy(server_id),
                    peer_id: EntityId::Legacy(id),
                })
                .await?;
            if !global.quiet {
                eprintln!("WireGuard peer deleted");
            }
            Ok(())
        }
        VpnPeersCommand::Subnets => {
            let subnets = controller.list_wireguard_peer_existing_subnets().await?;
            let out = output::render_list(
                &global.output,
                &subnets,
                |subnet| wireguard_peer_subnet_row(subnet, painter),
                Clone::clone,
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
    }
}

async fn handle_magic_site_to_site(
    controller: &Controller,
    args: MagicSiteToSiteVpnArgs,
    global: &GlobalOpts,
    painter: &output::Painter,
) -> Result<(), CliError> {
    util::ensure_session_access(controller, "magic site-to-site vpn").await?;

    match args.command {
        MagicSiteToSiteVpnCommand::List(list) => {
            let configs = util::apply_list_args(
                controller.list_magic_site_to_site_vpn_configs().await?,
                &list,
                util::matches_json_filter,
            );
            let out = output::render_list(
                &global.output,
                &configs,
                |config| magic_site_to_site_vpn_config_row(config, painter),
                |config| config.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
        MagicSiteToSiteVpnCommand::Get { id } => {
            let config = controller.get_magic_site_to_site_vpn_config(&id).await?;
            let out = output::render_single(
                &global.output,
                &config,
                magic_site_to_site_vpn_config_detail,
                |config| config.id.to_string(),
            );
            output::print_output(&out, global.quiet);
            Ok(())
        }
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

fn ipsec_sa_identity(sa: &IpsecSa) -> String {
    sa.name
        .clone()
        .or_else(|| sa.remote_ip.clone())
        .or_else(|| sa.local_ip.clone())
        .unwrap_or_default()
}
