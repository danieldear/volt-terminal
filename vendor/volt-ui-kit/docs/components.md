# Components

Every component is a Rust builder that returns an element. Components are
**controlled**: they show values from your state and report changes through
typed callbacks, so the state stays in one place. Each one has a live page in
the Catalog (`--page <name>`), with its source and API table.

Common rules:

- Interactive components need a `.key(..)` when they appear in dynamic content,
  and every interactive callback must be connected or `build()` fails.
- Selections take typed values (`Clone + PartialEq`): enums, integers or your
  own types, never label strings.
- Choosing the already-selected option does nothing and does not recompose.
- Popups, menus and dialogs are drawn inside the window and keep focus and
  input inside their scope while open.

## Buttons and actions

```rust
use volt_ui_kit::ui::prelude::*;
let actions: Element<()> = hstack![
    button("Save").key("save").variant(ButtonVariant::Primary).on_click(|_: &mut ()| {}),
    button("Preview").key("preview").variant(ButtonVariant::Soft).on_click(|_: &mut ()| {}),
    button("Cancel").key("cancel").variant(ButtonVariant::Ghost).on_click(|_: &mut ()| {}),
    button("Delete").key("delete").variant(ButtonVariant::Danger).size(ControlSize::Small)
        .on_click(|_: &mut ()| {}),
    button("Sort by").key("sort").disclosure(false).label_align(Align::Start)
        .width(140.).on_click(|_: &mut ()| {}),
]
.gap(8., Axis::Horizontal)
.into();
actions.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- Variants: `Neutral`, `Primary`, `Soft`, `Outline`, `Ghost`, `Danger`.
  Sizes: `Small` (28 px), `Medium` (36 px), `Large` (44 px). Both are theme
  presets; later `.background(..)`, `.color(..)` or `.hover_style(..)` calls win.
- `.disclosure(open)` adds a trailing chevron, as used by select and menu triggers.
- `button_bar![..]` groups actions; `.joined(true)` makes a segmented strip.
  `toolbar![..]` lays out a horizontal application toolbar.

## Navigation and tabs

```rust
use volt_ui_kit::ui::prelude::*;

