# Volt UI Kit

The cards, buttons, forms, palettes and vector icons behind Volt — extracted into a small, renderer-independent Rust library.

**Status:** working extraction, version 0.4.0, not published to crates.io. This is an embeddable toolkit foundation, not yet a complete desktop application framework.

## Polished component testing: Volt Catalog

```sh
cargo run --release --locked --features native --example catalog
```

Catalog is the primary component demonstration/acceptance app as of 2026-10-10.
It provides live previews, selectable Rust source, API tables, SF/SF Mono type,
six accents and light/dark/system themes. New work extends Catalog only; Atelier,
Desk and UI Lab remain existing regression fixtures. See [Catalog](docs/CATALOG.md)
for accessibility, GPU and opt-in glass launch options and acceptance boundaries.

## Coding-agent client: Relay

```sh
cargo run --release --locked --features native-gpu --example relay
```

A desktop agent client in the style of current coding-agent apps: frosted thread
sidebar, streaming transcript with collapsible work steps and file changes, a
composer with permission, model and effort menus, and an inspector card. Replies
come from a local scripted worker, so it runs offline. `--theme light`, `--new`,
`--thread N`, `--background opaque` and the shared `--snapshot` flags apply.

```sh
cargo run --features native --example ui_lab -- --section scrolling --theme light
cargo run --features native-gpu --example ui_lab -- --section window --background blur --chrome overlay
```

See [the examples guide](examples/README.md) for fixture sections and CLI flags;
use [Catalog](docs/CATALOG.md) for current component acceptance.
The smaller examples remain focused regression/learning fixtures, not the main
manual-testing workflow. Native window alpha/blur still requires platform acceptance.

The Controls section also includes reusable toolbars, joined button bars, typed
segmented controls and active-page-only tabs, controlled text fields/areas, bounded
sliders, typed selection popups and modal dialogs. See [the component API](docs/COMPONENTS.md)
for Rust declarations, keyboard behavior and honest platform limits.
See [performance measurement](docs/PERFORMANCE.md) for reproducible workloads
and the distinction between native timing, readback and utilization.
See [GPU drawing and dependencies](docs/GPU_RENDERING.md) for the optional Vello
renderer, feature-pruned backend policy and explicit software comparison.

## Polished app-scale examples

```sh
# Original multi-screen product showcase; also accessible inside UI Lab
cargo run --locked --features native --example showcase
# Original Codex-style desktop workbench, with local sample conversations
cargo run --locked --features native --example desk
```

**Volt Atelier** has Welcome, Workspace and Component Studio screens, paired
midnight/paper themes, original vector art and real Rust callbacks. **Volt Desk**
has conversation navigation, independently scrolling panes, an editable local composer
and optional inspector. SF/SF Mono resolve to installed Apple faces on macOS with
explicit sans/mono fallbacks elsewhere; no Apple fonts are bundled. Desk is not an
AI client: Send uses your edited text but replies remain fixed local samples.

See [showcase apps and verification](docs/SHOWCASE_APPS.md) for direct routes,
compact mode, font resolution, design tokens and interaction checklists.

## What is here

- A reusable icon-and-label `Button`, also used by Volt's Settings footer.
- Floating/docked/minimized cards, colored indicators, expandable rows and diagnostics.
- Settings/onboarding presentation, navigation geometry and footer controls.
- Action/task button strips and a three-field form with checkbox and status.
- Search palette presentation, Unicode match highlighting and pointer geometry.
- Tab-strip hit testing, reorder target geometry and accent colors.
- Theme-derived surfaces, outlines, text and semantic colors.
- Shared vector icon paths and rounded/line drawing primitives.
- A static SVG backend and standalone component gallery.
- Glass-inspired reusable `Panel` / surface tokens, brightness hysteresis, and clipped topmost hit routing.

The presentation models and paint recipes are real extracted implementations, not screenshots or HTML recreations of Volt. A few component names and default labels retain their Volt origins; use the customizable card footer and generic Button for application-independent UI. Generalizing the remaining labels is on the roadmap.

## Architecture

```text
Application state / native input / accessibility
                    |
Models + shared layout / hit testing
                    |
Widget paint recipes + theme + icons
                    |
Painter adapter (static dispatch)
          /                     \
Host GPU/text backend      SVG documentation backend
```

Implement `Painter` to emit triangles and shaped text into your own buffers. `Shape` and `Glyph` are host-selected types. The default component layer does not allocate a GPU device, start a window, run a timer, read files or execute commands. The separately opted-in native host creates one window/event loop and reads an explicitly supplied reload file. Volt's adapter reuses its existing wgpu device, glyph atlas, cosmic-text shaping, vertex buffers and per-widget geometry caches. Cache invalidation and event-driven redraw scheduling stay with the host.

