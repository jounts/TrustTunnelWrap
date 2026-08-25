//! Split tunneling manager: owns runtime state, applies policies on tunnel
//! connect/disconnect, and serves diagnostics.

pub mod firewall;
pub mod policy;

use crate::config::{effective_tunnel_settings, GeoIpSettings, SplitTunnelSettings};
use crate::geoip::db::GeoDb;
use std::net::ToSocketAddrs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

pub struct SplitTunnelManager {
    split_cfg: Mutex<SplitTunnelSettings>,
    geoip_cfg: Mutex<GeoIpSettings>,
    service: RwLock<Arc<crate::geoip::GeoService>>,
    active: AtomicBool,
    ipv6_enabled: AtomicBool,
    last_error: Mutex<String>,
}

impl SplitTunnelManager {
    pub fn new(
        split_cfg: SplitTunnelSettings,
        geoip_cfg: GeoIpSettings,
        has_ipv6: bool,
    ) -> Arc<Self> {
        let service = Self::build_service(&geoip_cfg);
        Arc::new(Self {
            split_cfg: Mutex::new(split_cfg),
            geoip_cfg: Mutex::new(geoip_cfg),
            service: RwLock::new(service),
            active: AtomicBool::new(false),
            ipv6_enabled: AtomicBool::new(has_ipv6),
            last_error: Mutex::new(String::new()),
        })
    }

    fn build_service(geoip_cfg: &GeoIpSettings) -> Arc<crate::geoip::GeoService> {
        if geoip_cfg.enabled_for_lookup() {
            Arc::new(crate::geoip::GeoService::new(geoip_cfg))
        } else {
            Arc::new(crate::geoip::GeoService::disabled())
        }
    }

    pub fn update_configs(
        &self,
        split_cfg: SplitTunnelSettings,
        geoip_cfg: GeoIpSettings,
        has_ipv6: bool,
    ) {
        self.ipv6_enabled.store(has_ipv6, Ordering::SeqCst);
        *self.split_cfg.lock().unwrap() = split_cfg;
        let rebuild_service = {
            let old = self.geoip_cfg.lock().unwrap();
            old.db_path != geoip_cfg.db_path
                || old.mode != geoip_cfg.mode
                || old.cache_ttl_hours != geoip_cfg.cache_ttl_hours
                || old.api_providers != geoip_cfg.api_providers
                || old.enabled_for_lookup() != geoip_cfg.enabled_for_lookup()
        };
        *self.geoip_cfg.lock().unwrap() = geoip_cfg;
        if rebuild_service {
            *self.service.write().unwrap() = Self::build_service(&self.geoip_cfg.lock().unwrap());
        }
    }

    /// Called after a successful database (re)build.
    pub fn reload_geoip(&self) {
        *self.service.write().unwrap() = Self::build_service(&self.geoip_cfg.lock().unwrap());
    }