#[derive(Clone, Copy, Default, PartialEq)]
enum Page { #[default] Overview, Activity }
#[derive(Default)]
struct App { page: Page, count: usize }

fn view(app: &App) -> Element<App> {
    vstack![
        toolbar![
            text("My workspace").font_weight(600),
            spacer(),
            button_bar![
                button("Add").key("add").on_click(|app: &mut App| app.count += 1),
                button("Clear").key("clear").enabled(app.count > 0)
                    .on_click(|app: &mut App| app.count = 0),
            ].joined(true),
        ],
        segmented_control([(Page::Overview, "Overview"), (Page::Activity, "Activity")])
            .key("page-picker")
            .selected(app.page)
            .on_select(|app: &mut App, page| app.page = page),
        tabs![
            tab(Page::Overview, "Overview", text(format!("Count: {}", app.count)))
                .key("overview-tab"),
            tab(Page::Activity, "Activity", text("Only the selected page is mounted"))
                .key("activity-tab"),
        ]
        .key("pages")
        .selected(app.page)
        .placement(TabPlacement::Top)
        .on_select(|app: &mut App, page| app.page = page),
    ]
    .gap(16., Axis::Vertical)
    .into()
}
view(&App::default()).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- `segmented_control`, `tab_bar![tab_item(..), ..]` and `tabs![tab(..), ..]`
  select typed values. `tabs` mounts only the selected page; keep page data in
  your state. Item `.enabled(false)` disables one option.
- `navigation([navigation_item(value, "Label"), ..])` is an app-level
  destination list with `NavigationPlacement::Sidebar`, `Rail` or `Bottom`.
- With an option focused, arrow keys and Home/End move the selection.

## Fields, value controls and dialogs

```rust
use volt_ui_kit::ui::prelude::*;
#[derive(Clone, Copy, Default, PartialEq)]
enum Order { #[default] Recent, Name }
#[derive(Default)]
struct App { draft: String, notes: String, level: f32, order: Order, picker: bool, review: bool }
fn view(app: &App) -> Element<App> {
    zstack![
        vstack![
            text_field(&app.draft).key("draft").placeholder("Draft title")
                .on_change(|app: &mut App, text| app.draft = text)
                .on_submit(|app: &mut App| app.review = true),
            text_area(&app.notes).key("notes").height(120.).soft_wrap(true)
                .on_change(|app: &mut App, text| app.notes = text),
            slider(app.level).key("level").range(0.0..=100.0).step(5.)
                .on_change(|app: &mut App, value| app.level = value),
            select([
                select_item(Order::Recent, "Recent").key("recent"),
                select_item(Order::Name, "Name").key("name"),
            ]).key("sort").selected(app.order).open(app.picker)
                .on_open_change(|app: &mut App, open| app.picker = open)
                .on_select(|app: &mut App, value| app.order = value),
            button("Review").key("review").on_click(|app: &mut App| app.review = true),
        ].gap(12., Axis::Vertical).padding(20., Edge::All),
        dialog(vstack![
            text(format!("Review: {}", app.draft)),
            button("Done").key("done").on_click(|app: &mut App| app.review = false),
        ].gap(12., Axis::Vertical))
            .key("review-dialog").title("Review draft").open(app.review)
            .on_dismiss(|app: &mut App| app.review = false),
    ].into()
}
view(&App::default()).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

**Text fields and areas** support selection, clipboard, undo/redo (64 steps),
IME composition and `.password(true)` masking. Enter calls `on_submit` in a
single-line field; Cmd/Ctrl+Enter submits a text area. `.soft_wrap(true)` wraps
long lines without changing the stored text. `.read_only(true)` allows a field
without `on_change`. `.leading_icon(Icon::Search)` and `.trailing_action(..)`
add adornments. Each editor holds up to 8 KiB of plain text: it is not a rich
text or code editor.

**Sliders** take a finite range and step and respond to dragging and the arrow,
Page and Home/End keys. `range_slider(lo, hi)` has two thumbs that cannot cross.

**Select** shows 1–128 typed options in a popup that flips above its trigger
near the window edge and scrolls long lists. `combobox` adds type-to-filter.

**Dialogs** are modal inside the window: they trap Tab, restore focus on close,
and dismiss with Escape or a backdrop click unless you disable either. Place the
dialog after the content it covers in a root `zstack!`.

## Forms

| Builder | Notes |
| --- | --- |
| `radio_group([(value, "Label")])` | `.selected(..)`, `.on_select(..)`, descriptions, horizontal layout |
| `number_input(v)` | `.range(0., 10.)`, `.step(1.)`, `.decimals(1)`, `.suffix(" kg")` |
| `combobox(items, &state)` | Type to filter, Enter picks the best match |
| `tag_input(&tags, &state)` | Enter adds a tag, × removes one |
| `color_picker("#3B82F6")` | Swatches, hue/saturation/lightness sliders and a hex field |
| `otp_input(code, 6)` | One-time codes; `.on_complete(..)` |
| `rating(n)` | `.max(5)`; read-only without `on_change` |
| `form_field("Email", control)` | Label, `.description(..)`, `.error(..)`, `.required(true)` |
| `drop_zone()` | Native file drag and drop; `.accept(["png"])`, `.on_drop(..)` |

Dates use `CalendarDate` (no date crate needed): `calendar`, `date_picker`,
`date_range_picker` and `time_picker`.

## Menus and overlays

```rust
use volt_ui_kit::ui::prelude::*;
#[derive(Default)]
struct App { menu_open: bool, inspector_open: bool, last_action: u8 }
fn view(app: &App) -> Element<App> {
    vstack![
        menu("Actions", [menu_item(1u8, "Save"), menu_separator(),
             menu_item(2, "Delete").danger(true), menu_item(3, "Later").enabled(false)])
            .key("actions").open(app.menu_open)
            .on_open_change(|app: &mut App, open| app.menu_open = open)
            .on_action(|app: &mut App, action| app.last_action = action),
        tooltip("Inspect this workspace",
            button("Inspector").key("inspect")
                .on_click(|app: &mut App| app.inspector_open = true))
            .key("inspect-help"),
        popover(text("A controlled, anchored card"))
            .key("inspector").anchor("inspect").open(app.inspector_open)
            .on_dismiss(|app: &mut App| app.inspector_open = false),
    ].gap(12., Axis::Vertical).into()
}
view(&App::default()).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- **Menus** support separators, disabled and danger items, check and radio
  items, shortcut hints and `menu_submenu` (which opens in place with a back row).
  Arrow keys skip disabled items; Enter commits; Escape or an outside click closes.
- **Context menus**: `context_menu(target, items)` opens on right-click at the
  pointer. Store the point from `.on_context_open(|app, point| ..)`.
- **Tooltips** appear immediately on hover or keyboard focus, need no state and
  never block input.
- **Popovers** and `hover_card` anchor to another element by key.
- `drawer`, `accordion`, `collapsible`, `toaster` and `command_palette`
  complete the set. Toasts are dismissed by your state; there is no timer.

These are drawn by the kit inside the window, not OS menus. For the macOS menu
bar, see [Application services](app-services.md#menus-and-tray).

## Large lists

```rust
use volt_ui_kit::ui::prelude::*;
struct App { window: std::ops::Range<usize>, selected: Option<usize> }
fn view(app: &App) -> Element<App> {
    virtual_list(10_000, 32., |index|
        data_item(index, format!("Record {index:05}")).key(format!("record-{index}")))
        .key("records").height(300.).window(app.window.clone()).overscan(2)
        .selected(app.selected)
        .on_window_change(|app: &mut App, window| app.window = window)
        .on_select(|app: &mut App, id| app.selected = Some(id))
        .into()
}
view(&App { window: 0..12, selected: None }).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Only the rows in the current window (at most 96) are built, so a model of any
size stays within the element budget. Rows have a fixed height of 8–512 px.
Scrolling reports a new window through `on_window_change`; store it and the
next `view` builds those rows.

## Tables, trees and split panes

```rust
use volt_ui_kit::ui::prelude::*;
#[derive(Default)]
struct App { selected: Option<usize>, expanded: Vec<usize>, ratio: f32 }
let table = table([table_column(0usize, "Name"), table_column(1, "Status")], [
    table_row(1usize, ["Beta", "Ready"]).key("beta"),
    table_row(2, ["Alpha", "Review"]).key("alpha"),
]).key("projects").sort(0, SortDirection::Ascending).selected(None)
    .on_sort(|_: &mut App, _column, _direction| {})
    .on_select(|app: &mut App, id| app.selected = Some(id));
let tree = tree_view([
    tree_node(1usize, "Workspace").key("workspace").children([
        tree_node(2, "main.rs").key("source-main")]),
]).key("sources").expanded([1]).selected(None)
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_expand(|app: &mut App, id, open| {
        app.expanded.retain(|value| *value != id);
        if open { app.expanded.push(id); }
    });
let ui: Element<App> = split(table, tree).key("workspace-split")
    .ratio(0.55).min_first(160.).min_second(120.)
    .on_resize(|app: &mut App, ratio| app.ratio = ratio).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- **Tables** sort rows by the displayed text, or by `.sort_by(..)` for numbers
  and your own orderings. Columns can `.align(Align::End)` for amounts. A table
  with a fixed size scrolls in both directions. Tables are not virtualized.
- **Trees** mount only expanded branches; selection and expansion are separate.
- **Split panes** have a draggable divider (arrow keys work too). Use `.ratio`
  or pixel sizing, minimum sizes for each side, and `.handle_style(..)` for a
  divider that appears only on hover.

## Progress and loading

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<String> = vstack![
    progress(0.42).width(280.).accent_color(ColorToken::Success),
    progress_indeterminate().width(280.).period_ms(1200),
    loading("Loading records"),
].gap(16., Axis::Vertical).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Indeterminate bars and spinners animate only while visible and stop under
reduced motion. Remove them when the work is done.

## Display

`avatar`, `avatar_group`, `alert`, `skeleton`, `kbd`, `breadcrumb`,
`pagination`, `steps`, `timeline`, `empty_state`, `stat`, `carousel` and
`image_view` (an image with a fallback).

## Charts

`sparkline`, `bar_chart` and `line_chart` draw small static charts. The larger
charts (`area_chart`, `candlestick_chart`, `donut_chart` and
`stacked_bar_chart`) become interactive when you give them a key:

```rust
use volt_ui_kit::ui::prelude::*;
#[derive(Default)]
struct App { quarter: Option<usize> }
fn revenue(app: &App) -> Element<App> {
    stacked_bar_chart(
        ["Q1", "Q2", "Q3", "Q4"],
        [("Product", vec![120., 160., 190., 240.]), ("Services", vec![80., 90., 120., 130.])],
    )
    .key("revenue")
    .size(480., 240.)
    .selected(app.quarter)
    .on_select(|app: &mut App, i| {
        app.quarter = if app.quarter == Some(i) { None } else { Some(i) };
    })
    .into()
}
revenue(&App::default()).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- With `.key(..)`, every data point gets a value tooltip on hover or keyboard
  focus, and a spoken label for screen readers.
- `.selected(..)` draws the highlight (a candle band and close line, an area
  rule with dots, a popped-out donut slice, an emphasised column);
  `.on_select(..)` reports clicks and Enter.
- Donut clicks count only inside the ring; legend rows select slices.
- `Candle::new(open, high, low, close).volume(v)` builds candlestick data;
  `.value_format(f)` formats tooltip and axis values.

## Icons

```rust
use volt_ui_kit::ui::prelude::*;
let logo = SvgIcon::parse(
    r##"<svg viewBox="0 0 24 24"><path d="M4 12h16M12 4v16" stroke="#000" stroke-width="2"/></svg>"##,
)?;
let row: Element<()> = hstack![
    icon(Icon::Search).size(16.),
    svg_icon(&logo).size(20.),
    svg_icon(&logo).size(20.).keep_colors(true),
]
.gap(8., Axis::Horizontal)
.into();
row.build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

- 65 built-in vector icons on a 24-unit grid (`icon(Icon::..)`), tinted by the
  surrounding text colour.
- Any SVG icon set works: `SvgIcon::parse(markup)` or `SvgIcon::load(path)`,
  then `svg_icon(&icon)`. SVG icons take the text colour unless
  `.keep_colors(true)` keeps their own colours.
- `IconSet::from_dir(path)` loads a folder of `.svg` files by file name and
  reports the ones it could not read; `set.icon("name")` returns an element.
- The SVG reader handles paths and basic shapes, transforms, fills and strokes.
  It never runs scripts or loads external resources.

## Known limits

- Range sliders expose one accessibility value; arrow keys move the last thumb
  you dragged.
- Comboboxes and command palettes pick the best match on Enter; there is no
  Up/Down navigation through results yet.
- Submenus open in place rather than cascading.
- `.offset(..)` and slide/ambient motion move pixels only; layout and hit
  targets stay where they were.
