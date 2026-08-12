# Changelog

All notable changes to this project are documented in this file.

## v0.1.2

### Fixed

- Keep tunnel monitor loop alive after runtime `disconnect`/`restart` so reconnect supervision continues on later `connect`.
- Add explicit `shutdown()` path for process termination and use it from signal handler.
- Drain child process `stdout` and `stderr` in background readers to prevent pipe-buffer stalls.
- Retry routing setup from watchdog when routing has not become active yet.
- Use the latest `reconnect_delay` from current settings for each respawn cycle.
- Make watchdog connectivity checks fail closed when `curl` is unavailable (to avoid false positives outside `opkgtun0`).

### Documentation

- Document NDMS 5 behavior: `dns-proxy` routes targeting `OpkgTun0` may be dropped after `opkg remove` while `object-group` lists remain; how to restore (`CONFIGURATION.md`, `CONFIGURATION_RU.md`).

### Changed

- Added `tunnel.custom_sni` support in wrapper config and generated `trusttunnel_client` TOML for compatibility with TrustTunnelClient `>= 1.0.3`.
- Endpoint address hostnames are now resolved for server host-route setup, preserving anti-loop routing behavior with TrustTunnelClient `>= 1.0.6`.
- Moved generated `dns_upstreams` into the client TOML `[endpoint]` section for TrustTunnelClient `v1.0.49`.
- Added TOML string escaping for endpoint credentials, SNI, routes, and listener settings.
- Updated CI, download scripts, and build documentation to use stable TrustTunnelClient `v1.0.49`.
