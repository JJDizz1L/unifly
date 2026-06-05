use crate::core_error::CoreError;
use crate::model::{Admin, Alarm, EntityId, HealthSummary, SysInfo, SystemInfo, VpnSetting};
use crate::session::models::{ChannelAvailability, RogueAp};

use super::Controller;
use super::support::{convert_health_summaries, require_session};

mod vpn;

const VPN_SETTING_KEYS: &[&str] = &[
    "teleport",
    "magic_site_to_site_vpn",
    "openvpn",
    "peer_to_peer",
];

impl Controller {
    pub async fn list_backups(&self) -> Result<Vec<serde_json::Value>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.list_backups().await?)
    }

    pub async fn download_backup(&self, filename: &str) -> Result<Vec<u8>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.download_backup(filename).await?)
    }

    pub async fn get_site_stats(
        &self,
        interval: &str,
        start: Option<i64>,
        end: Option<i64>,
        attrs: Option<&[String]>,
    ) -> Result<Vec<serde_json::Value>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.get_site_stats(interval, start, end, attrs).await?)
    }

    pub async fn get_device_stats(
        &self,
        interval: &str,
        macs: Option<&[String]>,
        attrs: Option<&[String]>,
    ) -> Result<Vec<serde_json::Value>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.get_device_stats(interval, macs, attrs).await?)
    }

    pub async fn get_client_stats(
        &self,
        interval: &str,
        macs: Option<&[String]>,
        attrs: Option<&[String]>,
    ) -> Result<Vec<serde_json::Value>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.get_client_stats(interval, macs, attrs).await?)
    }

    pub async fn get_gateway_stats(
        &self,
        interval: &str,
        start: Option<i64>,
        end: Option<i64>,
        attrs: Option<&[String]>,
    ) -> Result<Vec<serde_json::Value>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session
            .get_gateway_stats(interval, start, end, attrs)
            .await?)
    }

    pub async fn get_dpi_stats(
        &self,
        group_by: &str,
        macs: Option<&[String]>,
    ) -> Result<Vec<serde_json::Value>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.get_dpi_stats(group_by, macs).await?)
    }

    pub async fn list_admins(&self) -> Result<Vec<Admin>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        let raw = session.list_admins().await?;
        Ok(raw
            .into_iter()
            .map(|value| Admin {
                id: value
                    .get("_id")
                    .and_then(|value| value.as_str())
                    .map_or_else(
                        || EntityId::Legacy("unknown".into()),
                        |value| EntityId::Legacy(value.into()),
                    ),
                name: value
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_owned(),
                email: value
                    .get("email")
                    .and_then(|value| value.as_str())
                    .map(String::from),
                role: value
                    .get("role")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown")
                    .to_owned(),
                is_super: value
                    .get("is_super")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                last_login: None,
            })
            .collect())
    }

    pub async fn list_users(
        &self,
    ) -> Result<Vec<crate::session::models::SessionUserEntry>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.list_users().await?)
    }

    pub async fn list_rogue_aps(
        &self,
        within_secs: Option<i64>,
    ) -> Result<Vec<RogueAp>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.list_rogue_aps(within_secs).await?)
    }

    pub async fn list_channels(&self) -> Result<Vec<ChannelAvailability>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.list_channels().await?)
    }

    pub async fn get_client_roams(
        &self,
        mac: &str,
        limit: Option<u32>,
    ) -> Result<Vec<serde_json::Value>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.get_client_roams(mac, limit).await?)
    }

    pub async fn get_client_wifi_experience(
        &self,
        client_ip: &str,
    ) -> Result<serde_json::Value, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.get_client_wifi_experience(client_ip).await?)
    }

    pub async fn is_dpi_enabled(&self) -> Result<bool, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        let settings = session.get_site_settings().await?;
        let enabled = settings
            .iter()
            .find(|s| s.get("key").and_then(|v| v.as_str()) == Some("dpi"))
            .and_then(|s| s.get("enabled"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        Ok(enabled)
    }

    pub async fn list_alarms(&self) -> Result<Vec<Alarm>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        let raw = session.list_alarms().await?;
        Ok(raw.into_iter().map(Alarm::from).collect())
    }

    pub async fn get_system_info(&self) -> Result<SystemInfo, CoreError> {
        {
            let guard = self.inner.integration_client.lock().await;
            if let Some(ic) = guard.as_ref() {
                let info = ic.get_info().await?;
                let fields = &info.fields;
                return Ok(SystemInfo {
                    controller_name: fields
                        .get("applicationName")
                        .or_else(|| fields.get("name"))
                        .and_then(|value| value.as_str())
                        .map(String::from),
                    version: fields
                        .get("applicationVersion")
                        .or_else(|| fields.get("version"))
                        .and_then(|value| value.as_str())
                        .unwrap_or("unknown")
                        .to_owned(),
                    build: fields
                        .get("build")
                        .and_then(|value| value.as_str())
                        .map(String::from),
                    hostname: fields
                        .get("hostname")
                        .and_then(|value| value.as_str())
                        .map(String::from),
                    ip: None,
                    uptime_secs: fields.get("uptime").and_then(serde_json::Value::as_u64),
                    update_available: fields
                        .get("isUpdateAvailable")
                        .or_else(|| fields.get("update_available"))
                        .and_then(serde_json::Value::as_bool),
                });
            }
        }

        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        let raw = session.get_sysinfo().await?;
        Ok(SystemInfo {
            controller_name: raw
                .get("controller_name")
                .or_else(|| raw.get("name"))
                .and_then(|value| value.as_str())
                .map(String::from),
            version: raw
                .get("version")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_owned(),
            build: raw
                .get("build")
                .and_then(|value| value.as_str())
                .map(String::from),
            hostname: raw
                .get("hostname")
                .and_then(|value| value.as_str())
                .map(String::from),
            ip: raw
                .get("ip_addrs")
                .and_then(|value| value.as_array())
                .and_then(|values| values.first())
                .and_then(|value| value.as_str())
                .and_then(|value| value.parse().ok()),
            uptime_secs: raw.get("uptime").and_then(serde_json::Value::as_u64),
            update_available: raw
                .get("update_available")
                .and_then(serde_json::Value::as_bool),
        })
    }

    pub async fn get_site_health(&self) -> Result<Vec<HealthSummary>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        let raw = session.get_health().await?;
        Ok(convert_health_summaries(raw))
    }

    pub async fn get_sysinfo(&self) -> Result<SysInfo, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        let raw = session.get_sysinfo().await?;
        Ok(SysInfo {
            timezone: raw
                .get("timezone")
                .and_then(|value| value.as_str())
                .map(String::from),
            autobackup: raw.get("autobackup").and_then(serde_json::Value::as_bool),
            hostname: raw
                .get("hostname")
                .and_then(|value| value.as_str())
                .map(String::from),
            ip_addrs: raw
                .get("ip_addrs")
                .and_then(|value| value.as_array())
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            live_chat: raw
                .get("live_chat")
                .and_then(|value| value.as_str())
                .map(String::from),
            #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
            data_retention_days: raw
                .get("data_retention_days")
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as u32),
            extra: raw,
        })
    }

    pub async fn list_vpn_settings(&self) -> Result<Vec<VpnSetting>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        let raw = session.get_site_settings().await?;
        let mut settings = raw
            .iter()
            .filter_map(vpn_setting_from_raw)
            .collect::<Vec<_>>();
        settings.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(settings)
    }

    pub async fn get_vpn_setting(&self, key: &str) -> Result<VpnSetting, CoreError> {
        self.list_vpn_settings()
            .await?
            .into_iter()
            .find(|setting| setting.key == key)
            .ok_or_else(|| CoreError::NotFound {
                entity_type: "vpn setting".into(),
                identifier: key.into(),
            })
    }

    pub async fn update_vpn_setting(
        &self,
        key: &str,
        body: &serde_json::Value,
    ) -> Result<VpnSetting, CoreError> {
        if !VPN_SETTING_KEYS.contains(&key) {
            return Err(CoreError::NotFound {
                entity_type: "vpn setting".into(),
                identifier: key.into(),
            });
        }

        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        session.set_site_setting(key, body).await?;
        drop(guard);

        self.get_vpn_setting(key).await
    }

    pub async fn get_all_site_settings(&self) -> Result<Vec<serde_json::Value>, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.get_site_settings().await?)
    }

    pub async fn get_site_setting(&self, key: &str) -> Result<serde_json::Value, CoreError> {
        self.get_all_site_settings()
            .await?
            .into_iter()
            .find(|s| s.get("key").and_then(|v| v.as_str()) == Some(key))
            .ok_or_else(|| CoreError::NotFound {
                entity_type: "setting".into(),
                identifier: key.into(),
            })
    }

    pub async fn update_site_setting(
        &self,
        key: &str,
        body: &serde_json::Value,
    ) -> Result<(), CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        session.set_site_setting(key, body).await?;
        Ok(())
    }

    /// Send a raw GET request to an arbitrary path on the controller.
    ///
    /// The `path` is appended to the controller base URL + platform prefix
    /// (e.g. `/proxy/network/`). The response is returned as raw JSON
    /// without session envelope unwrapping.
    pub async fn raw_get(&self, path: &str) -> Result<serde_json::Value, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.raw_get(path).await?)
    }

    /// Send a raw POST request to an arbitrary path on the controller.
    pub async fn raw_post(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.raw_post(path, body).await?)
    }

    /// Send a raw PUT request to an arbitrary path on the controller.
    pub async fn raw_put(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.raw_put(path, body).await?)
    }

    /// Send a raw PATCH request to an arbitrary path on the controller.
    pub async fn raw_patch(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        Ok(session.raw_patch(path, body).await?)
    }

    /// Send a raw DELETE request to an arbitrary path on the controller.
    pub async fn raw_delete(&self, path: &str) -> Result<(), CoreError> {
        let guard = self.inner.session_client.lock().await;
        let session = require_session(guard.as_ref())?;
        session.raw_delete(path).await?;
        Ok(())
    }
}

