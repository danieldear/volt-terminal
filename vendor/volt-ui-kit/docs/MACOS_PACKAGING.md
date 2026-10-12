# macOS application packaging (Unreleased)

Updated: 2026-10-11. This is **local `.app` packaging**, not a release/publishing
service. It extends the existing optional `dev-tools` binary and adds no crates.
Default toolkit, native window appearance and Volt's vendored source are unchanged.

```text
Your Rust application
        |
  cargo build --release     developer chooses features / deployment target
        |
  prebuilt Mach-O + explicit metadata + selected files
        |
  cargo volt bundle
        |  validate -> reserve NEW directory -> bounded copies
        |  plist -> executable headers -> system libraries -> deployment
        |  optional ad-hoc sign -> strict signature verification
        v
  MyApp.app
    Contents/
      Info.plist
      PkgInfo
      MacOS/MyApp            executable named after the app (ASCII-safe)
      Resources/             only files explicitly selected
      _CodeSignature/        when ad-hoc signing is requested
```

## Build and bundle

Install the local tool once (the toolkit is not yet published to crates.io):

```sh
cargo install --locked --path /Users/neo/workspace/rust/volt-ui-kit \
  --features dev-tools --bin cargo-volt
```

For a generated application, build in its project directory, then package the
result. Change identity, name and minimum OS for **your** application:

```sh
cargo build --release --locked
cargo volt bundle \
  --binary target/release/my-volt-app \
  --output MyApp.app \
  --name 'My App' \
  --identifier com.example.myapp \
  --version 0.1.0 \
  --minimum-system-version 11.0
```

The command **does not run Cargo, select features, change the binary's deployment
baseline or launch the application**. GPU-first scaffolds remain GPU-first;
packaging does not force opacity, blur, navigation, a renderer or providers.
`--minimum-system-version` must be at least every Mach-O slice's compiled minimum.
If needed, rebuild using `MACOSX_DEPLOYMENT_TARGET`; merely lowering a plist entry
cannot lower compiled requirements. Application-specific APIs may need a *higher*
minimum. Only the current test Mac is runtime-accepted, not every older OS.

To try Catalog directly without installing the CLI:

```sh
cargo build --release --locked --features native-gpu,native-desktop --example catalog
cargo run --locked --features dev-tools --bin cargo-volt -- bundle \
  --binary target/release/examples/catalog \
  --output VoltCatalog.app \
  --name 'Volt Catalog' \
  --identifier dev.example.voltcatalog \
  --version 0.3.0 --minimum-system-version 11.0
open VoltCatalog.app
```

Choose a new output path for every run. An existing `.app`, file, directory or
symlink is never overwritten or removed by the command. Remove/replace an older
app yourself after confirming it is not running and preserving anything needed.
Do not rebuild a bundle while it is running.

## Options and limits

`cargo volt bundle --help` lists the interface:

| Option | Contract |
|---|---|
| `--binary PATH` | Required regular executable Mach-O, up to 1 GiB; no script/symlink/directory |
| `--output PATH.app` | Required named new directory; parent must already exist |
| `--name NAME` | Required nonblank display/name, at most 128 UTF-8 bytes; XML escaped |
| `--identifier REVERSE.DNS.ID` | Required nonempty ASCII letters/digits/hyphen segments, at least one dot, up to 255 bytes |
| `--version X.Y.Z` | Required numeric release version, no prerelease/build suffix |
| `--build-number N[.N[.N]]` | Defaults to `1`; 1–3 numeric components, first 1–9999, others 0–99 |
| `--minimum-system-version X.Y[.Z]` | Required; checked against macOS Mach-O deployment records |
| `--resource FILE` | Repeat up to 64 times; flat copies using UTF-8 filenames, at most 64 MiB each |
| `--icon FILE.icns` | Optional flat resource; ICNS container header/length checked, not visual design validation |
| `--usage KEY=TEXT` | Optional bounded public privacy description allowlist; no arbitrary plist-key override |
| `--sign ad-hoc\|none` | Default `ad-hoc`; **not** a Developer ID identity selector |

Resources and icon filenames must be distinct, including case-insensitive
collisions. Files remain flat; folders, symlinks and resource-code embedding are
not supported. Unicode filesystem collisions still fail at exclusive creation,
without overwriting. No automatic directory scanning copies credentials, fonts,
source trees or downloaded assets. Only bundle fonts/assets you have rights to
redistribute; SF/SF Mono system discovery is unchanged (no Apple font copies).

Privacy keys currently cover camera, microphone, location, contacts, calendar,
reminder, photo-library, Apple Events and speech usage descriptions. Applicability
varies by API/OS. **A description is not an entitlement or granted permission**;
your application must still implement the right APIs, authorization and denied
states. No permission prompt or entitlement is granted by packaging. Modern
notifications require the independent `native-notifications` adapter and an explicit
authorization action; these plist options do not implement or grant notifications.
See [local notifications](DESKTOP_NOTIFICATIONS.md).

