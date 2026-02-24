use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const TUN_NAME: &str = "tun0";
const OPKG_TUN_NAME: &str = "opkgtun0";
const NDM_IF_NAME: &str = "OpkgTun0";
const TUN_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const TUN_POLL_INTERVAL: Duration = Duration::from_millis(500);
const NDM_READY_TIMEOUT: Duration = Duration::from_secs(10);
const NDM_VERIFY_TIMEOUT: Duration = Duration::from_secs(5);
const NDM_IF_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const NDM_RETRY_BASE_DELAY_MS: u64 = 200;
const NDM_MAX_ATTEMPTS: u32 = 10;

fn run_cmd(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{} {:?}: {}", program, args, e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} {:?}: {}", program, args, stderr.trim()))
    }
}

fn run_cmd_ok(program: &str, args: &[&str]) {
    if let Err(e) = run_cmd(program, args) {
        log::debug!("[routing] ignoring: {}", e);
    }
}

pub fn current_wan_interface() -> Option<String> {
    let out = run_cmd("ip", &["-o", "route", "show", "default"]).ok()?;
    for line in out.lines() {
        if !line.trim_start().starts_with("default ") {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(i) = parts.iter().position(|&p| p == "dev") {
            if let Some(&dev) = parts.get(i + 1) {
                // Skip tunnel/LAN-like devices; we only want a real upstream WAN.
                let is_lan_like = dev == "lo"
                    || dev.starts_with("br")
                    || dev.starts_with("lan")
                    || dev.starts_with("vlan")
                    || dev.starts_with("wl");
                if dev != OPKG_TUN_NAME && dev != TUN_NAME && !is_lan_like {
                    return Some(dev.to_string());
                }
            }
        }
    }
    None
}

fn extract_server_ips(addresses: &[String]) -> Vec<String> {
    addresses
        .iter()
        .filter_map(|addr| {
            let ip = addr.split(':').next()?;
            if ip.parse::<std::net::IpAddr>().is_ok() {
                Some(ip.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn find_ndmc() -> &'static str {
    static PATHS: &[&str] = &["/usr/bin/ndmc", "/bin/ndmc", "/sbin/ndmc"];
    for p in PATHS {
        if std::path::Path::new(p).exists() {
            return p;
        }
    }
    "ndmc"
}

fn ndmc_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn is_ndm_transient_error(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("0xcffd0060")
        || m.contains("0xcffd009f")
        || m.contains("failed to initialize")
        || m.contains("unable to find opkgtun0")
        || m.contains("temporarily unavailable")
}

fn ndmc_exec_once(cmd: &str) -> Result<String, String> {
    let bin = find_ndmc();
    let output = Command::new(bin)
        .args(["-c", cmd])
        .output()
        .map_err(|e| format!("exec '{}' error: {}", cmd, e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let combined = match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{} | {}", stdout, stderr),
        (false, true) => stdout,
        (true, false) => stderr,
        (true, true) => String::new(),
    };
    if output.status.success() {
        Ok(combined)
    } else {
        Err(format!(
            "'{}' exit={} {}",
            cmd,
            output.status.code().unwrap_or(-1),
            combined
        ))
    }
}

fn summarize_ndmc_output(cmd: &str, output: &str) -> String {
    let trimmed = output.trim();
    if cmd == "show interface" {
        let lines = trimmed.lines().count();
        return format!("{} lines", lines);
    }

    const MAX_LEN: usize = 240;
    if trimmed.len() > MAX_LEN {
        format!("{}...", &trimmed[..MAX_LEN])
    } else {
        trimmed.to_string()
    }
}

fn ndmc(cmd: &str) -> Result<String, String> {
    let _guard = ndmc_lock().lock().unwrap();
    let mut last_err = String::new();
    for attempt in 1..=NDM_MAX_ATTEMPTS {
        match ndmc_exec_once(cmd) {
            Ok(output) => {
                let summary = summarize_ndmc_output(cmd, &output);
                let msg = if summary.is_empty() {
                    format!("[ndmc] ok: {}", cmd)
                } else {
                    format!("[ndmc] ok: {} ({})", cmd, summary)
                };
                log::info!("{}", msg);
                crate::logs::global_buffer().push(msg);
                return Ok(output);
            }
            Err(err) => {
                last_err = err;
                let transient = is_ndm_transient_error(&last_err);
                let msg = format!(
                    "[ndmc] attempt {}/{} failed: {}",
                    attempt, NDM_MAX_ATTEMPTS, last_err
                );
                log::warn!("{}", msg);
                crate::logs::global_buffer().push(msg);
                if transient && attempt < NDM_MAX_ATTEMPTS {
                    let backoff = (NDM_RETRY_BASE_DELAY_MS * attempt as u64).min(1000);
                    std::thread::sleep(Duration::from_millis(backoff));
                    continue;
                }
                return Err(last_err);
            }
        }
    }
    Err(last_err)
}

fn wait_ndm_ready() -> Result<(), String> {
    let start = std::time::Instant::now();
    while start.elapsed() < NDM_READY_TIMEOUT {
        // Probe NDM CLI readiness with a valid read-only command.
        match ndmc("show interface") {
            Ok(_) => return Ok(()),
            Err(e) => {
                if !is_ndm_transient_error(&e) {
                    return Err(format!("NDM check failed: {}", e));
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
    Err(format!(
        "NDM is not ready after {}s",
        NDM_READY_TIMEOUT.as_secs()
    ))
}

fn verify_ndm_default_route() -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < NDM_VERIFY_TIMEOUT {
        if let Ok(output) = ndmc("show interface") {
            if interface_defaultgw_is_yes(&output, NDM_IF_NAME) {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    false
}

fn interface_exists_in_show_output(show_output: &str, if_name: &str) -> bool {
    let needle = format!("id: {}", if_name);
    show_output.lines().any(|l| l.trim() == needle)
}

fn interface_defaultgw_is_yes(show_output: &str, if_name: &str) -> bool {
    let needle = format!("id: {}", if_name);
    let mut in_target = false;
    for raw in show_output.lines() {
        let line = raw.trim();
        if line.starts_with("id: ") {
            if in_target {
                // New interface block started; target block ended.
                return false;
            }
            in_target = line == needle;
            continue;
        }
        if in_target && line == "defaultgw: yes" {
            return true;
        }
    }
    false
}

fn wait_ndm_interface_exists() -> Result<(), String> {
    let start = std::time::Instant::now();
    while start.elapsed() < NDM_IF_WAIT_TIMEOUT {
        if let Ok(output) = ndmc("show interface") {
            if interface_exists_in_show_output(&output, NDM_IF_NAME) {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(format!(
        "interface {} did not appear in NDM within {}s",
        NDM_IF_NAME,
        NDM_IF_WAIT_TIMEOUT.as_secs()
    ))
}

fn ndmc_soft(cmd: &str) {
    if let Err(e) = ndmc(cmd) {
        let msg = format!("[ndmc] soft-fail: {}", e);
        log::warn!("{}", msg);
        crate::logs::global_buffer().push(msg);
    }
}

fn ndmc_required(cmd: &str) -> Result<(), String> {
    ndmc(cmd).map(|_| ())
}

fn get_tun_ip_mask() -> Option<(String, String)> {
    let out = run_cmd("ip", &["-o", "addr", "show", OPKG_TUN_NAME]).ok()?;
    for line in out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(i) = parts.iter().position(|&p| p == "inet") {
            let cidr = parts.get(i + 1)?;
            let mut split = cidr.split('/');
            let ip = split.next()?.to_string();
            let prefix: u8 = split.next().unwrap_or("32").parse().unwrap_or(32);
            let mask = prefix_to_netmask(prefix);
            return Some((ip, mask));
        }
    }
    None
}

fn get_tun_mtu() -> Option<u16> {
    let out = run_cmd("ip", &["-o", "link", "show", OPKG_TUN_NAME]).ok()?;
    let parts: Vec<&str> = out.split_whitespace().collect();
    let idx = parts.iter().position(|&p| p == "mtu")?;
    parts.get(idx + 1)?.parse::<u16>().ok()
}

fn prefix_to_netmask(prefix: u8) -> String {
    let bits: u32 = if prefix >= 32 {
        0xFFFF_FFFF
    } else {
        !((1u32 << (32 - prefix)) - 1)
    };
    format!(
        "{}.{}.{}.{}",
        (bits >> 24) & 0xFF,
        (bits >> 16) & 0xFF,
        (bits >> 8) & 0xFF,
        bits & 0xFF
    )
}

fn ensure_ndm_interface_object() -> Result<(), String> {
    wait_ndm_ready()?;
    let msg = format!("[routing] ensuring {} in NDM (ndmc={})", NDM_IF_NAME, find_ndmc());
    log::info!("{}", msg);
    crate::logs::global_buffer().push(msg);

    // Create NDM interface object first.
    ndmc_required(&format!("interface {}", NDM_IF_NAME))?;
    wait_ndm_interface_exists()?;
    Ok(())
}

fn apply_ndm_interface_settings() -> Result<(), String> {
    if let Some((ip, mask)) = get_tun_ip_mask() {
        ndmc_required(&format!("interface {} ip address {} {}", NDM_IF_NAME, ip, mask))?;
    }
    if let Some(mtu) = get_tun_mtu() {
        ndmc_required(&format!("interface {} ip mtu {}", NDM_IF_NAME, mtu))?;
    }

    ndmc_required(&format!("interface {} ip global auto", NDM_IF_NAME))?;
    ndmc_required(&format!("interface {} ip tcp adjust-mss pmtu", NDM_IF_NAME))?;
    ndmc_required(&format!("interface {} security-level public", NDM_IF_NAME))?;
    ndmc_required(&format!("interface {} up", NDM_IF_NAME))?;
    Ok(())
}

fn set_ndm_default_routes() -> Result<(), String> {
    // Shell implementation uses interface-based default route.
    ndmc_required(&format!("ip route default {}", NDM_IF_NAME))?;
    // IPv6 default route can fail on some configs/firmware, do not fail whole setup.
    ndmc_soft(&format!("ipv6 route default {}", NDM_IF_NAME));
    Ok(())
}

fn assert_ndm_default_route() -> Result<(), String> {
    if verify_ndm_default_route() {
        Ok(())
    } else {
        Err(format!(
            "NDM default route is not active for {} (defaultgw: no)",
            NDM_IF_NAME
        ))
    }
}

fn wait_for_tun() -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < TUN_WAIT_TIMEOUT {
        if std::path::Path::new(&format!("/sys/class/net/{}", TUN_NAME)).exists() {
            return true;
        }
        std::thread::sleep(TUN_POLL_INTERVAL);
    }
    false
}

/// Configure interface + NDM routing after the VPN client creates tun0.
/// Renames tun0 → opkgtun0 and sets default route via NDM.
/// Returns the detected WAN interface name on success.
pub fn setup_routing(server_addresses: &[String]) -> Result<String, String> {
    log::info!("[routing] waiting for {} ...", TUN_NAME);

    if !wait_for_tun() {
        return Err(format!(
            "{} did not appear within {}s",
            TUN_NAME,
            TUN_WAIT_TIMEOUT.as_secs()
        ));
    }
    std::thread::sleep(Duration::from_millis(500));

    // Remove stale opkgtun0 if leftover from a previous run
    run_cmd_ok("ip", &["link", "del", OPKG_TUN_NAME]);

    // Create NDM object before Linux rename. On some Keenetic builds this avoids
    // OpkgTun creation failure when opkgtun0 already exists.
    ensure_ndm_interface_object()?;

    log::info!("[routing] renaming {} → {}", TUN_NAME, OPKG_TUN_NAME);
    run_cmd("ip", &["link", "set", TUN_NAME, "down"])?;
    run_cmd("ip", &["link", "set", TUN_NAME, "name", OPKG_TUN_NAME])?;
    run_cmd("ip", &["link", "set", OPKG_TUN_NAME, "up"])?;

    // Apply runtime params after opkgtun0 exists and has IP/MTU.
    apply_ndm_interface_settings()?;

    let wan_if = current_wan_interface().ok_or("failed to detect WAN interface")?;
    log::info!("[routing] WAN interface: {}", wan_if);

    // Route VPN-server traffic through WAN to avoid a routing loop
    for ip in extract_server_ips(server_addresses) {
        let cidr = format!("{}/32", ip);
        run_cmd_ok("ip", &["route", "del", &cidr]);
        if let Err(e) = run_cmd("ip", &["route", "add", &cidr, "dev", &wan_if]) {
            log::warn!("[routing] server route {}: {}", cidr, e);
        }
    }

    log::info!("[routing] setting default route via {}", OPKG_TUN_NAME);
    set_ndm_default_routes()?;
    assert_ndm_default_route()?;

    log::info!("[routing] setup complete (WAN={})", wan_if);
    crate::logs::global_buffer().push(format!("[routing] setup complete (WAN={})", wan_if));
    Ok(wan_if)
}

/// Update only the server routes to go through a new WAN interface.
/// Does NOT touch the TUN device, iptables, or NDM.
pub fn reroute_server_via_wan(server_addresses: &[String], new_wan: &str) {
    log::info!("[routing] re-routing server IPs via {}", new_wan);
    for ip in extract_server_ips(server_addresses) {
        let cidr = format!("{}/32", ip);
        run_cmd_ok("ip", &["route", "del", &cidr]);
        if let Err(e) = run_cmd("ip", &["route", "add", &cidr, "dev", new_wan]) {
            log::warn!("[routing] server route {} via {}: {}", cidr, new_wan, e);
        }
    }
    let msg = format!("[routing] server routes updated to WAN={}", new_wan);
    log::info!("{}", msg);
    crate::logs::global_buffer().push(msg);
}

// --------------- watchdog helpers ---------------

pub fn is_tun_alive() -> bool {
    std::path::Path::new(&format!("/sys/class/net/{}", OPKG_TUN_NAME)).exists()
}

pub fn check_connectivity(check_url: &str, timeout: Duration) -> bool {
    let timeout_secs = timeout.as_secs().max(1);
    let connect_timeout = timeout_secs.to_string();
    let max_time = (timeout_secs + 2).to_string();

    let args_owned = vec![
        "--interface".to_string(),
        OPKG_TUN_NAME.to_string(),
        "--connect-timeout".to_string(),
        connect_timeout,
        "--max-time".to_string(),
        max_time,
        "-fsS".to_string(),
        "-o".to_string(),
        "/dev/null".to_string(),
        check_url.to_string(),
    ];
    let args: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();

    match run_cmd("curl", &args) {
        Ok(_) => true,
        Err(e) => {
            log::debug!("[routing] connectivity probe failed: {}", e);
            false
        }
    }
}

// --------------- teardown ---------------

/// Bring the tunnel link down and clear temporary server routes.
pub fn teardown_routing(server_addresses: &[String]) {
    log::info!("[routing] tearing down ...");

    for ip in extract_server_ips(server_addresses) {
        run_cmd_ok("ip", &["route", "del", &format!("{}/32", ip)]);
    }

    // Shell behavior: bring interface down and recreate it on next start.
    run_cmd_ok("ip", &["link", "set", OPKG_TUN_NAME, "down"]);

    log::info!("[routing] teardown complete");
    crate::logs::global_buffer().push("[routing] teardown complete".into());
}
