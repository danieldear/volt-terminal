# Application development: delivery plan and first slice

Updated: 2026-10-11. Scope: application-first, macOS-first. Theme editor is deferred.
**This milestone is not complete.** The current slice is explicitly separated
from remaining desktop services, application lifecycle and tooling below.

## Design contracts

- Developers choose every layout, theme, opacity, material and effect. No required
  shell, navigation placement, "blur only sidebars" rule, or AI provider.
- GPU-first generated standalone apps; automatic opaque software fallback.
  `RendererPreference::GpuRequired` instead returns an error when GPU cannot run.
- Existing `run_ui` options, default dependency footprint, static `Painter`,
  schema 1 and Volt's separate vendored consumer are preserved.
- New system services are opt-in. Capability flags report compiled adapters,
  not permission, hardware availability, deployment acceptance or live OS state.
- No silent dummy WebKit/map/AI view or notification-success stub.
- A UI template is not an editor engine, mail protocol implementation or agent.

### macOS chrome follow-up

`WindowChrome::Overlay` now uses public AppKit unified toolbar geometry on macOS
11+, rather than compact titlebar spacing. Reserve approximately 60 points of
header height and leading clearance (Catalog uses 100 points). AppKit owns native
button drawing and sizing, including light/dark and fullscreen. It does not force
blur or a particular background material. `CompactOverlay` preserves earlier
short-header geometry; default `Native` is unchanged. Detailed contracts are in
[Effects and windowing](EFFECTS_AND_WINDOWING.md#volt-style-window-chrome).

On the test Mac, close/minimize/zoom bounds are 14 by 14 points in both modes,
at x=19/42/65 and y=19 in windowed mode. Native zoom and fullscreen entry were
exercised; colored-circle visual acceptance remains outstanding because native
controls are obscured/omitted by the automation capture. Do not interpret square
control bounds as proof of the final painted silhouette on every macOS version.

**Runtime release gate:** repeated GPU animated zoom on a larger display stalled
in `GpuPresenter::present -> Surface::configure -> Metal wait`, then produced a
wait-for-idle validation error and queue-drop timeout. Normal windowed light/dark
rendering and a smaller-screen zoom round trip worked. A completion-gated resize
experiment avoided blocking but produced a redraw busy-loop; it was reverted,
not shipped. The GPU resize regression needs separate remediation and full
large-display/fullscreen acceptance. The framebuffer cap is now 64 MP, with corrected Vello viewport/spill
admission. Repeated 6K snapshots passed and an 8K scene took explicit software
fallback. Current native zoom/restore passed at 4112x2430; the original larger
display/driver failure is not certified eliminated. Surface retries now back off
with event-loop deadlines rather than immediate redraw loops. Evidence is in `work/window-chrome-2026-10-10/` (local, ignored).

```text
Developer-owned state + Rust view + callbacks
                  |
             Application
              /       \
      UI runtime       Optional desktop adapters
       /      \         (capability checked)
    Vello   software      menu / tray
      |
   Metal on this Mac

Independent components: navigation, tabs, panels, fields, tables...
Optional starter layouts: navigation / mail / settings / agent
No required shell, background material, layout or AI provider.
```

## Delivery phases

| Phase | Work | Status / acceptance |
|---|---|---|
| A | Additive Application entry point, explicit renderer/theme/background, typed background messages | Implemented; tests and generated-project checks |
| A | Controlled standalone navigation: sidebar, centered rail, bottom placement | Implemented basic label destinations; Catalog light/dark and callbacks; icons, nested groups, collapse/resizing and roving keyboard navigation remain |
| A | Local project scaffolding + dev rebuild/restart | Implemented; minimal/navigation/mail/settings/agent UI starters; no state-preserving hot reload |
| B | macOS application menu + tray menu and command delivery | Implemented optional adapter; menu callback and one native tray command accepted on macOS (human tray click + CUA counter); native tray enablement/modal acceptance remains; static menu layout, dynamic command enabled state, one standalone host/process |
| B | Modern notifications and permission/action lifecycle | Partial: independent `native-notifications` safe Rust 1.87-compatible macOS adapter; explicit alert decision, silent local requests/removal and Catalog witness. Settings/action/foreground delegates unsupported; explicit permission and actual native display accepted on the test Mac; delivered removal also accepted; pending-only cancellation and native tray enablement acceptance remain. See [notifications](DESKTOP_NOTIFICATIONS.md) |
| B | Public macOS window materials | Implemented for decorated GPU windows on macOS 10.14+; native light/dark, tint and zoom/restore checked. Borderless material, live preference-change acceptance and broader fullscreen/platform acceptance remain |
| C | Command registry/shortcuts/palette, close veto and dirty-document UI | Included in v0.4.0 as a single-window slice; Catalog in-memory draft, dynamic commands and owned guarded Quit. Bounded notes-file workflow now implemented; OS termination, state restoration and multi-window remain pending; see [command policies](APPLICATION_COMMANDS.md) and [document files](DOCUMENT_FILES.md) |
| C | macOS local bundles, icons, resources, privacy declarations | Implemented `cargo volt bundle` for prebuilt executables, dependency/deployment checks and local ad-hoc signing; Developer ID, hardened-runtime shipping, notarization and installers remain separate owner-controlled release work; see [packaging](MACOS_PACKAGING.md) |
| C | Deeper nav, document tabs, inspector/output/status compositions | Pending beyond basic reusable navigation |
| D | UI inspector, debug overlay, code-change development-loop improvements | Pending; existing trace timings remain available |
| E | Translation/scale/layout/spring motion with correct clipping/hit testing | Pending; existing bounded appearance effects retained |
| F | Embedded web, maps, Foundation Models and App Intents | Deferred until foundation/desktop services accepted |
| Later | Visual theme editor | Deferred explicitly by user |

## Application API (implemented)

```rust,no_run
use volt_ui_kit::{native::{Application, RendererPreference, WindowBackground}, ui::prelude::*};
#[derive(Default)]
struct App { count: u32 }
fn view(app: &App) -> Element<App> {
    vstack![
        text(format!("Count: {}", app.count)),
        button("Add").key("add").on_click(|app: &mut App| app.count += 1),
    ].padding(24., Edge::All).into()
}
Application::new("My application", App::default(), view)
    .size(1100, 760)
    .renderer(RendererPreference::Auto)
    .background(WindowBackground::Blur { opacity: 0.82 })
    .run()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Application` defaults to system theme, GPU-if-compiled, ordinary native chrome,
opaque background. Appearance is explicit Rust configuration, not launch flags.
It also accepts `theme_with`, `palette`, `chrome`, `reduced_motion`, `messages`
and optional `desktop`. Configuration is validated before creating an event loop.
The new renderer preference is authoritative; existing `run_ui` still honors
`VOLT_UI_RENDERER`. Renderer-required behavior also holds during GPU recovery.

Window alpha changes the root clear, not text/control opacity. Desktop background
blur, panel-backdrop blur and content blur are separate capabilities. Decorated macOS GPU
windows now request public AppKit behind-window material; other compositors may
ignore blur. Borderless macOS requests explicitly fall back to transparency. The material bridge logs the reduced-transparency preference at install time;
AppKit owns the material response. Live preference-toggle acceptance remains
separate. System capability flags do not promise universal blur support. Do not distribute it as a universally supported material.

## Scaffolding and development runner (implemented)

Until a registry release, supply an explicit local toolkit path:

```sh
cargo run --locked --features dev-tools --bin cargo-volt -- new /tmp/my-volt-app \
  --kit-path /Users/neo/workspace/rust/volt-ui-kit --template navigation
cargo run --manifest-path /tmp/my-volt-app/Cargo.toml
```

Templates: `minimal`, `navigation`, `mail`, `settings`, `agent`. `--desktop` adds
macOS menu/tray setup and the optional `native-desktop` feature. Without it, the
new project has no menu/tray dependencies. Generated projects use `native-gpu` by
default. Unsupported desktop adapters return explicit errors on other platforms.
Generated manifests explicitly select resolver 3 to prefer dependencies compatible
with their Rust 1.87 minimum, including when Cargo is launched with `--manifest-path`.
An initial fresh-project check caught a newer transitive UUID requiring Rust 1.89;
the generated resolver policy now selects a compatible version. Keep application
lockfiles in version control. External workspaces own their resolver configuration.
See [Cargo resolver versions](https://doc.rust-lang.org/cargo/reference/resolver.html#resolver-versions).

All five templates now use **Catalog-style unified window chrome on macOS**:
no separate native title strip, native close/minimize/fullscreen traffic lights
retained, and an editable 60pt Rust header marked `.drag_region(true)`. Its title
starts at 96pt on macOS to keep content clear of the native buttons. The header
uses SF and semantic theme colors; background blur/transparency is still the
developer's choice, not silently enabled. Other platforms retain native decorated
chrome rather than losing their window controls.

Generated `src/main.rs` contains `window_header()` and `window_chrome()` so you can
customize them directly. `Application`/`NativeOptions` defaults and existing
projects are unchanged. For a standard title bar use `.chrome(WindowChrome::Native)`;
adjust/remove your custom header as appropriate. Existing apps can opt in with
`.chrome(WindowChrome::Overlay)` **and a reserved draggable header**, not just
the option alone. Unified geometry requires macOS 11+; older macOS retains
compact native control geometry.

After updating the toolkit, reinstall `cargo-volt` (add `--force` when replacing
an installed copy). Re-running `new` never overwrites an existing project;
regenerate into a new directory or update your existing Rust source deliberately.

Scaffold chrome verification (2026-10-11): all five templates with/without desktop
integration compiled; ten isolated generated-app tests checked header height,
button clearance and content bounds at 1x/2x and 480/1100pt widths. The toolkit
all-feature/all-target suite passed 503 tests (13 ignored), strict Clippy, Rust
1.87 and Linux cross-check passed. A locally ad-hoc packaged generated navigation
app showed its in-content header without a separate title strip; CUA observed
native close/minimize/fullscreen accessibility controls and a Projects click
changed destination 0 to 1. Close exited the test app. Native drag motion,
fullscreen/multi-display behavior and other-OS visual acceptance are not implied
by these layout checks. Local receipts remain ignored under
`work/scaffold-chrome-2026-10-11/`.

To make `cargo volt` available locally:

```sh
cargo install --path /Users/neo/workspace/rust/volt-ui-kit --features dev-tools --bin cargo-volt
cargo volt dev /tmp/my-volt-app
```

`dev` executes the **trusted local project's Cargo build/build scripts and app**.
It watches source/manifest/.cargo metadata at 500ms intervals, rebuilds and
restarts after a successful build, and retains the running app on compiler errors.
It does not poll/render UI frames. It does not preserve unsaved app state, watch
path dependencies/assets, hot-swap Rust closures, sandbox build scripts or package
applications. One binary target is required. Use only on projects you trust.
Scaffolding never overwrites an existing destination and performs no build itself.

## Native desktop services (menu/tray and bounded notification slice)

`native::system::{DesktopConfig, DesktopMenu, DesktopCommand, TrayConfig}` defines
bounded validated configuration; `Application::desktop(config, callback)` routes
known enabled command IDs on the UI thread and recomposes the view. Menu/tray IDs
share commands; inconsistent definitions are rejected. Tray RGBA size is bounded
at 128x128. The application retains and drops OS menu/tray resources with its host.
No global background polling or OS subprocess is used.

macOS uses compatible pinned `muda 0.19.3` and `tray-icon 0.24.2`, defaults disabled;
no Linux GTK backend/dependencies are selected. They expose safe Rust APIs to this
crate, which retains `forbid(unsafe_code)`. Native calls internally require platform
FFI: "safe toolkit" is not a dependency-level security certification. OS services
are initialized in winit's resumed/main-thread lifecycle. The menu includes standard
About/Services/Hide/Quit actions. This is the **application menu**, not customization
of macOS's system Apple-logo menu. Predefined edit actions are intentionally not
added: custom text input is not automatically an AppKit responder.

Limitations: one process-global muda handler, single standalone host, static
menu layout and no tray-click callback/hide-to-tray lifecycle. Registered command
accelerators and close veto are now implemented; full OS quit protection and
multi-window/persistence remain pending. See [command policies](APPLICATION_COMMANDS.md). Existing embedders can own their adapters;
this host must not share a process-global menu handler with another framework.
Web/maps/AI/intents capabilities remain false. Notifications now use independent
`native-notifications` and pinned safe `objc2-user-notifications 0.3.2`, not a
deprecated NSUserNotification/AppleScript fallback. `system::capabilities()`
reports the compiled local-notification adapter; detailed capabilities explicitly
exclude live settings, action callbacks and forced foreground presentation.
See [notifications, feature boundaries and acceptance](DESKTOP_NOTIFICATIONS.md).

## Later public Apple bridges (planned, not implemented)

| Module | Public API / requirements |
|---|---|
| Window material | Public NSVisualEffectView bridge implemented for decorated macOS GPU windows; borderless and comprehensive fullscreen/reduced-transparency acceptance remain |
| Notifications | Bounded local authorization/send/removal implemented; no delivery receipt. Settings/action/foreground delegates and broader packaged native acceptance remain |
| Embedded web | WKWebView/native-view placement and lifecycle, URL/scheme/navigation policies, isolated optional JS bridge, permission/storage controls, clipping/focus |
| Maps | MapKit/native-view integration; developer-selected provider; explicit location permission only when location is requested; attribution/provider rules |
| On-device models | Foundation Models Swift bridge; runtime device/OS/model availability, cancellation, safety/resource bounds; no blanket Siri-chat API |
| Siri/Shortcuts | App Intents in application Swift packaging/metadata; developer-declared app actions, not unrestricted access to Siri internals |

Primary API references checked 2026-10-10:
[UserNotifications](https://developer.apple.com/documentation/usernotifications),
[WKWebView](https://developer.apple.com/documentation/webkit/wkwebview),
[MapKit](https://developer.apple.com/documentation/mapkit),
[App Intents](https://developer.apple.com/documentation/appintents),
[Apple Intelligence](https://developer.apple.com/apple-intelligence/),
[NSVisualEffectView](https://developer.apple.com/documentation/appkit/nsvisualeffectview).
These require narrow optional platform bridges, not unsafe modifications to the
renderer-independent core. Windows/Linux adapters and runtime acceptance follow.

## Release acceptance

- Default, rust-ui, native, GPU, desktop and dev-tools test/build/lint/MSRV gates.
- Generated applications compile and launch; real menus/tray callbacks update UI.
- Native permissions and user controls tested, not inferred from SDK compilation.
- Runtime input/IME/VoiceOver and Windows/Linux acceptance remain separate.
- New component visuals live in Catalog only. No theme editor or mandatory shell.
- No claim that a full code editor, mail backend, AI client, multi-window runtime
  or complete application framework was delivered in this first slice.

## First-slice verification (2026-10-10)

- All-features/all-targets suite: 440 passed, 12 explicitly ignored; the all-feature
  doctest suite separately passed 28, with 1 ignored. Default suite: 40 passed.
- Strict all-features/all-targets Clippy and native-only library/test Clippy passed.
  Rust 1.87 all-feature/all-target check passed. Generated native/GPU desktop
  navigation project also checked on Rust 1.87 using fresh dependency resolution.
- All five generated template kinds checked; no unused generated-state warnings.
- Real Cargo dev loop accepted initial launch, retained child on a deliberate
  compiler error, then replaced/reaped the child after successful compilation.
- Generated navigation app launched with Vello / Apple M4 Pro / Metal, with
  `framebuffer_upload_bytes=0`; navigation and native Workspace/Settings commands
  updated Rust state through real macOS UI interaction.
- Catalog Navigation accepted sidebar, rail and bottom callbacks, disabled
  destination behavior, and light/dark appearance on GPU.
- Tray resource installation compiled/launched, but status-item click acceptance
  remains unverified. No notification authorization, VoiceOver session, Windows/
  Linux native acceptance, new performance benchmark or Volt vendored integration
  acceptance is claimed. Logs/temporary generated fixtures are ignored under
  `work/application-foundation-2026-10-10/`.

## Command/lifecycle follow-up verification (2026-10-11, Unreleased)

- All-features/all-targets suite: 473 passed, 13 explicitly ignored. Separate
  doctests: 28 passed, 1 ignored. Default suite: 40 passed, 1 ignored.
- Strict all-features/all-targets, native-only library/test and rust-ui-only
  Clippy passed. Rust 1.87 all-feature/all-target compilation passed; Linux
  x86_64 all-feature/all-target cross-compilation checks passed (not native QA).
- Catalog's ignored hardware test explicitly ran all 38 pages at 1x/2x on
  Metal. Release Catalog built with `native-gpu,native-desktop`. Default package
  verification passed locally; no registry publication or new version is claimed.
- Validation includes dynamic enablement, exact shortcut modifiers, modal/IME
  suppression, legacy callback precedence, static disabled-menu gates, queue
  capacity/closure and close-policy re-evaluation. Definition validation is
  cached; native enabled-state writes occur only when that state changes.
- After the user unlocked the Mac, native Catalog command/close interaction
  checks passed in light/dark mode; see the acceptance receipt below.
  Discrete callback multiplicity now has a counted native witness in the
  [document follow-up](DOCUMENT_FILES.md). Native-menu physical held-key repeat
  still needs separate acceptance; old idempotent transitions were not proof.
- Sample Save is in-memory only. OS/Dock termination, persistent documents,
  multi-window lifecycle, notifications, signed packaging and the original
  larger-display GPU resize regression remain separate pending work. No new
  CPU/GPU utilization profile or Volt synchronization is claimed.
- Local logs and the disposable QA bundle are ignored under
  `work/lifecycle-2026-10-11/`.

### Native command/close acceptance after unlock (2026-10-11)

Tested the release `native-gpu,native-desktop` Catalog at code commit `5877194`,
in a disposable `.app` with an independent bundle ID. All input/menu interactions
used native UI automation, with accessibility state checked after transitions.
Only test-owned in-memory text was edited; no user documents were opened/saved.

| Native check | Observed result |
|---|---|
| Edit draft, Cmd+S and File > Save | Dirty badge changed to Saved in memory; exact draft text remained; Save became disabled |
| Cmd+K palette and Escape | Palette opened with disabled Save when clean; Escape restored the document view |
| Cmd+W inside palette | Window stayed open; palette retained focus (modal scope) |
| Native close button while dirty | Save / Discard / Cancel dialog appeared rather than terminating |
| Cancel | Dialog dismissed; edited text and Unsaved badge remained; editor still accepted input |
| Cmd+W and native File > Close | Both opened the same dirty-document confirmation |
| Cmd+S / Cmd+K inside confirmation | Neither saved behind the dialog nor opened a nested palette |
| Native File menu during confirmation | Save, palette and Close commands all reported disabled |
| Owned Cmd+Q and application-menu Quit | Dirty window remained open for confirmation; Cancel preserved the draft |
| Save & close | Test app exited; native log recorded accepted closing |
| Discard & close in light mode | Test app exited, wrapper recorded exit status 0 |
| Clean native close | Immediate exit without dirty confirmation |
| Light/dark dialog appearance | Rounded dialog, readable labels and all three actions rendered in both modes |

GPU logs report Vello on Apple M4 Pro / Metal at 2880x1840, with
`framebuffer_upload_bytes=0`. Retained save/clean/discard runs contain 56/2/19
rendered-frame trace records and accepted-close markers. No panic, validation
error or timeout was found in those retained logs. This is functional runtime
evidence, **not** a CPU/GPU utilization benchmark, idle-power certification or
resolution of the original larger-display resize issue.

Receipts are in local ignored `work/lifecycle-2026-10-11/`:
`native-save-close.log`, `native-clean-close.log`, `native-remaining.log`.
The first Discard attempt was followed by an automation observation that reopened
the app; its overwritten log is not counted as an exit receipt. The light-mode
Discard test was repeated with an append-only wrapper recording status 0.

At this receipt, counted dispatch, held-key repeat, real IME/VoiceOver and broader
keyboard-layout/platform coverage were still pending. The document follow-up
below addresses discrete counted dispatch and bounded notes persistence; physical
held-key repeat and the other native gates remain. OS/Dock termination,
multi-window, tray-click acceptance and notifications are outside both receipts. No new toolkit defect was established
by these interaction checks; this follow-up changes documentation only.

## Document follow-up (Unreleased, 2026-10-11)

Catalog now includes a real bounded Open/Edit/Save/Save As notes workflow with
public macOS file panels, background I/O, external-edit conflict errors and
async save-before-close. The earlier memory example is unchanged. No claim of
large-file editing, generic state persistence, OS quit protection or finished
application packaging is made. See [exact APIs and native receipt](DOCUMENT_FILES.md).


## Notifications/tray follow-up (2026-10-11; Unreleased)

Independent `native-notifications` implements explicit alert-only authorization,
silent one-shot stable-ID local submission, removal and synchronous
validation/main-thread/bundle errors. Catalog's Desktop services page and tray
share guarded registry IDs and a count witness. Only one additional pinned
framework crate; core/default footprint unchanged. OS acceptance is not visible
delivery. Settings queries/actions/foreground delegates remain unavailable.
The first disposable native QA launch was blocked by the locked Mac; after
unlock, the approved alert permission flowed correctly, the human supplied an
actual native notification screenshot, and a human-operated tray command changed
the CUA-observed counter from 0 to 1. After the CUA removal action, the human
confirmed the test entry disappeared without manual clearing. Pending-only
cancellation and native tray enablement/modal acceptance remain separate gates.
Catalog adds an explicit 60-second cancellation-test plan, preserves the original
delay in its completion witness, and regression-tests disabled/replayed controls
and modal background scope. No new crates or public host API. See the precise
evidence tiers
in [API, bounds and native receipt](DESKTOP_NOTIFICATIONS.md).

## Independent glass controls (2026-10-11)

`Application::background_from_state` updates native root opacity/blur from Rust
state. `UiTheme::with_background_color` changes the root clear RGB independently
of semantic component colors. See [Glass controls](SURFACES_THEMES_AND_SCROLLING.md).
Defaults remain unchanged; raw compositor blur is an explicit option using a
private macOS API. The requested in-app GPU backdrop renderer remains pending:
blur/saturation/tint/opacity with clipped panels, cached GPU targets and no
per-frame CPU readback, validated against actual application content—not wallpaper.
