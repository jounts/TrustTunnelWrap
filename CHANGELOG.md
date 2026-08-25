# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Added

- Add GeoIP subsystem with a compact binary country database (`v4.bin`/`v6.bin`), local parsers (GeoLite2 mmdb/CSV, IP2Location LITE CSV, per-country zone lists) and optional online API lookups (ip2c.org, ip2c.org-style generic JSON, ipapi.com) with TTL cache and rate limiting.
- Ship three keyless db providers in the default configuration (P3TERX GeoLite.mmdb, wp-statistics mmdb.gz via jsDelivr, IP2Location LITE DB1 CSV); selecting a provider (`POST /api/geoip/provider`, single-select) immediately downloads and builds its database.
- Automatically build the GeoIP database when split tunneling is enabled without an existing one; refuse to apply policies on a missing database instead of silently installing empty rulesets.
- Add OS-level split tunneling driven by GeoIP countries: `tunnel_all_except` and `tunnel_only_listed` policies compiled into ipset/iptables-mark/policy-routing rules without touching NDM static routes; manual bypass/tunnel rules take precedence over country lists.
- Add automatic database updates (ETag/Last-Modified conditional downloads, gzip/zip transparent decompression, atomic replacement with sanity checks and rollback) plus a cron request-file hook for out-of-schedule refreshes.
- Add WebUI endpoints `/api/geoip/status|providers|update`, `/api/splittunnel/policy`, `/api/splittunnel/test` and a "Split Tunneling" tab with policy editing, database status and route diagnostics.
- Add Manual and DeepLink modes to the WebUI tunnel configuration section.
- Import TrustTunnel `tt://?...` links into the manual configuration form without automatic saving.

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
- Prefer `.csv`/`.mmdb`/`.zone` entries over license/readme files when extracting GeoIP data from zip archives.
- Apply an exponential backoff (1 min → 2 h cap) to scheduled GeoIP updates after consecutive download failures; manual/cron requests are never delayed.
- Send `If-Modified-Since` for servers that only provide `Last-Modified`, so conditional downloads work without ETag.
- Cap downloaded GeoIP source size at 32 MB to protect low-RAM devices from oversized responses.

### Tests

- Add coverage for partial-config defaults, validation, TOML escaping, endpoint parsing, UTF-8 output summaries, the compact GeoIP binary format (round-trip lookup, range merging, priority on overlap), CSV/zone parsers, policy compilation (fwmark/ipset plans, CIDR expansion, route decision matrix), API rate limiting and config validation of the new `geoip`/`split_tunnel` sections.

### Documentation

- Document `geoip` and `split_tunnel` configuration sections, requirements (ipset/iptables on Entware, including the explicit `opkg install ipset iptables` step for firmware without out-of-the-box ipset), provider mirrors and killswitch interaction in CONFIGURATION.md/CONFIGURATION_RU.md.
- Document new GeoIP and split tunneling API endpoints in API.md/API_RU.md.
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
