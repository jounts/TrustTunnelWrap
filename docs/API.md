# API Reference

HTTP API for `trusttunnel-keenetic`. By default, the server listens on `http://0.0.0.0:8080`.

> WebUI uses plain HTTP without TLS. Session tokens are sent in cleartext, so port
> 8080 should be restricted with a firewall or used only on a trusted LAN.

Russian version: [`API_RU.md`](API_RU.md)

## Authentication

- `POST /api/login` and `GET /` are public.
- All other `/api/*` routes require:

```text
Authorization: <session-token>
```

Token is returned by `POST /api/login`. Session TTL is 1 hour and is refreshed on valid activity.

### Quick curl template

```sh
BASE_URL="http://192.168.1.1:8080"
TOKEN="<session-token>"
```

---

## POST /api/login

Authenticates against router NDM API (challenge-response).

### Request body

```json
{
  "login": "admin",
  "password": "secret"
}
```

### Responses

| Code | Meaning |
|-----|---------|
| 200 | Authorized |
| 400 | Invalid JSON or missing `login/password` |
| 401 | Invalid credentials |

### Success (200)

```json
{
  "token": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "status": "ok"
}
```

---

## GET /api/status

Returns current `trusttunnel_client` runtime status.

### Success (200)

```json
{
  "connected": true,
  "uptime_seconds": 3600,
  "last_error": "",
  "pid": 12345
}
```

---

## GET /api/config

Returns the current `tunnel` object from wrapper config.

---

## POST /api/config

Replaces the full `tunnel` block and saves config to disk.

Important: this is **not** a merge update. Send a full object compatible with `GET /api/config`.

### Responses

| Code | Meaning |
|-----|---------|
| 200 | Config updated |
| 400 | Invalid config JSON |
| 500 | Save error |

### Success (200)

```json
{
  "status": "updated"
}
```

---

## POST /api/control

Controls tunnel runtime state.

### Request body

```json
{
  "action": "connect"
}
```

### `action` values

| Value | Meaning |
|------|---------|
| `connect` | Start tunnel |
| `disconnect` | Stop tunnel |
| `restart` | Restart tunnel |

### Responses

| Code | Meaning |
|-----|---------|
| 200 | Action accepted |
| 400 | Invalid JSON, unknown action, or runtime start/restart error |

---

## GET /api/logs

Returns recent log lines from combined runtime sources.

### Query params

| Param | Type | Default | Limit |
|------|------|---------|-------|
| `limit` | number | `100` | max `500` |

### Success (200)

```json
{
  "lines": [
    "[tunnel] started PID 12345",
    "[routing] setup complete (WAN=eth0)"
  ],
  "total": 237
}
```

`total` is the current number of lines in wrapper in-memory buffer.

---

## GET /api/geoip/status

Returns GeoIP subsystem status: availability, active mode and database
metadata (provider, build date, record counts, on-disk size, trimmed
countries).

### Success (200)

```json
{
  "enabled": true,
  "policy": "tunnel_all_except",
  "active": true,
  "last_error": "",
  "geoip": {
    "available": true,
    "mode": "local",
    "meta": {
      "provider_id": "geolite2-country-mmdb",
      "format": "mmdb",
      "built_unix": 1724500000,
      "records_v4": 18342,
      "records_v6": 4210,
      "bytes_on_disk": 262144,
      "trimmed_countries": ["RU", "KZ"],
      "sanity_ok": true
    }
  },
  "config_enabled": true,
  "auto_update": { "enabled": true, "interval_hours": 168, "max_age_hours_hard": 720 }
}
```

## GET /api/geoip/providers

Returns the configured `db_providers` and `api_providers` arrays from the
`geoip` config section.

## POST /api/geoip/provider

Selects the active db provider (single-select: the chosen provider is
enabled, the rest are disabled) and immediately downloads/builds its
database. The config is persisted only after a successful build; on failure
the previous database remains active.

### Request body

```json
{ "id": "ip2location-lite-db1" }
```

### Success (200)

```json
{
  "status": "updated",
  "provider": "ip2location-lite-db1",
  "reports": [ { "provider_id": "ip2location-lite-db1", "...": "..." } ],
  "meta": { "provider_id": "ip2location-lite-db1", "...": "..." }
}
```

Errors return `400` (unknown id) or `500` (download/validation failed).

## POST /api/geoip/update

Triggers an immediate GeoIP database rebuild (all enabled providers). The
previous database remains in place until validation passes; on success the
lookup service reloads and an active split tunnel policy is re-applied.

### Request body (optional)

```json
{ "provider_id": "geolite2-country-mmdb" }
```

`provider_id` is reserved for per-provider updates; currently all enabled
providers are refreshed.

### Success (200)

```json
{
  "status": "updated",
  "reports": [
    { "provider_id": "geolite2-country-mmdb", "records_v4": 18342, "records_v6": 4210, "bytes_written": 262144, "elapsed_secs": 12.4 }
  ]
}
```

Errors return `500` with `{"error": "..."}`.

## GET /api/splittunnel/policy

Returns the current split tunnel policy plus live status.

```json
{
  "policy": "tunnel_all_except",
  "countries_bypass": ["RU", "KZ"],
  "countries_tunnel": [],
  "manual_bypass": ["example.local"],
  "manual_tunnel": [],
  "detection_mode": "local",
  "enabled": true,
  "status": { "active": true, "last_error": "", "...": "..." }
}
```

## POST /api/splittunnel/policy

Validates and saves a new policy, then hot-reapplies it without tearing the
tunnel down.

### Request body

```json
{
  "enabled": true,
  "policy": "tunnel_all_except",
  "countries_bypass": ["RU", "KZ"],
  "countries_tunnel": [],
  "manual_bypass": ["example.local", "192.168.50.0/24"],
  "manual_tunnel": ["some-blocked-service.com"],
  "detection_mode": "local"
}
```

### Responses

- `200 {"status":"applied"}` — saved and applied.
- `202 {"status":"saved","apply_error":"..."}` — saved; apply failed (e.g.
  missing `ipset`). Will be applied on next tunnel connect.
- `400` — validation error.

## POST /api/splittunnel/test

Resolves a target (IP or domain) and explains which route it would take.

### Request body

```json
{ "target": "ya.ru" }
```

### Success (200)

```json
{
  "target": "ya.ru",
  "results": [
    { "ip": "77.88.8.8", "country": "RU", "matched_rule": "country_list", "route": "direct" }
  ]
}
```

`route` is `"direct"` or `"tunnel"`; `matched_rule` is one of
`manual_bypass`, `manual_tunnel`, `country_list`, `default`.

---

## GET /

Returns embedded Web UI HTML.

The “Tunnel configuration” section provides two modes:

- **Manual** — the complete tunnel configuration form;
- **DeepLink** — paste a TrustTunnel `tt://?...` link and import its parameters into the manual form.

Import does not save the configuration automatically. Review the fields and click
“Save” to persist it. A DeepLink contains the endpoint username and password in
encoded but unencrypted form, so it must be treated as sensitive data.

---

## Error format

```json
{
  "error": "error message"
}
```
