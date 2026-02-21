#!/bin/bash
# Build an IPK package from compiled wrapper + pre-built client.
# Usage: ./package-ipk.sh <arch> <version> <wrapper_binary> <client_dir>

set -euo pipefail

ARCH="${1:?Usage: $0 <arch> <version> <wrapper_bin> <client_dir>}"
VERSION="${2:?}"
WRAPPER_BIN="${3:?}"
CLIENT_DIR="${4:?}"

PKG_NAME="trusttunnel-keenetic_${VERSION}_${ARCH}.ipk"
PKG_ROOT="ipk_root"

echo "Building IPK: ${PKG_NAME}"

# Clean
rm -rf "$PKG_ROOT" debian-binary control.tar.gz data.tar.gz

# Create directory structure
mkdir -p "$PKG_ROOT"/opt/bin
mkdir -p "$PKG_ROOT"/opt/etc/trusttunnel
mkdir -p "$PKG_ROOT"/CONTROL

# Copy wrapper binary
cp "$WRAPPER_BIN" "$PKG_ROOT/opt/bin/trusttunnel-keenetic"
chmod 755 "$PKG_ROOT/opt/bin/trusttunnel-keenetic"

# Copy TrustTunnelClient binaries
if [ -f "$CLIENT_DIR/trusttunnel_client" ]; then
    cp "$CLIENT_DIR/trusttunnel_client" "$PKG_ROOT/opt/bin/"
    chmod 755 "$PKG_ROOT/opt/bin/trusttunnel_client"
fi
if [ -f "$CLIENT_DIR/setup_wizard" ]; then
    cp "$CLIENT_DIR/setup_wizard" "$PKG_ROOT/opt/bin/"
    chmod 755 "$PKG_ROOT/opt/bin/setup_wizard"
fi

# Copy default config
cp package/etc/trusttunnel/config.json "$PKG_ROOT/opt/etc/trusttunnel/"

# Generate CONTROL/control
INSTALLED_SIZE=$(du -sk "$PKG_ROOT/opt" | cut -f1)
cat > "$PKG_ROOT/CONTROL/control" << EOF
Package: trusttunnel-keenetic
Version: ${VERSION}
Architecture: ${ARCH}
Maintainer: TrustTunnel Community
Section: net
Priority: optional
Description: TrustTunnel VPN wrapper for Keenetic/Netcraze routers
 Includes web management interface, NDM API auth, and auto-reconnect.
Homepage: https://github.com/TrustTunnel/TrustTunnelClient
Depends: libc
Installed-Size: ${INSTALLED_SIZE}
EOF

# Copy install scripts
cp package/CONTROL/postinst "$PKG_ROOT/CONTROL/"
cp package/CONTROL/prerm "$PKG_ROOT/CONTROL/"
cp package/CONTROL/conffiles "$PKG_ROOT/CONTROL/"
chmod 755 "$PKG_ROOT/CONTROL/postinst" "$PKG_ROOT/CONTROL/prerm"

# Build archives
(cd "$PKG_ROOT" && tar --numeric-owner --owner=0 --group=0 -czf ../data.tar.gz --exclude='CONTROL' .)
(cd "$PKG_ROOT/CONTROL" && tar --numeric-owner --owner=0 --group=0 -czf ../../control.tar.gz .)
echo "2.0" > debian-binary

# Assemble IPK (ar archive)
ar rc "$PKG_NAME" debian-binary control.tar.gz data.tar.gz

# Report
SIZE=$(stat -c%s "$PKG_NAME" 2>/dev/null || stat -f%z "$PKG_NAME")
echo "Package: ${PKG_NAME} — $((SIZE / 1024)) KB"

# Cleanup temp files
rm -rf "$PKG_ROOT" debian-binary control.tar.gz data.tar.gz

echo "Done."
