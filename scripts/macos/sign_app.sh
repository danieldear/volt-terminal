#!/bin/bash
# Sign an already-built app. Never silently downgrade a requested trust level.
set +x
set -euo pipefail
app=${1:?usage: sign_app.sh APP adhoc|developer-id|notarized}
mode=${2:?usage: sign_app.sh APP adhoc|developer-id|notarized}
[[ -x "$app/Contents/MacOS/Volt" ]] || { echo 'Missing Volt executable' >&2; exit 1; }
/usr/bin/plutil -lint "$app/Contents/Info.plist"
args=()
if [[ -n "${SIGNING_KEYCHAIN:-}" ]]; then args+=(--keychain "$SIGNING_KEYCHAIN"); fi
case "$mode" in
  adhoc)
    /usr/bin/codesign --force --sign - "$app"
    ;;
  developer-id|notarized)
    : "${SIGNING_IDENTITY:?Developer ID Application identity is required}"
    [[ "$SIGNING_IDENTITY" == 'Developer ID Application:'* ]] || {
      echo 'A Developer ID Application identity is required, not Apple Development.' >&2; exit 1;
    }
    # Volt contains one native executable, no nested frameworks/helpers. Do not
    # use --deep to paper over unsigned nested code or add broad entitlements.
    /usr/bin/codesign --force --options runtime --timestamp \
      ${args[@]+"${args[@]}"} --sign "$SIGNING_IDENTITY" "$app"
    ;;
  *) echo "Unknown signing mode: $mode" >&2; exit 1 ;;
esac
/usr/bin/codesign --verify --deep --strict --verbose=2 "$app"
if [[ "$mode" == notarized ]]; then
  temp=$(mktemp -d)
  trap 'rm -rf "$temp"' EXIT
  /usr/bin/ditto -c -k --keepParent "$app" "$temp/Volt.zip"
  if [[ -n "${NOTARYTOOL_PROFILE:-}" ]]; then
    auth=(--keychain-profile "$NOTARYTOOL_PROFILE")
    if [[ -n "${NOTARYTOOL_KEYCHAIN:-}" ]]; then
      auth+=(--keychain "$NOTARYTOOL_KEYCHAIN")
    fi
  else
    : "${APPLE_API_KEY_PATH:?notarization needs NOTARYTOOL_PROFILE or App Store Connect API credentials}"
    : "${APPLE_API_KEY_ID:?missing API key ID}"
    : "${APPLE_API_ISSUER:?missing API issuer}"
    auth=(--key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER")
  fi
  xcrun notarytool submit "$temp/Volt.zip" "${auth[@]}" --wait --timeout 20m --output-format json > "$temp/result.json"
  /usr/bin/plutil -extract status raw "$temp/result.json" | grep -qx Accepted || {
    echo 'Apple did not accept the notarization submission.' >&2; exit 1;
  }
  xcrun stapler staple "$app"
  xcrun stapler validate "$app"
  /usr/sbin/spctl --assess --type execute --verbose=2 "$app"
fi
