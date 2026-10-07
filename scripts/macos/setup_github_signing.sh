#!/bin/bash
# Run directly in your own terminal. Inputs go to GitHub via stdin, never Git.
set +x
set -euo pipefail
repo=danieldear/volt-terminal
[[ -t 0 ]] || { echo 'Run this interactively in your own terminal.' >&2; exit 1; }
command -v gh >/dev/null || { echo 'Install GitHub CLI first.' >&2; exit 1; }
gh auth status >/dev/null 2>&1 || { echo 'Sign in with gh auth login first.' >&2; exit 1; }
echo "Configure Apple Account signing secrets for $repo."
echo 'Export ONLY the Developer ID Application certificate AND its private key'
echo 'from Keychain Access as a password-protected .p12 outside the repository.'
read -r -p 'Path to exported .p12 file: ' certificate
if [[ "$certificate" == '~/'* ]]; then certificate="$HOME/${certificate#\~/}"; fi
[[ -f "$certificate" && ! -L "$certificate" && "$certificate" == *.p12 ]] || {
  echo 'Expected a regular .p12 file.' >&2; exit 1;
}
checkout=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
certificate=$(cd "$(dirname "$certificate")" && printf '%s/%s' "$(pwd -P)" "$(basename "$certificate")")
case "$certificate" in
  "$checkout"/*) echo 'Keep the private certificate export outside the repository.' >&2; exit 1 ;;
esac
source "$checkout/scripts/macos/notary_auth.sh"
identities=$(/usr/bin/security find-identity -v -p codesigning | \
  sed -n 's/^.*"\(Developer ID Application:.*\)".*$/\1/p')
[[ -n "$identities" ]] || { echo 'No valid local Developer ID Application identity found.' >&2; exit 1; }
if [[ "$identities" != *$'\n'* ]]; then
  echo "Detected signing identity: $identities"
  read -r -p 'Signing identity [press Enter to use detected identity]: ' identity_input
else
  printf 'Available signing identities:\n%s\n' "$identities"
  read -r -p 'Signing identity (name and Team ID, or full certificate name): ' identity_input
fi
if ! identity=$(volt_resolve_signing_identity "$identity_input" "$identities"); then
  echo 'Name does not match a valid local Developer ID Application certificate.' >&2
  echo 'Use the displayed certificate name, or press Enter when one identity is detected.' >&2
  exit 1
fi
default_team=$(printf '%s' "$identity" | sed -n 's/.*(\([A-Z0-9]\{10\}\))$/\1/p')
read -r -p "Developer Team ID [$default_team]: " team
team=${team:-$default_team}
[[ "$team" =~ ^[A-Z0-9]{10}$ ]] || { echo 'Expected a 10-character Team ID.' >&2; exit 1; }
[[ "$identity" == *"($team)" ]] || { echo 'Identity and Team ID differ.' >&2; exit 1; }
read -r -p 'Apple Account email: ' apple_id
[[ "$apple_id" == *@* && "$apple_id" != *' '* ]] || { echo 'Expected an email.' >&2; exit 1; }
# Let Apple's own secure prompt validate the password without exposing it in
# local process arguments. Enter the same password again for the GitHub upload.
xcrun notarytool store-credentials volt-notary --apple-id "$apple_id" --team-id "$team"
read -r -s -p 'Certificate export password: ' certificate_password; printf '\n'
read -r -s -p 'Same validated Apple app-specific password (for GitHub): ' app_password; printf '\n'
[[ -n "$certificate_password" && -n "$app_password" ]] || { echo 'Passwords cannot be empty.' >&2; exit 1; }
trap 'unset certificate_password app_password' EXIT
# Check the export password using a file descriptor, not an argv password.
if ! /usr/bin/openssl pkcs12 -in "$certificate" -noout -passin fd:3 \
    3<<< "$certificate_password" >/dev/null 2>&1; then
  echo 'Cannot open certificate export with this password. No GitHub settings changed.' >&2
  exit 1
fi
# The currently supported CI methods must not be mixed. Do not delete keys silently.
existing=$(gh secret list --repo "$repo" --json name --jq '.[].name')
if printf '%s\n' "$existing" | grep -Eq '^APPLE_API_(KEY|KEY_ID|ISSUER)$'; then
  echo 'API secrets already exist. Choose one method before running this helper.' >&2; exit 1
fi
read -r -p "Upload these signing credentials to $repo GitHub Actions Secrets? [y/N] " answer
[[ "$answer" == y || "$answer" == Y ]] || { echo 'No GitHub settings changed.'; exit 0; }
# No --body arguments: secret values cannot appear in gh process arguments.
/usr/bin/base64 < "$certificate" | gh secret set MACOS_CERTIFICATE_P12_BASE64 --repo "$repo"
printf '%s' "$certificate_password" | gh secret set MACOS_CERTIFICATE_PASSWORD --repo "$repo"
printf '%s' "$apple_id" | gh secret set APPLE_ID --repo "$repo"
printf '%s' "$app_password" | gh secret set APPLE_APP_SPECIFIC_PASSWORD --repo "$repo"
gh variable set MACOS_SIGNING_IDENTITY --repo "$repo" --body "$identity"
gh variable set MACOS_TEAM_ID --repo "$repo" --body "$team"
echo 'Credentials uploaded. Signing mode is unchanged until CI readiness is verified.'
echo 'Merge the updated workflow first; then enable MACOS_SIGNING_MODE=notarized.'
