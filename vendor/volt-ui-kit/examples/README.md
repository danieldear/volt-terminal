# Runnable examples

All native examples share the library host/runtime, rather than implementing
separate windowing or rendering systems. `native` uses software drawing;
`native-gpu` uses Vello GPU drawing with cosmic-text shaping and software fallback.
Use `VOLT_UI_RENDERER=gpu|software` to compare; see
[GPU rendering/dependencies](../docs/GPU_RENDERING.md). Widget backdrop blur and
full platform accessibility acceptance remain future work (the AccessKit adapter
and bounded semantic/actions foundation are implemented).

**Primary acceptance app:** `catalog`. New capability demos go here, not UI Lab
or the older Showcase. See [Catalog](../docs/CATALOG.md).

```sh
cargo run --locked --example gallery
cargo run --locked --features native --example rust_counter
cargo run --locked --features native --example rust_settings
cargo run --locked --features native --example rust_keyed_list
cargo run --locked --features native --example rust_layout
cargo run --locked --features native --example rust_responsive
cargo run --locked --features native --example rust_effects
cargo run --locked --features native --example native_window
cargo run --locked --features native --example panels_and_layout
cargo run --locked --features native --example buttons_and_events
cargo run --locked --features native --example declarative_ui
cargo run --locked --features native --example complete_app
cargo run --locked --features native --example components
cargo run --locked --features native --example northbound
cargo run --locked --features native --example specimen
cargo run --locked --features native --example catalog
```

| Example | Demonstrates | Expected result |
|---|---|---|
| rust_counter | Primary Rust UI, inline callbacks, disabled reset, directional padding/gap | Action updates Rust state and visible count |
| rust_settings | Reusable Rust functions, conditional panel, typed booleans/typography | Settings and details reflect inline callbacks |
| rust_keyed_list | Rust loops, stable keys, captured IDs | Reverse/remove target correct items; removed focus clears |
| rust_layout | HStack/VStack aliases, nine placements, selector padding/gap, pill/fractional rounding, dividers/spacers | Placement switches move the content; Add/Reset use typed Rust callbacks |
| rust_responsive | Responsive Stack direction/padding/gap, fixed stacks, layered ZStack, typed callbacks | Resize across768 logical pixels; front button wins overlap, exposed back area reaches back button |
| catalog | Documentation-style showcase of every component (shadcn/ui-like): sidebar, search palette, live previews with Preview/Code tabs, API tables, themes (Zinc/Blue/Violet/Rose/Green/Orange, light/dark), patterns and a real background worker | `--page table`, `--theme dark`, `--accent violet`, `--code`, `--palette`, `--dialog`, `--snapshot PNG` |
| specimen | Every component at its default styling, including button variants/sizes and disclosure triggers, for design review and visual regression | Dark by default; `--theme light`, `--open-overlays`, `--dialog`, `--snapshot PNG` |
| northbound | App-scale trip planner using every Rust UI component: tabs, segmented controls, toolbars, joined button bars, text field/area, slider, select, dialog, checkbox/toggle, canvas icons, clipped image backgrounds, themes/accents, hover/press/focus effects, keyed animations and fade transitions. Photos are generated in Rust at startup | Select stops, search, plan days, write journal entries, reserve a stay and switch Polar night / Midnight sun themes. Flags: `--screen route\|days\|journal\|settings`, `--theme light`, `--accent ember`, `--reserve` |
| gallery | Existing renderer-independent recipes | Static SVG files in target/gallery |
| native_window | Native lifecycle, resize, DPI | One window; close exits cleanly |
| panels_and_layout | Nested Panels, Row/Column, padding/gap/flex | Resize adjusts geometry; button prints a Rust callback |
| buttons_and_events | Hover/press/disabled/focus, closure + function callback | Increment changes label; Reset enables/disables from Rust state |
| declarative_ui | Editable JSON and F5 validation/reload | Valid edit updates UI; invalid edit retains last valid document |
| complete_app | Styled solid workbench | Preferences update Rust state, Apply enables after edits, status reports actions |
| components | Editable typography/colors and new controls | Checkbox/Toggle events update Rust; badges, separators and button styles |
| render_benchmark | Fixed original scene, CPU timings | Run with --bench COUNT; no window or performance parity claim |

## Rust-first authoring

Start with `rust_counter`, then `rust_settings`, `rust_keyed_list` and `rust_layout`. UI lives in
Rust; no JSON file or callback-name registry is needed. See [API/design](../docs/RUST_UI.md).
Source edits require rebuild. F5/--document apply only to JSON examples below.
The Rust examples support --snapshot, --size WIDTHxHEIGHT, --scale, --bench and --bench-target; their
benchmark includes Rust composition/validation/installation, not just bindings.

## Optional JSON edit/reload

