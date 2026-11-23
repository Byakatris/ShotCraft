#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="ShotCraft"
APP_VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
APP_SHORT_VER="$(echo "${APP_VERSION}" | awk -F. '{print $1"."$2}')"
DIST_DIR="$(pwd)/dist"
STAGE_DIR="${DIST_DIR}/win_stage"
INSTALLER_NAME="${APP_NAME}-v${APP_SHORT_VER}-Setup.exe"
INSTALLER_PATH="${DIST_DIR}/${INSTALLER_NAME}"

UNAME_S="$(uname -s || echo Unknown)"
case "${UNAME_S}" in
  MINGW*|MSYS*|CYGWIN*)
    echo "==> Building Windows x86_64 release binary natively (v${APP_VERSION})..."
    RUSTFLAGS="-D warnings" cargo build --release
    BIN_PATH="target/release/shotcraft.exe"
    "${BIN_PATH}" --smoke-test
    ;;
  *)
    echo "==> Cross-compiling Windows x86_64 release binary (v${APP_VERSION})..."
    RUSTFLAGS="-D warnings" cargo build --release --target x86_64-pc-windows-gnu
    BIN_PATH="target/x86_64-pc-windows-gnu/release/shotcraft.exe"
    ;;
esac

rm -rf "${STAGE_DIR}" "${INSTALLER_PATH}"
mkdir -p "${STAGE_DIR}"

cp "${BIN_PATH}" "${STAGE_DIR}/${APP_NAME}.exe"

if [ -f "assets/app_icon.ico" ]; then
  echo "==> Using prebuilt assets/app_icon.ico..."
  cp "assets/app_icon.ico" "${STAGE_DIR}/ShotCraft.ico"
else
  echo "==> Generating ShotCraft.ico from assets/app_icon.png..."
  python3 - <<'PY'
import io, struct
from PIL import Image

src = Image.open("assets/app_icon.png").convert("RGBA")
bbox = src.getbbox()
if bbox:
    src = src.crop(bbox)

images = []
for sz in (16, 32, 48, 256):
    resized = src.resize((sz, sz), Image.Resampling.LANCZOS)
    bio = io.BytesIO()
    resized.save(bio, format="PNG", optimize=True)
    images.append((sz, bio.getvalue()))

header = struct.pack("<HHH", 0, 1, len(images))
offset = 6 + 16 * len(images)
entries = bytearray()
payload = bytearray()
for sz, png in images:
    dim = 0 if sz >= 256 else sz
    entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(png), offset)
    payload += png
    offset += len(png)

with open("dist/win_stage/ShotCraft.ico", "wb") as f:
    f.write(header + entries + payload)
PY
fi

cat > "${STAGE_DIR}/installer.nsi" <<NSI
Unicode True
!include "MUI2.nsh"

!define APP_NAME "${APP_NAME}"
!define APP_VERSION "${APP_VERSION}"
!define APP_EXE "${APP_NAME}.exe"
!define UNINST_KEY "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\\${APP_NAME}"

Name "\${APP_NAME}"
OutFile "../${INSTALLER_NAME}"
InstallDir "\$LOCALAPPDATA\\\${APP_NAME}"
InstallDirRegKey HKCU "\${UNINST_KEY}" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma

VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey "ProductName" "\${APP_NAME}"
VIAddVersionKey "FileDescription" "\${APP_NAME} Installer"
VIAddVersionKey "FileVersion" "\${APP_VERSION}"
VIAddVersionKey "ProductVersion" "\${APP_VERSION}"
VIAddVersionKey "CompanyName" "\${APP_NAME}"
VIAddVersionKey "LegalCopyright" "\${APP_NAME}"

!define MUI_ICON "ShotCraft.ico"
!define MUI_UNICON "ShotCraft.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "\$INSTDIR\\\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Launch \${APP_NAME}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Ukrainian"

