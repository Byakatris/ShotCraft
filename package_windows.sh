#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

APP_NAME="ShotCraft"
APP_VERSION="0.5.0"
DIST_DIR="$(pwd)/dist"
STAGE_DIR="${DIST_DIR}/win_stage"
INSTALLER_PATH="${DIST_DIR}/${APP_NAME}-v0.5-Setup.exe"

echo "==> Building Windows x86_64 release binary..."
RUSTFLAGS="-D warnings" cargo build --release --target x86_64-pc-windows-gnu

rm -rf "${STAGE_DIR}" "${INSTALLER_PATH}"
mkdir -p "${STAGE_DIR}"

cp "target/x86_64-pc-windows-gnu/release/snaply.exe" "${STAGE_DIR}/${APP_NAME}.exe"

echo "==> Generating ShotCraft.ico from assets/app_icon.png..."
python3 - <<'PY'
import io, struct
from PIL import Image

# Crop the 824x824 squircle from assets/app_icon.png so Windows icons fill the tile crisply
src = Image.open("assets/app_icon.png").convert("RGBA")
bbox = src.getbbox()
if bbox:
    src = src.crop(bbox)

sizes = [16, 32, 48, 256]
images = []
for sz in sizes:
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

cat > "${STAGE_DIR}/installer.nsi" <<NSI
Unicode True
!include "MUI2.nsh"

!define APP_NAME "${APP_NAME}"
!define APP_VERSION "${APP_VERSION}"
!define APP_EXE "${APP_NAME}.exe"
!define UNINST_KEY "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\\${APP_NAME}"

Name "\${APP_NAME}"
OutFile "${INSTALLER_PATH}"
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
  ; Close running instance cleanly before upgrading
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

echo "==> Compiling Windows NSIS installer..."
makensis -V2 "${STAGE_DIR}/installer.nsi"

rm -rf "${STAGE_DIR}"
ls -lh "${INSTALLER_PATH}"
echo "==> Done! ${INSTALLER_PATH}"
