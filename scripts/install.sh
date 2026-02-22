#!/bin/sh
# TrustTunnelWrap installer for Keenetic/Netcraze routers.
# Steps:
# 1) Detect router architecture
# 2) Download matching IPK from latest GitHub release
# 3) Backup old config + remove previous package if installed
# 4) Install new package and restore config backup

set -eu

REPO="jounts/TrustTunnelWrap"
PKG_NAME="trusttunnel-keenetic"
API_URL="https://api.github.com/repos/${REPO}/releases/latest"
TMP_IPK="/tmp/${PKG_NAME}.ipk"
CONFIG_PATH="/opt/etc/trusttunnel/config.json"
BACKUP_PATH=""

log() {
  echo "[install] $*"
}

err() {
  echo "[install] ERROR: $*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || err "Required command not found: $1"
}

http_get() {
  url="$1"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$url"
  else
    err "Neither curl nor wget is available"
  fi
}

download_file() {
  url="$1"
  out="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fL --retry 3 "$url" -o "$out"
  elif command -v wget >/dev/null 2>&1; then
    wget -O "$out" "$url"
  else
    err "Neither curl nor wget is available"
  fi
}

detect_pkg_arch() {
  machine="$(uname -m 2>/dev/null || echo unknown)"
  case "$machine" in
    aarch64|arm64) echo "aarch64-3.10" ;;
    x86_64|amd64) echo "x86_64-3.2" ;;
    armv7l|armv7*|armhf) echo "armv7-3.2" ;;
    mipsel|mipsel*|mips) echo "mipsel-3.4" ;;
    *)
      # Fallback: try to infer from opkg architecture list.
      if command -v opkg >/dev/null 2>&1; then
        arch="$(opkg print-architecture 2>/dev/null | awk '{print $2}' | grep -E '^(aarch64-3\.10|mipsel-3\.4|armv7-3\.2|x86_64-3\.2)$' | head -n1 || true)"
        [ -n "$arch" ] && echo "$arch" && return 0
      fi
      return 1
      ;;
  esac
}

find_latest_asset_url() {
  pkg_arch="$1"
  json="$(http_get "$API_URL")" || return 1

  # Parse browser_download_url from release JSON without jq.
  printf '%s\n' "$json" \
    | sed -n 's/.*"browser_download_url":[[:space:]]*"\([^"]*\)".*/\1/p' \
    | grep -E "/${PKG_NAME}_[^/]*_${pkg_arch}\.ipk$" \
    | head -n1
}

backup_existing_config() {
  if [ -f "$CONFIG_PATH" ]; then
    ts="$(date +%Y%m%d-%H%M%S 2>/dev/null || date +%s)"
    BACKUP_PATH="/tmp/${PKG_NAME}-config-${ts}.json"
    cp "$CONFIG_PATH" "$BACKUP_PATH"
    log "Config backup created: $BACKUP_PATH"
  else
    log "No existing config found at $CONFIG_PATH"
  fi
}

restore_config_if_needed() {
  if [ -z "${BACKUP_PATH}" ] || [ ! -f "$BACKUP_PATH" ]; then
    return 0
  fi

  mkdir -p "/opt/etc/trusttunnel"

  # If package did not create config for some reason, restore backup directly.
  if [ ! -f "$CONFIG_PATH" ]; then
    cp "$BACKUP_PATH" "$CONFIG_PATH"
    chmod 600 "$CONFIG_PATH" 2>/dev/null || true
    log "Config restored from backup (no new template found): $CONFIG_PATH"
    return 0
  fi

  # Preserve new directives: merge NEW config with OLD values.
  # jq expression `.[0] * .[1]` keeps keys from new template and overrides
  # existing ones with user values from backup.
  if command -v jq >/dev/null 2>&1; then
    merged="/tmp/${PKG_NAME}-config-merged.json"
    if jq -s '.[0] * .[1]' "$CONFIG_PATH" "$BACKUP_PATH" > "$merged"; then
      cp "$merged" "$CONFIG_PATH"
      rm -f "$merged"
      chmod 600 "$CONFIG_PATH" 2>/dev/null || true
      log "Config merged with backup (new directives preserved): $CONFIG_PATH"
      return 0
    fi
    log "WARNING: jq merge failed, keeping new config. Backup saved at: $BACKUP_PATH"
    return 0
  fi

  # Without jq we cannot safely deep-merge JSON in POSIX sh.
  # Keep new config to avoid dropping directives and leave backup for manual merge.
  log "WARNING: jq not found, keeping new config to preserve directives."
  log "WARNING: previous config backup kept at: $BACKUP_PATH"
}

main() {
  require_cmd opkg

  pkg_arch="$(detect_pkg_arch)" || err "Unsupported architecture: $(uname -m 2>/dev/null || echo unknown)"
  log "Detected architecture: $pkg_arch"

  asset_url="$(find_latest_asset_url "$pkg_arch")" || true
  [ -n "${asset_url:-}" ] || err "Could not find ${pkg_arch} package in latest release of ${REPO}"

  log "Latest package URL: $asset_url"
  log "Downloading package to $TMP_IPK ..."
  download_file "$asset_url" "$TMP_IPK"

  if opkg list-installed 2>/dev/null | grep -q "^${PKG_NAME}[[:space:]]"; then
    log "Existing installation found, creating backup and removing old package..."
    backup_existing_config
    opkg remove "$PKG_NAME"
  else
    log "No previous installation found"
  fi

  log "Installing package..."
  opkg install "$TMP_IPK"

  restore_config_if_needed
  log "Done."
}

main "$@"
