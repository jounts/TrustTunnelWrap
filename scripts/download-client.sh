#!/bin/bash
# Download pre-built trusttunnel_client from GitHub Releases.
# Usage: ./download-client.sh <version_tag> <arch> <output_dir>
# Example: ./download-client.sh v0.99.105 linux-aarch64 client_bin

set -euo pipefail

VERSION="${1:?Usage: $0 <version> <arch> <output_dir>}"
ARCH="${2:?}"
OUTPUT_DIR="${3:?}"

REPO="TrustTunnel/TrustTunnelClient"
ASSET_NAME="trusttunnel_client-${VERSION}-${ARCH}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET_NAME}"

echo "Downloading ${ASSET_NAME}..."
mkdir -p "$OUTPUT_DIR"

curl -fSL --retry 3 "$URL" -o "/tmp/${ASSET_NAME}"
tar -xzf "/tmp/${ASSET_NAME}" -C "$OUTPUT_DIR"
rm -f "/tmp/${ASSET_NAME}"

# Ensure binaries are executable
chmod +x "$OUTPUT_DIR"/trusttunnel_client 2>/dev/null || true
chmod +x "$OUTPUT_DIR"/setup_wizard 2>/dev/null || true

echo "Downloaded to ${OUTPUT_DIR}:"
ls -lh "$OUTPUT_DIR"/
