# Menus, overlays, data and loading components

New visual acceptance uses [Catalog](CATALOG.md). Existing Showcase
Activity/Explorer and Lab remain regression fixtures, not places for new demos.

`.fill_width()` removes a builder's fixed-width default so a vertical parent can stretch the component. Virtual rows stretch with their viewport. Menu font/corner/background modifiers style the actual trigger.

## Typed actions and contextual help

```rust
use volt_ui_kit::ui::prelude::*;
#[derive(Default)]
struct App { menu_open: bool, inspector_open: bool, last_action: u8 }
fn view(app: &App) -> Element<App> {
    vstack![
        menu("Actions", [menu_item(1u8,"Save"), menu_separator(),
             menu_item(2,"Delete").danger(true), menu_item(3,"Later").enabled(false)])
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
    ].gap(12.,Axis::Vertical).into()
}
view(&App::default()).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

`context_menu(target, items)` uses the same typed items, `.open(...)`,
`.on_action(...)` and `.on_open_change(...)`. Supply `.position(app.point)` and
`.on_context_open(|app, logical_point| { app.point = logical_point; app.open = true; })`.
The native host routes **right-press** to `Runtime::context_menu_at`; it never
activates the target's ordinary click callback. Embedded hosts pass physical hit
coordinates; the callback receives logical pixels.

- Menu and context-menu keys are mandatory; callbacks own controlled open state.
  Disabled items/separators are skipped by arrow/Home/End navigation. Enter/Space
  commits an action, then requests close. Escape/outside press-release dismisses.
- Popup geometry is clamped to viewport and flips above an anchor near the bottom.
  Anchors must exist outside their own overlay subtree. Long action menus scroll.
- These are **in-window**, custom-painted menus/cards, not OS-native context menus.
  Popovers use the popup input/focus scope and restore focus on removal.
- Tooltips are immediate hover/keyboard-focus help: no delay timer, no callbacks,
  no focus trap, no input interception, and no idle frame loop. They disappear
  with their target, outside an active modal, or when the native window deactivates.

## Fixed-row virtualization

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

This is actual bounded **composition**, not painting all records and hiding them.
The factory runs only for the supplied window (at most 96 rows). It can borrow view
state, because it is invoked and dropped while composing, not captured in callbacks.
A 10,000-record model therefore does not consume the document's 4096-node budget.
Stable item keys/values are required; selected IDs can live outside the mounted window.

Rows are fixed height, 8..=512 logical pixels. Model count, total extent, viewport and
overscan are validated. Changing data is app-owned; keep windows in range when
removing records. Variable-height virtualization, unmounted-row keyboard navigation
and arbitrary data fetching are not implemented. Initial scroll initializes from
`window.start`; retained keyed scroll wins on subsequent composition.

Native hosts automatically use `Runtime::scroll_outcome` for wheel input and
`dispatch_virtual_windows` after layout/resize. Custom hosts must dispatch proposed
window ranges, apply state changes, rebuild/install the view, and layout again.
Do not use compatibility `scroll_at` alone and expect callbacks/recomposition.
Scrollbars drag normally; hidden indicators retain wheel/trackpad capability.

## Tables, trees and resizable panes

```rust
use volt_ui_kit::ui::prelude::*;
#[derive(Default)]
struct App { selected: Option<usize>, expanded: Vec<usize>, ratio: f32 }
let table = table([table_column(0usize,"Name"),table_column(1,"Status")], [
    table_row(1usize,["Beta","Ready"]).key("beta"),
    table_row(2,["Alpha","Review"]).key("alpha"),
]).key("projects").sort(0,SortDirection::Ascending).selected(None)
    .on_sort(|_: &mut App, _column, _direction| {})
    .on_select(|app: &mut App, id| app.selected = Some(id));
let tree = tree_view([
    tree_node(1usize,"Workspace").key("workspace").children([
        tree_node(2,"main.rs").key("source-main")]),
]).key("sources").expanded([1]).selected(None)
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_expand(|app: &mut App, id, open| {
        app.expanded.retain(|value| *value != id);
        if open { app.expanded.push(id); }
    });
let ui: Element<App> = split(table,tree).key("workspace-split")
    .ratio(0.55).min_first(160.).min_second(120.)
    .on_resize(|app: &mut App, ratio| app.ratio = ratio).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- Table sorting really reorders rows while retaining stable keys. Default comparison
  is lexical by displayed cell; use `.sort_by(|column, left_id, right_id| ...)` for
  numeric/application comparisons. Store requested sort column/direction in app state.
  Rows/cells are bounded by the document budget; this is not a virtualized spreadsheet.
- Tables default to `.rounded(12.)`; `.rounded(0.)` makes both the outer frame and
  inset rows square. Catalog shows both as live, sortable/selectable examples.
  `.align(Align::Start | Center | End)` on a column aligns its header and cells;
  use End for amounts. Column widths are minima; a wider table grows them
  proportionally. Bounded `.width(...).height(...)` viewports scroll on either
  axis. Only the 1px stroke is reserved internally; header top and sorted last-row
  bottom selection/focus corners follow the table radius. Cell padding is unchanged.
- Trees use vector disclosure chevrons, inset rows and semantic TreeItem parents.
  Selecting a row and expanding its branch remain separate actions.
- Trees mount only expanded branches. Expansion/selection remain controlled; stable
  node keys/IDs, bounded depth/model/visible-row budgets prevent oversized trees.
- Splits have a real captured draggable divider, not a decorative line. Ratio is
  controlled. `.axis(Direction::Vertical)` switches orientation. Minima constrain
  available space; unsatisfiable minima proportionally shrink instead of producing
  negative geometry. A focused divider supports arrows, Page keys and Home/End.
  Keys/disabled/removal/viewport changes cancel incompatible capture.

## Loading and editing polish

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<String> = vstack![
    progress(0.42).width(280.).accent_color(ColorToken::Success),
    progress_indeterminate().width(280.).period_ms(1200),
    loading("Loading records"),
    text_area("A long paragraph can wrap without changing the stored text.")
        .key("notes").soft_wrap(true).width(280.)
        .on_change(|notes: &mut String, value| *notes = value),
].gap(16.,Axis::Vertical).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Determinate progress accepts a finite fraction in 0..=1. Indeterminate bars/spinners
animate **only while mounted and visible**, and freeze under reduced motion,
inactivity or occlusion. Remove them when work completes. Styling and painting use
shared software/GPU paths; a component does not allocate its own device or window.

`text_area(...).soft_wrap(true)` uses whitespace-preferred wrapping with grapheme
fallback and shared independently shaped visual segments for paint, caret,
selection, hit testing and IME preedit. Stored text is unchanged. Explicit-line
mode is still the default; single-line soft wrap is rejected. This is not full
Unicode UAX14 line breaking or bidi visual-order editing. Foreign-language OS IME
and VoiceOver acceptance remain separate platform gates.

Offscreen **mounted** controls remain in Tab order; focus reveals nested scroll
ancestors without making clipped controls pointer-clickable. Visible scrollbars
capture dragging; hidden indicators do not expose an invisible pointer target.
