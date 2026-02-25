# Configuration Reference

Main wrapper config path:

```text
/opt/etc/trusttunnel/config.json
```

Default template is installed from:

```text
package/etc/trusttunnel/config.json
```

## Full Example

```json
{
  "tunnel": {
    "hostname": "",
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
| `addresses` | string[] | `[]` | Endpoint addresses (`IP:port`) |
| `username` | string | `""` | Endpoint username |
| `password` | string | `""` | Endpoint password |
| `upstream_protocol` | string | `"http2"` | Upstream protocol (`http2`/`http3`) |
| `certificate` | string | `""` | Optional endpoint PEM certificate |
| `skip_verification` | bool | `false` | Skip TLS verification |
| `vpn_mode` | string | `"general"` | `general` or `selective` |
| `dns_upstreams` | string[] | `["tls://1.1.1.1"]` | DNS upstreams through VPN |
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

## Interface Names (Keenetic)

- Linux interface: `opkgtun0` (lowercase), visible in `ip link`.
- NDM interface: `OpkgTun0` (CamelCase), visible in `ndmc -c 'show interface'`.

## Related Docs

- Overview: [`OVERVIEW.md`](OVERVIEW.md)
- API: [`API.md`](API.md)
- Russian version: [`CONFIGURATION_RU.md`](CONFIGURATION_RU.md)
