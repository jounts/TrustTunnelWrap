//! Pure policy compilation: split tunnel settings → ordered command plans.
//! No side effects, fully unit-testable without a router.

use crate::config::SplitTunnelSettings;

/// fwmark values ("TT" and "TT+1") and policy routing table ids.
pub const MARK_BYPASS: &str = "0x5454";
pub const MARK_TUNNEL: &str = "0x5455";
pub const TABLE_BYPASS: &str = "117"; // default via WAN (direct)
pub const TABLE_TUNNEL: &str = "118"; // default via opkgtun0
pub const CHAIN: &str = "TT_SPLIT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Inet,
    Inet6,
}

impl Family {
    pub fn ipset_family(self) -> &'static str {
        match self {
            Family::Inet => "inet",
            Family::Inet6 => "inet6",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SetSpec {
    pub name: String,
    #[allow(dead_code)]
    pub family: Family,
}

#[derive(Debug, Clone)]
pub struct Cmd {
    pub program: &'static str,
    pub args: Vec<String>,
    /// Best-effort commands (cleanup, idempotent creates) ignore failures.
    pub ignore_errors: bool,
}

impl Cmd {
    fn new(program: &'static str, args: &[&str]) -> Self {
        Self {
            program,
            args: args.iter().map(|s| s.to_string()).collect(),
            ignore_errors: false,
        }
    }

    fn best_effort(program: &'static str, args: &[&str]) -> Self {
        let mut c = Self::new(program, args);
        c.ignore_errors = true;
        c
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn display(&self) -> String {
        format!("{} {}", self.program, self.args.join(" "))
    }
}

#[derive(Debug, Default)]
pub struct ApplyPlan {
    pub sets: Vec<SetSpec>,
    pub cmds: Vec<Cmd>,
    pub set_contents: Vec<(String, Vec<String>)>, // (set name, cidrs)
}

/// Set names used by the policy engine.
pub fn set_names() -> [(String, Family); 8] {
    [
        ("tt_ovr_bypass".to_owned(), Family::Inet),
        ("tt_ovr_tunnel".to_owned(), Family::Inet),
        ("tt_cc_bypass".to_owned(), Family::Inet),
        ("tt_cc_tunnel".to_owned(), Family::Inet),
        ("tt_ovr_bypass6".to_owned(), Family::Inet6),
        ("tt_ovr_tunnel6".to_owned(), Family::Inet6),
        ("tt_cc_bypass6".to_owned(), Family::Inet6),
        ("tt_cc_tunnel6".to_owned(), Family::Inet6),
    ]
}

/// Compiles the active policy into an execution plan.
///
/// `wan_if` is the current WAN interface for the bypass table; `ipv6`
/// controls whether inet6 sets/rules are emitted.
pub fn build_apply_plan(st: &SplitTunnelSettings, wan_if: &str, ipv6: bool) -> ApplyPlan {
    let mut plan = ApplyPlan::default();

    // 1. Create all sets idempotently.
    let families: &[Family] = if ipv6 {
        &[Family::Inet, Family::Inet6]
    } else {
        &[Family::Inet]
    };
    for family in families {
        for (base, f) in set_names() {
            if f != *family {
                continue;
            }
            plan.sets.push(SetSpec {
                name: base.clone(),
                family: *family,
            });
            plan.cmds.push(Cmd::best_effort(
                "ipset",
                &[
                    "create",
                    &base,
                    "hash:net",
                    "family",
                    f.ipset_family(),
                    "-exist",
                ],
            ));
        }
    }

    // 2. Policy routing tables + rules.
    plan.cmds.push(Cmd::best_effort(
        "ip",
        &[
            "route",
            "replace",
            "table",
            TABLE_BYPASS,
            "default",
            "dev",
            wan_if,
        ],
    ));
    plan.cmds.push(Cmd::best_effort(
        "ip",
        &[
            "route",
            "replace",
            "table",
            TABLE_TUNNEL,
            "default",
            "dev",
            "opkgtun0",
        ],
    ));
    plan.cmds.push(Cmd::best_effort(
        "ip",
        &[
            "rule",
            "add",
            "priority",
            "5000",
            "fwmark",
            MARK_BYPASS,
            "table",
            TABLE_BYPASS,
        ],
    ));
    plan.cmds.push(Cmd::best_effort(
        "ip",
        &[
            "rule",
            "add",
            "priority",
            "5001",
            "fwmark",
            MARK_TUNNEL,
            "table",
            TABLE_TUNNEL,
        ],
    ));

    // 3. mangle chain.
    plan.cmds
        .push(Cmd::best_effort("iptables", &["-t", "mangle", "-N", CHAIN]));
    plan.cmds
        .push(Cmd::new("iptables", &["-t", "mangle", "-F", CHAIN]));
    ensure_jump(&mut plan, "PREROUTING");
    ensure_jump(&mut plan, "OUTPUT");

    add_mark_rule(&mut plan, "tt_ovr_bypass"); // priority 1: manual bypass
    add_mark_rule(&mut plan, "tt_ovr_tunnel"); // priority 2: manual tunnel

    // Priority 3: country sets according to the base policy.
    match st.policy.as_str() {
        "tunnel_only_listed" => add_mark_rule(&mut plan, "tt_cc_tunnel"),
        _ => add_mark_rule(&mut plan, "tt_cc_bypass"),
    }

    // Default: unmarked → follow the main routing table (NDM default).
    plan.cmds.push(Cmd::new(
        "iptables",
        &["-t", "mangle", "-A", CHAIN, "-j", "RETURN"],
    ));

    if ipv6 {
        plan.cmds.push(Cmd::best_effort(
            "ip6tables",
            &["-t", "mangle", "-N", CHAIN],
        ));
        plan.cmds
            .push(Cmd::new("ip6tables", &["-t", "mangle", "-F", CHAIN]));
        ensure_jump6(&mut plan, "PREROUTING");
        ensure_jump6(&mut plan, "OUTPUT");
        add_mark_rule6(&mut plan, "tt_ovr_bypass6");
        add_mark_rule6(&mut plan, "tt_ovr_tunnel6");
        match st.policy.as_str() {
            "tunnel_only_listed" => add_mark_rule6(&mut plan, "tt_cc_tunnel6"),
            _ => add_mark_rule6(&mut plan, "tt_cc_bypass6"),
        }
        plan.cmds.push(Cmd::new(
            "ip6tables",
            &["-t", "mangle", "-A", CHAIN, "-j", "RETURN"],
        ));
    }

    plan
}

fn ensure_jump(plan: &mut ApplyPlan, hook: &str) {
    plan.cmds.push(Cmd::best_effort(
        "iptables",
        &["-t", "mangle", "-C", hook, "-j", CHAIN],
    ));
    plan.cmds.push(Cmd::new(
        "iptables",
        &["-t", "mangle", "-I", hook, "1", "-j", CHAIN],
    ));
}

fn ensure_jump6(plan: &mut ApplyPlan, hook: &str) {
    plan.cmds.push(Cmd::best_effort(
        "ip6tables",
        &["-t", "mangle", "-C", hook, "-j", CHAIN],
    ));
    plan.cmds.push(Cmd::new(
        "ip6tables",
        &["-t", "mangle", "-I", hook, "1", "-j", CHAIN],
    ));
}

fn mark_for(set_base: &str) -> &'static str {
    if set_base.contains("bypass") {
        MARK_BYPASS
    } else {
        MARK_TUNNEL
    }
}

fn add_mark_rule(plan: &mut ApplyPlan, set_base: &str) {
    let mark = mark_for(set_base);
    plan.cmds.push(Cmd::new(
        "iptables",
        &[
            "-t",
            "mangle",
            "-A",
            CHAIN,
            "-m",
            "set",
            "--match-set",
            set_base,
            "dst",
            "-j",
            "MARK",
            "--set-xmark",
            &format!("{}/{}", mark, mark),
        ],
    ));
}

fn add_mark_rule6(plan: &mut ApplyPlan, set_name: &str) {
    let mark = mark_for(set_name);
    plan.cmds.push(Cmd::new(
        "ip6tables",
        &[
            "-t",
            "mangle",
            "-A",
            CHAIN,
            "-m",
            "set",
            "--match-set",
            set_name,
            "dst",
            "-j",
            "MARK",
            "--set-xmark",
            &format!("{}/{}", mark, mark),
        ],
    ));
}

/// Teardown plan: removes chains, jumps, rules and routes. All best-effort.
pub fn build_teardown_plan(ipv6: bool) -> Vec<Cmd> {
    let mut cmds = vec![
        Cmd::best_effort("iptables", &["-t", "mangle", "-D", "OUTPUT", "-j", CHAIN]),
        Cmd::best_effort(
            "iptables",
            &["-t", "mangle", "-D", "PREROUTING", "-j", CHAIN],
        ),
        Cmd::best_effort("iptables", &["-t", "mangle", "-F", CHAIN]),
        Cmd::best_effort("iptables", &["-t", "mangle", "-X", CHAIN]),
        Cmd::best_effort("ip", &["rule", "del", "priority", "5000"]),
        Cmd::best_effort("ip", &["rule", "del", "priority", "5001"]),
        Cmd::best_effort("ip", &["route", "flush", "table", TABLE_BYPASS]),
        Cmd::best_effort("ip", &["route", "flush", "table", TABLE_TUNNEL]),
    ];
    if ipv6 {
        cmds.extend([
            Cmd::best_effort("ip6tables", &["-t", "mangle", "-D", "OUTPUT", "-j", CHAIN]),
            Cmd::best_effort(
                "ip6tables",
                &["-t", "mangle", "-D", "PREROUTING", "-j", CHAIN],
            ),
            Cmd::best_effort("ip6tables", &["-t", "mangle", "-F", CHAIN]),
            Cmd::best_effort("ip6tables", &["-t", "mangle", "-X", CHAIN]),
            Cmd::best_effort("ip", &["-6", "rule", "del", "priority", "5000"]),
            Cmd::best_effort("ip", &["-6", "rule", "del", "priority", "5001"]),
            Cmd::best_effort("ip", &["-6", "route", "flush", "table", TABLE_BYPASS]),
            Cmd::best_effort("ip", &["-6", "route", "flush", "table", TABLE_TUNNEL]),
        ]);
    }
    cmds
}

/// Decision used by the diagnostics endpoint — mirrors the firewall rule order.
pub fn decide_route(
    st: &SplitTunnelSettings,
    manual_bypass_hit: bool,
    manual_tunnel_hit: bool,
    country_selected: Option<bool>,
) -> &'static str {
    if manual_bypass_hit {
        return "direct";
    }
    if manual_tunnel_hit {
        return "tunnel";
    }
    match country_selected {
        Some(selected) => {
            if st.policy == "tunnel_only_listed" {
                if selected {
                    "tunnel"
                } else {
                    "direct"
                }
            } else if selected {
                "direct"
            } else {
                "tunnel"
            }
        }
        // Unknown country falls through to the policy default.
        None => {
            if st.policy == "tunnel_only_listed" {
                "direct"
            } else {
                "tunnel"
            }
        }
    }
}

/// True when `ip` is inside `cidr` (both v4 or both v6).
pub fn cidr_contains(cidr: &str, ip: std::net::IpAddr) -> bool {
    match crate::geoip::db::cidr_str_to_range(cidr) {
        Some((_, s, e)) => {
            let val: u128 = match ip {
                std::net::IpAddr::V4(v4) => u32::from(v4) as u128,
                std::net::IpAddr::V6(v6) => u128::from(v6),
            };
            val >= s && val <= e
        }
        None => false,
    }
}

/// Converts an inclusive numeric range into the minimal list of CIDR blocks.
pub fn range_to_cidrs(start: u128, end: u128, is_v4: bool) -> Vec<String> {
    let width: u32 = if is_v4 { 32 } else { 128 };
    let mut out = Vec::new();
    let mut cur = start;
    while cur <= end {
        // Largest aligned block size at `cur`.
        let align = if cur == 0 {
            width
        } else {
            width - cur.trailing_zeros().min(width)
        };
        // Grow the prefix (smaller block) until it fits inside [cur, end].
        let mut prefix = align;
        while prefix < width {
            let block: u128 = 1u128 << (width - prefix);
            if cur.saturating_add(block - 1) <= end {
                break;
            }
            prefix += 1;
        }
        let block: u128 = 1u128 << (width - prefix);
        if is_v4 {
            let bytes = (cur as u32).to_be_bytes();
            out.push(format!(
                "{}.{}.{}.{}/{}",
                bytes[0], bytes[1], bytes[2], bytes[3], prefix
            ));
        } else {
            out.push(format!("{}/{}", std::net::Ipv6Addr::from_bits(cur), prefix));
        }
        match cur.checked_add(block) {
            Some(next) => cur = next,
            None => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st_default() -> SplitTunnelSettings {
        SplitTunnelSettings {
            enabled: true,
            policy: "tunnel_all_except".into(),
            countries_bypass: vec!["RU".into()],
            ..Default::default()
        }
    }

    #[test]
    fn plan_contains_core_elements() {
        let plan = build_apply_plan(&st_default(), "eth0", false);
        let text: Vec<String> = plan.cmds.iter().map(|c| c.display()).collect();
        assert!(text
            .iter()
            .any(|c| c.contains("create tt_cc_bypass hash:net family inet")));
        assert!(text
            .iter()
            .any(|c| c.contains("route replace table 117 default dev eth0")));
        assert!(text
            .iter()
            .any(|c| c.contains("rule add priority 5000 fwmark 0x5454 table 117")));
        assert!(text.iter().any(|c| c.contains("-t mangle -A TT_SPLIT -m set --match-set tt_ovr_bypass dst -j MARK --set-xmark 0x5454/0x5454")));
        assert!(text
            .iter()
            .any(|c| c.contains("--match-set tt_cc_bypass dst")));
        assert!(text
            .iter()
            .any(|c| c.ends_with("-t mangle -A TT_SPLIT -j RETURN")));
        // no v6 elements when ipv6 disabled
        assert!(!text.iter().any(|c| c.contains("ip6tables")
            || c.contains("inet6")
            || c.ends_with('6') && c.contains("match-set")));
    }

    #[test]
    fn plan_respects_policy_direction() {
        let mut st = st_default();
        st.policy = "tunnel_only_listed".into();
        st.countries_bypass.clear();
        st.countries_tunnel = vec!["DE".into()];
        let plan = build_apply_plan(&st, "eth0", true);
        let text: Vec<String> = plan.cmds.iter().map(|c| c.display()).collect();
        assert!(text
            .iter()
            .any(|c| c.contains("--match-set tt_cc_tunnel dst")));
        assert!(text.iter().any(|c| c.contains("family inet6")));
    }

    #[test]
    fn teardown_removes_marks() {
        let cmds = build_teardown_plan(true);
        let text: Vec<String> = cmds.iter().map(|c| c.display()).collect();
        assert!(text.iter().any(|c| c.contains("-t mangle -X TT_SPLIT")));
        assert!(text.iter().any(|c| c.contains("rule del priority 5001")));
        assert!(text.iter().any(|c| c.contains("-6 route flush table 117")));
    }

    #[test]
    fn route_decision_matrix() {
        let st = st_default(); // tunnel_all_except, RU bypassed
        assert_eq!(decide_route(&st, true, false, None), "direct"); // override wins
        assert_eq!(decide_route(&st, false, true, Some(true)), "tunnel"); // tunnel override beats country
        assert_eq!(decide_route(&st, false, false, Some(true)), "direct"); // country in bypass
        assert_eq!(decide_route(&st, false, false, Some(false)), "tunnel"); // other country
        assert_eq!(decide_route(&st, false, false, None), "tunnel"); // unknown → default

        let mut st2 = st_default();
        st2.policy = "tunnel_only_listed".into();
        assert_eq!(decide_route(&st2, false, false, Some(true)), "tunnel");
        assert_eq!(decide_route(&st2, false, false, Some(false)), "direct");
        assert_eq!(decide_route(&st2, false, false, None), "direct");
    }

    #[test]
    fn cidr_containment() {
        assert!(cidr_contains("10.0.0.0/8", "10.1.2.3".parse().unwrap()));
        assert!(!cidr_contains("10.0.0.0/8", "11.0.0.1".parse().unwrap()));
        assert!(cidr_contains(
            "2001:db8::/32",
            "2001:db8::1".parse().unwrap()
        ));
    }

    #[test]
    fn range_to_cidrs_minimal_blocks() {
        use crate::geoip::db::{cidr_range_v4, ipv4_to_u32};
        let (s, e) = cidr_range_v4("192.168.1.0".parse().unwrap(), 24).unwrap();
        let nets = range_to_cidrs(s as u128, e as u128, true);
        assert_eq!(nets, vec!["192.168.1.0/24"]);

        let (s, e) = cidr_range_v4("10.0.0.0".parse().unwrap(), 16).unwrap();
        assert_eq!(
            range_to_cidrs(s as u128, e as u128, true),
            vec!["10.0.0.0/16"]
        );

        // A /24 minus first and last addresses decomposes into several blocks;
        // verify full lossless coverage instead of an exact layout.
        let (s, e) = cidr_range_v4("10.0.0.0".parse().unwrap(), 24).unwrap();
        let nets = range_to_cidrs(s as u128 + 1, e as u128 - 1, true);
        let total_addrs: u64 = nets
            .iter()
            .map(|n| {
                let p: u8 = n.rsplit_once('/').unwrap().1.parse().unwrap();
                1u64 << (32 - p)
            })
            .sum();
        assert_eq!(total_addrs, 254);

        let v6 = range_to_cidrs(
            0x2001_0db8_0000_0000_0000_0000_0000_0000,
            0x2001_0db8_ffff_ffff_ffff_ffff_ffff_ffff,
            false,
        );
        assert_eq!(v6, vec!["2001:db8::/32"]);
    }
}
