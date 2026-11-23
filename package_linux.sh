#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="ShotCraft"
PKG_NAME="shotcraft"
APP_VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
APP_SHORT_VER="$(echo "${APP_VERSION}" | awk -F. '{print $1"."$2}')"
DIST_DIR="$(pwd)/dist"
DEB_STAGE="${DIST_DIR}/deb_stage"
TAR_STAGE_NAME="${APP_NAME}-v${APP_SHORT_VER}-linux-x86_64"
TAR_STAGE="${DIST_DIR}/tar_stage/${TAR_STAGE_NAME}"
DEB_PATH="${DIST_DIR}/${APP_NAME}-v${APP_SHORT_VER}-linux-amd64.deb"
TAR_PATH="${DIST_DIR}/${APP_NAME}-v${APP_SHORT_VER}-linux-x86_64.tar.gz"

echo "==> Building Linux x86_64 release binary (v${APP_VERSION})..."
RUSTFLAGS="-D warnings" cargo build --release

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

cat > "${DEB_STAGE}/DEBIAN/control" <<CONTROL
Package: ${PKG_NAME}
Version: ${APP_VERSION}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Byaka_tris <d3n.1y0@gmail.com>
Depends: libc6, libgtk-3-0 | libgtk-3-0t64, libayatana-appindicator3-1 | libappindicator3-1, libxcb1, libgl1, libxkbcommon0
Description: ShotCraft - Lightweight screenshot and live screen-annotation app
 Fast cross-platform screenshot, annotation, and live screen-draw utility written in Rust.
CONTROL

echo "==> Building ${DEB_PATH}..."
dpkg-deb --build "${DEB_STAGE}" "${DEB_PATH}"

echo "==> Building ${TAR_PATH}..."
cp "target/release/shotcraft" "${TAR_STAGE}/${APP_NAME}"
chmod 755 "${TAR_STAGE}/${APP_NAME}"
cp "assets/app_icon.png" "${TAR_STAGE}/${PKG_NAME}.png"
cp "${DEB_STAGE}/usr/share/applications/${PKG_NAME}.desktop" "${TAR_STAGE}/${PKG_NAME}.desktop"
cat > "${TAR_STAGE}/install.sh" <<'INSTALL'
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons/hicolor/512x512/apps"
cp ShotCraft "$HOME/.local/bin/shotcraft"
chmod 755 "$HOME/.local/bin/shotcraft"
cp shotcraft.png "$HOME/.local/share/icons/hicolor/512x512/apps/shotcraft.png"
sed "s|Exec=shotcraft|Exec=$HOME/.local/bin/shotcraft|" shotcraft.desktop > "$HOME/.local/share/applications/shotcraft.desktop"
echo "Installed ShotCraft to $HOME/.local/bin/shotcraft"
INSTALL
chmod 755 "${TAR_STAGE}/install.sh"
tar -C "${DIST_DIR}/tar_stage" -czf "${TAR_PATH}" "${TAR_STAGE_NAME}"

rm -rf "${DEB_STAGE}" "${DIST_DIR}/tar_stage"
ls -lh "${DEB_PATH}" "${TAR_PATH}"
echo "==> Done!"
