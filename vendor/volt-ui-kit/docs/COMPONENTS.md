# Reusable Rust UI components

## Delivered Rust-first component family (2026-10-09)

These are real reusable builders under `rust-ui`, not mock controls or JSON:

| Component | Contract |
| --- | --- |
| `button_bar![]` / `button_bar(items)` | Horizontal actions; independent Rust callbacks; optional joined outer corners |
| `toolbar![]` / `toolbar(items)` | Horizontal application toolbar; accepts text, spacers, actions and other elements |
| `segmented_control([(value, label), ...])` | Joined, controlled typed selection |
| `tab_bar![tab_item(value, label), ...]` | Controlled typed selection with per-item keys and disabled options |
| `tabs![tab(value, label, content), ...]` | Tab strip plus **only the selected page**, above or below the page |
| `text_field(value)` / `text_area(value)` | Controlled plain-text editing, selection, bounded undo/redo, IME and native text clipboard |
| `slider(value)` | Controlled finite `f32` range/step, captured drag and keyboard input |
| `select([select_item(value, label), ...])` | Typed selection with controlled, anchored popup and disabled options |
| `dialog(content)` | Controlled in-window modal, input isolation, trapped/restored focus, dismissal |

Bars lower to existing Rows, Columns and Buttons; editors/sliders/overlays carry
Rust-only metadata through that document. No separate window, GPU device or renderer
is allocated per component. Unicode segmentation is optional under `rust-ui`; text
clipboard support is native-only. Default embedded dependencies remain unchanged. These are custom-painted UI
components, **not** OS-native toolbars/menus or an accessibility implementation.

```text
Application state: Page enum, density, counters
                   |
                   v
Rust view() -> toolbar / button_bar / segmented_control / tabs
                   |
                   v
Existing document + typed callback registry
                   |
                   v
Shared layout / pointer / focus / painting
                   |
           selection event
                   |
                   +--> Rust on_select -> update App -> recompose
```

## Variants, sizes and refined defaults (2026-10-10)

Review every default together with `cargo run --features native --example specimen`
(add `--theme light`, or `--snapshot PNG --size 1400x1000` for headless review).

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
.background(ColorToken::Primary.alpha(8))
.into();
actions.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- `ButtonVariant` (Neutral, Primary, Soft, Outline, Ghost, Danger) and
  `ControlSize` (Small 28px/12pt, Medium 36/14, Large 44/16) are theme-token
  presets; later `.background`/`.color`/`.hover_style` calls still override them.
- `.disclosure(open)` draws a trailing vector chevron (up while open) and
  left-aligns the label; select and menu triggers use it. `.label_align(Align)`
  places any button label at the start, centre or end.
- Theme tokens accept an opacity: `ColorToken::Primary.alpha(15)` (`"@primary/15"`)
  resolves from the active palette, so tints follow light/dark and accents.
- Primary buttons pair their label with `PrimaryContent` explicitly, so an
  inherited text colour can no longer make them unreadable; built-in palettes
  derive `on_primary` by contrast with the primary fill.
- Defaults: buttons and bars use 6px corners (JSON documents keep their
  legacy recipe); `panel(...)` is a padded surface (12px, override with
  `.padding`); joined bars and segmented controls show 1px seams.
- Tables: sortable headers are left-aligned ghost buttons and fixed columns are
  plain labels (no disabled look); rows are ghost targets with a soft selected
  tint; cells are inset to align with headers; height is natural unless set.
  Trees use left-aligned ghost rows with a soft selection and muted expanders,
  and report the specific validation problem.

## A complete declaration

