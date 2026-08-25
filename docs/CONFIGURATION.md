# Configuration Reference

Main wrapper config path:

```text
/opt/etc/trusttunnel/config.json
```

If the configuration file is missing, the wrapper exits instead of silently starting
with defaults. The configuration and generated client TOML are written with `0600`
permissions because they contain passwords.

Default template is installed from:

```text
package/etc/trusttunnel/config.json
```

## Full Example

```json
{
  "tunnel": {
    "hostname": "",
    "custom_sni": "",
    "addresses": [],
    "username": "",
    "password": "",
    "upstream_protocol": "http2",
    "certificate": "",
    "skip_verification": false,
    "vpn_mode": "general",
    "dns_upstreams": ["tls://1.1.1.1"],
    "killswitch_enabled": false,
    "killswitch_allow_ports": [],
    "post_quantum_group_enabled": true,
    "exclusions": [],
    "included_routes": ["0.0.0.0/0", "2000::/3"],
    "excluded_routes": ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"],
    "mtu_size": 1280,
    "bound_if": "",
    "change_system_dns": false,
    "anti_dpi": false,
    "has_ipv6": true,
    "client_random": "",
    "socks_address": "",
    "socks_username": "",
    "socks_password": "",
    "reconnect_delay": 5,
    "loglevel": "info"
  },
  "webui": {
    "port": 8080,
    "bind": "0.0.0.0",
    "ndm_host": "",
    "ndm_port": 80
  },
  "logging": {
    "level": "info",
    "max_lines": 500,
    "file_enabled": true,
    "file_path": "/var/log/trusttunnel-keenetic/trusttunnel-keenetic.log",
    "rotate_size": "512KB",
    "rotate_keep": 1
  },
  "routing": {
    "enabled": true,
    "watchdog_enabled": true,
    "watchdog_interval": 30,
    "watchdog_failures": 3,
    "watchdog_check_url": "http://connectivitycheck.gstatic.com/generate_204",
    "watchdog_check_timeout": 5
  }
}
```

## `tunnel`

| Key | Type | Default | Description |
|---|---|---|---|
| `hostname` | string | `""` | Endpoint hostname (SNI) |
| `custom_sni` | string | `""` | Optional TLS SNI override, separate from endpoint hostname |
| `addresses` | string[] | `[]` | Endpoint addresses (`IP:port`) |
| `username` | string | `""` | Endpoint username |
| `password` | string | `""` | Endpoint password |
| `upstream_protocol` | string | `"http2"` | Upstream protocol (`http2`/`http3`) |
| `certificate` | string | `""` | Optional endpoint PEM certificate |
| `skip_verification` | bool | `false` | Skip TLS verification |
| `vpn_mode` | string | `"general"` | `general` or `selective` |
| `dns_upstreams` | string[] | `["tls://1.1.1.1"]` | DNS upstreams through VPN; generated in the client's `[endpoint]` section |
| `killswitch_enabled` | bool | `false` | Block traffic outside VPN |
| `killswitch_allow_ports` | number[] | `[]` | Allowed local ports while killswitch is active |
| `post_quantum_group_enabled` | bool | `true` | Enable post-quantum group negotiation |
| `exclusions` | string[] | `[]` | Domain/IP/CIDR exclusions |
| `included_routes` | string[] | `["0.0.0.0/0","2000::/3"]` | Routes sent via VPN |
| `excluded_routes` | string[] | `["10.0.0.0/8","172.16.0.0/12","192.168.0.0/16"]` | Routes bypassing VPN |
| `mtu_size` | number | `1280` | Tunnel MTU |
| `bound_if` | string | `""` | Outbound interface (`""` = auto) |
| `change_system_dns` | bool | `false` | Allow client to alter system DNS |
| `anti_dpi` | bool | `false` | Enable anti-DPI mode |
| `has_ipv6` | bool | `true` | Enable IPv6 routing via endpoint |
| `client_random` | string | `""` | TLS ClientHello random (`hex[/mask]`) |
| `socks_address` | string | `""` | Optional SOCKS5 address |
| `socks_username` | string | `""` | SOCKS5 username |
| `socks_password` | string | `""` | SOCKS5 password |
| `reconnect_delay` | number | `5` | Reconnect delay (seconds) |
| `loglevel` | string | `"info"` | Client log level |

