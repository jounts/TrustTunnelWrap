use crate::config::{generate_client_toml, TunnelSettings};
use crate::logs;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
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
}

impl TunnelManager {
    pub fn new(settings: TunnelSettings) -> Arc<Self> {
        Arc::new(Self {
            settings: Mutex::new(settings),
            status: Mutex::new(TunnelStatus::default()),
            child: Mutex::new(None),
            running: AtomicBool::new(false),
            should_stop: AtomicBool::new(false),
            connect_time: Mutex::new(None),
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
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir failed: {}", e))?;
        }
        std::fs::write(CLIENT_TOML, toml_content)
            .map_err(|e| format!("write toml failed: {}", e))
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
        self.running.store(true, Ordering::SeqCst);
        self.spawn_process()
    }

    pub fn stop(&self) {
        self.should_stop.store(true, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        self.kill_child();
    }

    fn kill_child(&self) {
        let mut child_lock = self.child.lock().unwrap();
        if let Some(ref mut child) = *child_lock {
            let pid = child.id();
            log::info!("Stopping tunnel (PID: {})", pid);

            // Try SIGTERM first
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGTERM,
            );

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

    /// Main monitoring loop -- call from a dedicated thread.
    /// Watches the child process and respawns on crash.
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
                            let msg = format!(
                                "[tunnel] process exited: {}",
                                exit.code().unwrap_or(-1)
                            );
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
                    // No child but running is true -- need to respawn
                    true
                }
            };

            if exited && self.running.load(Ordering::SeqCst) && !self.should_stop.load(Ordering::SeqCst) {
                log::info!("Reconnecting in {} seconds...", reconnect_delay);
                logs::global_buffer().push(format!(
                    "[tunnel] reconnecting in {}s...",
                    reconnect_delay
                ));
                std::thread::sleep(Duration::from_secs(reconnect_delay));

                if self.running.load(Ordering::SeqCst) && !self.should_stop.load(Ordering::SeqCst) {
                    if let Err(e) = self.spawn_process() {
                        log::error!("Respawn failed: {}", e);
                        self.status.lock().unwrap().last_error = e;
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(500));
        }
    }
}
