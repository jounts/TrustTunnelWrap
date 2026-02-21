fn main() {
    let version = std::env::var("TRUSTTUNNEL_VERSION")
        .unwrap_or_else(|_| std::env::var("CARGO_PKG_VERSION").unwrap());
    let version = version.trim_start_matches('v');
    println!("cargo:rustc-env=TRUSTTUNNEL_VERSION={}", version);
    println!("cargo:rerun-if-env-changed=TRUSTTUNNEL_VERSION");
}