## `webui`

| Key | Type | Default | Description |
|---|---|---|---|
| `port` | number | `8080` | HTTP listen port |
| `bind` | string | `"0.0.0.0"` | Bind address |
| `ndm_host` | string | `""` | NDM API host; auto-detected if empty |
| `ndm_port` | number | `80` | NDM API port |

## `logging`

| Key | Type | Default | Description |
|---|---|---|---|
| `level` | string | `"info"` | Wrapper log level |
| `max_lines` | number | `500` | Ring buffer size (lines) |
| `file_enabled` | bool | `true` | Enable file logging |
| `file_path` | string | `"/var/log/trusttunnel-keenetic/trusttunnel-keenetic.log"` | Log file path |
| `rotate_size` | string\|number | `"512KB"` | Rotation threshold (`1048576`, `512KB`, `10MB`, `1GB`) |
| `rotate_keep` | number | `1` | Number of rotated files to keep |

## `routing`

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Enable route updates via NDM on connect/disconnect |
| `watchdog_enabled` | bool | `true` | Enable tunnel watchdog |
| `watchdog_interval` | number | `30` | Health-check interval (seconds) |
| `watchdog_failures` | number | `3` | Failure threshold before restart |
| `watchdog_check_url` | string | `"http://connectivitycheck.gstatic.com/generate_204"` | Health-check URL |
| `watchdog_check_timeout` | number | `5` | Health-check timeout (seconds) |

## `geoip`

GeoIP subsystem used by split tunneling. Disabled by default; when enabled,
the wrapper downloads country databases, converts them into a compact binary
format (`v4.bin` / `v6.bin` under `db_path`) and answers IP→country lookups.

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable the GeoIP subsystem |
| `mode` | string | `"local"` | Lookup mode: `local` / `api` / `hybrid` |
| `db_providers` | object[] | `[]` | Downloadable database sources (see below) |
| `api_providers` | object[] | `[]` | Online lookup APIs (see below) |
| `trim_to_selected_countries` | bool | `true` | Keep only ranges of countries selected in `split_tunnel`; reduces DB size dramatically |
| `db_path` | string | `"/opt/etc/trusttunnel/geoip/"` | Directory for built databases. Put it on USB storage to reduce internal flash wear |
| `cache_ttl_hours` | number | `24` | TTL of API-lookup cache entries |
| `auto_update.enabled` | bool | `false` | Periodic background updates |
| `auto_update.interval_hours` | number | `168` | Update interval in hours |
| `auto_update.jitter_minutes` | number | `30` | Randomized initial delay to spread mirror load |
| `auto_update.on_startup_if_stale` | bool | `true` | Build the database at startup when none exists |
| `auto_update.max_age_hours_hard` | number | `720` | Hard-staleness threshold |

**db_providers[] fields:** `id` (unique string), `url` (http/https), `format`
(one of `mmdb`, `geolite2-csv`, `ip2location-csv`, `zone`), `priority`
(lower wins on overlapping data), `enabled`. Downloads support gzip and zip
containers transparently and use ETag/Last-Modified conditional requests.

The package ships with three keyless providers preconfigured; exactly one is
active at a time (single-select):

| id | Source | URL | Format |
|---|---|---|---|
| `geolite2-p3terx` | P3TERX/GeoLite.mmdb | `https://github.com/P3TERX/GeoLite.mmdb/raw/download/GeoLite2-Country.mmdb` | mmdb |
| `geolite2-wpstatistics` | wp-statistics via jsDelivr | `https://cdn.jsdelivr.net/npm/geolite2-country/GeoLite2-Country.mmdb.gz` | mmdb (gzip) |
| `ip2location-lite-db1` | lite.ip2location.com | `https://download.ip2location.com/lite/IP2LOCATION-LITE-DB1.CSV.ZIP` | ip2location-csv |

All three download directly without registration or API keys. Selecting a
provider in the WebUI (`POST /api/geoip/provider`) immediately downloads and
builds its database; the previous database stays in place until the new one
passes validation. Enabling split tunneling without an existing database
also triggers an automatic first build.

