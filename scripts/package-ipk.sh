#!/bin/bash
# Build an IPK package from compiled wrapper + pre-built client.
# Usage: ./package-ipk.sh <arch> <version> <wrapper_binary> <client_dir>

set -euo pipefail

TAR_FMT=""
_test_tar=$(mktemp)
if tar --format=gnu -cf "$_test_tar" --files-from /dev/null 2>/dev/null; then
    TAR_FMT="--format=gnu"
elif tar --format=gnutar -cf "$_test_tar" --files-from /dev/null 2>/dev/null; then
    TAR_FMT="--format=gnutar"
fi
rm -f "$_test_tar"

ARCH="${1:?Usage: $0 <arch> <version> <wrapper_bin> <client_dir>}"
VERSION="${2:?}"
VERSION="${VERSION#v}"
WRAPPER_BIN="${3:?}"
CLIENT_DIR="${4:?}"

case "$ARCH" in
    aarch64) OPKG_ARCH="aarch64-3.10" ;;
    mipsel)  OPKG_ARCH="mipsel-3.4" ;;
    armv7)   OPKG_ARCH="armv7-3.2" ;;
    x86_64)  OPKG_ARCH="x86_64-3.2" ;;
    *)       echo "Unknown arch: $ARCH"; exit 1 ;;
esac

PKG_NAME="trusttunnel-keenetic_${VERSION}_${OPKG_ARCH}.ipk"
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
else
    echo "WARNING: trusttunnel_client not found in $CLIENT_DIR — package will not be functional!" >&2
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
Architecture: ${OPKG_ARCH}
Maintainer: jounts
Section: net
Priority: optional
Description: TrustTunnel VPN wrapper for Keenetic/Netcraze routers
 Includes web management interface, NDM API auth, and auto-reconnect.
Homepage: https://github.com/jounts/TrustTunnelWrap
Depends: libc, iptables, jq
Installed-Size: ${INSTALLED_SIZE}
EOF

# Copy install scripts
cp package/CONTROL/postinst "$PKG_ROOT/CONTROL/"
cp package/CONTROL/prerm "$PKG_ROOT/CONTROL/"
cp package/CONTROL/postrm "$PKG_ROOT/CONTROL/"
cp package/CONTROL/conffiles "$PKG_ROOT/CONTROL/"
chmod 755 "$PKG_ROOT/CONTROL/postinst" "$PKG_ROOT/CONTROL/prerm" "$PKG_ROOT/CONTROL/postrm"

# Build archives (Entware uses tar.gz outer format, NOT ar like standard OpenWrt)
WORK="$(mktemp -d)"
(cd "$PKG_ROOT" && tar $TAR_FMT --numeric-owner --owner=0 --group=0 -czf "$WORK/data.tar.gz" --exclude='CONTROL' .)
(cd "$PKG_ROOT/CONTROL" && tar $TAR_FMT --numeric-owner --owner=0 --group=0 -czf "$WORK/control.tar.gz" .)
printf "2.0\n" > "$WORK/debian-binary"

cd "$WORK"
tar $TAR_FMT --numeric-owner --owner=0 --group=0 \
    -czf "$OLDPWD/$PKG_NAME" ./debian-binary ./control.tar.gz ./data.tar.gz
cd "$OLDPWD"

# Report
SIZE=$(stat -c%s "$PKG_NAME" 2>/dev/null || stat -f%z "$PKG_NAME")
echo "Package: ${PKG_NAME} — $((SIZE / 1024)) KB"

# Cleanup
rm -rf "$PKG_ROOT" "$WORK"

echo "Done."