```rust
use volt_ui_kit::ui::prelude::*;

#[derive(Clone, Copy, Default, PartialEq)]
enum Page { #[default] Overview, Activity }
#[derive(Default)]
struct App { page: Page, count: usize }

fn view(app: &App) -> Element<App> {
    vstack![
        toolbar![
            text("My workspace").font_weight(600), spacer(),
            button_bar![
                button("Add").key("add")
                    .on_click(|app: &mut App| app.count += 1),
                button("Clear").key("clear").enabled(app.count > 0)
                    .on_click(|app: &mut App| app.count = 0),
            ].joined(true).rounded(8.),
        ],
        segmented_control([
            (Page::Overview, "Overview"), (Page::Activity, "Activity"),
        ]).key("page-picker").selected(app.page).rounded(Rounding::Full)
            .on_select(|app: &mut App, page| app.page = page),
        tabs![
            tab(Page::Overview, "Overview",
                text(format!("Count: {}", app.count)).font_size(24.))
                .key("overview-tab"),
            tab(Page::Activity, "Activity", text("A second, mounted-on-demand page"))
                .key("activity-tab"),
        ].key("pages").selected(app.page).placement(TabPlacement::Bottom)
            .on_select(|app: &mut App, page| app.page = page)
            .gap(16., Axis::Vertical),
    ].padding(20.5, [Edge::Horizontal, Edge::Top])
        .gap(18., Axis::Vertical).font_family("sans-serif").into()
}

let app = App::default();
view(&app).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

The duplicated selector here demonstrates sharing a typed enum between widgets;
normally choose either a segmented picker or tab strip for the same navigation.
Use `native::run_ui` to drive a view, or supply your own document/input host.

### Item keys and disabled choices

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<usize> = tab_bar![
    tab_item(0, "Editor").key("editor-tab"),
    tab_item(1, "Preview").key("preview-tab"),
    tab_item(2, "Later").key("later-tab").enabled(false),
].key("navigation").selected(0)
 .on_select(|page: &mut usize, next| *page = next)
 .height(44.).scroll(Scroll::Horizontal).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Use explicit item keys for reordered/dynamic options. The tuple-based segmented
factory uses positional automatic keys; use `tab_bar(items).joined(true)` when
you need keyed segmented items. Values need `Clone + PartialEq + 'static`; they
can be enums, integers or application types. Labels are display text, not IDs.

## Behavior and boundaries

- Selection is **controlled**: `.selected(app.value)` and `.on_select(...)`.
  The host must rebuild/install the view after activation, as `run_ui` does.
- Missing callbacks, duplicate values, empty selections, a selected value not in
  the options, duplicate keys and invalid styles fail `build()` before installation.
  If `.selected(...)` is omitted, the first option is selected.
- Disabled items do not activate; `.enabled(false)` disables all items in a
  selection control. A disabled item may remain the app's selected value.
- Selecting the already-selected option emits **no** callback and requests no
  recomposition. Keyboard selection wraps and skips disabled/unlaid-out options.
- With an item focused, native Left/Right and Home/End select within its group.
  Embedded hosts use `Runtime::selection_key`. Enter/Space and Tab/Shift-Tab retain
  the existing button contract. This is not yet ARIA/platform roving-tab semantics.
- `tabs` installs callbacks and layout only for the active page. Keep persistent
  page data in App; inactive pages do not retain a hidden widget/cursor subtree.
  Rust expressions passed to `tab(...)` are still evaluated eagerly during view
  construction. This is active-only **mounting**, not lazy evaluation of arbitrary
  page-producing functions. Build expensive content conditionally in your view.
- `TabPlacement::Bottom` moves the strip; it does not add an iOS/Android host.
- Container styling/spacing/font/theme/clip/scroll modifiers work on the group.
  Selected options explicitly use `PrimaryContent` for the primary surface so
  a general inherited foreground cannot make pale accent buttons unreadable.
  Individual button styling remains available in button bars/toolbars. This first
  selection API does not provide arbitrary per-option view templates.
- `.joined(true)` removes horizontal gaps and squares internal corners. `.rounded`
  and `.rounded_on` choose the **outer** corners; Full stays symbolic. Joined child
  corners intentionally replace each child's independent radius.
- Toolbars are horizontal, not automatically wrapping or overflowing into menus.
  Use responsive stacks/scrolling around large groups. Keyboard focus auto-reveal,
  touch navigation and screen-reader semantics remain separate follow-ups.
- Legacy widgets/Painter and the schema1 JSON format remain unchanged. Group
  keyboard behavior is a Rust-only opt-in; ordinary buttons ignore selection keys.

## Test in Catalog

```sh
cargo run --release --locked --features native-gpu --example catalog -- --page button-group
cargo run --release --locked --features native-gpu --example catalog -- --page tabs
cargo test --locked --features rust-ui --test component_bars
cargo test --locked --all-features --example catalog
```

Catalog contains live bars/tabs, disabled options and callback witnesses. Existing
Showcase/Lab examples remain regression fixtures. Automated checks cover pointer
and keyboard callbacks, selected corners, disabled items, active-page-only
handlers, DPI and compact layouts. Real IME and other-platform interaction remain
separate [validation gates](VALIDATION.md).

## Editable fields, value controls and modal declarations

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
            text_area(&app.notes).key("notes").height(120.).font_family("SF Mono")
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
    ].font_family("SF").into()
}
view(&App::default()).build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

