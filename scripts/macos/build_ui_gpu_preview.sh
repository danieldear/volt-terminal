#!/usr/bin/env bash
# Build a separate real Volt preview. Never replace/install the released app.
set -euo pipefail
umask 077
OPEN=false
case "${1:-}" in
  --open) OPEN=true ;;
  --help|-h) echo "Usage: $0 [--open]"; exit 0 ;;
  '') ;;
  *) echo "Usage: $0 [--open]" >&2; exit 1 ;;
esac
[[ $# -le 1 ]] || { echo "Too many arguments" >&2; exit 1; }
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
BASE="$ROOT/target/macos-ui-gpu"
APP="$BASE/Volt UI GPU Preview.app"
if ps -axo command | awk -v bin="$APP/Contents/MacOS/Volt" '$0==bin || index($0,bin" ")==1 {found=1} END {exit !found}'; then
  echo "Close Volt UI GPU Preview before rebuilding it." >&2
  exit 1
fi
cargo build --locked --release -p volt --features ui-gpu-prototype
mkdir -p "$BASE" "$BASE/config/volt"
STAGE="$(mktemp -d "$BASE/stage.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/Contents/MacOS" "$STAGE/Contents/Resources/shell-integration"
cp target/release/volt "$STAGE/Contents/MacOS/Volt"
cp scripts/shell-integration/volt.* "$STAGE/Contents/Resources/shell-integration/"
cp assets/AppIcon.icns "$STAGE/Contents/Resources/Volt.icns"
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
cat > "$STAGE/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Volt UI GPU Preview</string>
<key>CFBundleDisplayName</key><string>Volt UI GPU Preview</string>
<key>CFBundleIdentifier</key><string>rs.volt.terminal.uigpupreview</string>
<key>CFBundleExecutable</key><string>Volt</string>
<key>LSEnvironment</key><dict>
<key>XDG_CONFIG_HOME</key><string>$BASE/config</string>
<key>VOLT_UI_GPU_SETTINGS</key><string>1</string>
</dict>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$VERSION</string>
<key>CFBundleVersion</key><string>$VERSION</string>
<key>CFBundleIconFile</key><string>Volt</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSMinimumSystemVersion</key><string>12.0</string>
</dict></plist>
PLIST
plutil -lint "$STAGE/Contents/Info.plist"
codesign --force --sign - "$STAGE"
codesign --verify --deep --strict "$STAGE"
# This ignored build output belongs only to this helper; no system install.
rm -rf "$APP"
mv "$STAGE" "$APP"
if [[ ! -e "$BASE/config/volt/config.toml" ]]; then
  cat > "$BASE/config/volt/config.toml" <<'CONFIG'
theme = "gruvbox"
[font]
family = "SF Mono"
size = 14.0
[shell]
integration = true
prompt = "meow"
[setup]
completed = true
[session]
restore = false
CONFIG
fi
chmod 600 "$BASE/config/volt/config.toml"
echo "Built real Volt preview: $APP"
echo "Development ad-hoc signature only; not a new notarized release."
echo "Preview settings are isolated at: $BASE/config/volt/config.toml"
if [[ "$OPEN" == true ]]; then
  open -n --env "XDG_CONFIG_HOME=$BASE/config" --env 'VOLT_UI_GPU_SETTINGS=1' \
    --stdout "$BASE/stdout.log" --stderr "$BASE/stderr.log" "$APP"
else
  printf 'To run: open -n --env %q --env VOLT_UI_GPU_SETTINGS=1 %q\n' "XDG_CONFIG_HOME=$BASE/config" "$APP"
fi
