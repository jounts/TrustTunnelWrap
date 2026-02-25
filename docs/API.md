# API Reference

HTTP API for `trusttunnel-keenetic`. By default, the server listens on `http://0.0.0.0:8080`.

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

## GET /

Returns embedded Web UI HTML.

---

## Error format

```json
{
  "error": "error message"
}
```