Note: wp-statistics previously published per-country `.zone` files; it now
distributes a single gzipped mmdb (listed above). The `zone` format remains
supported for other sources — set a two-letter country code inside the
provider id (e.g. `"ru-zone"`).

**api_providers[] fields:** `id`, `kind` (`ip2c`, `ipapi`, or
`generic-json` with an `{ip}` placeholder), `url`, optional `api_key`,
`priority`, `rate_limit_per_min`. API lookups happen only per new connection /
diagnostics request — never per packet — and are cached for `cache_ttl_hours`.
Providers that require credentials (non-empty mandatory `api_key`) are never
queried automatically on failure paths beyond their rate/error handling; the
lookup chain simply falls through to the next provider.

## `split_tunnel`

Policy-driven OS-level split tunneling. The client stays in `general` VPN
mode (or is switched to empty `selective` automatically); routing decisions
are made with `ipset` + iptables marks + policy routing, so NDM never sees
thousands of static routes. Requires `geoip.enabled`.

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable split tunneling |
| `policy` | string | `"tunnel_all_except"` | `tunnel_all_except`: everything through VPN except listed countries; `tunnel_only_listed`: only listed traffic enters the tunnel |
| `countries_bypass` | string[] | `[]` | ISO 3166-1 alpha-2 codes routed directly (`tunnel_all_except`) |
| `countries_tunnel` | string[] | `[]` | ISO codes routed through VPN (`tunnel_only_listed`) |
| `manual_bypass` | string[] | `[]` | Domain/IP/CIDR always direct (highest priority) |
| `manual_tunnel` | string[] | `[]` | Domain/IP/CIDR always via VPN (second priority) |
| `detection_mode` | string | `"local"` | Country detection source for diagnostics/API lookups: `local` / `api` / `hybrid` |

Rule priority (first match wins): manual bypass → manual tunnel → country
list → default direction of the base policy. Domain entries in manual rules
are resolved to IPs at apply time.

Requirements on Entware: `ipset` binary + kernel modules (`ip_set`,
`ip_set_hash_net`, `xt_set`), `iptables` mangle table. The wrapper verifies
these before applying a policy (`check_capabilities()`) and reports missing
pieces in the WebUI; while any of them is missing, policies are not applied.
On some Keenetic firmware versions `ipset`/kernel modules are not available
out of the box — install them from Entware first: `opkg install ipset iptables`.
Caveat: combine with `killswitch_enabled` carefully — killswitch blocks all
traffic outside the tun device, including split-tunnel "direct" flows.

A cron hook may `touch /opt/etc/trusttunnel/geoip/.update-request` to trigger
an out-of-schedule database update.

## Interface Names (Keenetic)

- Linux interface: `opkgtun0` (lowercase), visible in `ip link`.
- NDM interface: `OpkgTun0` (CamelCase), visible in `ndmc -c 'show interface'`.

## NDMS 5: DNS routes and package reinstall

On Keenetic/Netcraze (NDMS 5), **Routing → DNS routes** creates `object-group fqdn` entries and `dns-proxy` rules such as `route object-group … OpkgTun0 …`.

Package scripts **do not** edit `dns-proxy`. However, `opkg remove trusttunnel-keenetic` tears down the NDM interface (`no interface OpkgTun0`). The firmware may then **drop `dns-proxy` rules that reference a missing `OpkgTun0`**, while **domain lists (`object-group`) often remain**.

**You can restore those routes** — after installing the new package and starting the service, `OpkgTun0` comes back, but you must **re-attach** the DNS routes (via the web UI or CLI), or **restore** the relevant snippet from a saved `show running-config` / router backup.

**Tip:** before upgrading the package, save the `dns-proxy` block that references `OpkgTun0` (or the full `running-config`) so you can reapply the same lines after the upgrade.

## Related Docs

- Overview: [`OVERVIEW.md`](OVERVIEW.md)
- API: [`API.md`](API.md)
- Russian version: [`CONFIGURATION_RU.md`](CONFIGURATION_RU.md)
