use serde::de::{self, Deserializer};
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
    pub custom_sni: String,
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
    #[serde(default = "default_dns_upstreams")]
    pub dns_upstreams: Vec<String>,
    #[serde(default)]
    pub killswitch_enabled: bool,
    #[serde(default)]
    pub killswitch_allow_ports: Vec<u16>,
    #[serde(default = "default_true")]
    pub post_quantum_group_enabled: bool,
    #[serde(default)]
    pub exclusions: Vec<String>,
    #[serde(default = "default_included_routes")]
    pub included_routes: Vec<String>,
    #[serde(default = "default_excluded_routes")]
    pub excluded_routes: Vec<String>,
    #[serde(default = "default_mtu")]
    pub mtu_size: u16,
    #[serde(default)]
    pub bound_if: String,
    #[serde(default = "default_false")]
    pub change_system_dns: bool,
    #[serde(default)]
    pub anti_dpi: bool,
    #[serde(default = "default_true")]
    pub has_ipv6: bool,
    #[serde(default)]
    pub client_random: String,
    #[serde(default)]
    pub socks_address: String,
    #[serde(default)]
    pub socks_username: String,
    #[serde(default)]
    pub socks_password: String,
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
    #[serde(default = "default_file_enabled")]
    pub file_enabled: bool,
    #[serde(default = "default_log_file_path")]
    pub file_path: String,
    #[serde(
        default = "default_rotate_size_bytes",
        deserialize_with = "deserialize_rotate_size"
    )]
    pub rotate_size: u64,
    #[serde(default = "default_rotate_keep")]
    pub rotate_keep: usize,
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
    #[serde(default = "default_watchdog_check_url")]
    pub watchdog_check_url: String,
    #[serde(default = "default_watchdog_check_timeout")]
    pub watchdog_check_timeout: u64,
}

impl Default for RoutingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            watchdog_enabled: true,
            watchdog_interval: default_watchdog_interval(),
            watchdog_failures: default_watchdog_failures(),
            watchdog_check_url: default_watchdog_check_url(),
            watchdog_check_timeout: default_watchdog_check_timeout(),
        }
    }
}

fn default_watchdog_interval() -> u64 {
    30
}
fn default_watchdog_failures() -> u32 {
    3
}
fn default_watchdog_check_url() -> String {
    "http://connectivitycheck.gstatic.com/generate_204".into()
}
fn default_watchdog_check_timeout() -> u64 {
    5
}

