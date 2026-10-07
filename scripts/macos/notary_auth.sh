#!/bin/bash
# Shared CI authentication selection. Never print credential values.
volt_validate_notary_auth() {
  local api=false account=false
  if [[ -n "${APPLE_API_KEY:-}${APPLE_API_KEY_ID:-}${APPLE_API_ISSUER:-}" ]]; then api=true; fi
  if [[ -n "${APPLE_ID:-}${APPLE_APP_SPECIFIC_PASSWORD:-}${APPLE_TEAM_ID:-}" ]]; then account=true; fi
  if [[ "$api" == true && "$account" == true ]]; then
    echo 'Configure only one notarization method: API key or Apple Account.' >&2
    return 1
  fi
  if [[ "$api" == true ]]; then
    : "${APPLE_API_KEY:?missing notarization API private key}"
    : "${APPLE_API_KEY_ID:?missing notarization API key ID}"
    : "${APPLE_API_ISSUER:?missing notarization issuer}"
    VOLT_NOTARY_AUTH=api
  elif [[ "$account" == true ]]; then
    : "${APPLE_ID:?missing Apple Account email}"
    : "${APPLE_APP_SPECIFIC_PASSWORD:?missing app-specific password}"
    : "${APPLE_TEAM_ID:?missing Developer Team ID}"
    VOLT_NOTARY_AUTH=account
  else
    echo 'Missing notarization credentials: configure API key or Apple Account.' >&2
    return 1
  fi
}

volt_store_notary_profile() {
  # This is a disposable CI keychain, never the user's login keychain.
  xcrun notarytool store-credentials volt-ci-notary \
    --keychain "$SIGNING_KEYCHAIN" --apple-id "$APPLE_ID" \
    --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" >/dev/null
  export NOTARYTOOL_PROFILE=volt-ci-notary
  export NOTARYTOOL_KEYCHAIN="$SIGNING_KEYCHAIN"
}

# Resolve a human-entered name against valid local Developer ID identities.
# A single detected identity may be selected by pressing Enter. Never infer a
# choice when multiple identities are available, or accept another cert type.
volt_resolve_signing_identity() {
  local input=$1 detected=$2 candidate expected
  if [[ -z "$input" ]]; then
    [[ -n "$detected" && "$detected" != *$'\n'* ]] || return 1
    candidate=$detected
  elif [[ "$input" == 'Developer ID Application:'* ]]; then
    candidate=$input
  else
    candidate="Developer ID Application: $input"
  fi
  while IFS= read -r expected; do
    if [[ "$candidate" == "$expected" && "$expected" == 'Developer ID Application:'* ]]; then
      printf '%s' "$candidate"
      return 0
    fi
  done <<< "$detected"
  return 1
}