```text
Rust state: draft / notes / level / order / open flags
                         |
                         v
view -> controlled fields + slider + select + dialog
                         |
            +------------+-------------+
            |                          |
runtime: caret/selection/IME     portal: topmost input scope
         undo/redo/clipping             focus + dismissal
            |                          |
            +---------- typed callbacks +--> App -> recompose
```

### Exact contracts and limits

- Mutable fields require `on_change`; `.read_only(true)` permits a field without
  it. `.enabled(false)` disables input. Single-line Enter calls `on_submit`;
  multi-line Enter inserts a newline; **Cmd/Ctrl+Enter** calls its submit callback.
  Tab changes focus, not document indentation.
- Stable keys retain caret, selection and history through controlled recomposition.
  An externally replaced value resets undo history. Limits: **8192 UTF-8 bytes per
  editor and 64 undo snapshots**. Oversized inserts are rejected, not byte-truncated.
- Arrows/Shift-arrows, Home/End, select-all and Cmd/Ctrl undo/redo are supported.
  Native clipboard initialization is lazy and retained; a failed copy must not cause
  cut deletion. OS IME preedit is paint-only until commit, with a candidate rectangle.
  There is deliberately **no idle caret-blink repaint timer**.
- `.password(true)` masks painted text and blocks copy/cut and editor diagnostic
  text. **App state remains ordinary plaintext in memory**, not an encrypted vault.
- `text_area` is plain text, not rich text, syntax highlighting, spellchecking or a
  production code editor. Lines follow explicit newlines; wheel scrolling and caret-following use a clipped
  internal viewport. `.soft_wrap(true)` opts into bounded measured wrapping;
  fully correct complex bidi visual navigation/selection is not claimed.
- Sliders validate finite values, ascending bounded range, representable positive
  step and in-range initial value. Pointer capture survives controlled recomposition;
  equal values emit no callback. Arrows/Home/End/PageUp/PageDown change value.
- Select requires a stable `.key(...)`, **1–128 unique typed options**, `on_select`
  and `on_open_change`. Up/Down/Home/End reveal offscreen choices; Enter commits.
  Closed/disabled popups unmount their options. The popup flips above its anchor
  near the bottom edge, escapes ancestor clips, and scrolls long fixed-height lists.
- Dialog styles/width target the card, not its full-window backdrop. Place it after
  covered content in a root `zstack`. Escape and backdrop dismissal can be disabled;
  close callbacks must update the controlled open flag. Nested scopes isolate input,
  trap Tab, restore surviving opener focus and cancel stale press captures.
- These are **custom-painted in-window controls**, not OS dialogs, native menus,
  accessibility roles or an automatic focus-reveal implementation for arbitrary
  scroll containers. Keyboard reveal is guaranteed for Select's own option list.
- Schema1 JSON/legacy widgets remain unchanged; these richer builders are Rust-only.
  Optional declarative `Event`/`EventKind` gain a numeric event; downstream exhaustive
  matches using that optional API may require a new arm. This does not change the
  default embedded API or synchronize Volt's separate vendored copy automatically.

Custom hosts should route `pointer_down_outcome` / `pointer_move_outcome` (with
handlers/context), `pointer_up_outcome`, `slider_key(ValueKey)`, `overlay_key`,
`dismiss_overlay`, editor keys/text and IME methods. The legacy bool pointer helpers
cannot dispatch continuous slider changes. The native host supplies this routing.

## Additional Rust components

These APIs are Rust-only; schema-1 JSON retains its existing format. Values and
callbacks remain controlled by application state. Catalog's Utilities, Icons,
Date & time, Form inputs, Overlays, Data display and Charts pages demonstrate them.

### Layout, text and style utilities

