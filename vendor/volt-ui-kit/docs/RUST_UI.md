# Rust-first declarative UI

**Implemented first vertical slice, 2026-10-08.** Rust is the primary UI authoring
API. JSON is an optional data-driven frontend, not the underlying authoring
language you must use. No WYSIWYG editor, QML interpreter, embedded JavaScript,
proc-macro compiler or JSON serialization bridge is involved.

## A complete small application

Enable `native` for a standalone window, or `rust-ui` for composition with your
own host. `rust-ui` currently includes `declarative` and its optional serde
libraries because both frontends share the validated tree/runtime. Default
builds still depend only on unicode-width.

```rust
use volt_ui_kit::{
    native::{run_ui, NativeOptions},
    ui::{column, prelude::*},
};
#[derive(Default)]
struct App { count: usize, preview: bool }
fn view(app: &App) -> Element<App> {
    column![
        text("Workspace").font_size(28.).font_weight(700),
        panel![
            text(format!("Count: {}", app.count)).font_family("monospace"),
            row![
                button("Add").key("add")
                    .on_click(|app: &mut App| app.count += 1),
                toggle("Preview", app.preview).key("preview")
                    .on_change(|app: &mut App, value| app.preview = value),
            ].gap(12., Axis::Both),
        ].padding(20., Edge::All).gap(14., Axis::Both).background("#1C2533").rounded(10),
    ].padding(24., Edge::All).gap(18., Axis::Both).font_family("system").into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_ui(NativeOptions::default(), App::default(), view)
}
```

Import `column` explicitly alongside the prelude: Rust's standard `column!`
source-location macro otherwise conflicts with a glob-imported UI macro.
`row!`/`column!`/`panel!` and `hstack!`/`vstack!` only convert heterogeneous
builders into child vectors;
the remaining code is ordinary Rust. No custom language/toolchain is required.

## State, events and composition

```text
Initial Rust App state
         |
         v
   view(&App) -> Element<App>
         |
         v
   build() -> validated Document + directly connected Rust closures
         |
         v
   shared layout / text measurement / focus / hit geometry
         |
         v
   existing component recipes -> native raster/presentation
         ^
         |
Mouse/key action -> closure(&mut App) -> view(&App) -> build + install

Hover, resize, expose, cancelled release -> redraw/layout only
                                          NOT a view-body call

Legacy Volt models/paint -> legacy Painter adapter (unchanged path)
```

The host calls the view once initially and once after each successfully
**dispatched callback**, including a callback that leaves state unchanged.
The body receives `&App`; application state remains in the Rust context. It
should describe UI without side effects. Callbacks run synchronously on the UI
thread. Queue expensive work outside them, rather than blocking rendering.

This is **whole-tree recomposition**, not dependency-tracked state/signals,
subtree diffing or retained widget-local state. Merely changing state from a
background task does not automatically notify this first native runner; a public
external-update/event-proxy API is still future work. Native rendering remains
CPU/software-based, Solid-only, one window and event-driven (`ControlFlow::Wait`).
Opt-in animations schedule active-only ticks; no idle polling/redraw timer runs.
See [effects and windowing](EFFECTS_AND_WINDOWING.md).

## Seamless Rust callback integration

- `button(label).on_click(FnMut(&mut App))`
- `checkbox(label, checked).on_change(FnMut(&mut App, bool))`
- `toggle(label, checked).on_change(FnMut(&mut App, bool))`

Functions and inline closures both work. The checkable event proposes the next
value; assign it to application state in Rust. Callbacks/captures are owned
`'static`; capture an ID/string/Arc by `move`, not a borrowed view-body reference.
There are no user-facing callback-name strings or manual handler registration.
Internal names connect the lowered tree to the existing typed handler registry.

```rust
fn reset(app: &mut App) { app.count = 0; }
let reset_button = button("Reset").on_click(reset);
let item_id = 42_u64;
let remove_button = button("Remove").on_click(move |app: &mut Tasks| {
    app.items.retain(|item| item.id != item_id);
});
```

Store durable values in App, not in callback captures you expect to survive
recomposition: successful composition creates fresh closures. Capturing stable
item identity prevents reordering from redirecting events to another item.
Wrong callback argument types/component-only methods fail at Rust compilation.
Missing callbacks fail during `build()`, even for a disabled control, rather than
silently creating a no-op interactive component. Panics in application callbacks
are not sandboxed/caught; Rust application logic remains responsible for them.