fn vpn_setting_from_raw(raw: &serde_json::Value) -> Option<VpnSetting> {
    let object = raw.as_object()?;
    let key = object.get("key")?.as_str()?;
    if !VPN_SETTING_KEYS.contains(&key) {
        return None;
    }

    let mut fields = object.clone();
    fields.remove("_id");
    fields.remove("key");
    fields.remove("site_id");
    let fields = redact_sensitive_value(&serde_json::Value::Object(fields))
        .as_object()
        .cloned()
        .unwrap_or_default();

    Some(VpnSetting {
        key: key.to_owned(),
        enabled: fields.get("enabled").and_then(serde_json::Value::as_bool),
        fields,
    })
}

fn redact_sensitive_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        if should_redact_field(key) {
                            serde_json::Value::String("***REDACTED***".into())
                        } else {
                            redact_sensitive_value(value)
                        },
                    )
                })
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(redact_sensitive_value).collect())
        }
        _ => value.clone(),
    }
}

fn should_redact_field(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "private",
        "password",
        "secret",
        "token",
        "psk",
        "shared_key",
        "certificate",
        "dh_key",
    ]
    .into_iter()
    .any(|needle| key.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{redact_sensitive_value, vpn_setting_from_raw};

    #[test]
    fn vpn_setting_from_raw_filters_to_known_keys() {
        let raw = serde_json::json!({
            "key": "teleport",
            "enabled": true,
            "_id": "abc123",
            "site_id": "default",
        });
        let setting = vpn_setting_from_raw(&raw).expect("teleport should be recognized");

        assert_eq!(setting.key, "teleport");
        assert_eq!(setting.enabled, Some(true));
        assert!(!setting.fields.contains_key("_id"));
        assert!(!setting.fields.contains_key("site_id"));
    }

    #[test]
    fn redact_sensitive_value_masks_nested_vpn_secrets() {
        let redacted = redact_sensitive_value(&serde_json::json!({
            "enabled": true,
            "public_key": "keep-me",
            "x_private_key": "secret",
            "nested": {
                "psk": "hide-me",
                "certificatePem": "hide-me-too"
            }
        }));

        assert_eq!(
            redacted.get("enabled").and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            redacted
                .get("public_key")
                .and_then(serde_json::Value::as_str),
            Some("keep-me")
        );
        assert_eq!(
            redacted
                .get("x_private_key")
                .and_then(serde_json::Value::as_str),
            Some("***REDACTED***")
        );
        assert_eq!(redacted["nested"]["psk"].as_str(), Some("***REDACTED***"));
        assert_eq!(
            redacted["nested"]["certificatePem"].as_str(),
            Some("***REDACTED***")
        );
    }
}
