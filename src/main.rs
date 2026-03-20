mod auth;
mod config;
mod logger;
mod logs;
mod routing;
mod tunnel;
mod webui;

use clap::Parser;
use config::WrapperConfig;
use std::sync::{Arc, Mutex};

const DEFAULT_CONFIG: &str = "/opt/etc/trusttunnel/config.json";

/// TrustTunnel VPN wrapper for Keenetic/Netcraze routers
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to wrapper config file (JSON)
    #[arg(short, long, default_value = DEFAULT_CONFIG)]
    config: String,

    /// Run as daemon (fork to background)
    #[arg(short, long)]
    daemon: bool,

    /// Run in foreground (overrides --daemon)
    #[arg(short, long)]
    foreground: bool,

    /// Test configuration and exit
    #[arg(short, long)]
    test: bool,
}

fn main() {
    let args = Args::parse();

    let cfg = match WrapperConfig::load(&args.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = logger::init(&cfg.logging) {
        eprintln!("Failed to initialize logger: {}", e);
        std::process::exit(1);
    }
    logs::init_global_buffer(cfg.logging.max_lines);

    log::info!(
        "trusttunnel-keenetic v{} starting",
        env!("TRUSTTUNNEL_VERSION")
    );

    if args.test {
        println!("Configuration OK");
        println!("Endpoint hostname: {}", cfg.tunnel.hostname);
        println!("Endpoint addresses: {}", cfg.tunnel.addresses.join(", "));
        println!("Protocol: {}", cfg.tunnel.upstream_protocol);
        println!("VPN mode: {}", cfg.tunnel.vpn_mode);
        println!("WebUI port: {}", cfg.webui.port);
        std::process::exit(0);
    }

    // Daemonize if requested
    if args.daemon && !args.foreground {
        daemonize();
    }

    // Shared config
    let config = Arc::new(Mutex::new(cfg.clone()));

    // Create tunnel manager
    let tunnel = tunnel::TunnelManager::new(cfg.tunnel.clone(), &cfg.routing);

    // Set up signal handlers
    let tunnel_for_signal = tunnel.clone();
    ctrlc_handler(tunnel_for_signal);

    // Start tunnel monitor in background thread
    let tunnel_monitor = tunnel.clone();
    std::thread::Builder::new()
        .name("tunnel-monitor".into())
        .spawn(move || {
            tunnel_monitor.monitor_loop();
        })
        .expect("failed to spawn tunnel monitor thread");

    // Auto-connect if endpoint is configured
    if !cfg.tunnel.hostname.is_empty() && !cfg.tunnel.addresses.is_empty() {
        if let Err(e) = tunnel.start() {
            log::warn!("Auto-connect failed: {}", e);
        }
    }

    // Start WebUI (blocks on the main thread)
    let ndm_host = if cfg.webui.ndm_host.is_empty() {
        auth::detect_ndm_host()
    } else {
        cfg.webui.ndm_host.clone()
    };
    log::info!("NDM API endpoint: {}:{}", ndm_host, cfg.webui.ndm_port);

    let web = webui::WebUI::new(tunnel, config, args.config, ndm_host, cfg.webui.ndm_port);
    web.run(&cfg.webui.bind, cfg.webui.port);
}

fn daemonize() {
    #[cfg(unix)]
    {
        use nix::unistd::{fork, setsid, ForkResult};

        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => {
                std::process::exit(0);
            }
            Ok(ForkResult::Child) => {
                let _ = setsid();
                let _ = nix::unistd::chdir("/");
                // Redirect stdio to /dev/null
                let devnull = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open("/dev/null");
                if let Ok(f) = devnull {
                    use std::os::unix::io::AsRawFd;
                    let fd = f.as_raw_fd();
                    let _ = nix::unistd::dup2(fd, 0);
                    let _ = nix::unistd::dup2(fd, 1);
                    let _ = nix::unistd::dup2(fd, 2);
                }
            }
            Err(e) => {
                log::error!("Fork failed: {}", e);
                std::process::exit(1);
            }
        }
    }

    #[cfg(not(unix))]
    {
        log::warn!("--daemon is only supported on Unix-like systems");
    }
}

fn ctrlc_handler(tunnel: Arc<tunnel::TunnelManager>) {
    #[cfg(unix)]
    {
        use nix::sys::signal::{pthread_sigmask, SigSet, SigmaskHow, Signal};

        // Block signals in the main thread before worker threads are started.
        // New threads inherit the mask; a dedicated waiter thread handles shutdown.
        let mut mask = SigSet::empty();
        mask.add(Signal::SIGTERM);
        mask.add(Signal::SIGINT);
        if let Err(e) = pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&mask), None) {
            log::error!("Failed to install signal mask: {}", e);
            return;
        }

        let _ = std::thread::Builder::new()
            .name("signal-handler".into())
            .spawn(move || loop {
                match mask.wait() {
                    Ok(sig) => {
                        log::info!("Received signal {:?}, shutting down...", sig);
                        tunnel.stop();
                        std::process::exit(0);
                    }
                    Err(_) => continue,
                }
            });
    }

    #[cfg(not(unix))]
    {
        let _ = tunnel;
    }
}
