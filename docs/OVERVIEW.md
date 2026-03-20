# Overview

`trusttunnel-keenetic` is a router-oriented wrapper around `trusttunnel_client` for Keenetic/Netcraze devices with Entware.

## What It Provides

- Embedded Web UI on `:8080` by default
- Authentication through the router NDM API (same local router accounts)
- Runtime control (`connect`, `disconnect`, `restart`) via UI/API
- In-memory session handling (token TTL: 1 hour, extended on activity)
- Process supervision with reconnect logic and watchdog checks
- Route and interface management via NDM (`OpkgTun0`) and Linux interfaces (`opkgtun0`/`tun0`)
- Wrapper log ring buffer and optional file logging with rotation

## Architecture (High Level)

```text
trusttunnel-keenetic (Rust wrapper)
  |
  +- WebUI HTTP server (tiny_http)
  |    +- /api/login
  |    +- /api/status
  |    +- /api/config
  |    +- /api/control
  |    +- /api/logs
  |    \- /
  |
  \- Tunnel Manager
       +- Generates TOML for trusttunnel_client
       +- Starts/stops child process trusttunnel_client
       +- Monitors process and reconnects on failure
       +- Runs routing/watchdog checks
       \- Handles graceful shutdown
```

## Security Notes

- Web UI authorization uses router credentials through NDM API.
- Session tokens are memory-only and not persisted.
- Endpoint credentials are stored in `/opt/etc/trusttunnel/config.json`; keep permissions strict (`chmod 600`).
- If UI should not be remotely reachable, set `webui.bind` to `127.0.0.1`.

## Related Docs

- Configuration reference: [`CONFIGURATION.md`](CONFIGURATION.md)
- API reference: [`API.md`](API.md)
- Build and development: [`BUILDING.md`](BUILDING.md)
- Russian version: [`OVERVIEW_RU.md`](OVERVIEW_RU.md)