Rendering coordinates are physical pixels; layout inputs include an explicit scale and optional top inset. Text uses a top-origin, 1.2-em convention. Colored text callbacks use UTF-8 **byte offsets**. Use a valid positive finite viewport; `Viewport::new` validates the basic dimensions and scale.

## Glass-inspired surfaces (optional)

Inputs from Glass Browser informed three new portable building blocks:

- `Panel`: general-purpose rounded frame, subtle top highlight and an opaque fallback.
- `SurfaceStyle` / `TintPicker`: smoked/frosted material tokens with lightness hysteresis. The **host** supplies brightness only when its backdrop changes; invalid samples are ignored.
- `HitMap<T>`: register targets in paint order, then route to the topmost visible target. Supports clipping, rounded corners and a `Block` target so a floating panel can consume clicks without activating content beneath it.

Existing Volt widgets keep their current theme and render path. Glass is **opt-in**, not a replacement theme. `Panel::paint` is an opaque fallback, not fake blur. A compositor-capable host can consume `backdrop_request()` then use `paint_composited_frame()`; it must implement backdrop capture, tint, blur, correct compositing order and caching itself. Requests use physical pixels; logical corner/blur radii are scaled once. `reduced_transparency()` disables blur and uses an opaque fill.

The source of these ideas is Glass Browser's `crates/ui/src/{theme,tint,view}.rs` and `crates/glass/src/lib.rs`. They are independently implemented here; browser-specific state, CEF, Vello, WGSL pipelines and source files were not copied into this crate. A real GPU glass backend and system accessibility preference detection remain future work. Bounded appearance animation is available in the optional Rust UI runtime; the gallery still demonstrates **opaque surface previews**, not backdrop compositing.

## Use it locally

```toml
[dependencies]
volt-ui-kit = { path = "/path/to/volt-ui-kit" }
```

```rust
use volt_ui_kit::{Button, ButtonColors, CardIcon, Painter, Viewport};
use volt_ui_kit::svg::SvgPainter;

let mut painter = SvgPainter::new(Viewport::new(480., 160., 1.).unwrap()).unwrap();
let (mut shapes, mut glyphs) = (vec![], vec![]);
let button = Button { label: "Apply", icon: Some(CardIcon::Check), enabled: true };
let rect = [24., 24., 112., 32.];
button.paint(&mut painter, &mut shapes, &mut glyphs, rect, 1., ButtonColors {
    surface: [0.87, 0.70, 0.40, 1.], text: [0.09, 0.10, 0.10, 1.],
});
assert!(button.hit(rect, 50., 40.));
let svg = painter.document(&shapes, &glyphs, [0.09, 0.10, 0.10, 1.]);
assert!(svg.contains("Apply"));
```

## Component gallery

```sh
cargo run --locked --example gallery
# Open target/gallery/index.html in a browser.

cargo run --locked --example gallery -- --light --output target/gallery-light
cargo run --locked --example gallery -- --scale 2 --output target/gallery-2x
```

The gallery is **static SVG documentation**, not a native interactive application. It runs without Volt, a terminal engine, a windowing dependency or a GPU. Browser font metrics/rasterization may differ from Volt's native text backend; Volt's existing GPU fixtures are the visual regression authority.

