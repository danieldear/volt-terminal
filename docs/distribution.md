# macOS signing and notarization

Ordinary local builds do not require notarization. Release trust levels are
explicit; a requested notarized build fails rather than falling back to ad-hoc.

```text
build app -> Developer ID + hardened runtime + secure timestamp
          -> Apple notarytool (must be Accepted)
          -> staple + validate ticket -> Gatekeeper assessment -> ZIP
```

## Local

```sh
bash scripts/macos/build_app.sh --release
SIGNING_IDENTITY='Developer ID Application: Your Name (TEAMID)' \
  scripts/macos/sign_app.sh target/macos/Volt.app developer-id
```

For notarization, set `NOTARYTOOL_PROFILE` to an existing `notarytool` keychain
profile, and use mode `notarized`. Alternatively provide `APPLE_API_KEY_PATH`,
`APPLE_API_KEY_ID`, and `APPLE_API_ISSUER` for a team App Store Connect API key.
Store credentials securely; never commit private keys, certificates or passwords.
The script requests no relaxed entitlements and does not use `--deep` for signing.
If nested helpers/frameworks are added later, sign them inside-out explicitly.

## GitHub release job

Repository variables:
- `MACOS_SIGNING_MODE=notarized` (default when absent is `adhoc`)
- `MACOS_SIGNING_IDENTITY=Developer ID Application: Your Name (TEAMID)`

Repository secrets (set through GitHub settings, not committed files):
- `MACOS_CERTIFICATE_P12_BASE64`: base64 PKCS#12 export including private key
- `MACOS_CERTIFICATE_PASSWORD`
- `APPLE_API_KEY`: contents of the team App Store Connect `.p8` private key
- `APPLE_API_KEY_ID`
- `APPLE_API_ISSUER`

The release job imports into a temporary keychain, grants codesign access only,
and deletes the keychain/files on exit. It does not change the login/default
keychain. Secrets are used only by the trusted release workflow, never PR jobs.
Protect release tags and review workflow changes before creating a release.

`macos-signing-status.txt` records the completed mode, and generated release
notes reflect it. Packaging runs only after validation succeeds. Setting mode
`notarized` with any missing credentials is an error, not an unsigned release.

## Current validation / external gate

A locally available Developer ID Application identity successfully signed the
validation bundle with hardened runtime and a secure timestamp; strict signature
verification passed. This is **not** a notarization result. No GitHub signing
secrets were configured and no notarytool profile was supplied during this pass.
No private signing material was exported or uploaded. The existing v0.1.2 release
has not been modified or relabelled and remains ad-hoc signed.

References:
- [Apple notarization documentation](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- Local `xcrun notarytool --help`, `codesign`, `stapler` and `spctl` validation.
