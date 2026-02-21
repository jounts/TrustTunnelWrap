#!/bin/bash
# Build release binary for a target architecture using cross.
# Usage: ./build-release.sh [target-triple]
# Example: ./build-release.sh aarch64-unknown-linux-musl

set -euo pipefail

TARGET="${1:-x86_64-unknown-linux-musl}"

echo "Building for ${TARGET}..."
cross build --release --target "$TARGET"

BINARY="target/${TARGET}/release/trusttunnel-keenetic"
if [ -f "$BINARY" ]; then
    SIZE=$(stat -c%s "$BINARY" 2>/dev/null || stat -f%z "$BINARY")
    echo "Binary: ${BINARY} — $((SIZE / 1024)) KB"
else
    echo "ERROR: Binary not found at ${BINARY}"
    exit 1
fi
