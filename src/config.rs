use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Wrapper's own configuration (read from JSON).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WrapperConfig {
    #[serde(default)]
    pub tunnel: TunnelSettings,
    #[serde(default)]
    pub webui: WebUISettings,
    #[serde(default)]
    pub logging: LogSettings,
    #[serde(default)]
    pub routing: RoutingSettings,
}

/// Settings that map to TrustTunnelClient's TOML config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelSettings {
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_upstream_protocol")]
    pub upstream_protocol: String,
    #[serde(default)]
    pub certificate: String,
    #[serde(default)]
    pub skip_verification: bool,
    #[serde(default = "default_vpn_mode")]
    pub vpn_mode: String,
    #[serde(default)]
    pub dns_upstreams: Vec<String>,
    #[serde(default)]
    pub killswitch_enabled: bool,
    #[serde(default)]
    pub included_routes: Vec<String>,
    #[serde(default)]
    pub excluded_routes: Vec<String>,
    #[serde(default = "default_mtu")]
    pub mtu_size: u16,
    #[serde(default)]
    pub anti_dpi: bool,
    #[serde(default)]
    pub socks_address: String,
    #[serde(default = "default_reconnect_delay")]
    pub reconnect_delay: u64,
    #[serde(default = "default_loglevel")]
    pub loglevel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUISettings {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_true")]
    pub auth: bool,
    #[serde(default)]
    pub ndm_host: String,
    #[serde(default = "default_ndm_port")]
    pub ndm_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSettings {
    #[serde(default = "default_loglevel")]
    pub level: String,
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub watchdog_enabled: bool,
    #[serde(default = "default_watchdog_interval")]
    pub watchdog_interval: u64,
    #[serde(default = "default_watchdog_failures")]
    pub watchdog_failures: u32,
}

impl Default for RoutingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            watchdog_enabled: true,
            watchdog_interval: default_watchdog_interval(),
            watchdog_failures: default_watchdog_failures(),
        }
    }
}

fn default_watchdog_interval() -> u64 { 30 }
fn default_watchdog_failures() -> u32 { 3 }

fn default_upstream_protocol() -> String { "http2".into() }
fn default_vpn_mode() -> String { "general".into() }
fn default_mtu() -> u16 { 1280 }
fn default_reconnect_delay() -> u64 { 5 }
fn default_loglevel() -> String { "info".into() }
fn default_port() -> u16 { 8080 }
fn default_bind() -> String { "0.0.0.0".into() }
fn default_true() -> bool { true }
fn default_max_lines() -> usize { 500 }
fn default_ndm_port() -> u16 { 80 }

impl Default for TunnelSettings {
    fn default() -> Self {
        Self {
            hostname: String::new(),
            addresses: Vec::new(),
            username: String::new(),
            password: String::new(),
            upstream_protocol: default_upstream_protocol(),
            certificate: String::new(),
            skip_verification: false,
            vpn_mode: default_vpn_mode(),
            dns_upstreams: vec!["tls://1.1.1.1".into()],
            killswitch_enabled: false,
            included_routes: vec!["0.0.0.0/0".into(), "2000::/3".into()],
            excluded_routes: vec![
                "10.0.0.0/8".into(),
                "172.16.0.0/12".into(),
                "192.168.0.0/16".into(),
            ],
            mtu_size: default_mtu(),
            anti_dpi: false,
            socks_address: String::new(),
            reconnect_delay: default_reconnect_delay(),
            loglevel: default_loglevel(),
        }
    }
}

impl Default for WebUISettings {
    fn default() -> Self {
        Self {
            port: default_port(),
            bind: default_bind(),
            auth: true,
            ndm_host: String::new(),
            ndm_port: default_ndm_port(),
        }
    }
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            level: default_loglevel(),
            max_lines: default_max_lines(),
        }
    }
}


impl WrapperConfig {
    pub fn load(path: &str) -> Result<Self, String> {
        if !Path::new(path).exists() {
            log::warn!("Config not found at {}, using defaults", path);
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config {}: {}", path, e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse config: {}", e))
    }

    pub fn save(&self, path: &str) -> Result<(), String> {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir: {}", e))?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(path, content)
            .map_err(|e| format!("Failed to write config: {}", e))
    }
}

/// Generates a valid TOML config file for `trusttunnel_client`.
pub fn generate_client_toml(settings: &TunnelSettings) -> String {
    let mut toml = String::with_capacity(1024);

    toml.push_str(&format!("loglevel = \"{}\"\n", settings.loglevel));
    toml.push_str(&format!("vpn_mode = \"{}\"\n", settings.vpn_mode));
    toml.push_str(&format!(
        "killswitch_enabled = {}\n",
        settings.killswitch_enabled
    ));
    toml.push_str(&format!("anti_dpi = {}\n", settings.anti_dpi));

    if !settings.dns_upstreams.is_empty() {
        toml.push_str(&format!(
            "dns_upstreams = [{}]\n",
            settings
                .dns_upstreams
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    toml.push_str("\n[endpoint]\n");
    toml.push_str(&format!("hostname = \"{}\"\n", settings.hostname));
    toml.push_str(&format!(
        "addresses = [{}]\n",
        settings
            .addresses
            .iter()
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    toml.push_str(&format!("username = \"{}\"\n", settings.username));
    toml.push_str(&format!("password = \"{}\"\n", settings.password));
    toml.push_str(&format!(
        "upstream_protocol = \"{}\"\n",
        settings.upstream_protocol
    ));
    toml.push_str(&format!(
        "skip_verification = {}\n",
        settings.skip_verification
    ));
    if !settings.certificate.is_empty() {
        toml.push_str(&format!("certificate = \"{}\"\n", settings.certificate));
    }

    toml.push_str("\n[listener.tun]\n");
    let included = if settings.included_routes.is_empty() && settings.vpn_mode == "general" {
        vec!["0.0.0.0/0".to_string(), "2000::/3".to_string()]
    } else {
        settings.included_routes.clone()
    };
    if !included.is_empty() {
        toml.push_str(&format!(
            "included_routes = [{}]\n",
            included
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !settings.excluded_routes.is_empty() {
        toml.push_str(&format!(
            "excluded_routes = [{}]\n",
            settings
                .excluded_routes
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    toml.push_str(&format!("mtu_size = {}\n", settings.mtu_size));

    if !settings.socks_address.is_empty() {
        toml.push_str("\n[listener.socks]\n");
        toml.push_str(&format!("address = \"{}\"\n", settings.socks_address));
    }

    toml
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_toml() {
        let s = TunnelSettings {
            hostname: "vpn.example.com".into(),
            addresses: vec!["1.2.3.4:443".into()],
            username: "user".into(),
            password: "pass".into(),
            ..Default::default()
        };
        let toml = generate_client_toml(&s);
        assert!(toml.contains("[endpoint]"));
        assert!(toml.contains("hostname = \"vpn.example.com\""));
        assert!(toml.contains("username = \"user\""));
        assert!(toml.contains("[listener.tun]"));
    }
}