JSON files live in `examples/ui`. Initial example loads from the canonical folder
via CARGO_MANIFEST_DIR. F5 reloads that file without rebuilding Rust. No polling
watcher is installed. Callbacks remain registered once in Rust; reload only swaps
the validated tree. Failure changes the window title and prints path + diagnostic
on stderr while preserving the prior UI. Syntax errors have line/column; semantic
errors have tree/property path or widget ID. Rust callback changes require rebuild.

- Tab / Shift+Tab: cycle enabled buttons, checkboxes and toggles in document order.
- Enter / Space: activate the focused control on matching release.
- Escape / focus loss: cancel a pending press.
- Pointer press then release outside: no activation.
- F5: reload (not supported by native_window's embedded-only document).

## CLI and reproducible screenshots

```sh
cargo run --locked --features native --example complete_app -- --help
cargo run --locked --features native --example complete_app -- --document examples/ui/complete_app.json
cargo run --locked --features native --example complete_app -- --snapshot target/complete-app.png
cargo run --locked --features native --example complete_app -- --snapshot target/complete-app-2x.png --scale 2
```

Snapshot mode runs the same layout/raster adapter without a window. It is not
native input/lifecycle proof. Fonts come from the system, so snapshots are not
portable pixel-golden references. On Linux install a system font (e.g. DejaVu Sans),
a display server, and winit/softbuffer platform build prerequisites for native runs.
The native adapter uses Avenir Next for `system` on macOS when available and
generic sans-serif otherwise. Font family/weight/italic are shared between
measurement and drawing. Headless tests do not require a display. macOS runtime is the initial QA target;
Linux/Windows runtime and assistive technology testing remain separate gates.

## Performance diagnostics

```sh
cargo run --locked --features native --example render_benchmark -- --bench 60 --scale 2
VOLT_UI_TRACE=1 cargo run --locked --features native --example complete_app
```

The benchmark reports warm CPU p50/p95 stage timings, excluding OS event queue and
presentation. Native tracing measures handled event-to-present-return, not display
scan-out. Both run on demand; no idle timer is added. See [measurements](../docs/PERFORMANCE.md).

## Boundaries

See [responsive Rust layouts](../docs/RESPONSIVE_UI.md) for narrow/wide commands,
viewport breakpoints and layered hit semantics. Width adaptation does not imply
scrolling or minimum-height usability.

One window; constrained flex layout (not CSS), explicit shape/path clipping and
opt-in nested scrolling. Editing/IME/clipboard, sliders/selects and modal dialogs
have delivered foundations; see [components](../docs/COMPONENTS.md). Platform
menus, virtualized lists, widget backdrop blur, multi-window and platform
accessibility completion remain open. Buttons clip when too narrow.

### Placement-demo small-window limit

`rust_layout` is a dense showcase, not a scrollable/adaptive screen. At the host's
320x240 minimum, its preview can be shorter than fixed padding and hide Add/Reset;
hidden controls are excluded from focus/input. Real-font matrix tests verify
visible callbacks at640x480 and960x760 across1/1.25/1.5/2/3x, but native resize /
readability acceptance is still separate. Keep a comfortable window size until
this particular placement fixture gains adaptive layout. Other workbench sections
already demonstrate scrolling.

### Effects / integrated chrome

`rust_effects`: optional blurred panel shadow, animated hover glow/border, pressed
opacity, square/default/selected-side buttons, keyed fade notice and custom
draggable macOS header retaining native controls. `--chrome native` compares
standard decorations; `--reduced-motion` settles appearance without motion.
Header controls must click rather than drag. Overlay uses native decorations on
non-macOS. Shared Rust CLI accepts both options; screenshots show settled motion.
See [effects/windowing](../docs/EFFECTS_AND_WINDOWING.md) for limits and OS gates.

UI Lab `--section runtime` and `--section data` remain regression routes;
new extended-component and worker-message demonstrations belong in Catalog.

## Current component and glass-window acceptance

New polished tests extend **Catalog**, not UI Lab or the older app showcases.
Those examples remain regression fixtures; do not duplicate new demonstrations.

```sh
cargo run --release --locked --features native-gpu,native-desktop --example catalog
cargo run --release --locked --features native-gpu --example catalog -- --background opaque
cargo run --release --locked --features native-gpu --example catalog -- --page form-inputs --theme light
```

Catalog defaults to public Material where supported by the GPU/native host.
Opacity and tint are independent; foreground controls remain opaque. Private
Compositor blur is an explicit opt-in with unresolved desktop-corner acceptance.
See [Catalog](../docs/CATALOG.md), [window glass](../docs/SURFACES_THEMES_AND_SCROLLING.md)
and [validation](../docs/VALIDATION.md). Existing app-scale routes remain documented
in [Showcase apps](../docs/SHOWCASE_APPS.md).
