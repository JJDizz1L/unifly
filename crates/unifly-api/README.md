# unifly-api

[![Crates.io](https://img.shields.io/crates/v/unifly-api.svg)](https://crates.io/crates/unifly-api)
[![Documentation](https://docs.rs/unifly-api/badge.svg)](https://docs.rs/unifly-api)
[![License](https://img.shields.io/crates/l/unifly-api.svg)](https://github.com/hyperb1iss/unifly/blob/main/LICENSE)

Async Rust client for UniFi controller APIs.

## Overview

`unifly-api` provides the HTTP transport layer for communicating with Ubiquiti UniFi Network controllers. It supports two distinct API surfaces:

- **Integration API**: RESTful OpenAPI-based interface authenticated via `X-API-KEY` header. Primary surface for CRUD operations on devices, clients, networks, firewall rules, and other managed entities.
- **Session API**: Session-cookie + CSRF authenticated endpoints under `/proxy/network/api/` and `/proxy/network/v2/api/`, plus `X-API-KEY` on UniFi OS session HTTP. Covers events, traffic stats, Wi-Fi observability, DPI, admin users, system info, switch port management, firewall groups, site settings, and the WebSocket event stream.

Both clients share a common `TransportConfig` for reqwest-based HTTP transport with configurable TLS verification (system CA, custom PEM, or danger-accept for self-signed controllers) and timeout settings.

## Features

- Integration API client with API key authentication
- Session API client with cookie + CSRF token handling
- WebSocket event stream with auto-reconnect
- Configurable TLS modes (system CA, custom CA bundle, danger-accept-invalid)
- Async/await with `tokio` runtime
- Comprehensive error types with context
- Support for UniFi OS and standalone controller platforms

## Quick Example

```rust
use unifly_api::{IntegrationClient, TransportConfig, TlsMode, ControllerPlatform};
use secrecy::SecretString;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure transport with TLS verification disabled (for self-signed certs)
    let transport = TransportConfig::new(TlsMode::DangerAcceptInvalid);

    // Create Integration API client
    let client = IntegrationClient::from_api_key(
        "https://192.168.1.1",
        &SecretString::from("your-api-key"),
        &transport,
        ControllerPlatform::UnifiOs,
    )?;

    // Fetch devices from the default site
    let devices = client.list_devices("default").await?;
    println!("Found {} devices", devices.len());

    Ok(())
}
```

This crate also includes the high-level `Controller` with reactive `DataStore` and `EntityStream` for automatic data merging and live subscriptions. See the [docs](https://docs.rs/unifly-api) for the full API.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](https://github.com/hyperb1iss/unifly/blob/main/LICENSE) for details.