## Components, conditionals and lists

Reusable components are ordinary functions returning `Element<App>`:

```rust
fn heading(title: &str) -> Element<App> {
    text(title).font_size(24.).font_weight(700).into()
}
let status: Element<App> = if app.preview {
    badge("Preview enabled").into()
} else {
    text("Preview disabled").into()
};
let items: Vec<Element<Tasks>> = app.items.iter().map(|item| {
    text(item.title.clone()).key(format!("item-{}", item.id)).into()
}).collect();
let list = column(items).gap(8., Axis::Both);
```

Use an explicit `Element<App>` result for heterogeneous conditional branches.
Builder-to-Element conversions use normal Rust type inference; `.into()` inside
an untyped conditional can need that annotation.

`.key(...)` supplies **globally unique** identity. Keys accept 1..=128 ASCII
letters/digits/underscore/hyphen. Duplicate/empty/invalid keys fail at build.
Unkeyed nodes get deterministic structural IDs (`auto-root-...`); these work for
static trees, **not stable identity for insertable/reordered interactive lists**.
Explicit keys also must not collide with generated IDs. Key each dynamic control
and dynamic component root deliberately.

On successful replacement, focus survives only when its key still names the
same enabled interactive **kind**. Button-to-Toggle, removal or disabling clears
focus. Pending pointer/key presses and hover are cancelled, so stale releases
cannot activate a replacement. Keyboard navigation follows current tree order.

## Layout and typography

Implemented components: Panel, Row, Column, Stack, ZStack, Text, Button, Checkbox, Toggle, Badge,
Separator/Divider and Spacer. `hstack`/`vstack` (functions and macros) alias
Row/Column; they preserve the same layout defaults, not SwiftUI default alignment.
Containers support `.padding(value, Edge)`, `.gap(value, Axis)`, `.flow(Flow)` and
`.align(Align)`; all builders support
`.width`, `.height`, `.flex`, `.key` and common visual modifiers:

| Area | Modifiers |
|---|---|
| Text | `font_family`, `font_size`, `font_weight`, `font_style(FontStyle::Italic)`, `italic(bool)`, `color` |
| Surface | `background`, `border_color`, `border_width`, `rounded(number/preset)`; numeric `radius` alias |
| Interaction | `accent_color`, Button `primary`, Button/checkable `enabled` |

Fonts/text color inherit through containers; surface/accent/border/radius do not.
Text measurement and drawing use the same resolved font/style. Colors are
`#RRGGBB` or `#RRGGBBAA`. Geometry is logical pixels, scaled by the shared runtime.
Font size 8..=96; weight 100..=900 in steps of 100; border 0..=16; radius 0..=256;
finite geometry 0..=16384; text/labels up to 8 KiB; tree <=4096 nodes, depth <=48 (whole-tree rebuild per action: ~25ms at 4096 nodes).
These constraints/styles are validated at build, not all at Rust compilation.
See [styling semantics](STYLING.md). Font availability/fallback is host-dependent.

This is basic constrained flex/wrap layout, not CSS/Compose layout parity.
Overflow is clipped/shrunk, not scrolled. There is no text editor/Input/IME,
virtualized list, menus, multi-window adapter or platform accessibility yet.

## Directional padding and gap

All values are logical pixels on Panel/Row/Column (HStack/VStack) builders:

```rust
use volt_ui_kit::ui::prelude::*;
let content: Element<()> = vstack![
    text("Count"),
    hstack![text("Left"), text("Right")].gap(12., Axis::Horizontal),
]
.padding(24., Edge::All)          // all four inner edges
.padding(20.5, Edge::Horizontal)  // left + right
.padding(12., Edge::Vertical)     // top + bottom
.padding(8., Edge::Top)
.padding(16., Edge::Right)
.padding(20., Edge::Bottom)
.padding(10., Edge::Left)
.gap(12., Axis::Vertical)         // between this stack's children
.into();
```

```text
+-------------------------------+
|           top padding         |
| left   [child A]        right  |
|            vertical gap       |
|        [child B]               |
|          bottom padding       |
+-------------------------------+

HStack/Row: [child A] -- horizontal gap -- [child B]
```