    pub fn split_settings(&self) -> SplitTunnelSettings {
        self.split_cfg.lock().unwrap().clone()
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Applies the configured policy. Called after routing setup succeeds and
    /// from the WebUI "Apply" action.
    pub fn apply_now(&self) -> Result<(), String> {
        let st = self.split_cfg.lock().unwrap().clone();
        let geoip_cfg = self.geoip_cfg.lock().unwrap().clone();
        if !st.enabled {
            return Ok(());
        }
        if !geoip_cfg.enabled {
            return Err("split_tunnel.enabled requires geoip.enabled".into());
        }

        // Never apply an empty policy silently when there is no database yet.
        let countries = st.selected_countries();
        if !crate::geoip::is_db_present(&geoip_cfg) {
            let msg = "geoip database is not built yet; update it first".to_string();
            *self.last_error.lock().unwrap() = msg.clone();
            return Err(msg);
        }

        let problems = firewall::check_capabilities();
        if !problems.is_empty() {
            let msg = format!("split tunneling unavailable: {}", problems.join("; "));
            log::warn!("[split] {}", msg);
            *self.last_error.lock().unwrap() = msg.clone();
            return Err(msg);
        }

        let wan_if = crate::routing::current_wan_interface()
            .ok_or("no active WAN interface for bypass table")?;
        let ipv6 = self.ipv6_enabled.load(Ordering::SeqCst);

        // Ranges of the selected countries feed exactly one country set,
        // depending on the base policy direction.
        let (cc_v4, cc_v6) = load_country_cidrs(&geoip_cfg, &countries)?;
        let country_set4 = match st.policy.as_str() {
            "tunnel_only_listed" => "tt_cc_tunnel",
            _ => "tt_cc_bypass",
        };
        let country_set6 = format!("{}6", country_set4);

        let mut plan = policy::build_apply_plan(&st, &wan_if, ipv6);
        attach(
            &mut plan,
            "tt_ovr_bypass",
            resolve_targets_v4ish(&st.manual_bypass),
        );
        attach(
            &mut plan,
            "tt_ovr_tunnel",
            resolve_targets_v4ish(&st.manual_tunnel),
        );
        attach(&mut plan, country_set4, cc_v4);
        if ipv6 {
            attach(
                &mut plan,
                "tt_ovr_bypass6",
                resolve_targets_v6(&st.manual_bypass),
            );
            attach(
                &mut plan,
                "tt_ovr_tunnel6",
                resolve_targets_v6(&st.manual_tunnel),
            );
            attach(&mut plan, &country_set6, cc_v6);
        }

        firewall::execute(&plan)?;
        self.active.store(true, Ordering::SeqCst);
        *self.last_error.lock().unwrap() = String::new();
        log::info!(
            "[split] policy '{}' applied (countries {:?}, {} manual bypass, {} manual tunnel)",
            st.policy,
            countries,
            st.manual_bypass.len(),
            st.manual_tunnel.len()
        );
        Ok(())
    }

    /// Removes all split tunnel artifacts (best effort).
    pub fn teardown(&self) {
        self.active.store(false, Ordering::SeqCst);
        let ipv6 = self.ipv6_enabled.load(Ordering::SeqCst);
        for cmd in policy::build_teardown_plan(ipv6) {
            if let Err(e) = firewall::run_cmd(cmd.program, &cmd.args) {
                log::debug!("[split] teardown ignoring: {}", e);
            }
        }
    }

    /// Refreshes the direct-route table after a WAN interface change.
    pub fn refresh_wan(&self, new_wan: &str) {
        if !self.active.load(Ordering::SeqCst) {
            return;
        }
        let args: Vec<String> = [
            "route",
            "replace",
            "table",
            policy::TABLE_BYPASS,
            "default",
            "dev",
            new_wan,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        if let Err(e) = firewall::run_cmd("ip", &args) {
            log::warn!("[split] WAN refresh failed: {}", e);
        }
    }

    pub fn status_json(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled": self.split_cfg.lock().unwrap().enabled,
            "policy": self.split_cfg.lock().unwrap().policy,
            "active": self.is_active(),
            "last_error": self.last_error.lock().unwrap().clone(),
            "geoip": self.service.read().unwrap().status_json(),
        })
    }

    /// Diagnostics endpoint logic: resolves a target and explains the decision.
    pub fn test_target(&self, target: &str) -> Result<serde_json::Value, String> {
        let st = self.split_cfg.lock().unwrap().clone();
        let target_trimmed = target.trim();
        if target_trimmed.is_empty() {
            return Err("empty target".into());
        }
        let ips = resolve_target_ips(target_trimmed);
        if ips.is_empty() {
            return Err(format!("could not resolve '{}'", target_trimmed));
        }

        let manual_hit = |list: &[String]| -> bool {
            list.iter().any(|entry| {
                entry.eq_ignore_ascii_case(target_trimmed)
                    || ips.iter().any(|ip| policy::cidr_contains(entry, *ip))
            })
        };

        let service = self.service.read().unwrap();
        let mut results = Vec::new();
        for ip in &ips {
            let country = service
                .lookup(*ip)
                .map(|a| String::from_utf8_lossy(&a.cc).to_string());
            let upper = country.as_deref().map(|c| c.to_ascii_uppercase());
            let selected = upper
                .as_ref()
                .map(|c| st.countries_bypass.contains(c) || st.countries_tunnel.contains(c));
            let matched_rule = if manual_hit(&st.manual_bypass) {
                "manual_bypass"
            } else if manual_hit(&st.manual_tunnel) {
                "manual_tunnel"
            } else if selected == Some(true) {
                "country_list"
            } else {
                "default"
            };
            let route = policy::decide_route(
                &st,
                manual_hit(&st.manual_bypass),
                manual_hit(&st.manual_tunnel),
                selected,
            );
            results.push(serde_json::json!({
                "ip": ip.to_string(),
                "country": country,
                "matched_rule": matched_rule,
                "route": route,
            }));
        }
        Ok(serde_json::json!({ "target": target_trimmed, "results": results }))
    }
}

