# Volt Catalog — component acceptance app

As of 2026-10-10, new component demonstrations and visual acceptance belong in
`examples/catalog.rs` and `examples/catalog/`. Existing Showcase, Desk and UI Lab
remain regression fixtures, not places to add duplicate demos.

## Analysis and bounded delivery plan

The starting point was Claude's `bbcfaf8`: 35 pages, live Preview/Code views, API
references, generated artwork, a search palette, six accents and light/dark/system
modes. It shares the existing native host; it is not another rendering framework.

This batch completes the following bounded follow-up:

1. Analyze Catalog and reproduce defects before editing (two failing AX tests).
2. Add explicit control names and composite selection/expansion semantics.
3. Route mounted focus reveal, scroll-into-view and scoped paging through AccessKit.
4. Demonstrate these in Catalog; preserve its typography, neutral surfaces and accents.
5. Fix final worker-message loss, stale overlays and argument-order/validation issues.
6. Test software/GPU rendering and document actual results and remaining gates.

No legacy Painter changes, new dependencies, JSON schema change, version bump,
Volt source/vendor synchronization or old showcase redesign is part of this batch.

## Run

```sh
cargo run --release --locked --features native --example catalog
cargo run --release --locked --features native --example catalog -- \
    --page accessibility --theme dark --accent blue
cargo run --release --locked --features native-gpu --example catalog
cargo run --release --locked --features native-gpu --example catalog -- \
    --page theming --theme dark --background opaque
```

`native` is software drawing. `native-gpu` adds feature-pruned Vello/wgpu.
`VOLT_UI_RENDERER=gpu` requires GPU drawing; `software` selects software explicitly.

- `--page <slug>` opens a page; `--code` shows its first source panel, independently
  of argument order. `--theme` accepts only `light`, `dark`, `system`.
- `--accent` accepts `zinc`, `blue`, `violet`, `rose`, `green`, `orange`.
- `--background` accepts `opaque`, `transparent`, `blur`/`material`, `compositor`.
  `compositor` explicitly opts into winit's raw blur (private macOS API).
  The default remains public `blur`
  with `native-gpu` (frosted sidebar) and `opaque` with software drawing, which
  cannot present a translucent window.
- `--window-opacity` is finite and in 0..=1, default 0.72 for translucent modes.
  The **Background opacity** slider changes the root clear directly (10–100%).
  **Tint strength** independently mixes the selected accent into that background
  (0–100%, default 0; `--tint` sets its starting percentage). Neither changes
  text/control opacity. The blur switch and Material/Compositor selector are live.
  Public material contributes its own system tint; select Compositor for the
  Terminal-style opacity behavior. Its radius is backend-controlled, not a slider.
- `--palette`, `--dialog`, `--native-chrome` remain available; `--compact-chrome`
  selects the earlier compact overlay for comparison.
- Shared `--snapshot`, `--size`, `--scale`, `--bench`, `--bench-target` remain available.

