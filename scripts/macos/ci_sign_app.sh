#!/bin/bash
# Release jobs only: ephemeral keychain, no changes to login/default keychain.
set +x
set -euo pipefail
app=${1:?usage: ci_sign_app.sh APP}
mode=${MACOS_SIGNING_MODE:-adhoc}
case "$mode" in
  adhoc) scripts/macos/sign_app.sh "$app" adhoc ;;
  notarized)
    : "${MACOS_CERTIFICATE_P12_BASE64:?missing signing certificate}"
    : "${MACOS_CERTIFICATE_PASSWORD:?missing certificate password}"
    : "${SIGNING_IDENTITY:?missing Developer ID identity}"
    source scripts/macos/notary_auth.sh
    volt_validate_notary_auth
    # Never inherit a runner-local profile or select an unintended auth method.
    unset NOTARYTOOL_PROFILE NOTARYTOOL_KEYCHAIN APPLE_API_KEY_PATH
    temp=$(mktemp -d)
    export SIGNING_KEYCHAIN="$temp/release.keychain-db"
    cleanup() {
      /usr/bin/security delete-keychain "$SIGNING_KEYCHAIN" >/dev/null 2>&1 || true
      rm -rf "$temp"
    }
    trap cleanup EXIT
    umask 077
    printf '%s' "$MACOS_CERTIFICATE_P12_BASE64" | /usr/bin/base64 --decode > "$temp/certificate.p12"
    password=$(/usr/bin/openssl rand -hex 32)
    /usr/bin/security create-keychain -p "$password" "$SIGNING_KEYCHAIN"
    /usr/bin/security set-keychain-settings -lut 1800 "$SIGNING_KEYCHAIN"
    /usr/bin/security unlock-keychain -p "$password" "$SIGNING_KEYCHAIN"
    /usr/bin/security import "$temp/certificate.p12" -k "$SIGNING_KEYCHAIN" \
      -P "$MACOS_CERTIFICATE_PASSWORD" -T /usr/bin/codesign >/dev/null
    /usr/bin/security set-key-partition-list -S apple-tool:,apple:,codesign: -s \
      -k "$password" "$SIGNING_KEYCHAIN" >/dev/null
    if [[ "$VOLT_NOTARY_AUTH" == api ]]; then
      printf '%s' "$APPLE_API_KEY" > "$temp/notary.p8"
      export APPLE_API_KEY_PATH="$temp/notary.p8"
    else
      volt_store_notary_profile
      unset APPLE_APP_SPECIFIC_PASSWORD
    fi
    scripts/macos/sign_app.sh "$app" notarized
    ;;
  *) echo 'MACOS_SIGNING_MODE must be adhoc or notarized' >&2; exit 1 ;;
esac
mkdir -p dist
printf '%s\n' "$mode" > dist/macos-signing-status.txt
