use crate::config::{generate_client_toml, TunnelSettings};
use crate::logs;
use crate::routing;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CLIENT_BIN: &str = "/opt/bin/trusttunnel_client";
const CLIENT_TOML: &str = "/opt/etc/trusttunnel/trusttunnel_client.toml";

#[derive(Debug, Clone, Default)]
pub struct TunnelStatus {
    pub connected: bool,
    pub uptime_seconds: u64,
    pub last_error: String,
    pub pid: Option<u32>,
}

pub struct TunnelManager {
    settings: Mutex<TunnelSettings>,
    status: Mutex<TunnelStatus>,
    child: Mutex<Option<Child>>,
    running: AtomicBool,
    should_stop: AtomicBool,
    connect_time: Mutex<Option<Instant>>,
    routing_enabled: bool,
    routing_active: Arc<AtomicBool>,
    routing_setup_in_progress: Arc<AtomicBool>,
    // watchdog
    watchdog_enabled: bool,
    watchdog_interval: Duration,
    watchdog_max_failures: u32,
    watchdog_check_url: String,
    watchdog_check_timeout: Duration,
    watchdog_failures: AtomicU32,
    last_watchdog_check: Mutex<Instant>,
    last_wan_interface: Arc<Mutex<String>>,
}

impl TunnelManager {
    pub fn new(settings: TunnelSettings, routing: &crate::config::RoutingSettings) -> Arc<Self> {
        Arc::new(Self {
            settings: Mutex::new(settings),
            status: Mutex::new(TunnelStatus::default()),
            child: Mutex::new(None),
            running: AtomicBool::new(false),
            should_stop: AtomicBool::new(false),
            connect_time: Mutex::new(None),
            routing_enabled: routing.enabled,
            routing_active: Arc::new(AtomicBool::new(false)),
            routing_setup_in_progress: Arc::new(AtomicBool::new(false)),
            watchdog_enabled: routing.watchdog_enabled,
            watchdog_interval: Duration::from_secs(routing.watchdog_interval),
            watchdog_max_failures: routing.watchdog_failures,
            watchdog_check_url: routing.watchdog_check_url.clone(),
            watchdog_check_timeout: Duration::from_secs(routing.watchdog_check_timeout),
            watchdog_failures: AtomicU32::new(0),
            last_watchdog_check: Mutex::new(Instant::now()),
            last_wan_interface: Arc::new(Mutex::new(String::new())),
        })
    }

    pub fn get_status(&self) -> TunnelStatus {
        let mut st = self.status.lock().unwrap().clone();
        if let Some(t) = *self.connect_time.lock().unwrap() {
            if st.connected {
                st.uptime_seconds = t.elapsed().as_secs();
            }
        }
        st
    }

    pub fn update_settings(&self, new: TunnelSettings) {
        *self.settings.lock().unwrap() = new;
    }

