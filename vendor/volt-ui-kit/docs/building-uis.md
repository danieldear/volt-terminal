# Building interfaces

Everything in this guide is ordinary Rust: no markup language, macros beyond
simple child lists, or code generation.

## The model

```text
App state ──view(&App)──▶ Element<App> ──build()──▶ validated tree + closures
    ▲                                                     │
    │                                        layout · paint · input
    │                                                     │
    └──────────── callback(&mut App) ◀── click, key, change, message
```

- **`view` is a function of state.** The host calls it once at start and again
  after each dispatched callback, background message batch or command. Hover,
  scrolling, resizing and animation frames do not call it.
- **The whole tree is recomposed**, then validated and installed atomically.
  Layout reuses the measurements of nodes that did not change, so recomposing a
  large screen stays cheap.
- **State lives in your type.** Components are controlled: a text field shows
  `app.draft` and reports edits through `on_change`; nothing hides its own copy.

## Callbacks

Callbacks are closures or functions that take `&mut App` plus the event's value:

```rust
use volt_ui_kit::ui::prelude::*;

#[derive(Default)]
struct App { count: usize, preview: bool, items: Vec<(u64, String)> }

fn reset(app: &mut App) {
    app.count = 0;
}

fn controls(app: &App) -> Element<App> {
    let rows: Vec<Element<App>> = app
        .items
        .iter()
        .map(|(id, title)| {
            let id = *id;
            hstack![
                text(title.clone()),
                spacer(),
                button("Remove")
                    .key(format!("remove-{id}"))
                    .on_click(move |app: &mut App| app.items.retain(|(i, _)| *i != id)),
            ]
            .key(format!("row-{id}"))
            .into()
        })
        .collect();
    vstack![
        button("Add").key("add").on_click(|app: &mut App| app.count += 1),
        button("Reset").key("reset").enabled(app.count > 0).on_click(reset),
        toggle("Preview", app.preview)
            .key("preview")
            .on_change(|app: &mut App, on| app.preview = on),
        vstack(rows).gap(4., Axis::Vertical),
    ]
    .gap(8., Axis::Vertical)
    .into()
}
controls(&App::default()).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- Captures must be owned (`'static`). Capture an ID with `move`, not a reference
  into the view; capturing the ID (not the index) keeps a click pointed at the
  right row after the list changes.