fn default_upstream_protocol() -> String {
    "http2".into()
}
fn default_vpn_mode() -> String {
    "general".into()
}
fn default_mtu() -> u16 {
    1280
}
fn default_reconnect_delay() -> u64 {
    5
}
fn default_dns_upstreams() -> Vec<String> {
    vec!["tls://1.1.1.1".into()]
}
fn default_included_routes() -> Vec<String> {
    vec!["0.0.0.0/0".into(), "2000::/3".into()]
}
fn default_excluded_routes() -> Vec<String> {
    vec![
        "10.0.0.0/8".into(),
        "172.16.0.0/12".into(),
        "192.168.0.0/16".into(),
    ]
}
fn default_loglevel() -> String {
    "info".into()
}
fn default_port() -> u16 {
    8080
}
fn default_bind() -> String {
    "0.0.0.0".into()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_max_lines() -> usize {
    500
}
fn default_ndm_port() -> u16 {
    80
}
fn default_file_enabled() -> bool {
    true
}
fn default_log_file_path() -> String {
    "/var/log/trusttunnel-keenetic/trusttunnel-keenetic.log".into()
}
fn default_rotate_size_bytes() -> u64 {
    512 * 1024
}
fn default_rotate_keep() -> usize {
    1
}

fn parse_size_with_units(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(bytes) = trimmed.parse::<u64>() {
        return Some(bytes);
    }

    let upper = trimmed.to_ascii_uppercase();
    let mut digits_end = 0usize;
    for (idx, ch) in upper.char_indices() {
        if ch.is_ascii_digit() {
            digits_end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    if digits_end == 0 {
        return None;
    }

    let number = upper[..digits_end].parse::<u64>().ok()?;
    let suffix = upper[digits_end..].trim();
    let multiplier = match suffix {
        "K" | "KB" => 1024u64,
        "M" | "MB" => 1024u64 * 1024,
        "G" | "GB" => 1024u64 * 1024 * 1024,
        "B" | "" => 1u64,
        _ => return None,
    };
    number.checked_mul(multiplier)
}

fn deserialize_rotate_size<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum RotateSize {
        Number(u64),
        Text(String),
    }

    match RotateSize::deserialize(deserializer)? {
        RotateSize::Number(v) => Ok(v),
        RotateSize::Text(s) => parse_size_with_units(&s).ok_or_else(|| {
            de::Error::custom(format!(
                "invalid rotate_size '{}', use bytes (1048576) or units (512KB, 10MB, 1GB)",
                s
            ))
        }),
    }
}

impl Default for TunnelSettings {
    fn default() -> Self {
        Self {
            hostname: String::new(),
            custom_sni: String::new(),
            addresses: Vec::new(),
            username: String::new(),
            password: String::new(),
            upstream_protocol: default_upstream_protocol(),
            certificate: String::new(),
            skip_verification: false,
            vpn_mode: default_vpn_mode(),
            dns_upstreams: vec!["tls://1.1.1.1".into()],
            killswitch_enabled: false,
            killswitch_allow_ports: Vec::new(),
            post_quantum_group_enabled: true,
            exclusions: Vec::new(),
            included_routes: vec!["0.0.0.0/0".into(), "2000::/3".into()],
            excluded_routes: vec![
                "10.0.0.0/8".into(),
                "172.16.0.0/12".into(),
                "192.168.0.0/16".into(),
            ],
            mtu_size: default_mtu(),
            bound_if: String::new(),
            change_system_dns: false,
            anti_dpi: false,
            has_ipv6: true,
            client_random: String::new(),
            socks_address: String::new(),
            socks_username: String::new(),
            socks_password: String::new(),
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
            file_enabled: default_file_enabled(),
            file_path: default_log_file_path(),
            rotate_size: default_rotate_size_bytes(),
            rotate_keep: default_rotate_keep(),
        }
    }
}

impl WrapperConfig {
    pub fn load(path: &str) -> Result<Self, String> {
        if !Path::new(path).exists() {
            return Err(format!("Config not found at {}", path));
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config {}: {}", path, e))?;
        let config: Self =
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<(), String> {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {}", e))?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        write_private_atomic(path, &content).map_err(|e| format!("Failed to write config: {}", e))
    }

    pub fn validate(&self) -> Result<(), String> {
        self.tunnel.validate()?;
        if self.webui.port == 0 || self.webui.ndm_port == 0 {
            return Err("WebUI and NDM ports must be between 1 and 65535".into());
        }
        if self.logging.max_lines == 0 {
            return Err("logging.max_lines must be greater than zero".into());
        }
        if self.routing.watchdog_enabled
            && (self.routing.watchdog_interval == 0
                || self.routing.watchdog_failures == 0
                || self.routing.watchdog_check_timeout == 0)
        {
            return Err(
                "watchdog_interval, watchdog_failures and watchdog_check_timeout must be greater than zero".into(),
            );
        }
        Ok(())
    }
}

impl TunnelSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(576..=9000).contains(&self.mtu_size) {
            return Err("tunnel.mtu_size must be between 576 and 9000".into());
        }
        if self.reconnect_delay == 0 {
            return Err("tunnel.reconnect_delay must be greater than zero".into());
        }
        if !matches!(self.upstream_protocol.as_str(), "http2" | "http3") {
            return Err("tunnel.upstream_protocol must be http2 or http3".into());
        }
        if !matches!(self.vpn_mode.as_str(), "general" | "selective") {
            return Err("tunnel.vpn_mode must be general or selective".into());
        }
        Ok(())
    }
}

pub(crate) fn write_private_atomic(path: &str, content: &str) -> Result<(), String> {
    let tmp_path = format!("{}.tmp.{}", path, std::process::id());
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp_path)
            .map_err(|e| format!("open {}: {}", tmp_path, e))?;
        file.write_all(content.as_bytes())
            .map_err(|e| format!("write {}: {}", tmp_path, e))?;
        file.sync_all()
            .map_err(|e| format!("sync {}: {}", tmp_path, e))?;
    }
    #[cfg(not(unix))]
    fs::write(&tmp_path, content).map_err(|e| format!("write {}: {}", tmp_path, e))?;
    #[cfg(windows)]
    if Path::new(path).exists() {
        fs::remove_file(path).map_err(|e| format!("replace {}: {}", path, e))?;
    }
    fs::rename(&tmp_path, path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("rename {} to {}: {}", tmp_path, path, e)
    })
}