Use `include_bytes!` for small compile-time assets, or locate packaged Resources
relative to `std::env::current_exe()` (`MacOS/../Resources`) for your known leaf
names. Finder/Launch Services do not promise the project's working directory.
Treat bundle contents as immutable: write notes/settings to a developer-selected
user path/Application Support, never to Resources. Changing a signed bundle's
resources invalidates its seal. Catalog's QA only **read** its bundled fixture.

## Checks and safety boundaries

macOS uses fixed `/usr/bin/plutil`, `/usr/bin/otool` and `/usr/bin/codesign`, with
argument arrays, never shell evaluation. The copied executable is inspected
before signing. Every observed Mach-O header must be executable; non-macOS build
records, too-low deployment declarations and non-system dylibs are errors.
Only `/System/Library/` and `/usr/lib/` dependencies pass; `@rpath`, project paths
and third-party frameworks require an embedding/signing tool not supplied here.

The output directory is reserved using `create_dir`, not overwrite/merge.
Copies use exclusive creation and streaming byte caps; normal failure removes
only the directory reserved by that call. No pre-existing destination is cleaned.
This is **trusted local build tooling, not an untrusted-path sandbox**: malicious
filesystem mutation, tool parser exploits and symlink races are not eliminated.
A concurrent reader may observe construction; a killed process may leave a
partial bundle, and cleanup errors can leave files. No power-loss transaction,
installer, updater, helper-service embedding or automatic dylib relocation.

Other hosts return an explicit macOS-only error. Cross-compilation checks do not
constitute Linux/Windows bundle packaging. Thin binaries stay thin; universal
assembly, Intel runtime and minimum-version testing are separate release gates.

## Local signing versus shipping

The default ad-hoc signature is locally verified. It has no Developer ID trust,
hardened-runtime shipping guarantee, notarization ticket or sandbox entitlements.
`--sign none` skips **bundle** signing; copied Mach-O code may already have a
linker/source signature. Neither option claims Gatekeeper distribution acceptance.

For shipping, complete a separate owner-controlled process:

1. Review assets, privacy declarations, entitlements and deployment/architectures.
2. Sign actual nested code inside-out (if present), then the application with your
   Developer ID Application identity, hardened runtime and secure timestamp.
   This packager does not embed nested code or choose entitlements.
3. Strictly verify signatures and test launch, file panels, denied permissions,
   native menus/tray, shutdown/dirty-state protection and every target OS/CPU.
4. Archive/upload with Apple's `notarytool` using your credential profile, inspect
   results, staple the ticket, and validate downloaded/quarantined distribution.
   No upload, keychain modification, certificate use or Gatekeeper bypass occurs
   in `cargo volt bundle`.

See Apple's [bundle structure reference](https://developer.apple.com/go/?id=bundle-structure),
[Core Foundation plist keys](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/CoreFoundationKeys.html)
and [notarization requirements](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
Local notarization is not required merely to test your own build; signing a
bundle ad-hoc does not make it suitable for distribution to other Macs.

## Acceptance receipt

The 2026-10-11 local Catalog bundle used the release `native-gpu,native-desktop`
executable, an explicit disposable identifier, a system generic ICNS used only
for local QA, one plain-text resource and one camera description. No permission
was requested. `plutil` and strict code-signature verification passed; the
receipt reports an arm64 executable and ad-hoc signature. Launch Services opened
Catalog as **Volt Catalog**. Documents opened its packaged plain-text fixture via
the native panel, showed exact text and a clean draft, then closed normally.
The bundle/receipts are ignored under `work/packaging-2026-10-11/`.

This is not Developer ID/notarization acceptance, CPU/GPU utilization profiling,
large-display GPU-resize acceptance, real IME/VoiceOver or OS-termination protection.
Notifications, state restoration, richer navigation and multi-window remain later
application-foundation batches. Theme tooling remains deferred.

Automated gates: **491 passed / 13 ignored** across all features/targets; separate
CLI suite 10 passed (8 new packaging tests), doctests 28 passed / 1 ignored,
default suite 40 passed / 1 ignored. Strict all-feature Clippy and Linux-target
CLI Clippy passed. Rust 1.87 and Linux all-feature/all-target compilation passed;
Linux is not runtime-accepted. Local package verification passed; no publication.
A native negative test refused the existing bundle without changing its plist or
signature; a too-low deployment declaration failed and removed its newly reserved
output. Cargo.toml/Cargo.lock and the default `unicode-width` dependency tree are
unchanged. Existing hardware/GPU driver gates were not re-certified by this work.
