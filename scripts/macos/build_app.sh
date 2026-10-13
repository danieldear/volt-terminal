#!/usr/bin/env bash

set -euo pipefail

MODE="release"
OPEN_AFTER_BUILD="false"
INSTALL_TO_USER_APPS="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --debug)
      MODE="debug"
      shift
      ;;
    --release)
      MODE="release"
      shift
      ;;
    --open)
      OPEN_AFTER_BUILD="true"
      shift
      ;;
    --install)
      INSTALL_TO_USER_APPS="true"
      shift
      ;;
    *)
      echo "Unknown option: $1" >&2
      echo "Usage: $0 [--debug|--release] [--open] [--install]" >&2
      exit 1
      ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${ROOT_DIR}"
APP_DIR="${ROOT_DIR}/target/macos/Volt.app"
USER_APPS_DIR="${HOME}/Applications"
INSTALL_APP="${USER_APPS_DIR}/Volt.app"

ensure_closed() {
  local bin="$1/Contents/MacOS/Volt"
  if ps -axo command | awk -v bin="$bin" '$0==bin || index($0,bin" ")==1 {found=1} END {exit !found}'; then
    echo "Close Volt before replacing its running bundle: $1" >&2
    exit 1
  fi
}
ensure_closed "${APP_DIR}"
if [[ "${INSTALL_TO_USER_APPS}" == "true" ]]; then
  ensure_closed "${INSTALL_APP}"
  if [[ -e "${INSTALL_APP}" ]]; then
    ID="$(plutil -extract CFBundleIdentifier raw "${INSTALL_APP}/Contents/Info.plist")"
    [[ "${ID}" == rs.volt.terminal ]] || {
      echo "Refusing to replace an unrelated app: ${INSTALL_APP}" >&2
      exit 1
    }
  fi
fi

if [[ "$MODE" == "release" ]]; then
  BUILD_ARGS=(build --locked --release -p volt)
  BIN_PATH="${ROOT_DIR}/target/release/volt"
else
  BUILD_ARGS=(build --locked -p volt)
  BIN_PATH="${ROOT_DIR}/target/debug/volt"
fi

echo "Building Volt (${MODE})..."
cargo "${BUILD_ARGS[@]}"

if [[ ! -x "${BIN_PATH}" ]]; then
  echo "Built binary not found at: ${BIN_PATH}" >&2
  exit 1
fi

VERSION="$(
  awk -F'=' '
    /^\[workspace\.package\]/ { in_workspace = 1; next }
    in_workspace && /^version/ {
      gsub(/[ "]/, "", $2)
      print $2
      exit
    }
  ' "${ROOT_DIR}/Cargo.toml"
)"

if [[ -z "${VERSION}" ]]; then
  VERSION="0.1.0"
fi

mkdir -p "${ROOT_DIR}/target/macos"
STAGE="$(mktemp -d "${ROOT_DIR}/target/macos/stage.XXXXXX")"
INSTALL_STAGE=""
trap 'rm -rf "${STAGE}"; if [[ -n "${INSTALL_STAGE}" ]]; then rm -rf "${INSTALL_STAGE}"; fi' EXIT
STAGED_APP="${STAGE}/Volt.app"
CONTENTS_DIR="${STAGED_APP}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"
PLIST_PATH="${CONTENTS_DIR}/Info.plist"

echo "Creating app bundle at ${APP_DIR}..."
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}"
mkdir -p "${RESOURCES_DIR}/shell-integration"
cp "${ROOT_DIR}/scripts/shell-integration/volt."* "${RESOURCES_DIR}/shell-integration/"

cp "${BIN_PATH}" "${MACOS_DIR}/Volt"
chmod +x "${MACOS_DIR}/Volt"

ICON_KEY=""
ICON_SOURCE=""
if [[ -f "${ROOT_DIR}/assets/Volt.icns" ]]; then
  ICON_SOURCE="${ROOT_DIR}/assets/Volt.icns"
elif [[ -f "${ROOT_DIR}/assets/AppIcon.icns" ]]; then
  ICON_SOURCE="${ROOT_DIR}/assets/AppIcon.icns"
fi
if [[ -n "${ICON_SOURCE}" ]]; then
  cp "${ICON_SOURCE}" "${RESOURCES_DIR}/Volt.icns"
  ICON_KEY=$'  <key>CFBundleIconFile</key>\n  <string>Volt</string>'
fi

cat > "${PLIST_PATH}" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>Volt</string>
  <key>CFBundleDisplayName</key>
  <string>Volt</string>
  <key>CFBundleIdentifier</key>
  <string>rs.volt.terminal</string>
  <key>CFBundleExecutable</key>
  <string>Volt</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
${ICON_KEY}
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

bash "${SCRIPT_DIR}/sign_app.sh" "${STAGED_APP}" adhoc
ensure_closed "${APP_DIR}"
rm -rf "${APP_DIR}"
mv "${STAGED_APP}" "${APP_DIR}"
LAUNCH_APP="${APP_DIR}"

if [[ "${INSTALL_TO_USER_APPS}" == "true" ]]; then
  mkdir -p "${USER_APPS_DIR}"
  INSTALL_STAGE="$(mktemp -d "${USER_APPS_DIR}/.volt-install.XXXXXX")"
  ditto "${APP_DIR}" "${INSTALL_STAGE}/Volt.app"
  codesign --verify --deep --strict "${INSTALL_STAGE}/Volt.app"
  ensure_closed "${INSTALL_APP}"
  BACKUP=""
  if [[ -e "${INSTALL_APP}" ]]; then
    mkdir -p "${ROOT_DIR}/target/macos/install-backups"
    BACKUP="$(mktemp -d "${ROOT_DIR}/target/macos/install-backups/previous.XXXXXX")"
    mv "${INSTALL_APP}" "${BACKUP}/Volt.app"
  fi
  if ! mv "${INSTALL_STAGE}/Volt.app" "${INSTALL_APP}"; then
    if [[ -n "${BACKUP}" ]]; then mv "${BACKUP}/Volt.app" "${INSTALL_APP}"; fi
    exit 1
  fi
  LAUNCH_APP="${INSTALL_APP}"
  echo "Installed to ${INSTALL_APP}"
  if [[ -n "${BACKUP}" ]]; then echo "Previous app saved at ${BACKUP}"; fi
fi

echo "Done: ${APP_DIR}"
echo "Open from Finder to launch Volt directly as a GUI app (no Terminal window)."

if [[ "${OPEN_AFTER_BUILD}" == "true" ]]; then
  open "${LAUNCH_APP}"
fi
