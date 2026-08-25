//! Firewall/routing execution helpers (ipset, iptables, ip rule).
//! Thin wrappers over `std::process::Command` with logging.

use super::policy::{ApplyPlan, Cmd};
use std::io::Write;
use std::process::Command;

pub fn run_cmd(program: &str, args: &[String]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{}: {}", program, e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{} {:?}: {}", program, args, stderr.trim()))
    }
}

fn run_best_effort(cmd: &Cmd) {
    match run_cmd(cmd.program, &cmd.args) {
        Ok(_) => {}
        Err(e) => {
            if cmd.ignore_errors {
                log::debug!("[split] ignoring: {}", e);
            } else {
                log::warn!("[split] command failed: {}", e);
            }
        }
    }
}

/// Executes an ipset restore batch from a generated script.
fn ipset_restore(script: &str) -> Result<(), String> {
    let mut child = Command::new("ipset")
        .arg("restore")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("ipset restore: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(script.as_bytes())
            .map_err(|e| format!("ipset restore write: {}", e))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("ipset restore wait: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "ipset restore failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// Verifies that the required binaries and kernel support are present.
/// Returns a list of problems; empty list means go.
pub fn check_capabilities() -> Vec<String> {
    let mut problems = Vec::new();
    for bin in ["ipset", "iptables", "ip"] {
        if Command::new(bin).arg("--version").output().is_err() {
            problems.push(format!("binary '{}' not found in PATH", bin));
        }
    }
    // Kernel-side probes: these fail when ipset/xt_set modules are missing.
    if run_cmd("ipset", &["list".to_string(), "-name".to_string()]).is_err() {
        problems.push("kernel ip_set support not available ('ipset list' failed)".into());
    }
    if run_cmd(
        "iptables",
        &[
            "-t".to_string(),
            "mangle".to_string(),
            "-L".to_string(),
            "-n".to_string(),
        ],
    )
    .is_err()
    {
        problems.push("iptables mangle table not available".into());
    }
    problems
}

/// Applies the plan: executes commands and bulk-fills sets via `ipset restore`.
pub fn execute(plan: &ApplyPlan) -> Result<(), String> {
    for cmd in &plan.cmds {
        run_best_effort(cmd);
    }

    // Bulk-fill set contents.
    if !plan.set_contents.is_empty() {
        let mut script = String::with_capacity(64 * 1024);
        for (name, cidrs) in &plan.set_contents {
            script.push_str(&format!("flush {}\n", name));
            for cidr in cidrs {
                script.push_str(&format!("add {} {} -exist\n", name, cidr));
            }
        }
        ipset_restore(&script)?;
    }
    Ok(())
}