Section "\${APP_NAME} (required)" SecMain
  SectionIn RO
  nsExec::ExecToStack 'taskkill /F /IM \${APP_EXE}'
  Pop \$0
  Pop \$1
  Sleep 250

  SetOutPath "\$INSTDIR"
  File "\${APP_EXE}"
  File "ShotCraft.ico"

  WriteUninstaller "\$INSTDIR\\Uninstall.exe"

  CreateDirectory "\$SMPROGRAMS\\\${APP_NAME}"
  CreateShortCut "\$SMPROGRAMS\\\${APP_NAME}\\\${APP_NAME}.lnk" "\$INSTDIR\\\${APP_EXE}" "" "\$INSTDIR\\ShotCraft.ico"
  CreateShortCut "\$SMPROGRAMS\\\${APP_NAME}\\Uninstall \${APP_NAME}.lnk" "\$INSTDIR\\Uninstall.exe" "" "\$INSTDIR\\ShotCraft.ico"

  WriteRegStr HKCU "\${UNINST_KEY}" "DisplayName" "\${APP_NAME}"
  WriteRegStr HKCU "\${UNINST_KEY}" "DisplayVersion" "\${APP_VERSION}"
  WriteRegStr HKCU "\${UNINST_KEY}" "Publisher" "\${APP_NAME}"
  WriteRegStr HKCU "\${UNINST_KEY}" "InstallLocation" "\$INSTDIR"
  WriteRegStr HKCU "\${UNINST_KEY}" "DisplayIcon" "\$INSTDIR\\ShotCraft.ico"
  WriteRegStr HKCU "\${UNINST_KEY}" "UninstallString" '"\$INSTDIR\\Uninstall.exe"'
  WriteRegDWORD HKCU "\${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "\${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Desktop Shortcut" SecDesktop
  CreateShortCut "\$DESKTOP\\\${APP_NAME}.lnk" "\$INSTDIR\\\${APP_EXE}" "" "\$INSTDIR\\ShotCraft.ico"
SectionEnd

Section "Start with Windows" SecStartup
  WriteRegStr HKCU "Software\\Microsoft\\Windows\\CurrentVersion\\Run" "\${APP_NAME}" '"\$INSTDIR\\\${APP_EXE}"'
SectionEnd

Section "Uninstall"
  nsExec::ExecToStack 'taskkill /F /IM \${APP_EXE}'
  Pop \$0
  Pop \$1
  Sleep 250

  Delete "\$INSTDIR\\\${APP_EXE}"
  Delete "\$INSTDIR\\ShotCraft.ico"
  Delete "\$INSTDIR\\Uninstall.exe"
  RMDir "\$INSTDIR"

  Delete "\$DESKTOP\\\${APP_NAME}.lnk"
  Delete "\$SMPROGRAMS\\\${APP_NAME}\\\${APP_NAME}.lnk"
  Delete "\$SMPROGRAMS\\\${APP_NAME}\\Uninstall \${APP_NAME}.lnk"
  RMDir "\$SMPROGRAMS\\\${APP_NAME}"

  DeleteRegValue HKCU "Software\\Microsoft\\Windows\\CurrentVersion\\Run" "\${APP_NAME}"
  DeleteRegKey HKCU "\${UNINST_KEY}"
SectionEnd
NSI

MAKENSIS_BIN="$(command -v makensis || true)"
if [ -z "${MAKENSIS_BIN}" ] && [ -x "/c/Program Files (x86)/NSIS/makensis.exe" ]; then
  MAKENSIS_BIN="/c/Program Files (x86)/NSIS/makensis.exe"
fi
if [ -z "${MAKENSIS_BIN}" ] && [ -x "/c/Program Files/NSIS/makensis.exe" ]; then
  MAKENSIS_BIN="/c/Program Files/NSIS/makensis.exe"
fi
if [ -z "${MAKENSIS_BIN}" ]; then
  echo "ERROR: makensis not found" >&2
  exit 1
fi

echo "==> Compiling Windows NSIS installer..."
(cd "${STAGE_DIR}" && "${MAKENSIS_BIN}" -V2 installer.nsi)

rm -rf "${STAGE_DIR}"
ls -lh "${INSTALLER_PATH}"
echo "==> Done! ${INSTALLER_PATH}"
