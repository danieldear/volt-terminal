# Responsive and layered Rust layouts

Enable `rust-ui` for builders or `native` for the runnable host. No JSON, source
parser, WYSIWYG editor, or new default dependency is required.

| API | Arrangement |
|---|---|
| `vstack!` / `column!` | Fixed vertical |
| `hstack!` / `row!` | Fixed horizontal |
| `stack!` | Vertical by default; `.direction(...)` accepts fixed/responsive direction |
| `zstack!` | Independent overlapping layers; declaration order is back to front |
| `panel!` | Existing vertical surface/container |

Functions with the same names accept iterators of `Element<App>`. Fixed stacks
deliberately have no `.direction` method. Their old defaults remain unchanged;
familiar SwiftUI naming does not promise identical default alignment or sizing.

## One tree, adaptive properties

```rust
use volt_ui_kit::ui::prelude::*;
let screen: Element<()> = stack![
    panel![text("Navigation")].width(180.),
    panel![text("Content")].flex(1.),
]
.direction(responsive(Direction::Vertical)
    .at(Breakpoint::Md, Direction::Horizontal))
.padding(responsive(12.).at(Breakpoint::Md, 24.), Edge::All)
.gap(responsive(8.).at(Breakpoint::Md, 16.)
    .at(Breakpoint::Lg, 24.), Axis::Both)
.into();
screen.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

```text
Below Md                         At Md and above
+---------------------+          +----------+---------------------+
| Navigation          |          | Nav      | Content             |
+---------------------+          |          |                     |
| Content             |          +----------+---------------------+
+---------------------+

12px inner padding / 8px gap      24px inner padding / 16px gap
```

Rules are mobile-first minimum widths: base applies everywhere; the largest
applicable threshold wins. Equal thresholds use the last `.at` call, including
`Md` and `MinWidth(768.)`. Distinct thresholds do not depend on declaration order.
Rules are inclusive: Md applies at768 and above, not below768.

Named logical-pixel thresholds: Sm640, Md768, Lg1024, Xl1280, Xxl1536. Applications
can use `Breakpoint::MinWidth(700.)` or a shared custom constant rather than the
named defaults. This is not CSS/rem evaluation: query width is always the physical
viewport width divided by scale. A1536px framebuffer at2x selects the same rules
as a768px framebuffer at1x. Nested stacks still query the **viewport**, not their
own content widths. Container queries are a separate future feature.

All containers accept both scalar and responsive padding/gap; existing calls
such as `.padding(20.5, Edge::Horizontal).gap(12., Axis::Both)` still compile.
The latest setter affecting an edge/axis replaces that property's **entire rule
set**, not just its currently active value. All affects four edges; Both affects
two gap axes. Values do not inherit into child containers. Direction selects the
active gap axis; Flow selects placement, never direction or child wrapping.

There are at most32 stored rules per property; spacing must be finite0..=16384,
custom thresholds finite0..=65536. Build validates base values and every retained
rule, even on inactive axes/thresholds. An invalid view follows existing atomic
installation rules; application state changed by a callback is not rolled back.

## Layered placement

```rust
use volt_ui_kit::ui::prelude::*;
let layers: Element<usize> = zstack![
    button("Back").width(220.).height(96.)
        .on_click(|count: &mut usize| *count += 1),
    button("Front").width(100.).height(38.)
        .on_click(|count: &mut usize| *count += 10),
].height(120.).flow(Flow::Center).into();
layers.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

```text
+--------------------------+
| Back button              |
|       +----------+       |
|       | Front    |       |
|       +----------+       |
+--------------------------+
```

- Layers center independently by default; `.flow(...)` places each in both axes.
- Intrinsic size is the maximum child extent per axis plus padding, not the sum.
- `.align(Align::Stretch)` fills each unspecified child dimension. Explicit
  widths/heights remain constrained; other Align values do not replace Flow.
- Gap and child flex weights do not distribute space between overlapping layers.
- Parent paints before children; later child subtrees paint in front. Pointer
  hits choose the topmost **enabled interactive** target inside its bounds/clip
  and rounded shape. Decorative/disabled layers pass through to targets below.
- This is **not a modal input barrier**. Underlying controls can still be reached
  by keyboard, whose order stays tree order. Explicit blocking/modal semantics,
  arbitrary coordinates/offsets and z-index reordering are not implemented.
- Clipping is rectangular, including on rounded backgrounds; no rounded child
  clipping is implied. Overlarge padding can omit descendants entirely.

## Resize and callbacks

```text
physical viewport / scale
             |
resolve direction + padding + active gap once per node
             |
measure -> allocate -> place -> hit-test -> paint

Existing nodes / keys / Rust callbacks retained
```

Native resize/scale changes cancel pending input and invalidate/layout/redraw;
they do not call the Rust view body or build new handlers. Compatible visible
focus stays keyed. View bodies still run initially and after dispatched actions
only; callback captures are replaced after composition, not after pure resize.

## Try and test

```sh
cargo run --locked --features native --example rust_responsive
cargo run --locked --features native --example rust_responsive -- --size 480x760
cargo run --locked --features native --example rust_responsive -- --size 960x760
cargo run --locked --features native --example rust_responsive -- --size 480x760 --snapshot target/responsive-narrow.png
cargo run --locked --features native --example rust_responsive -- --size 960x760 --snapshot target/responsive-wide.png --scale 2
cargo test --locked --features rust-ui --test responsive_stacks
cargo test --locked --all-features --all-targets
```

Resize across768, toggle compact spacing, click the front/exposed back buttons,
Reset, and exercise Tab/Shift+Tab/Enter/Space. The same host handles these events.
Headless tests/raster snapshots are not OS-input acceptance. Width rules do not
solve height overflow: no scrolling, grids, child wrapping or responsive hiding
yet. Dense content can shrink/clip at320x240. See [validation](VALIDATION.md).