    /// Write the TOML config to disk so trusttunnel_client can read it.
    fn write_toml_config(&self) -> Result<(), String> {
        let settings = self.settings.lock().unwrap().clone();
        let toml_content = generate_client_toml(&settings);

        if let Some(parent) = std::path::Path::new(CLIENT_TOML).parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir failed: {}", e))?;
        }
        std::fs::write(CLIENT_TOML, toml_content).map_err(|e| format!("write toml failed: {}", e))
    }

    fn spawn_process(&self) -> Result<(), String> {
        self.write_toml_config()?;

        let loglevel = self.settings.lock().unwrap().loglevel.clone();

        let child = Command::new(CLIENT_BIN)
            .arg("--config")
            .arg(CLIENT_TOML)
            .arg("--loglevel")
            .arg(&loglevel)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", CLIENT_BIN, e))?;

        let pid = child.id();
        *self.child.lock().unwrap() = Some(child);
        *self.connect_time.lock().unwrap() = Some(Instant::now());

        {
            let mut st = self.status.lock().unwrap();
            st.connected = true;
            st.pid = Some(pid);
            st.last_error.clear();
        }

        log::info!("Tunnel process started (PID: {})", pid);
        logs::global_buffer().push(format!("[tunnel] started PID {}", pid));
        Ok(())
    }

    pub fn start(&self) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let settings = self.settings.lock().unwrap();
        if settings.hostname.is_empty() || settings.addresses.is_empty() {
            return Err("Endpoint hostname and addresses are required".into());
        }
        drop(settings);

        self.should_stop.store(false, Ordering::SeqCst);
        if let Err(e) = self.spawn_process() {
            self.running.store(false, Ordering::SeqCst);
            self.should_stop.store(false, Ordering::SeqCst);
            return Err(e);
        }
        self.running.store(true, Ordering::SeqCst);
        self.spawn_routing_setup();
        Ok(())
    }

    fn spawn_routing_setup(&self) {
        if !self.routing_enabled {
            return;
        }
        if self
            .routing_setup_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            log::warn!("[routing] setup already in progress, skipping duplicate trigger");
            return;
        }
        let addresses = self.settings.lock().unwrap().addresses.clone();
        let flag = self.routing_active.clone();
        let wan_ref = self.last_wan_interface.clone();
        let in_progress = self.routing_setup_in_progress.clone();

        let spawn_result = std::thread::Builder::new()
            .name("routing-setup".into())
            .spawn(move || match routing::setup_routing(&addresses) {
                Ok(wan) => {
                    flag.store(true, Ordering::SeqCst);
                    *wan_ref.lock().unwrap() = wan;
                }
                Err(e) => log::error!("[routing] setup failed: {}", e),
            });
        match spawn_result {
            Ok(handle) => {
                std::thread::spawn(move || {
                    let _ = handle.join();
                    in_progress.store(false, Ordering::SeqCst);
                });
            }
            Err(e) => {
                in_progress.store(false, Ordering::SeqCst);
                log::error!("[routing] failed to spawn setup thread: {}", e);
            }
        }
    }

    fn teardown_if_active(&self) {
        if self.routing_active.swap(false, Ordering::SeqCst) {
            let addresses = self.settings.lock().unwrap().addresses.clone();
            routing::teardown_routing(&addresses);
        }
    }

    pub fn stop(&self) {
        self.should_stop.store(true, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        self.kill_child();
        self.teardown_if_active();
    }

    fn kill_child(&self) {
        let mut child_lock = self.child.lock().unwrap();
        if let Some(ref mut child) = *child_lock {
            let pid = child.id();
            log::info!("Stopping tunnel (PID: {})", pid);

            // Try graceful termination first on Unix.
            #[cfg(unix)]
            {
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(pid as i32),
                    nix::sys::signal::Signal::SIGTERM,
                );
            }

            // Wait up to 5 seconds
            for _ in 0..50 {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    _ => std::thread::sleep(Duration::from_millis(100)),
                }
            }

            // Force kill if still alive
            let _ = child.kill();
            let _ = child.wait();
        }
        *child_lock = None;

        let mut st = self.status.lock().unwrap();
        st.connected = false;
        st.pid = None;
        *self.connect_time.lock().unwrap() = None;

        logs::global_buffer().push("[tunnel] stopped".into());
    }

    pub fn restart(&self) -> Result<(), String> {
        self.stop();
        std::thread::sleep(Duration::from_secs(1));
        self.start()
    }

    fn respawn_with_delay(&self, reconnect_delay: u64) {
        self.teardown_if_active();

        log::info!("Reconnecting in {} seconds...", reconnect_delay);
        logs::global_buffer().push(format!("[tunnel] reconnecting in {}s...", reconnect_delay));
        std::thread::sleep(Duration::from_secs(reconnect_delay));

        if self.running.load(Ordering::SeqCst) && !self.should_stop.load(Ordering::SeqCst) {
            if let Err(e) = self.spawn_process() {
                log::error!("Respawn failed: {}", e);
                self.status.lock().unwrap().last_error = e;
            } else {
                self.spawn_routing_setup();
                self.watchdog_failures.store(0, Ordering::SeqCst);
            }
        }
    }

    fn full_restart(&self, reason: &str, reconnect_delay: u64) {
        let msg = format!("[watchdog] {}, restarting...", reason);
        log::warn!("{}", msg);
        logs::global_buffer().push(msg.clone());
        self.status.lock().unwrap().last_error = msg;

        self.kill_child();
        self.respawn_with_delay(reconnect_delay);
    }

    fn reroute(&self, new_wan: &str) {
        let msg = format!(
            "[watchdog] WAN changed → {}, updating server routes",
            new_wan
        );
        log::info!("{}", msg);
        logs::global_buffer().push(msg);

        let addresses = self.settings.lock().unwrap().addresses.clone();
        routing::reroute_server_via_wan(&addresses, new_wan);
        *self.last_wan_interface.lock().unwrap() = new_wan.to_string();
        self.watchdog_failures.store(0, Ordering::SeqCst);
    }

    fn run_watchdog_check(&self, reconnect_delay: u64) {
        if !self.watchdog_enabled || !self.routing_active.load(Ordering::SeqCst) {
            return;
        }

        let elapsed = self.last_watchdog_check.lock().unwrap().elapsed();
        if elapsed < self.watchdog_interval {
            return;
        }
        *self.last_watchdog_check.lock().unwrap() = Instant::now();

        if !routing::is_tun_alive() {
            self.full_restart("OpkgTun0 interface disappeared", reconnect_delay);
            return;
        }

        if let Some(current_wan) = routing::current_wan_interface() {
            let saved_wan = self.last_wan_interface.lock().unwrap().clone();
            if !saved_wan.is_empty() && current_wan != saved_wan {
                log::warn!("[watchdog] WAN changed: {} -> {}", saved_wan, current_wan);
                self.reroute(&current_wan);
                return;
            }
        }

        if !routing::check_connectivity(&self.watchdog_check_url, self.watchdog_check_timeout) {
            let fails = self.watchdog_failures.fetch_add(1, Ordering::SeqCst) + 1;
            log::warn!(
                "[watchdog] connectivity check failed ({}/{})",
                fails,
                self.watchdog_max_failures
            );
            if fails >= self.watchdog_max_failures {
                self.full_restart(
                    &format!("connectivity lost ({} failures)", fails),
                    reconnect_delay,
                );
            }
        } else {
            let prev = self.watchdog_failures.swap(0, Ordering::SeqCst);
            if prev > 0 {
                log::info!("[watchdog] connectivity restored");
            }
        }
    }

    /// Main monitoring loop -- call from a dedicated thread.
    /// Watches the child process, respawns on crash, and runs watchdog checks.
    pub fn monitor_loop(self: &Arc<Self>) {
        let reconnect_delay = self.settings.lock().unwrap().reconnect_delay;

        while !self.should_stop.load(Ordering::SeqCst) {
            if !self.running.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }

            // Check if child exited
            let exited = {
                let mut child_lock = self.child.lock().unwrap();
                if let Some(ref mut child) = *child_lock {
                    match child.try_wait() {
                        Ok(Some(exit)) => {
                            let msg =
                                format!("[tunnel] process exited: {}", exit.code().unwrap_or(-1));
                            log::warn!("{}", msg);
                            logs::global_buffer().push(msg.clone());
                            {
                                let mut st = self.status.lock().unwrap();
                                st.connected = false;
                                st.last_error = msg;
                                st.pid = None;
                            }
                            *child_lock = None;
                            true
                        }
                        Ok(None) => false,
                        Err(e) => {
                            log::error!("Error checking child: {}", e);
                            false
                        }
                    }
                } else {
                    true
                }
            };

            if exited
                && self.running.load(Ordering::SeqCst)
                && !self.should_stop.load(Ordering::SeqCst)
            {
                self.respawn_with_delay(reconnect_delay);
            } else if !exited {
                self.run_watchdog_check(reconnect_delay);
            }

            std::thread::sleep(Duration::from_millis(500));
        }
    }
}
