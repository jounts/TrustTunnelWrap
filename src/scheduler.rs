//! Background scheduler for GeoIP database auto-updates.
//!
//! Runs a dedicated thread (same pattern as the tunnel monitor loop). A cron
//! hook can additionally `touch /opt/etc/trusttunnel/geoip/.update-request`
//! to request an immediate update; the flag file is consumed by this loop.

use crate::config::WrapperConfig;
use crate::split_tunnel::SplitTunnelManager;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const POLL_INTERVAL_SECS: u64 = 60;
const BACKOFF_BASE_SECS: u64 = 60;
const BACKOFF_MAX_SECS: u64 = 2 * 60 * 60;

/// Exponential cooldown after consecutive failed updates:
/// 1min, 2min, 4min, ... capped at 2h. Prevents hammering a dead mirror
/// every poll tick. Manual/cron requests are never delayed.
fn backoff_secs(consecutive_failures: u32) -> u64 {
    let shift = consecutive_failures.saturating_sub(1);
    BACKOFF_BASE_SECS
        .checked_shl(shift)
        .unwrap_or(BACKOFF_MAX_SECS)
        .min(BACKOFF_MAX_SECS)
}

pub fn spawn(config: Arc<Mutex<WrapperConfig>>, split: Arc<SplitTunnelManager>) {
    let spawn_result = std::thread::Builder::new()
        .name("geoip-scheduler".into())
        .spawn(move || scheduler_loop(config, split));
    if let Err(e) = spawn_result {
        log::error!("[geoip] failed to spawn scheduler thread: {}", e);
    }
}

fn scheduler_loop(config: Arc<Mutex<WrapperConfig>>, split: Arc<SplitTunnelManager>) {
    // Jittered initial delay to spread load on public mirrors.
    let (interval_hours, jitter) = {
        let cfg = config.lock().unwrap();
        (
            cfg.geoip.auto_update.interval_hours,
            cfg.geoip.auto_update.jitter_minutes,
        )
    };
    let jitter_secs = pseudo_random_jitter(jitter.max(1));
    log::info!(
        "[geoip] scheduler started (interval {}h, first check in ~{} min)",
        interval_hours,
        jitter_secs / 60
    );
    std::thread::sleep(Duration::from_secs(jitter_secs.min(300)));

    let mut consecutive_failures: u32 = 0;
    let mut next_auto_attempt: Option<std::time::Instant> = None;

    loop {
        let snapshot = config.lock().unwrap().clone();
        let geoip = &snapshot.geoip;
        let st = &snapshot.split_tunnel;

        let mut should_update = false;
        let mut reason = String::new();

        // Cron-style request file. Manual requests bypass the failure backoff.
        let request_file = format!("{}/.update-request", geoip.db_path.trim_end_matches('/'));
        if std::path::Path::new(&request_file).exists() {
            let _ = std::fs::remove_file(&request_file);
            should_update = true;
            reason.push_str("manual/cron request");
        }

        if !should_update && geoip.enabled && geoip.auto_update.enabled && st.enabled {
            if let Some(wait_until) = next_auto_attempt {
                if std::time::Instant::now() < wait_until {
                    log::debug!("[geoip] auto-update postponed by failure backoff");
                } else {
                    next_auto_attempt = None;
                }
            }
            if next_auto_attempt.is_none() {
                match staleness_hours(geoip) {
                    Some(age) => {
                        if age as u64 >= geoip.auto_update.interval_hours {
                            should_update = true;
                            reason = format!("database is {:.0}h old", age);
                        } else if age as u64 >= geoip.auto_update.max_age_hours_hard {
                            should_update = true;
                            reason = format!("database hard-stale at {:.0}h", age);
                        }
                    }
                    None => {
                        if geoip.auto_update.on_startup_if_stale {
                            should_update = true;
                            reason = "no database present".into();
                        }
                    }
                }
            }
        }

        if should_update {
            log::info!("[geoip] update triggered ({})", reason);
            match crate::geoip::rebuild_for_split_tunnel(geoip, st) {
                Ok(_) => {
                    consecutive_failures = 0;
                    next_auto_attempt = None;
                    split.reload_geoip();
                    logs_push("[geoip] database updated");
                    // Re-apply the active policy with fresh country data.
                    if split.is_active() {
                        if let Err(e) = split.apply_now() {
                            log::warn!("[geoip] policy re-apply after update failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    consecutive_failures += 1;
                    let wait = backoff_secs(consecutive_failures);
                    next_auto_attempt = Some(std::time::Instant::now() + Duration::from_secs(wait));
                    log::warn!(
                        "[geoip] scheduled update failed ({} in a row): {}; retrying in {}s",
                        consecutive_failures,
                        e,
                        wait
                    );
                    logs_push(&format!(
                        "[geoip] update failed: {} (retry in {}s)",
                        e, wait
                    ));
                }
            }
        }

        std::thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
    }
}

/// Database age in hours; None when there is no usable database.
fn staleness_hours(geoip: &crate::config::GeoIpSettings) -> Option<f64> {
    let meta =
        crate::geoip::db::DbMeta::load(std::path::Path::new(geoip.db_path.trim_end_matches('/')))?;
    let built = meta.built_unix;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as f64;
    Some((now - built as f64) / 3600.0)
}

/// Deterministic-but-varied jitter in [30, 30+jitter_minutes*60/2] seconds.
fn pseudo_random_jitter(jitter_minutes: u64) -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let span = (jitter_minutes * 60 / 2).max(1);
    30 + nanos % span
}

fn logs_push(msg: &str) {
    log::info!("{}", msg);
    crate::logs::global_buffer().push(msg.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_exponentially_and_caps() {
        assert_eq!(backoff_secs(1), 60);
        assert_eq!(backoff_secs(2), 120);
        assert_eq!(backoff_secs(3), 240);
        assert_eq!(backoff_secs(4), 480);
        // Cap at 2 hours regardless of the failure count.
        assert_eq!(backoff_secs(8), BACKOFF_MAX_SECS);
        assert_eq!(backoff_secs(100), BACKOFF_MAX_SECS);
    }
}
