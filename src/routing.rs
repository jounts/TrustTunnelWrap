use std::process::Command;
use std::time::Duration;

const TUN_NAME: &str = "tun0";
const OPKG_TUN_NAME: &str = "opkgtun0";
const ROUTE_METRIC: &str = "500";
const TUN_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const TUN_POLL_INTERVAL: Duration = Duration::from_millis(500);

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
            return parts.get(i + 1).map(|s| s.to_string());
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

    log::info!(
        "[routing] default route via {} metric {}",
        OPKG_TUN_NAME,
        ROUTE_METRIC
    );
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

// --------------- watchdog helpers ---------------

pub fn is_tun_alive() -> bool {
    std::path::Path::new(&format!("/sys/class/net/{}", OPKG_TUN_NAME)).exists()
}

pub fn check_connectivity() -> bool {
    // Single ICMP ping through the tunnel interface, 5s timeout
    run_cmd(
        "ping",
        &["-c1", "-W5", "-I", OPKG_TUN_NAME, "1.1.1.1"],
    )
    .is_ok()
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

    run_cmd_ok("ip", &["link", "del", OPKG_TUN_NAME]);

    log::info!("[routing] teardown complete");
    crate::logs::global_buffer().push("[routing] teardown complete".into());
}