| Item | Usage |
|---|---|
| Cursor styles | `.cursor(Cursor::Pointer)`; buttons/links → pointer, editors → I-beam, split handles → resize |
| Text alignment | `.text_align(TextAlign::Center)` |
| Line height | `.line_height(1.5)` (multiple of font size) |
| Truncation | `.max_lines(1)` → ellipsis; `.no_wrap(true)` |
| Case / decoration | `.uppercase()`, `.underline(true)`, `.strikethrough(true)` |
| Min/max sizes | `.min_width() .max_width() .min_height() .max_height()` |
| Fractional sizes | `.width_fraction(0.5)` (share of the parent's space) |
| Aspect ratio | `.aspect_ratio(16. / 9.)` |
| Justify | `.justify(Justify::SpaceBetween)` (start/center/end/between/around/evenly) |
| Wrapping rows | `.wrap(true)` on a row |
| Grid | `grid(children).columns(3)` or `.min_column_width(220.)`; child `.column_span(2)` |
| Single-side borders | `.border_sides(Sides::BOTTOM)` |
| Dashed / dotted borders | `.border_style(BorderStyle::Dashed)` |
| Gradients | `.background_gradient(Gradient::linear(angle, stops))`, radial |
| Translate / hover lift | `.offset(x, y)`, `.hover_style(\|s\| s.offset(0., -2.))` |
| Pulse (skeletons) | `Ambient::pulse(0.4)` opacity animation |
| Slide / fade transitions | `Transition::slide(dx, dy)` alongside fade |
| Custom easing / delay | `Easing::CubicBezier(..)`, `Animation::delay_ms` |
| Disabled / focus-visible styles | `.disabled_style(..)`; focus rings only after keyboard focus |
| Responsive visibility / width / font size | `.visible(responsive(false).at(Md, true))`, responsive width / font size |
| Programmatic scroll | `.scroll_to(key, request)` on a scroll view |
| Bundled fonts | `native::register_font(bytes)` |
| SVG path import | `VectorPath::from_svg(d)` |
| Design tokens | `tokens::space::*`, `tokens::radius::*`, `tokens::text::*` scales |

### Icons

A built-in, theme-aware vector icon set (`icon(Icon::Search)`), 65 glyphs,
original drawings on a 24-unit grid.

### Date and time

`CalendarDate` (no external date crate), `calendar`, `date_picker`,
`date_range_picker`, `time_picker`.

### Form inputs

| Item | Usage |
|---|---|
| Radio group | `radio_group([(v, "Label")]).selected(v).on_select(..)`, descriptions, horizontal |
| Number input | `number_input(v).range(0., 10.).step(1.).decimals(1).suffix(" kg")` |
| Combobox | `combobox(items, &state).state(..).on_select(..)`; type to filter, Enter picks |
| Tag input | `tag_input(&tags, &state).on_change(..)`; Enter adds, × removes |
| Range slider | `range_slider(lo, hi).on_change(\|app, lo, hi\| ..)`; nearer thumb, no crossing |
| Colour picker | `color_picker("#3B82F6")`: swatches, H/S/L sliders, hex field (`Hsl`) |
| Drop zone | `drop_zone().accept(["png"]).on_drop(..)`; native drag and drop via winit |
| One-time code | `otp_input(code, 6).on_complete(..)` |
| Rating | `rating(n).max(5).on_change(..)`; read-only without a handler |
| Adornments | `text_field(..).leading_icon(Icon::Search).trailing_action(..)` |
| Form field | `form_field("Email", control).description(..).error(..).required(true)` |

Runtime additions: two-thumb sliders (`SliderProps.lower`), `Event::FilesDropped`
with `Runtime::file_hover` / `drop_files`, and `Popover::keep_anchor_interactive`
so a combobox field keeps focus and typing while its list is open.

### Overlays and disclosure

`accordion`, `collapsible`, `drawer`, `toaster`, `command_palette`, menu
check/radio items + submenus + shortcut hints, `hover_card`.

Submenus open in place of the parent list with a back row (drill-down)
rather than as cascading side panels; both are controlled by app state.

### Display

`avatar` / `avatar_group`, `alert`, `skeleton`, `kbd`, `breadcrumb`,
`pagination`, `steps`, `timeline`, `empty_state`, `stat`, charts
(`sparkline`, `bar_chart`, `line_chart`), `carousel`, `image` with fallback
(`image_view`).

### Limits and unfinished interaction polish

- Range sliders currently expose one accessibility value; arrow keys move the
  last grabbed thumb. Independent keyboard/screen-reader thumb targets remain.
- Combobox Enter chooses the best match; command palettes support Tab. Dedicated
  Up/Down result navigation remains to be added.
- Toasts are dismissed by application state, not a built-in auto-dismiss timer.
- Submenus use controlled drill-down/back rows, not cascading multi-popup focus.
- Charts are static vectors; interactive tooltips and labelled axes are deferred.
- Offsets and slide/ambient motion are paint-only: layout and hit targets stay put.
- Registered fonts share immutable bytes across windows, with 64 files / 256 MiB
  total. `VectorPath::from_svg` parses path data, not complete SVG documents.
- Native IME, assistive technology, other-OS interactions, energy and device-loss
  acceptance are distinct from API availability; see [validation](VALIDATION.md).
