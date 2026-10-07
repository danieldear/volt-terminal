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

Signing secrets (set through GitHub settings, never committed):
- `MACOS_CERTIFICATE_P12_BASE64`: base64 PKCS#12 export including private key
- `MACOS_CERTIFICATE_PASSWORD`

Choose exactly one notarization method:

**Apple Account / app-specific password** (also supported by the local setup):
- Secrets: `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`
- Variable: `MACOS_TEAM_ID`

**Team App Store Connect API key**:
- Secrets: `APPLE_API_KEY` (team `.p8` contents), `APPLE_API_KEY_ID`, `APPLE_API_ISSUER`
- Leave Apple Account credentials and `MACOS_TEAM_ID` unset for this method.

Partial or mixed credentials fail closed. The Apple Account method stores a
validated profile in the same disposable CI keychain used for signing, not in a
runner's default keychain. The local `volt-notary` profile is never exported.

### Secure interactive GitHub setup

1. In Keychain Access, select **My Certificates** and locate the **Developer ID
   Application** identity used for Volt. Expand it and verify its private key is
   present. Export **only this identity**, including its private key, as a
   password-protected `.p12`; do not export the whole keychain. Save outside the
   checkout in a private location.
2. From the checkout, run this in your own terminal:
   ```sh
   bash scripts/macos/setup_github_signing.sh
   ```
   The helper is fixed to `danieldear/volt-terminal`. It asks for the export path,
   signing identity (automatically detected when unique), team and email, validates notarization through Apple's own
   secure prompt, and asks for hidden password inputs for the GitHub upload.
   Export-password verification uses a file descriptor; GitHub secret values go
   through stdin rather than `gh --body` arguments. No private values enter Git
   or script output. Never run it with terminal recording enabled or share a
   screenshot containing credentials.
3. The helper leaves `MACOS_SIGNING_MODE` unchanged. Merge the updated workflows
   and signing scripts. Run **macOS signing validation** from main; it forces
   notarized mode for its own test, verifies the extracted ZIP, and uploads a
   seven-day validation artifact without publishing or replacing a release:
   ```sh
   gh workflow run signing-validation.yml --repo danieldear/volt-terminal --ref main
   ```
   Only after this succeeds, enable notarized mode for future release tags:
   ```sh
   gh variable set MACOS_SIGNING_MODE --repo danieldear/volt-terminal --body notarized
   ```
4. Create a new versioned release from reviewed main. Its macOS job must complete
   notarization, staple validation and Gatekeeper assessment before packaging.
   Validate the downloaded ZIP as well. Existing ad-hoc releases are not changed.

If an upload fails midway, secrets may be partially configured; the helper is
safe to rerun with the same intended inputs. Never commit the export or keychain.

The release job imports into a temporary keychain, grants codesign access only,
validates the exact Developer ID identity, and deletes the keychain/files on
exit. It temporarily adds the signing keychain to the user search list while
preserving existing entries for certificate-chain discovery, and restores that
list on exit. It does not change the login/default keychain's contents or default
selection. Secrets are used only by the trusted release workflow, never PR jobs.
Protect release tags and review workflow changes before creating a release.

`macos-signing-status.txt` records the completed mode, and generated release
notes reflect it. Packaging runs only after validation succeeds. Setting mode
`notarized` with any missing credentials is an error, not an unsigned release.

## Current public release / external gate

The public [v0.1.10 release](https://github.com/danieldear/volt-terminal/releases/tag/v0.1.10)
is **ad-hoc signed, not Developer ID signed or notarized**. Gatekeeper may block
that downloaded app. Do not relabel an older ad-hoc release as notarized.

A separate local validation build (including pending changes) was Developer ID
signed with hardened runtime and secure timestamp, **Accepted** by Apple's
notary service, and stapled. Gatekeeper reported `Notarized Developer ID`; the
packaged ZIP was extracted and its signature, ticket and Gatekeeper assessment
passed again. Submission: `e2f094d4-c382-441a-b0b5-d9baa5bcf261`.

Hosted [macOS signing validation](https://github.com/danieldear/volt-terminal/actions/runs/37572203978)
also passed on reviewed main commit `133d96ae93d0ee8b3ea07b4dda7267076688fc7e`.
The GitHub runner signed, notarized and stapled the app, revalidated the extracted
ZIP, and uploaded a validation artifact. The downloaded artifact's SHA-256,
strict signature, staple and Gatekeeper assessment passed again locally;
Gatekeeper reported `Notarized Developer ID`.

`MACOS_SIGNING_MODE=notarized` is now enabled for future release tags. This
validation did not publish, replace or relabel v0.1.10. A new versioned release
must still complete the release workflow; its downloaded public artifact needs
an install/launch test before that release is advertised as fully validated.

References:
- [Apple notarization documentation](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- Local `xcrun notarytool --help`, `codesign`, `stapler` and `spctl` validation.