- Callbacks run synchronously on the UI thread. Start slow work on a thread and
  report back with a [background message](app-services.md#background-work).
- Wrong callback signatures fail to compile. A missing callback on an
  interactive control fails at `build()`.

## Composition

Reusable pieces are functions that return `Element<App>`. Conditionals and loops
are plain Rust; annotate the type when branches return different builders:

```rust
use volt_ui_kit::ui::prelude::*;
struct App { saved: bool }
fn status(app: &App) -> Element<App> {
    if app.saved {
        badge("Saved").into()
    } else {
        text("Unsaved changes").color(ColorToken::Warning).into()
    }
}
status(&App { saved: true }).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

`vstack!`, `hstack!`, `zstack!`, `stack!`, `panel!` (and `column!`/`row!`)
accept a list of children of any element type. The functions with the same
names accept an iterator of `Element<App>`.

When you glob-import the prelude, also import `column` explicitly
(`use volt_ui_kit::ui::{column, prelude::*};`); otherwise it clashes with the
standard library's `column!` macro. `vstack!` has no such clash.

## Keys and identity

`.key("...")` gives an element a stable, globally unique identity. Keys keep
focus, text selection, scroll position, pressed state and animations attached
to the right element across recomposition.

- Keys are 1–128 ASCII letters, digits, `_` or `-`. Duplicates fail at `build()`.
- Unkeyed elements get structural IDs, which are fine for static content but
  not for lists that insert, remove or reorder. Key those rows and controls.
- Focus survives recomposition when its key still names the same kind of
  enabled control; otherwise it is cleared.

## Layout

### Stacks and containers

| Builder | Arrangement |
| --- | --- |
| `vstack!` / `column!` | Children top to bottom |
| `hstack!` / `row!` | Children left to right |
| `stack!` | Vertical by default; `.direction(..)` takes a fixed or responsive direction |
| `zstack!` | Children on top of each other, first is backmost |
| `panel!` | A padded vertical surface |
| `grid(children)` | `.columns(3)` or `.min_column_width(220.)`; children can `.column_span(2)` |

### Spacing

Padding takes edges; gaps take axes. Both accept one selector or a list:

```rust
use volt_ui_kit::ui::prelude::*;
let card: Element<()> = vstack![text("Title"), text("Body")]
    .padding(24., Edge::Horizontal)
    .padding(16., [Edge::Top, Edge::Bottom])
    .gap(8., Axis::Vertical)
    .into();
card.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

`Edge::All` and `Axis::Both` set everything. The last setter for an edge wins.
Gaps go between children only; use padding for outer space. Padding and gap do
not inherit into nested containers.

### Placement and sizing

- `.flow(Flow::Center)` places the children as a group anywhere in the
  container: `TopLeft`, `Top`, `TopRight`, `Left`, `Center`, `Right`,
  `BottomLeft`, `Bottom`, `BottomRight`. It needs spare space to move into.
- `.align(Align::Start | Center | End | Stretch)` aligns children on the cross axis.
- `.justify(Justify::SpaceBetween)` distributes children on the main axis
  (`Start`, `Center`, `End`, `SpaceBetween`, `SpaceAround`, `SpaceEvenly`).
- `.width(..)`, `.height(..)`, `.min_width(..)`, `.max_width(..)` (and heights),
  `.width_fraction(0.5)`, `.aspect_ratio(16. / 9.)` and `.fill_width()`.
- `.flex(1.)` shares the remaining main-axis space by weight.
- `spacer()` is invisible flexible space; `divider()` is a one-pixel line.
- `.wrap(true)` on a row wraps children onto new lines.

```rust
use volt_ui_kit::ui::prelude::*;
let toolbar: Element<()> = hstack![
    text("Title").font_weight(600),
    spacer(),
    badge("Live").rounded(Rounding::Full),
]
.gap(12., Axis::Horizontal)
.padding(12., Edge::All)
.into();
toolbar.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

All sizes are logical pixels; the host scales them for the display.

## Responsive layout

Any spacing, the direction of `stack!`, visibility, width and font size can
change with the window width:

```rust
use volt_ui_kit::ui::prelude::*;
let screen: Element<()> = stack![
    panel![text("Navigation")].width(200.),
    panel![text("Content")].flex(1.),
]
.direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
.padding(responsive(12.).at(Breakpoint::Md, 24.), Edge::All)
.gap(responsive(8.).at(Breakpoint::Lg, 24.), Axis::Both)
.into();
screen.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Rules are mobile-first minimum widths: the base value applies everywhere and the
largest matching breakpoint wins. Named breakpoints are `Sm` 640, `Md` 768,
`Lg` 1024, `Xl` 1280 and `Xxl` 1536 logical pixels; `Breakpoint::MinWidth(700.)`
sets your own. Rules read the window width, not the parent's width. Resizing
re-resolves them during layout without calling `view`.

`.visible(responsive(false).at(Breakpoint::Md, true))` hides an element below a
width; a hidden element takes no space and receives no input or focus.

## Layers

`zstack!` overlaps children. Each layer is placed independently (centred unless
you set `.flow(..)`), and the stack is as large as its largest child. Clicks go
to the topmost enabled interactive layer; decorative layers let clicks through.
A `zstack!` is not a modal barrier: use a [dialog](components.md#fields-value-controls-and-dialogs)
when content behind must be blocked.

## Scrolling

```rust
use volt_ui_kit::ui::prelude::*;
let rows = (0..100).map(|i| text(format!("Row {i}")).height(32.).into());
let list: Element<()> = vstack![
    text("Fixed header").height(40.),
    vstack(rows).key("rows").flex(1.).scroll(Scroll::Auto),
]
.into();
list.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- Modes: `Vertical`, `Horizontal`, `Both`, `Auto` (scrolls only the axes that
  overflow). Give a scroll view a height, width or flex share so it has a viewport.
- Wheel and trackpad input scroll the innermost view and pass the rest to its
  ancestors. Shift-wheel scrolls horizontally.
- Offsets are kept per key across recomposition. Scrollbars are thin, draggable
  overlays; `.scrollbars(Scrollbars::Hidden)` hides them but keeps scrolling.
- `.scroll_to(key, request)` brings the child with `key` to the top whenever
  `request` changes; bump a counter in a callback to scroll again.
- Keyboard focus scrolls offscreen controls into view.
- An empty overlay above a scroll view can pass wheel events through with
  `.scroll_passthrough(true)`.

For thousands of rows, use [`virtual_list`](components.md#large-lists) so only
the visible rows are built.

## Validation and limits

`build()` checks the whole tree before it is installed: unique keys, connected
callbacks, valid colours and sizes, and these limits:

| Limit | Value |
| --- | --- |
| Elements per tree | 4,096 (virtual lists do not count unmounted rows) |
| Nesting depth | 48 |
| Text per element | 8 KiB |
| Font size / weight | 8–96 / 100–900 in steps of 100 |
| Corner radius / border width | 0–256 / 0–16 |
| Sizes and spacing | 0–16,384 logical pixels |

If a later `view` produces an invalid tree, the host prints the error, keeps the
last valid screen and recovers on the next valid update. State that a callback
already changed is not rolled back.

## Custom hosts

Without `native`, `rust-ui` gives you the builders and `declarative::Runtime`:
build an element, then call `Runtime::new` or `replace_document` and route
pointer, keyboard, scroll and text input into the runtime yourself. The native
host in `src/native/window.rs` is the reference implementation.