- `.padding(v, Edge::All)` sets every edge and resets earlier overrides.
- `Edge::Horizontal` sets left/right only; `Edge::Vertical` sets top/bottom only.
  Individual `Top/Right/Bottom/Left` change only the selected edge.
- **The latest setter affecting an edge wins**. For example,
  `.padding(8., Edge::Top).padding(20., Edge::Vertical)` results in top/bottom20;
  the reverse leaves top8/bottom20. These are setters, not additive wrapper layers.
- `.gap(v, Axis::Both)` sets spacing for both axes and resets prior overrides.
  Row/HStack uses horizontal, Column/VStack/Panel uses vertical. A directional
  gap changes only that axis; it does not propagate to nested containers. These
  layouts do not wrap children into additional rows/columns.
- Gap is only between children, never before the first/after the last. There is
  deliberately no `Edge::Top` gap: the compiler rejects Edge where Axis is needed.
  Use padding for outer-edge space.
- Left/right are absolute directions, not RTL-aware leading/trailing.
- Spacing remains finite0..=16384; every stored override, even an unused axis,
  is validated at build. Excessive padding hides/clips children.

This deliberately corrects the **in-development Rust UI** API to selectors,
replacing the previous scalar methods and separate directional helper methods.
Rust does not support method overloading by argument count. Use `Edge::All` /
`Axis::Both` when no directional restriction is needed. Existing **Volt legacy
widget/Painter APIs are not changed**. Ordinary Rust uses qualified enum values
such as `Edge::Horizontal`, not Swift's leading-dot `.horizontal` syntax; no
custom parser or proc macro is needed. See [Rust paths](https://doc.rust-lang.org/reference/expressions/path-expr.html).

The optional JSON frontend keeps its scalar properties and matching directional
numeric properties. Its precedence is edge > axis > scalar, independent of JSON
property order. Rust call order is resolved by the builder before installation.

## Stack direction and content placement

Direction and placement are separate concepts:

| API | Meaning |
|---|---|
| `vstack![...]` / `column![...]` | Stack children vertically |
| `hstack![...]` / `row![...]` | Stack children horizontally |
| `panel![...]` | A vertical surface/container |
| `stack![...].direction(...)` | Fixed/responsive horizontal or vertical arrangement |
| `zstack![...]` | Independent overlapping layers, back to front |
| `.flow(Flow::Center)` | Place the child group centrally inside available content bounds |
| `.align(Align::Center)` | Existing cross-axis-only alignment, main-axis start retained |

```rust
use volt_ui_kit::ui::prelude::*;
let preview: Element<()> = panel![
    vstack![
        text("Centered content").font_size(24.).font_weight(700),
        hstack![text("Left"), text("Right")].gap(12., Axis::Horizontal),
    ].gap(12., Axis::Vertical).flow(Flow::Center),
]
.height(200.)
.padding(20.5, Edge::Horizontal)
.padding(16., Edge::Vertical)
.flow(Flow::Center)
.rounded(12)
.into();
```

```text
Content placement (inside padding):
+-------------------------------------------+
| TopLeft             Top          TopRight |
|                                           |
| Left               Center           Right |
|                                           |
| BottomLeft         Bottom     BottomRight |
+-------------------------------------------+

Center example:
+-------------------------------------------+
|                                           |
|             [Centered content]            |
|                [Left][Right]              |
|                                           |
+-------------------------------------------+
```

`Top` means top-center; `Left` means center-left, and similarly for `Bottom` /
`Right`. Corners select both axes explicitly. Flow offsets the group on its main
axis, then aligns individual children on its cross axis. It changes **placement**,
not stack direction, and is **not a wrapping-flow layout**.

Explicit Flow disables the column's implicit cross-axis stretching, so content
can actually center or align to an edge. Explicit `Align::Stretch` still fills
unspecified cross sizes; other Align offsets yield to Flow regardless of call
order. Width/height constraints and padding remain honored. With no Flow set,
existing start/align/stretch behavior is unchanged. Placement needs unused space:
an intrinsically sized container may have nothing to offset; give it height /
width / flex or use the root window's bounds. Flex/spacers consume remaining main
space before Flow computes an offset.

`stack!` defaults vertical and is the only builder with `.direction(...)`.
Container padding/gap accept scalar or typed responsive values. `zstack!` defaults
to centered layers with independent Flow placement. See [responsive layout
contracts and runnable example](RESPONSIVE_UI.md) for breakpoints, logical units,
layer hit policy, validation and deferred container queries/scrolling.

## Rounding, divider and spacer

```rust
badge::<()>("Pill").rounded(Rounding::Full);
text::<()>("Card").background("#1C2533").rounded(4);
text::<()>("Fractional").background("#1C2533").rounded(4.5);

let toolbar: Element<()> = hstack![
    text("Title"),
    spacer(),
    badge("Status").rounded(Rounding::Full),
].gap(12., Axis::Horizontal).into();
let fixed: Element<()> = spacer().flex(0.).height(12.).into();
let line: Element<()> = divider().background("#334154").into();
```

- Numeric `.rounded` accepts integer/fractional Rust values in logical pixels,
  finite0..=256. `.radius(f32)` remains a compatible numeric alias.
- `Rounding::Full` resolves **after layout** to half the smaller dimension: a
  pill for rectangular bounds, a circle for square bounds. It responds to resize
  and DPI, and the same resolved radius is used for painting and interactive hits.
  Rounding is local, not inherited; the last rounding/radius modifier wins.
  Background rounding does **not** add rounded clipping of descendants; their
  existing rectangular clipping remains.
- `divider()` is the existing noninteractive **horizontal** `separator()` recipe,
  one logical pixel high unless overridden. Existing separator naming still works.
- `spacer()` is invisible, never takes focus or receives events, and defaults to
  `flex(1.)` along the parent's main axis. Multiple spacers share remaining space
  by flex weight. It paints nothing even if given visual modifiers. For fixed
  space set `flex(0.)` and width (HStack) or height (VStack/Panel).

## Atomic updates and failure behavior

`Element::build()` returns `BuiltUi<App>`. `into_parts()` returns the validated
Document and its closures. A custom host can use `Runtime::new` and
`Runtime::replace_document`, then immediately install matching handlers.
No serialized document or binding projection is required for Rust UI; the tree
contains snapshots of visible text/checked/enabled state, with empty Bindings.

The native host builds/validates candidates **before** replacing tree/handlers.
An invalid initial body returns an error before window creation. An invalid later
body prints a diagnostic, marks the title `UI rejected`, cancels transient input,
and keeps the last valid tree/handlers/focus. A subsequent valid update recovers.
**State already mutated by the callback is not rolled back.** Until recovery,
the displayed tree can therefore lag application state. Rejection is a visible
error condition, not a transaction over application logic or silent success.

`NativeOptions.document_path` is rejected by `run_ui`: Rust source edits require
rebuilding. F5 applies only to the separate JSON examples, not Rust hot reload.

## Runnable examples and tests

```sh
cargo run --locked --features native --example rust_counter
cargo run --locked --features native --example rust_settings
cargo run --locked --features native --example rust_keyed_list
cargo run --locked --features native --example rust_layout
cargo run --locked --features native --example rust_responsive
cargo run --locked --features native --example rust_settings -- --snapshot target/rust-settings.png --scale 2
cargo run --locked --features native --example rust_counter -- --bench 60 --scale 2
cargo test --locked --features rust-ui
cargo test --locked --all-features --all-targets
```

Counter: text/state/direct callbacks/disabled reset. Settings: reusable functions,
conditional panels, typed booleans, fonts/styles. Keyed list: Rust loops,
ID-capturing callbacks, reverse/remove/empty state, preserved compatible focus.
Snapshot mode is not native input proof. Benchmark includes body construction,
validation, handler installation and layout, then paint/raster/packing; excludes
OS event queue/presentation. See [recorded evidence](PERFORMANCE.md).

This adopts the **function-of-state authoring idea**, not an assertion of parity
with mature [Compose](https://developer.android.com/develop/ui/compose/mental-model)
or [SwiftUI](https://developer.apple.com/documentation/swiftui/view). Future
signals/subtree reconciliation and hot-source tooling must preserve this API's
ownership, focus and compatibility contracts rather than require a JSON rewrite.

## Selective effects and windows

See [effects/windowing](EFFECTS_AND_WINDOWING.md) for multi-edge/axis selections,
independent corner rounding, new 2px Rust button default, opt-in effects and
keyed animation/fade/window chrome. These are implemented Rust APIs; existing
JSON and legacy Volt visuals retain their original defaults.