/// Generates a valid TOML config file for `trusttunnel_client`.
pub fn generate_client_toml(settings: &TunnelSettings) -> String {
    let mut toml = String::with_capacity(1024);

    toml.push_str(&format!("loglevel = {}\n", toml_string(&settings.loglevel)));
    toml.push_str(&format!("vpn_mode = {}\n", toml_string(&settings.vpn_mode)));
    toml.push_str(&format!(
        "killswitch_enabled = {}\n",
        settings.killswitch_enabled
    ));
    toml.push_str(&format!(
        "killswitch_allow_ports = [{}]\n",
        settings
            .killswitch_allow_ports
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    toml.push_str(&format!(
        "post_quantum_group_enabled = {}\n",
        settings.post_quantum_group_enabled
    ));
    toml.push_str(&format!(
        "exclusions = [{}]\n",
        settings
            .exclusions
            .iter()
            .map(|s| toml_string(s))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    toml.push_str("\n[endpoint]\n");
    toml.push_str(&format!("hostname = {}\n", toml_string(&settings.hostname)));
    toml.push_str(&format!(
        "addresses = [{}]\n",
        settings
            .addresses
            .iter()
            .map(|s| toml_string(s))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    toml.push_str(&format!("username = {}\n", toml_string(&settings.username)));
    toml.push_str(&format!("password = {}\n", toml_string(&settings.password)));
    toml.push_str(&format!(
        "upstream_protocol = {}\n",
        toml_string(&settings.upstream_protocol)
    ));
    toml.push_str(&format!("has_ipv6 = {}\n", settings.has_ipv6));
    toml.push_str(&format!(
        "client_random = {}\n",
        toml_string(&settings.client_random)
    ));
    toml.push_str(&format!(
        "skip_verification = {}\n",
        settings.skip_verification
    ));
    toml.push_str(&format!("anti_dpi = {}\n", settings.anti_dpi));
    if !settings.certificate.is_empty() {
        toml.push_str(&format!(
            "certificate = {}\n",
            toml_string(&settings.certificate)
        ));
    }
    if !settings.custom_sni.is_empty() {
        toml.push_str(&format!(
            "custom_sni = {}\n",
            toml_string(&settings.custom_sni)
        ));
    }
    if !settings.dns_upstreams.is_empty() {
        toml.push_str(&format!(
            "dns_upstreams = [{}]\n",
            settings
                .dns_upstreams
                .iter()
                .map(|s| toml_string(s))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    toml.push_str("\n[listener.tun]\n");
    toml.push_str(&format!("bound_if = {}\n", toml_string(&settings.bound_if)));
    toml.push_str(&format!(
        "change_system_dns = {}\n",
        settings.change_system_dns
    ));
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
                .map(|s| toml_string(s))
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
                .map(|s| toml_string(s))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    toml.push_str(&format!("mtu_size = {}\n", settings.mtu_size));

    if !settings.socks_address.is_empty() {
        toml.push_str("\n[listener.socks]\n");
        toml.push_str(&format!(
            "address = {}\n",
            toml_string(&settings.socks_address)
        ));
        if !settings.socks_username.is_empty() {
            toml.push_str(&format!(
                "username = {}\n",
                toml_string(&settings.socks_username)
            ));
        }
        if !settings.socks_password.is_empty() {
            toml.push_str(&format!(
                "password = {}\n",
                toml_string(&settings.socks_password)
            ));
        }
    }

    toml
}

fn toml_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\u{08}' => escaped.push_str("\\b"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\r' => escaped.push_str("\\r"),
            c if c.is_control() => escaped.push_str(&format!("\\u{:04X}", c as u32)),
            c => escaped.push(c),
        }
    }
    format!("\"{}\"", escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_toml() {
        let s = TunnelSettings {
            hostname: "vpn.example.com".into(),
            custom_sni: "sni.example.com".into(),
            addresses: vec!["1.2.3.4:443".into()],
            username: "user".into(),
            password: "pass".into(),
            ..Default::default()
        };
        let toml = generate_client_toml(&s);
        assert!(toml.contains("[endpoint]"));
        assert!(toml.contains("hostname = \"vpn.example.com\""));
        assert!(toml.contains("custom_sni = \"sni.example.com\""));
        assert!(toml.contains("[endpoint]\n"));
        assert!(toml.contains("dns_upstreams = [\"tls://1.1.1.1\"]"));
        assert!(toml.contains("username = \"user\""));
        assert!(toml.contains("[listener.tun]"));
    }

    #[test]
    fn test_escape_toml_strings() {
        let s = TunnelSettings {
            username: "user\"name".into(),
            password: "pa\\ss".into(),
            ..Default::default()
        };
        let toml = generate_client_toml(&s);
        assert!(toml.contains("username = \"user\\\"name\""));
        assert!(toml.contains("password = \"pa\\\\ss\""));
    }

    #[test]
    fn test_parse_size_with_units() {
        assert_eq!(parse_size_with_units("1048576"), Some(1_048_576));
        assert_eq!(parse_size_with_units("512KB"), Some(512 * 1024));
        assert_eq!(parse_size_with_units("10mb"), Some(10 * 1024 * 1024));
        assert_eq!(parse_size_with_units("1 G"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size_with_units(""), None);
        assert_eq!(parse_size_with_units("oops"), None);
    }

    #[test]
    fn partial_tunnel_config_keeps_non_empty_defaults() {
        let settings: TunnelSettings =
            serde_json::from_str(r#"{"hostname":"vpn.example.com"}"#).unwrap();
        assert_eq!(settings.dns_upstreams, vec!["tls://1.1.1.1"]);
        assert_eq!(settings.included_routes, vec!["0.0.0.0/0", "2000::/3"]);
        assert_eq!(
            settings.excluded_routes,
            vec!["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16",]
        );
    }

    #[test]
    fn validation_rejects_invalid_tunnel_values() {
        let mut settings = TunnelSettings {
            mtu_size: 0,
            ..Default::default()
        };
        assert!(settings.validate().is_err());

        settings.mtu_size = 1280;
        settings.upstream_protocol = "invalid".into();
        assert!(settings.validate().is_err());
    }

    #[test]
    fn control_characters_are_escaped_in_toml() {
        let settings = TunnelSettings {
            username: "user\u{0001}name".into(),
            ..Default::default()
        };
        let toml = generate_client_toml(&settings);
        assert!(toml.contains("username = \"user\\u0001name\""));
    }
}
