pub mod action;
pub mod app;
pub mod component;
pub mod data_bridge;
pub mod effects;
pub mod event;
pub(crate) mod forms;
#[cfg(feature = "tui-graphics")]
pub mod graphics;
pub mod render_caps;
pub mod screen;
pub mod screens;
pub mod terminal;
#[allow(dead_code)]
pub mod theme;
pub mod widgets;

use std::sync::Arc;

use color_eyre::eyre::Result;
use secrecy::SecretString;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use unifly_api::{AuthCredentials, Controller, ControllerConfig, TlsVerification};

use crate::cli::args::{GlobalOpts, TuiArgs};
use crate::config;
use crate::sanitizer::Sanitizer;

/// Launch the real-time terminal dashboard.
///
/// Sets up file-based tracing, installs panic hooks, initializes the theme,
/// builds a controller (with graceful fallback), and runs the TUI app loop.
#[allow(clippy::future_not_send)]
pub async fn launch(global: &GlobalOpts, args: TuiArgs) -> Result<()> {
    terminal::install_hooks()?;

    let _log_guard = setup_tracing(global.verbose, &args.log_file);

    let loaded_config = config::load_config().ok();
    let theme_name = global.theme.as_deref().or_else(|| {
        loaded_config
            .as_ref()
            .and_then(|config| config.defaults.theme.as_deref())
    });
    theme::initialize(theme_name);
    render_caps::initialize(
        loaded_config
            .as_ref()
            .and_then(|config| config.defaults.chart_quality.as_deref()),
    );

    info!(
        url = global.controller.as_deref().unwrap_or("(not set)"),
        site = global.site.as_deref().unwrap_or("default"),
        "starting unifly tui"
    );

    let controller = build_controller_direct(global)
        .or_else(|| build_controller_from_config(global.profile.as_deref()));

    let sanitizer = resolve_sanitizer(global);
    let effects_enabled = resolve_effects_enabled(global);

    let mut app = app::App::new(controller, sanitizer, effects_enabled);
    app.run().await?;

    Ok(())
}

/// Resolve whether TUI effects should run this session.
///
/// Resolution order (first "off" wins):
///   `--no-effects` flag → `NO_EFFECTS` env var → `[defaults].effects` config → default on.
fn resolve_effects_enabled(global: &GlobalOpts) -> bool {
    if global.no_effects {
        return false;
    }
    if std::env::var_os("NO_EFFECTS").is_some() {
        return false;
    }
    config::load_config().map_or(true, |c| c.defaults.effects)
}

fn setup_tracing(verbosity: u8, log_file: &std::path::Path) -> WorkerGuard {
    let log_level = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("unifly={log_level},unifly_api={log_level}")));

    let log_dir = log_file.parent().unwrap_or(std::path::Path::new("/tmp"));
    let log_filename = log_file
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("unifly-tui.log"));

    let file_appender = tracing_appender::rolling::never(log_dir, log_filename);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true),
        )
        .init();

    guard
}

fn build_controller_direct(global: &GlobalOpts) -> Option<Controller> {
    let is_cloud = global.host_id.is_some();
    let url_str = global.controller.as_deref().or({
        if is_cloud {
            Some(crate::config::DEFAULT_CLOUD_CONTROLLER_URL)
        } else {
            None
        }
    })?;
    let url = url_str.parse().expect("invalid controller URL");

    let api_key = SecretString::from(global.api_key.as_ref()?.clone());

    let auth = if let Some(ref host_id) = global.host_id {
        AuthCredentials::Cloud {
            api_key,
            host_id: host_id.clone(),
        }
    } else {
        try_hybrid_from_config(&api_key).unwrap_or(AuthCredentials::ApiKey(api_key))
    };

    let tls = if is_cloud {
        TlsVerification::SystemDefaults
    } else if global.insecure {
        TlsVerification::DangerAcceptInvalid
    } else {
        TlsVerification::SystemDefaults
    };

    let site = global.site.clone().unwrap_or_else(|| "default".into());

    let totp_token = global
        .totp
        .as_ref()
        .map(|t| secrecy::SecretString::from(t.clone()));

    let controller_config = ControllerConfig {
        url,
        auth,
        site,
        tls,
        timeout: std::time::Duration::from_secs(global.timeout),
        refresh_interval_secs: if is_cloud { 60 } else { 10 },
        websocket_enabled: !is_cloud,
        polling_interval_secs: if is_cloud { 30 } else { 10 },
        totp_token,
        profile_name: global.profile.clone(),
        no_session_cache: global.no_cache || is_cloud,
    };

    Some(Controller::new(controller_config))
}

fn try_hybrid_from_config(api_key: &SecretString) -> Option<AuthCredentials> {
    let cfg = config::load_config().ok()?;
    let name = cfg.default_profile.as_deref().unwrap_or("default");
    let profile = cfg.profiles.get(name)?;

    if profile.auth_mode != "hybrid" {
        return None;
    }

    let (username, password) = config::resolve_session_credentials(profile, name).ok()?;

    Some(AuthCredentials::Hybrid {
        api_key: api_key.clone(),
        username,
        password,
    })
}

fn build_controller_from_config(profile_name: Option<&str>) -> Option<Controller> {
    let cfg = match config::load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::warn!("failed to load config: {e}");
            return None;
        }
    };

    let profile_name = profile_name
        .or(cfg.default_profile.as_deref())
        .unwrap_or("default");

    let Some(profile) = cfg.profiles.get(profile_name) else {
        tracing::warn!(
            "profile '{profile_name}' not found in config (available: {:?})",
            cfg.profiles.keys().collect::<Vec<_>>()
        );
        return None;
    };

    match config::profile_to_controller_config(profile, profile_name) {
        Ok(controller_config) => Some(Controller::new(controller_config)),
        Err(e) => {
            tracing::warn!("failed to build controller from profile '{profile_name}': {e}");
            None
        }
    }
}

fn resolve_sanitizer(global: &GlobalOpts) -> Option<Arc<Sanitizer>> {
    let mut demo_config = config::load_config().map(|c| c.demo).unwrap_or_default();

    if global.demo {
        demo_config.enabled = true;
    }

    if demo_config.enabled {
        info!("demo mode active — PII will be sanitized");
        Some(Arc::new(Sanitizer::new(&demo_config)))
    } else {
        None
    }
}
