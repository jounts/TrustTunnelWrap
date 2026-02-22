use std::process::Command;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const TUN_NAME: &str = "tun0";
const OPKG_TUN_NAME: &str = "OpkgTun0";
const NDM_IF_NAME: &str = "OpkgTun0";
const ROUTE_METRIC: &str = "500";
const TUN_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const TUN_POLL_INTERVAL: Duration = Duration::from_millis(500);
const CONNECTIVITY_ROUTE_PROBE_IP: &str = "1.1.1.1";
const CONNECTIVITY_TCP_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECTIVITY_TCP_PROBES: &[(&str, u16)] = &[("1.1.1.1", 443), ("8.8.8.8", 53)];

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
    let out = run_cmd("ip", &["-o", "route", "show", "to", "default"]).ok()?;
    for line in out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(i) = parts.iter().position(|&p| p == "dev") {
            if let Some(&dev) = parts.get(i + 1) {
                // Skip our own tunnel — we want the real WAN
                if dev != OPKG_TUN_NAME && dev != TUN_NAME {
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

fn ndmc(cmd: &str) {
    let bin = find_ndmc();
    let output = match Command::new(bin).args(&["-c", cmd]).output() {
        Ok(o) => o,
        Err(e) => {
            let msg = format!("[ndmc] exec '{}' error: {}", cmd, e);
            log::warn!("{}", msg);
            crate::logs::global_buffer().push(msg);
            return;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let combined = match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{} | {}", stdout, stderr),
        (false, true) => stdout,
        (true, false) => stderr,
        (true, true) => String::new(),
    };
    if output.status.success() {
        let msg = format!("[ndmc] ok: {} {}", cmd, combined);
        log::info!("{}", msg);
        crate::logs::global_buffer().push(msg);
    } else {
        let msg = format!("[ndmc] '{}' exit={} {}", cmd, output.status.code().unwrap_or(-1), combined);
        log::warn!("{}", msg);
        crate::logs::global_buffer().push(msg);
    }
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

fn register_ndm_interface() {
    let msg = format!("[routing] ensuring {} in NDM (ndmc={})", NDM_IF_NAME, find_ndmc());
    log::info!("{}", msg);
    crate::logs::global_buffer().push(msg);

    ndmc(&format!("interface {}", NDM_IF_NAME));

    if let Some((ip, mask)) = get_tun_ip_mask() {
        ndmc(&format!("interface {} ip address {} {}", NDM_IF_NAME, ip, mask));
    }

    ndmc(&format!("interface {} ip global auto", NDM_IF_NAME));
    ndmc(&format!("interface {} security-level public", NDM_IF_NAME));
    ndmc(&format!("interface {} up", NDM_IF_NAME));
}

fn set_ndm_default_routes() {
    if let Some((ip, _)) = get_tun_ip_mask() {
        ndmc(&format!("ip route default {} {}", ip, NDM_IF_NAME));
    } else {
        let msg = format!(
            "[routing] could not detect {} IPv4 address, skipping NDM default IPv4 route",
            NDM_IF_NAME
        );
        log::warn!("{}", msg);
        crate::logs::global_buffer().push(msg);
    }
    // IPv6 default route is harmless to request even when IPv6 is disabled.
    ndmc(&format!("ipv6 route default {}", NDM_IF_NAME));
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

/// Configure kernel routing and firewall after the VPN client creates tun0.
/// Renames tun0 → opkgtun0, adds default route, iptables FORWARD + MASQUERADE.
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

    log::info!("[routing] renaming {} → {}", TUN_NAME, OPKG_TUN_NAME);
    run_cmd("ip", &["link", "set", TUN_NAME, "down"])?;
    run_cmd("ip", &["link", "set", TUN_NAME, "name", OPKG_TUN_NAME])?;
    run_cmd("ip", &["link", "set", OPKG_TUN_NAME, "up"])?;

    // Register in NDM first — NDM may reconfigure iptables/routes on registration
    register_ndm_interface();

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
    set_ndm_default_routes();

    // Keep a kernel-route fallback for environments where NDM route update lags.
    let _ = run_cmd(
        "ip",
        &["route", "add", "default", "dev", OPKG_TUN_NAME, "metric", ROUTE_METRIC],
    );

    // iptables: allow forwarding LAN ↔ tunnel (br+ matches br0, br1, …)
    log::info!("[routing] configuring iptables forwarding + NAT");
    if let Err(e) = run_cmd(
        "iptables",
        &["-I", "FORWARD", "-i", "br+", "-o", OPKG_TUN_NAME, "-j", "ACCEPT"],
    ) {
        log::warn!("[routing] FORWARD br+→tunnel: {} (iptables installed?)", e);
    }
    let _ = run_cmd(
        "iptables",
        &[
            "-I", "FORWARD", "-i", OPKG_TUN_NAME, "-o", "br+",
            "-m", "state", "--state", "RELATED,ESTABLISHED", "-j", "ACCEPT",
        ],
    );
    let _ = run_cmd(
        "iptables",
        &["-t", "nat", "-A", "POSTROUTING", "-o", OPKG_TUN_NAME, "-j", "MASQUERADE"],
    );

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

fn route_uses_tunnel(ip: &str) -> bool {
    let out = match run_cmd("ip", &["route", "get", ip]) {
        Ok(v) => v,
        Err(_) => return false,
    };
    out.contains(&format!("dev {}", OPKG_TUN_NAME))
}

fn tcp_probe(ip: &str, port: u16) -> bool {
    let addr: SocketAddr = match format!("{}:{}", ip, port).parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    TcpStream::connect_timeout(&addr, CONNECTIVITY_TCP_TIMEOUT).is_ok()
}

pub fn check_connectivity() -> bool {
    if !route_uses_tunnel(CONNECTIVITY_ROUTE_PROBE_IP) {
        return false;
    }
    CONNECTIVITY_TCP_PROBES
        .iter()
        .any(|(ip, port)| tcp_probe(ip, *port))
}

// --------------- teardown ---------------

/// Remove firewall rules, routes, and the tunnel interface.
pub fn teardown_routing(server_addresses: &[String]) {
    log::info!("[routing] tearing down ...");

    run_cmd_ok(
        "iptables",
        &["-D", "FORWARD", "-i", "br+", "-o", OPKG_TUN_NAME, "-j", "ACCEPT"],
    );
    run_cmd_ok(
        "iptables",
        &[
            "-D", "FORWARD", "-i", OPKG_TUN_NAME, "-o", "br+",
            "-m", "state", "--state", "RELATED,ESTABLISHED", "-j", "ACCEPT",
        ],
    );
    run_cmd_ok(
        "iptables",
        &["-t", "nat", "-D", "POSTROUTING", "-o", OPKG_TUN_NAME, "-j", "MASQUERADE"],
    );

    run_cmd_ok(
        "ip",
        &["route", "del", "default", "dev", OPKG_TUN_NAME, "metric", ROUTE_METRIC],
    );

    for ip in extract_server_ips(server_addresses) {
        run_cmd_ok("ip", &["route", "del", &format!("{}/32", ip)]);
    }

    // Keep NDM interface object persistent across restarts so GUI metadata is stable.
    ndmc(&format!("interface {} down", NDM_IF_NAME));
    run_cmd_ok("ip", &["link", "del", OPKG_TUN_NAME]);

    log::info!("[routing] teardown complete");
    crate::logs::global_buffer().push("[routing] teardown complete".into());
}
