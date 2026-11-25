#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="ShotCraft"
PKG_NAME="shotcraft"
APP_VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
DIST_DIR="$(pwd)/dist"
DEB_STAGE="${DIST_DIR}/deb_stage"
TAR_STAGE_NAME="${APP_NAME}-v${APP_VERSION}-linux-x86_64"
TAR_STAGE="${DIST_DIR}/tar_stage/${TAR_STAGE_NAME}"
DEB_PATH="${DIST_DIR}/${APP_NAME}-v${APP_VERSION}-linux-amd64.deb"
TAR_PATH="${DIST_DIR}/${APP_NAME}-v${APP_VERSION}-linux-x86_64.tar.gz"

echo "==> Building Linux x86_64 release binary (v${APP_VERSION})..."
cargo build --release

rm -rf "${DEB_STAGE}" "${DIST_DIR}/tar_stage" "${DEB_PATH}" "${TAR_PATH}"
mkdir -p \
  "${DEB_STAGE}/DEBIAN" \
  "${DEB_STAGE}/usr/bin" \
  "${DEB_STAGE}/usr/share/applications" \
  "${DEB_STAGE}/usr/share/icons/hicolor/512x512/apps" \
  "${TAR_STAGE}"

cp "target/release/shotcraft" "${DEB_STAGE}/usr/bin/${PKG_NAME}"
chmod 755 "${DEB_STAGE}/usr/bin/${PKG_NAME}"

cp "assets/app_icon.png" "${DEB_STAGE}/usr/share/icons/hicolor/512x512/apps/${PKG_NAME}.png"

cat > "${DEB_STAGE}/usr/share/applications/${PKG_NAME}.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=${APP_NAME}
Comment=Lightweight screenshot and live screen-annotation app
Exec=${PKG_NAME}
Icon=${PKG_NAME}
Terminal=false
Categories=Graphics;Utility;
DESKTOP

INSTALLED_KB="$(du -sk "${DEB_STAGE}/usr" | awk '{print $1}')"

cat > "${DEB_STAGE}/DEBIAN/control" <<CONTROL
Package: ${PKG_NAME}
Version: ${APP_VERSION}
Section: utils
Priority: optional
Architecture: amd64
Installed-Size: ${INSTALLED_KB}
Maintainer: Byaka_tris <d3n.1y0@gmail.com>
Depends: libc6, libgtk-3-0 | libgtk-3-0t64, libayatana-appindicator3-1 | libappindicator3-1, libxcb1, libxcb-shm0, libxcb-randr0, libegl1, libgl1, libx11-6, libxdo3, libxkbcommon0, libdbus-1-3, libpipewire-0.3-0, curl
Recommends: xdg-utils, libnotify-bin, zenity | kdialog
Description: ShotCraft - Lightweight screenshot and live screen-annotation app
 Fast cross-platform screenshot, annotation, and live screen-draw utility written in Rust.
CONTROL

echo "==> Building ${DEB_PATH}..."
dpkg-deb --root-owner-group --build "${DEB_STAGE}" "${DEB_PATH}"

echo "==> Building ${TAR_PATH}..."
cp "target/release/shotcraft" "${TAR_STAGE}/${APP_NAME}"
chmod 755 "${TAR_STAGE}/${APP_NAME}"
cp "assets/app_icon.png" "${TAR_STAGE}/${PKG_NAME}.png"
cat > "${TAR_STAGE}/install.sh" <<'INSTALL'
#!/bin/sh
set -eu
cd "$(dirname "$0")"
TARGET_BIN="${SHOTCRAFT_INSTALL_BIN:-$HOME/.local/bin/shotcraft}"
TARGET_DIR="$(dirname "$TARGET_BIN")"
mkdir -p "$TARGET_DIR" "$HOME/.local/share/applications" "$HOME/.local/share/icons/hicolor/512x512/apps"
install -m 755 ShotCraft "${TARGET_BIN}.tmp"
mv -f "${TARGET_BIN}.tmp" "$TARGET_BIN"
install -m 644 shotcraft.png "$HOME/.local/share/icons/hicolor/512x512/apps/shotcraft.png"
cat > "$HOME/.local/share/applications/shotcraft.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=ShotCraft
Comment=Lightweight screenshot and live screen-annotation app
Exec="$TARGET_BIN"
Icon=shotcraft
Terminal=false
Categories=Graphics;Utility;
DESKTOP
echo "Installed ShotCraft to $TARGET_BIN"
INSTALL
chmod 755 "${TAR_STAGE}/install.sh"
tar -C "${DIST_DIR}/tar_stage" -czf "${TAR_PATH}" "${TAR_STAGE_NAME}"

rm -rf "${DEB_STAGE}" "${DIST_DIR}/tar_stage"
ls -lh "${DEB_PATH}" "${TAR_PATH}"
echo "==> Done!"
