# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Fixed

- Keep tunnel monitor loop alive after runtime `disconnect`/`restart` so reconnect supervision continues on later `connect`.
- Add explicit `shutdown()` path for process termination and use it from signal handler.
- Drain child process `stdout` and `stderr` in background readers to prevent pipe-buffer stalls.
- Retry routing setup from watchdog when routing has not become active yet.
- Use the latest `reconnect_delay` from current settings for each respawn cycle.
- Make watchdog connectivity checks fail closed when `curl` is unavailable (to avoid false positives outside `opkgtun0`).
