# Editable JSON UI and Rust integration (schema 1)

Primary application UI now uses [Rust-first declarations](RUST_UI.md). This document describes the optional JSON frontend; JSON is not required by the Rust API.

## Implemented

Enable `declarative` for parsing, typed callbacks, named bindings and a shared
headless runtime. Enable `native` for the optional single-window software host.
Default builds keep only unicode-width and the existing APIs.

```text
Editable JSON -> bounded parser -> validated Document
                                       |
Rust handlers + typed bindings --------+-> Runtime layout/input
                                       |
                                  Painter recipes
                                       |
                           shaped text + native pixels
```

See `examples/ui/complete_app.json` and `examples/complete_app.rs` for the full
working pairing. Documents are data; Rust callbacks own all business behavior.

## Schema

```json
{"schema_version":1,"root":{"type":"column","id":"root","padding":24,"gap":16,"children":[
  {"type":"text","id":"status","bind_text":"status","font_size":20},
  {"type":"button","id":"save","label":"Save","on_activate":"save","bind_enabled":"can_save","primary":true}
]}}
```

Every node needs a globally unique ASCII id (letters/digits/underscore/hyphen,
1–128 bytes). Only schema version1 is accepted. Unknown or wrong-kind properties
are rejected, not ignored. Dimensions are logical pixels, scaled once in layout.

| Component | Properties beyond type/id/width/height/flex |
|---|---|
| panel/row/column | children, padding, gap, align, flow |
| stack | container properties; direction: vertical (default) / horizontal |
| zstack | container properties; independent centered/flow-positioned layers |
| text | exactly one of text/bind_text; font_size; tone |
| badge | exactly one of text/bind_text |
| checkbox/toggle | label; on_toggle_changed; checked or bind_checked; enabled; bind_enabled |
| separator | shared layout and style |
| spacer | shared layout; flex defaults1, set0 for fixed width/height |
| button | label, on_activate; enabled; bind_enabled; primary |

- Every node accepts optional `style`; see [editable styling](STYLING.md). Text color
  and typography inherit; surfaces, borders and radii do not.
- Width/height optionally constrain a node; flex distributes remaining main-axis space.
- A Row arranges horizontally; Column/Panel arrange vertically.
- Stack has static direction; ZStack overlays children in declaration order.
  Responsive value objects are currently Rust-only, not accepted by schema1 JSON.
- Padding and gap default0. align: start (default), center, end, stretch.
- Optional flow: top_left/top/top_right/left/center/right/bottom_left/bottom/bottom_right;
  places content in both axes, not wrapping. Top/Bottom are horizontally centered.
- Columns with start align and no flow stretch unspecified child widths. Row widths shrink
  proportionally on overflow; wrapped content is remeasured at allocated widths.
- Text font_size defaults14 and accepts8–96. tone: body/muted/accent/title;
  title is a semantic tone, not an automatic font-size override.
- Button defaults enabled=true, primary=false; handler signature must be Activate.
- No expressions, imports, arbitrary code or document-side file access.

Limits:256KiB document,4096 nodes,depth48,8KiB per text value; numeric layout values
0–16384. Runtime scale0.25–4 and native framebuffer64MP maximum (GPU scene and opacity budgets are separate). Oversized/mistyped named
bindings are rejected. Public Document cannot bypass validation with Deserialize.
Syntax errors have line/column; semantic errors identify tree path or widget ID.

## Rust pairing

```rust
use volt_ui_kit::declarative::{Bindings, Event, Handlers};
#[derive(Default)] struct App { saves: usize }
let mut handlers = Handlers::default();
handlers.on_activate("save", |app: &mut App| app.saves += 1)?;
let mut values = Bindings::default();
values.set_text("status", "Ready");
values.set_bool("can_save", true);
let mut app = App::default();
handlers.dispatch("save", Event::Activate, &mut app)?;
# Ok::<(), volt_ui_kit::declarative::HandlerError>(())
```

Functions, mutable closures and borrowed captures work with the headless registry;
the native run API requires 'static handlers/update closure for the event loop.
The caller supplies &mut application context at dispatch, with no global state.
Missing names, incompatible payloads and duplicate registration produce errors
before execution/replacement. ToggleChanged(bool) connects Checkbox/Toggle controls to Rust; the host then
projects updated state back through bind_checked. This is a typed event plus
one-way state projection, not hidden document-side mutation. TextChanged(String)
exists as a signature, but Input/text editing is not implemented.
Handlers execute synchronously and do not catch panics. Enqueue expensive or
async work in your host; no toolkit executor or threads are started.

Native `run(options, source, state, handlers, update)` projects Rust state into
Bindings after actions, validates it, lays out and requests one redraw. State
projection is explicit, not automatic dependency-tracked reactivity. It provides
text/boolean one-way bindings; two-way editing bindings are future work.

## Lifecycle

Headless Runtime::reload parses and validates the complete replacement before
mutation. Native F5 uses a bounded regular-file reader, retains the last valid
UI on error, and cancels capture/press on success. Unique stable button IDs retain
compatible enabled focus after relayout. Rust handlers do not belong to document
nodes, so reload does not duplicate listeners. No filesystem watcher/timer.

Rust tree builders and build-time connection validation are implemented in the
[Rust-first frontend](RUST_UI.md). Custom-component registration, capture/bubble
listeners, two-way editing bindings and GPU hosts remain later APIs.
No complete accessibility claim: node IDs/kinds/labels exist, but platform semantic
trees/actions are not implemented.

References: [Serde formats](https://serde.rs/data-format.html),
[Rust closure traits](https://doc.rust-lang.org/reference/types/closure.html).

## Directional spacing (additive schema1 properties)

Panel/Row/Column accept numeric `padding_horizontal`, `padding_vertical`,
`padding_top`, `padding_right`, `padding_bottom`, `padding_left`,
`gap_horizontal`, `gap_vertical`. Scalar `padding`/`gap` keep their existing
meaning. Insets resolve edge > axis > scalar; JSON property order is irrelevant.
Row uses horizontal gap, Column/Panel vertical gap; gap adds no outer edge space
and is not inherited. Every supplied spacing value must be finite0..=16384.
These properties are rejected on non-container nodes. See [Rust spacing](RUST_UI.md#directional-padding-and-gap).

Optional container `flow` and `style.radius: "full"` are additive schema1 values.
Full rounding resolves to half the minimum laid-out dimension and shares paint/hit
geometry, without adding rounded descendant clipping. `spacer` defaults to flex1
unless explicitly overridden; it has no painting/events. Rust primary authoring
uses selectors and direct closures rather than editing JSON; see [Rust UI](RUST_UI.md).