On macOS, Catalog's default overlay uses public AppKit unified chrome (macOS 11+).
The traffic lights sit over a 48-point drag strip at the top of the full-height
sidebar, so the content header needs no leading inset. Native traffic lights
retain their system-drawn size and shape in light/dark modes; no custom buttons
or per-button frame transforms are used. `--native-chrome` retains the standard
system titlebar. Toolkit applications with short headers can choose
`WindowChrome::CompactOverlay` to preserve the earlier compact geometry.
`VOLT_UI_TRACE=1` includes read-only native button geometry diagnostics.
Large-display GPU animated-zoom acceptance is not complete: see the recorded
surface reconfiguration timeout in
[Application development](APPLICATION_DEVELOPMENT.md#macos-chrome-follow-up).

A translucent root must not be covered by an opaque full-window container. In
translucent mode the sidebar sits directly on the window material and the header
and page live on a solid sheet with rounded leading corners, so body text never
reads over blur. Opaque mode keeps the same layout with a tinted sidebar.
Software/native fallback stays opaque. Decorated macOS GPU windows use public
AppKit `NSVisualEffectView` by default; unsupported/borderless material requests
log transparency without material. Explicit Compositor mode uses winit blur,
checks Reduce Transparency on redraw, and returns to an opaque clear when that
accessibility preference is enabled. It is not an App Store-safe claim. Other compositors may ignore blur. Application packaging,
permission and dependency review still gate App Store distribution claims.
See [validation and platform gates](VALIDATION.md).
The in-app landscape demo illustrates surface alpha, **not desktop blur**.
Glass Browser-style in-app GPU backdrop blur is still pending; native window blur
must not be represented as blur of application content.
See [window opacity/tint/blur](SURFACES_THEMES_AND_SCROLLING.md).

The pinned wgpu 26 Metal backend advertises `PostMultiplied`, but macOS consumes
premultiplied surface pixels. Volt compensates only in the Metal presentation
shader; otherwise light-mode translucent fills wash to white and antialiased
text/borders lose their intended appearance. This is not a tint or font change.
`VOLT_UI_TRACE=1` now reports `VOLT_UI_SURFACE` pixel encoding and `VOLT_UI_FONTS`
resolved SF Text/Display/Mono families. See the
[surface encoding policy](GPU_RENDERING.md) for evidence and limits.

## Screen structure and design

```text
+-----------------------------------------------------------------------+
| Volt UI    Docs  Components  Themes  Patterns   Search   Accent   Mode  |
+---------------------+-------------------------------------------------+
| Filter              | Docs > Getting started > Accessibility          |
|                     |                                                 |
| Getting started     | Accessibility                                   |
|   Introduction      | Named controls, selection and mounted reveal    |
|   Theming           |                                                 |
| > Accessibility     | Names and selection           [Preview | Code]  |
|   Layout            | +---------------------------------------------+ |
|                     | | Profile name [Ada Lovelace]       [bell]    | |
| Components          | | [Overview | Details]                       | |
|   Button / Bars     | | [Quiet | Balanced | All]                   | |
|   Tabs / Fields     | +---------------------------------------------+ |
|   Scroll / Data     |                                                 |
|   Images / Effects  | Scroll and focus reveal       [Preview | Code]  |
|                     | +---------------------------------------------+ |
| Patterns            | | Actions 1..12 in a bounded scroll viewport | |
|   Background tasks  | +---------------------------------------------+ |
|   Sign-in / Settings| API reference                                   |
+---------------------+-------------------------------------------------+
```

- **SF**: interface text and headings; **SF Mono**: Rust source and technical labels.
  Fonts are resolved from the host; SF is not redistributed with this example.
  Platforms without it retain the renderer's system fallback.
- Dark zinc surfaces: `#09090B`, `#18181B`, `#27272A`; text `#FAFAFA`, muted `#A1A1AA`.
  Light surfaces: `#FFFFFF`, `#F4F4F5`, `#E4E4E7`; text `#09090B`, muted `#71717A`.
- Keep semantic colors, small control corners, larger preview/card corners and the
  original restrained spacing. Source remains selectable and read-only.
- At narrower desktop widths, header navigation can scroll independently while
  search/accent/mode controls retain their places. This is not mobile adaptation.

## Declarative names and semantics

```rust
text_field(&app.name)
    .key("profile-name")
    .accessible_label("Profile name")
    .placeholder("Your full name")
    .on_change(|app: &mut App, value| app.name = value);

button("")
    .key("notifications")
    .accessible_label("Show notifications")
    .on_click(|app: &mut App| app.show_notifications = true);
```

Names do not change painted labels, layout or callbacks. Explicit names must be
nonempty, control-free and at most 8 KiB. Placeholders remain independent hints;
password text/length is still withheld. Select/menu names attach to their actual
interactive trigger rather than an inert outer wrapper.

Tabs and segmented choices expose tab/radio groups and selected states. Select
options and mounted virtual rows expose list options and selection; virtual lists
report their total size and zero-based mounted row index. Table targets receive
bounded, Unicode-safe joined cell names and selection. Tree items expose selection,
expansion and zero-based logical depth; disclosure targets say Expand/Collapse.
Tables now expose header/cell ancestry, dimensions, indices and sort direction.
Trees preserve logical TreeItem parents. Complete tree arrow-key navigation and
spoken screen-reader acceptance remain separate.

Mounted clipped nodes remain available to assistive technology. Focus reveals the
control from the innermost scroll view outward, at the actual DPI scale. It does
not invent or mount arbitrary virtual records. Closed/unmounted controls and modal
background targets remain unreachable.

AccessKit routes Focus, Click, SetValue, Increment/Decrement, ScrollIntoView and
ScrollUp/Down/Left/Right. Paging moves only the addressed viewport by 80% of its
inner extent, clamps to its range and updates virtual windows through the existing
callback path. Invalid targets/directions are no-ops before focus/state changes.

## Worker and application-state fixes

Intermediate progress may coalesce under queue backpressure. Completion retries
on the **worker thread** until enqueued, cancelled or closed; it never blocks the
UI thread. Cancellation/generation checks still discard stale results, and dropping
the app cancels outstanding retries. Page navigation clears page-owned dialogs as
well as menus/popups. Notification identities wrap without a debug overflow panic.

## Validation commands and limits

```sh
cargo test --locked --all-features --all-targets
cargo test --locked --all-features --doc
cargo test --locked --features native-gpu --example catalog -- \
    --ignored --nocapture
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo +1.87.0 check --locked --all-features --all-targets
VOLT_UI_RENDERER=software cargo run --release --locked --features native-gpu \
    --example catalog -- --page table --theme dark --bench 60 \
    --bench-target invoices-column-0 --size 1440x920 --scale 1
```

Catalog tests cover every page in both themes, Preview/Code, mounted overlays,
CLI/state/worker contracts, named callbacks and nested focus reveal, plus full vs
incremental software pixels. The explicit hardware test checks every page at 1x/2x
and translucent root alpha, and rejects silent GPU fallback.

Native macOS AX inspection and activation are distinct from a spoken VoiceOver
walkthrough. Headless benchmark/readback timings are **not** OS input latency,
CPU/GPU utilization, energy, FPS or a cross-framework comparison. Full AX snapshots
now include mounted clipped nodes; measure active assistive technology separately.
This batch is semantic/interaction hardening, not a claimed renderer speedup.

Still separate: VoiceOver/Narrator/Orca workflows, physical IME/device acceptance,
Linux/Windows runtime, native energy/memory/latency profiling, dense mixed emoji/image
atlas stress, device loss and approximation-based Vello buffer admission. Fine-grained
recomposition, global theme metrics, button icon slots and indeterminate checkboxes
are future API work, not silently marked delivered here.

## First follow-up results — 2026-10-10 (before the data-view overhaul)

- All-feature/all-target tests: **423 passed**, 11 ignored hardware/manual fixtures.
  Doc tests: **28 passed**, 1 ignored. Default tests: **40 passed**, 1 ignored.
  Pure `declarative` tests: **65 passed**, 1 ignored. The AX suite contains 12 tests.
- Strict Clippy passed for all features/all targets and pure `declarative`/all targets.
  Rust 1.87 all-feature/all-target checking, warning-denied rustdoc, formatting,
  whitespace checks and all-feature package verification passed.
- Explicit Catalog hardware acceptance: **1 passed**. Every one of the 36 pages
  rendered in dark appearance at 1x/2x without an over-budget/software fallback;
  the translucent root preserved its expected alpha. This includes blocking GPU
  readback and is not a native presentation or energy benchmark.
- Native macOS AX inspection confirmed named fields/controls, selected tabs and
  radio choices. Activating mounted, clipped **Action 12** revealed it, retained
  focus and reached its Rust callback. A GPU-required blur-window launch succeeded;
  this does not independently prove compositor blur quality or spoken VoiceOver.
- An isolated Volt workspace snapshot matched the canonical source and its default
  workspace check passed. The scratch consumer's dependency constraint had to be
  changed from `^0.1.0` to `^0.2.0`, reflecting the **previous** minor-version bump.
  No real Volt checkout was changed; integration/visual/runtime acceptance is still
  separate. Default dependencies and legacy Painter source are unchanged this batch.

Three paired release/software headless runs compared Claude's `bbcfaf8` with this
batch: table-sort callback, 1440x920 at 1x, 60 samples after four warmups per run.
Medians across runs:

| Release-to-pixels | Before | After | Difference |
|---|---:|---:|---:|
| p50 | 4.782 ms | 4.803 ms | +0.021 ms |
| p95 | 5.439 ms | 5.591 ms | +0.152 ms |

No speedup is demonstrated. These receipts measure a headless callback/recomposition/
software-render path, not OS input dispatch, idle CPU, GPU utilization or power.
Local receipts are under ignored `work/catalog-review-2026-10-10/`; the earlier
unpaired timings are diagnostic, not the final paired comparison.

## Table and tree visual overhaul — 2026-10-10

The Table page contains **two live previews**, not screenshots:

```text
Rounded table                      .rounded(12.) (default)
  +-----------------------------+
  | Invoice  Status      Amount |
  |-----------------------------|
  | INV001   Paid       $250.00 |
  | ...                         |
  +-----------------------------+

Square table                       .rounded(0.)
  +-----------------------------+
  | Invoice  Status      Amount |
  |-----------------------------|
  | INV001   Paid       $250.00 |
  | ...                         |
  +-----------------------------+
```

ASCII cannot represent the exact corner curvature; use the live previews. Each
has its own stable table/row keys and Preview/Code controls. Both use the same
controlled Rust selection and sorting state, so changing one updates both.
Scroll the page to see the second preview on smaller windows.

Tables reserve only a 1px border-stroke inset (no separate 4px gutter), with a
40px header, 44px rows and single-pixel row
rules, with no competing text/hit overlays. Header top corners and the sorted
last row's bottom selection/focus corners follow the outer table silhouette.
Square frames also use square rows. `.align(Align::End)` aligns numeric values and their header together;
column widths are minima and wider table widths grow columns proportionally.
Narrow/height-bounded tables scroll on both axes rather than squash columns.
Table semantics include ColumnHeader/Cell roles, row/column indices, total
counts and sort direction; inert headers/rows do not advertise click actions.

Trees have 6px frame insets, 36px row controls and vector disclosure chevrons,
not clipped font glyphs. Indentation no longer consumes the row's selection
background. Expansion and selection have independent callbacks; collapsed
children unmount. Semantic children belong to their logical TreeItem parent.

Regression tests exercise rounded/square first and last rows in both light/dark
modes at 1x/2x. Software comparisons are pixel-exact on the outer stroke band wholly outside
the content inset; fractional pixels straddling border/content are excluded
because they blend both surfaces. Corner/outline geometry tests and actual
header/last-row silhouette tests complement those checks. GPU comparisons
allow at most one readback channel LSB for Vello antialias quantization, and reject
larger edge changes and over-budget fallback. Callback/recomposition tests verify
sorting and shared selection in both Catalog previews. These are not native
energy, input-latency or spoken VoiceOver measurements.

### Data-view validation receipts

- All-feature/all-target tests: **429 passed**, zero failures, 12 ignored fixtures.
  Doc tests: **28 passed**, 1 ignored. Explicit Catalog GPU tests: **2 passed**.
- Strict all-feature/all-target Clippy, Rust 1.87 checks, pure declarative Clippy,
  default/declarative tests, warning-denied rustdoc and all-feature package
  verification passed. The 15-test data component suite includes overflow,
  column alignment, cell ancestry and independent vector tree disclosures.
- Updated native GPU Catalog: square Amount-header sorting reordered both previews;
  square last-row and rounded-row activation reached Rust and updated both status
  labels. Light/dark screenshots showed square and rounded frame treatments.
  Earlier native tree expand/collapse/selection inspection also passed; no spoken
  screen-reader or cross-platform runtime acceptance is implied.
- Three release/software headless runs of the **new two-preview Table page**,
  1440x920 at 1x, 60 samples plus four warmups: median across runs **p50 4.918ms**,
  **p95 5.267ms** release-to-pixels; layout/composition p50 1.923ms and raster
  p50 2.660ms. The workload differs from the earlier single-table comparison,
  so these are current-workload receipts, **not a speedup claim**. Redundant
  per-cell clip masks were removed while retaining existing rectangle/outer
  rounded clips. Receipts are in ignored `work/data-overhaul-2026-10-10/`.

### Rounding correction after visual feedback — 2026-10-10

The newer colors/dividers/typography remain intact. The four-pixel internal gutter
and independently hard-coded four-pixel header/last-row corners were the wrong
geometry: they made the contents look detached from the table's rounded frame.
The table now reserves only its one-pixel stroke, uses a continuous corner outline,
and derives the header top and last-row bottom curves from the table's own radius.
Selected/hovered/focused surfaces follow those curves after sorting; square tables
remain square. Interior cell padding is unchanged. Parent rounded clipping remains
active through the existing scroll viewport; no per-cell masks were added.

New regressions record the actual fill/focus outline corners at radii 0/12/20,
DPI 1/2 and ascending/descending sorts. A Catalog raster test verifies real pointer
row/header callbacks and that header/last focus do not escape the rounded silhouette.
Native GPU screenshots confirmed header focus and post-sort last-row focus/selection
in light/dark. This replaces the former gutter-based approach, not a new restyling.
Receipts are under ignored `work/table-rounding-2026-10-10/`.

Validation for the rounding correction: **431 all-feature/all-target tests passed**,
12 fixtures ignored; data-component suite **16 passed**, Catalog **11 passed**,
and both explicit Catalog GPU checks passed. Strict all-target/all-feature Clippy
and Rust 1.87 checks passed. These receipts replace neither OS screen-reader nor
cross-platform acceptance gates.

## Application-foundation follow-on (2026-10-10)

Catalog now has 37 pages, including Navigation. The independent label sidebar,
centered rail and bottom destinations share controlled state; callbacks, disabled
destinations and light/dark rendering were accepted on macOS Vello/Metal.
No new visual demos were added to UI Lab or the old showcases. Optional application
scaffolding and desktop adapters are a separate additive batch; see
[application development](APPLICATION_DEVELOPMENT.md) for dependencies and limits.


## Application lifecycle (unreleased continuation)

`--page application-lifecycle` adds an editable in-memory draft, dirty badge,
state-aware command rows, a custom command palette and Save/Discard/Cancel.
Cmd/Ctrl+S/W/K and the native close button use real Application policies. Enable
`native-desktop` on macOS for the owned guarded Quit menu/Cmd+Q; default system
Quit, Dock/forced termination and disk persistence are not implemented by this
example. See [application commands](APPLICATION_COMMANDS.md).

## Documents and counted commands (Unreleased)

Catalog now has **40 pages**. `--page documents` adds native macOS Open/Save panels
and a real 8 KiB UTF-8 notes workflow with asynchronous I/O, conflicts/errors,
Save As and guarded closing. Use `native-gpu,native-desktop` for GPU/native panels.
The Application lifecycle page retains its memory-only draft and adds a counted
Cmd+Shift+P execution witness. See [document files](DOCUMENT_FILES.md) for limits,
APIs and verified native outcomes. UI Lab and older showcases are unchanged.

## Running Catalog as a macOS app

The optional `cargo volt bundle` command packages a prebuilt Catalog executable
with explicit identity, selected resources/icon and local ad-hoc signing. Launch
Services and native file-panel resource reading were accepted on the test Mac;
no new shell or theme is imposed. See [macOS packaging](MACOS_PACKAGING.md) for
commands, limits and the separate Developer ID/notarization release process.


## Desktop services (Unreleased)

The new Desktop services page demonstrates explicit local-notification permission,
bounded title/body input, delayed silent submission, visible errors and removal.
Build with `native-notifications` and launch a packaged `.app`; an unbundled Cargo
run reports a missing-bundle error rather than asking permission automatically.
With `native-desktop`, a V-shaped status item shares the same command registry and
count witness, with state-aware send/remove enablement. No UI Lab/old Showcase
changes. See [API, safety limits and native acceptance](DESKTOP_NOTIFICATIONS.md).