fn attach(plan: &mut policy::ApplyPlan, name: &str, cidrs: Vec<String>) {
    if cidrs.is_empty() {
        return;
    }
    if plan.sets.iter().any(|s| s.name == name) {
        plan.set_contents.push((name.to_string(), cidrs));
    }
}

/// Extracts the minimal CIDR lists covering the selected countries from the
/// compact databases on disk.
fn load_country_cidrs(
    geoip_cfg: &GeoIpSettings,
    countries: &[String],
) -> Result<(Vec<String>, Vec<String>), String> {
    let dir = std::path::PathBuf::from(geoip_cfg.db_path.trim_end_matches('/'));
    let wanted: Vec<[u8; 2]> = countries
        .iter()
        .filter_map(|c| c.as_bytes().get(..2).map(|b| [b[0], b[1]]))
        .collect();

    let mut v4_out = Vec::new();
    let mut v6_out = Vec::new();

    if let Ok(db) = GeoDb::load(&dir.join("v4.bin")) {
        for range in db.iter_ranges_v4() {
            if wanted.contains(&range.cc) {
                v4_out.extend(policy::range_to_cidrs(
                    range.start as u128,
                    range.end as u128,
                    true,
                ));
            }
        }
    }
    if let Ok(db) = GeoDb::load(&dir.join("v6.bin")) {
        for range in db.iter_ranges_v6() {
            if wanted.contains(&range.cc) {
                v6_out.extend(policy::range_to_cidrs(range.start, range.end, false));
            }
        }
    }

    if v4_out.len() + v6_out.len() > 500_000 {
        return Err(format!(
            "selected countries expand to {} CIDR blocks, which exceeds safe ipset limits",
            v4_out.len() + v6_out.len()
        ));
    }
    Ok((v4_out, v6_out))
}

/// Resolves each target; keeps IPv4 entries only.
fn resolve_targets_v4ish(targets: &[String]) -> Vec<String> {
    resolve_targets(targets)
        .into_iter()
        .filter(|s| !s.contains(':'))
        .collect()
}

fn resolve_targets_v6(targets: &[String]) -> Vec<String> {
    resolve_targets(targets)
        .into_iter()
        .filter(|s| s.contains(':'))
        .collect()
}

/// Resolves targets: IPs/CIDRs pass through, domains go through getaddrinfo.
fn resolve_targets(targets: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for t in targets {
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        if t.parse::<std::net::IpAddr>().is_ok() || crate::geoip::db::cidr_str_to_range(t).is_some()
        {
            out.push(t.to_string());
            continue;
        }
        if let Ok(addrs) = (t, 0u16).to_socket_addrs() {
            out.extend(addrs.map(|s| s.ip().to_string()));
        } else {
            log::warn!("[split] could not resolve manual target '{}'", t);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Resolves an arbitrary diagnostic target into IP addresses.
fn resolve_target_ips(target: &str) -> Vec<std::net::IpAddr> {
    if let Ok(ip) = target.parse::<std::net::IpAddr>() {
        return vec![ip];
    }
    if let Some((addr, _)) = target.rsplit_once('/') {
        if let Ok(ip) = addr.parse::<std::net::IpAddr>() {
            return vec![ip];
        }
    }
    (target, 0u16)
        .to_socket_addrs()
        .map(|it| it.map(|s| s.ip()).collect::<Vec<_>>())
        .unwrap_or_default()
}

/// Public helper used by TunnelManager when generating client TOML.
pub fn effective_client_settings(
    tunnel: &crate::config::TunnelSettings,
    mgr: &SplitTunnelManager,
) -> crate::config::TunnelSettings {
    effective_tunnel_settings(tunnel, &mgr.split_cfg.lock().unwrap())
}
