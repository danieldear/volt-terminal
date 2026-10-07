#!/bin/bash
# Search-list management for a disposable CI keychain. Never change the default
# keychain or the contents/trust settings of an existing keychain.
volt_capture_keychain_search_list() {
  local listing line
  listing=$(security list-keychains -d user) || return 1
  VOLT_ORIGINAL_KEYCHAINS=()
  while IFS= read -r line; do
    [[ -n "$line" ]] || continue
    # security emits one indented, quoted path per line, including spaces.
    line="${line#"${line%%[![:space:]]*}"}"
    [[ "$line" == \"*\" ]] || return 1
    line="${line#\"}"
    line="${line%\"}"
    VOLT_ORIGINAL_KEYCHAINS+=("$line")
  done <<< "$listing"
}

volt_add_signing_keychain() {
  security list-keychains -d user -s "$SIGNING_KEYCHAIN" \
    ${VOLT_ORIGINAL_KEYCHAINS[@]+"${VOLT_ORIGINAL_KEYCHAINS[@]}"}
}

volt_restore_keychain_search_list() {
  security list-keychains -d user -s \
    ${VOLT_ORIGINAL_KEYCHAINS[@]+"${VOLT_ORIGINAL_KEYCHAINS[@]}"}
}

volt_validate_ci_identity() {
  local identities names
  [[ "$SIGNING_IDENTITY" == 'Developer ID Application:'* ]] || {
    echo 'CI signing requires a Developer ID Application identity.' >&2
    return 1
  }
  identities=$(security find-identity -v -p codesigning "$SIGNING_KEYCHAIN") || return 1
  names=$(printf '%s\n' "$identities" | sed -n 's/^[^"]*"\([^"]*\)".*$/\1/p')
  if ! printf '%s\n' "$names" | grep -Fxq -- "$SIGNING_IDENTITY"; then
    echo 'The imported CI keychain has no valid matching Developer ID identity.' >&2
    echo 'Verify the .p12 contains this certificate AND its private key, and that the certificate is valid.' >&2
    # Certificate names/hashes and trust errors are public metadata, not keys.
    security find-identity -p codesigning "$SIGNING_KEYCHAIN" >&2 || true
    return 1
  fi
}
