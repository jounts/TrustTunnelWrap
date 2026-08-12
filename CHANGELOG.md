# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Fixed

- Drain TrustTunnelClient output continuously so full stdout/stderr pipes cannot stall the client.
- Serialize tunnel lifecycle operations and wait for routing setup during shutdown.
- Remove tunnel default routes during routing teardown.
- Resolve hostname endpoint addresses when installing server host routes.
- Preserve meaningful tunnel defaults when loading partial configuration objects.
- Fail fast when the configured wrapper config file is missing.
- Validate MTU, reconnect, watchdog, protocol, VPN mode, port, and log buffer settings.
- Prevent UTF-8 boundary panics and escape TOML control characters correctly.
- Write wrapper and client configuration files atomically with restrictive permissions.

### Tests

- Add coverage for partial-config defaults, validation, TOML escaping, endpoint parsing, and UTF-8 output summaries.

### Documentation

- Warn that the WebUI remains plain HTTP on `0.0.0.0` and session tokens can be intercepted.

## v0.1.3

### Changed

- Add `tunnel.custom_sni` support for TrustTunnelClient `>= 1.0.3`.
- Update wrapper configuration and generated TOML for TrustTunnelClient `v1.0.49`.
- Move `dns_upstreams` into the client TOML `[endpoint]` section.
- Add TOML string escaping for endpoint credentials, SNI, routes, and listener settings.
- Update CI, client download scripts, and build documentation to use TrustTunnelClient `v1.0.49`.

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