## Develop and validate

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo +1.87.0 check --locked --all-targets
cargo package --locked
```

Rust 1.87 or newer. Only **default** runtime dependency: `unicode-width` (optional document/native features add dependencies). The library forbids unsafe code. SVG text is escaped; it never loads assets or executes embedded scripts. A host must still implement secure input, accessibility and safe application actions.

## Volt integration and source ownership

This project is the canonical editable source. Volt consumes a **reproducible vendored snapshot**, so its public checkout and CI remain self-contained before this library has a remote release.

```sh
python3 scripts/sync_into_volt.py --volt ../terminal-public
python3 scripts/sync_into_volt.py --volt ../terminal-public --check
```

The sync tool only updates `vendor/volt-ui-kit`; it refuses an unrecognized nonempty destination. `SOURCE.json` records version and hashes, with no local machine paths. Volt's Python validation verifies that snapshot's file inventory and content. Do not edit the vendored copy independently. After publishing a stable library version, Volt can switch to a normal pinned registry dependency.

The original UI implementations in Volt are now compatibility re-exports and cache/backend adapters, not a second copy of widget painting logic. PTY/parser/grid, task execution/trust, Git/project discovery, config persistence and AppKit integration stay in Volt.

## License

MIT, preserving the original Volt copyright notice.

## Rust-first declarative UI and optional native host

The primary authoring API declares UI **inside Rust**, with typed components,
layout macros, fluent style modifiers and direct closures/functions. State lives
in your Rust application; `run_ui` calls a view body initially and after actual
actions. Conditionals/loops/reusable components are ordinary Rust.

```rust,ignore
use volt_ui_kit::ui::{column, prelude::*};
fn view(app: &App) -> Element<App> {
    column![
        text(format!("Count: {}", app.count)).font_size(24.).font_weight(700),
        button("Add").key("add").on_click(|app: &mut App| app.count += 1),
    ].padding(24., Edge::All).gap(12., Axis::Both).into()
}
// native::run_ui(NativeOptions::default(), App::default(), view)?;
```

```sh
cargo run --locked --features native --example rust_counter
cargo run --locked --features native --example rust_settings
cargo run --locked --features native --example rust_keyed_list
cargo run --locked --features native --example rust_layout
cargo run --locked --features native --example rust_responsive
cargo run --locked --features native --example rust_effects
# Optional JSON frontend with explicit F5 reload:
cargo run --locked --features native --example components
```

Both frontends use the same validated tree/layout/input runtime, with no JSON
round-trip for Rust composition. Panel/Row/Column/Text/Button/Checkbox/Toggle/Badge/
Separator/Divider and Spacer support layout; visible components support font
family/size/weight/italic and text/surface/border styles.
The optional native host uses winit and cosmic-text. `native` draws through
tiny-skia/softbuffer; `native-gpu` draws through Vello/wgpu for opaque and alpha
windows, with software fallback. Window blur is an OS request, not widget backdrop
blur or a complete cross-platform framework.

```sh
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example ui_lab
VOLT_UI_RENDERER=software cargo run --locked --features native-gpu --example ui_lab
```
Rust source edits require rebuild. Current composition rebuilds the whole tree,
not dependency-tracked signals/subtree diffing. Background state updates use the bounded `UiSender` message API to wake the
native runner without polling; see [worker messages](docs/APP_RUNTIME.md).

Default builds keep existing widget/Painter APIs and depend only on unicode-width.
`declarative` adds serde/serde_json; `rust-ui` currently shares that feature;
`native` includes Rust UI plus optional host/text libraries. No Volt/Glass edits
or snapshot synchronization are performed by building these examples.
See [Rust UI design](docs/RUST_UI.md), [all examples](examples/README.md),
[optional JSON format](docs/DECLARATIVE_UI.md), [styles](docs/STYLING.md) and
[validation gates](docs/VALIDATION.md). Local development profiles optimize
CPU rendering; consumer workspaces retain their own profiles. See
[measured performance](docs/PERFORMANCE.md) for evidence and remaining limits.

Stack aliases `vstack!`/`hstack!` preserve `column!`/`row!`. Direction and content
placement are separate: use `.flow(Flow::Center)` (or Top/Bottom/Left/Right/corners)
inside a panel/stack. Spacing uses `.padding(20.5, Edge::Horizontal)` and
`.gap(12., Axis::Vertical)`; select All/Both for unrestricted spacing. Rounding
accepts `.rounded(4)`, `.rounded(4.5)` and `.rounded(Rounding::Full)`.
`divider()` is a horizontal separator; `spacer()` consumes remaining main-axis
space without painting/input. Try `rust_layout` for interactive placement presets.

`stack!` adds viewport-responsive horizontal/vertical direction; all containers
accept typed responsive padding/gap as well as their existing scalar values.
`zstack!` independently overlays children in back-to-front order. Resize resolves
layout properties without rebuilding Rust callbacks. Try `rust_responsive` and see
[responsive contracts](docs/RESPONSIVE_UI.md); container queries/adaptive grids and free-coordinate placement remain future work.
Opt-in overflow scrolling is described below.

## Effects and custom window chrome

Rust spacing/corner modifiers accept single or multiple selections. New Rust
buttons default to 2 logical-pixel corners; legacy widgets/JSON retain their visuals.
Opt-in shadows, glows, subtree opacity, hover/press/focus styles, keyed paint-only
animation and fade transitions work in the native software renderer.
`NativeOptions::with_chrome(WindowChrome::Overlay)` and `run_ui_with_options`
provide Volt-style integrated macOS chrome without changing old option literals.
See [effects/windowing contracts and limits](docs/EFFECTS_AND_WINDOWING.md).
Native OS chrome/input/timer acceptance remains separate from headless tests.

## Semantic surfaces and scrolling (Rust UI)

Panels and stacks share image backgrounds, background-only opacity, explicit
rounded/circle/path clipping and semantic `ColorToken` styling. `canvas` retains
Rust vector commands. Opt into overflow with `.scroll(Scroll::Auto)` or a specific
axis; native wheel/trackpad input bubbles through nested views.

- `cargo run --features native --example rust_surfaces` — images, vectors, theme toggle
- `cargo run --features native --example rust_scroll` — fixed headers and nested scrolling
- `cargo run --features native-gpu --example rust_transparent` — optional real window alpha

See [Themes, surfaces, clipping and scrolling](docs/SURFACES_THEMES_AND_SCROLLING.md)
for APIs, limits, compatibility and native-blur platform caveats.

### Application runtime and extended components

Typed bounded worker messages wake the native event loop without idle polling. **Catalog is the current acceptance app.** Existing **Showcase Activity** (jobs, menus, tooltip/popover, wrapping), **Explorer** (10,000 virtual rows, tables, trees, split panes), and Lab routes remain regression fixtures; new demos are not added there. GPU Showcase defaults to a translucent blur-request window; `--background opaque` restores a solid window.

```sh
cargo run --locked --features native-gpu --example showcase -- --screen activity
cargo run --locked --features native-gpu --example showcase -- --screen explorer
```

See [runtime](docs/APP_RUNTIME.md), [declarations and limits](docs/EXTENDED_COMPONENTS.md),
and [validation gates](docs/VALIDATION.md).

### Extended Rust components (Unreleased)

Catalog now includes Utilities, Icons, Date & time, Form inputs, Overlays,
Data display and Charts. The added APIs are Rust-only; schema-1 JSON and the
legacy default renderer/dependencies are unchanged.

- Layout/style: grids and spans, wrapping, justification, sizing constraints,
  responsive visibility/width/font size, gradients, text alignment, borders,
  cursors and paint-time motion.
- Inputs: calendars/date ranges/time, radio groups, number inputs, searchable
  comboboxes, tags, two-thumb ranges, colors, OTP, ratings, labelled fields and
  native file drop zones.
- Composition: drawers, disclosure, command palettes, toasts, avatars, alerts,
  breadcrumbs, pagination, steps, timelines, empty states and static charts.
- Assets: 65 vector icons, bounded SVG path import and registered bundled fonts
  (64 files / 256 MiB total, shared between windows).

Values and callbacks remain controlled by Rust application state. See the
[component APIs and limits](docs/COMPONENTS.md) for examples and explicit limitations;
component availability is not a claim of complete shadcn/Tailwind equivalence
or full cross-platform runtime acceptance.

## Application development (first slice)

The additive `native::Application` builder supports app-owned views/state,
explicit renderer/window configuration and bounded worker messages. New local
project templates prefer GPU without renderer flags; existing embedded/default
contracts remain unchanged. New templates use Catalog-style unified macOS chrome
with native traffic lights and an editable, safely inset draggable header;
other platforms keep their native decorations. Blur remains developer-controlled.
Independent typed navigation is demonstrated in
Catalog's Navigation page. Optional `native-desktop` adds macOS application
menus and tray menus. Independent optional `native-notifications` adds explicit
macOS alert authorization, silent local requests and removal, with bounded input
and honest accepted-not-displayed status. Settings/action/foreground delegates
and other native bridges remain pending. See [desktop notifications](docs/DESKTOP_NOTIFICATIONS.md).

```sh
cargo run --locked --features dev-tools --bin cargo-volt -- new /tmp/my-volt-app --kit-path . --template navigation
cargo run --manifest-path /tmp/my-volt-app/Cargo.toml
cargo run --locked --features native-gpu --example catalog -- --page navigation
```

Package a prebuilt macOS executable with `cargo volt bundle`: explicit identity,
selected resources/icon/privacy descriptions, dependency/deployment validation and
local ad-hoc signing. Existing outputs are never overwritten; this is not notarization.
See [macOS packaging](docs/MACOS_PACKAGING.md).


See [application development and remaining phases](docs/APPLICATION_DEVELOPMENT.md)
for APIs, optional desktop templates, rebuild/restart tooling and honest limits.


Unreleased application-policy continuation: [state-aware commands, shortcuts and
guarded closing](docs/APPLICATION_COMMANDS.md), demonstrated in Catalog's
`--page application-lifecycle`. The host remains single-window; persistence and
complete OS termination handling are not implied.

Unreleased: [native file panels and bounded notes workflow](docs/DOCUMENT_FILES.md)
are demonstrated in Catalog Documents; full OS termination, state restoration
and shipping bundles remain separate work.
