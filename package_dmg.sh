#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="ShotCraft"
APP_VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
APP_SHORT_VER="$(echo "${APP_VERSION}" | awk -F. '{print $1"."$2}')"
DIST_DIR="$(pwd)/dist"
APP_DIR="${DIST_DIR}/${APP_NAME}.app"
DMG_PATH="${DIST_DIR}/${APP_NAME}-v${APP_SHORT_VER}.dmg"
ICONSET_DIR="${DIST_DIR}/AppIcon.iconset"
STAGE_DIR="${DIST_DIR}/dmg_stage"

echo "==> Building release binary (v${APP_VERSION})..."
RUSTFLAGS="-D warnings" cargo build --release

rm -rf "${APP_DIR}" "${ICONSET_DIR}" "${STAGE_DIR}" "${DMG_PATH}"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources" "${ICONSET_DIR}" "${STAGE_DIR}"

cp "target/release/shotcraft" "${APP_DIR}/Contents/MacOS/${APP_NAME}"
chmod +x "${APP_DIR}/Contents/MacOS/${APP_NAME}"

echo "==> Generating AppIcon.icns from assets/app_icon.png..."
for sz in 16 32 128 256 512; do
  sips -z $sz $sz assets/app_icon.png --out "${ICONSET_DIR}/icon_${sz}x${sz}.png" >/dev/null
  sz2=$((sz * 2))
  sips -z $sz2 $sz2 assets/app_icon.png --out "${ICONSET_DIR}/icon_${sz}x${sz}@2x.png" >/dev/null
done
iconutil -c icns "${ICONSET_DIR}" -o "${APP_DIR}/Contents/Resources/AppIcon.icns"
rm -rf "${ICONSET_DIR}"

cat > "${APP_DIR}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>ShotCraft</string>
    <key>CFBundleDisplayName</key>
    <string>ShotCraft</string>
    <key>CFBundleIdentifier</key>
    <string>com.shotcraft.app</string>
    <key>CFBundleVersion</key>
    <string>${APP_VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${APP_VERSION}</string>
    <key>CFBundleExecutable</key>
    <string>ShotCraft</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSScreenCaptureUsageDescription</key>
    <string>ShotCraft needs Screen Recording permission to capture screenshots.</string>
</dict>
</plist>
PLIST

echo "==> Signing ShotCraft.app (ad-hoc)..."
codesign --force --deep --sign - "${APP_DIR}"

echo "==> Creating ShotCraft.dmg..."
cp -R "${APP_DIR}" "${STAGE_DIR}/"
ln -s /Applications "${STAGE_DIR}/Applications"
if diskutil image create from --srcfolder "${STAGE_DIR}" --volumeName "${APP_NAME}" "${DMG_PATH}" >/dev/null 2>&1; then
  :
else
  hdiutil create -volname "${APP_NAME}" -srcfolder "${STAGE_DIR}" -ov -format UDZO "${DMG_PATH}" >/dev/null
fi
rm -rf "${STAGE_DIR}"

ls -lh "${DMG_PATH}"
echo "==> Done! ${DMG_PATH}"
