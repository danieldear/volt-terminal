# Editable components and styles

The primary UI authoring API is [Rust-first declarations](RUST_UI.md); the same style semantics apply to Rust modifiers. This guide also documents optional JSON style objects and F5 reload.

The optional `native` examples use the same validated document/runtime as the
optional `declarative` layer. A JSON file describes layout, component properties
and appearance. Rust owns bindings and typed callbacks. There is no WYSIWYG
editor, scripting engine, CSS parser, shell execution or second renderer hidden
inside the document.

```text
Editable JSON                       Rust application
layout + styles                     state + registered handlers
        |                                     |
        +---------- validated UI tree --------+
                              |
                     layout / input / paint
                              |
                       native Solid window
```

## Try it

```sh
cargo run --locked --features native --example components
cargo run --locked --features native --example complete_app
```

Edit `examples/ui/components.json` or `examples/ui/complete_app.json` and press
F5. Font, color and style edits do not require a Rust rebuild. Rust handler changes
do require one. An invalid document preserves the last valid tree and prints a
diagnostic; it does not partly install a broken screen.

## Component reference

Each node requires a stable, unique `id` and a supported `type`. Optional common
properties are `width`, `height`, `flex`, and `style`. Sizes are logical pixels;
the runtime scales them to physical pixels using the window's DPI scale.

| Type | Main properties | Behavior |
|---|---|---|
| `row` | `children`, `padding`, `gap`, `align`, `flow` | Places children horizontally |
| `column` | `children`, `padding`, `gap`, `align`, `flow` | Places children vertically |
| `panel` | `children`, `padding`, `gap`, `align`, `flow` | Solid framed vertical container |
| `text` | Exactly one of `text` or `bind_text`; optional `font_size`, `tone` | Measured, wrapped text |
| `button` | `label`, `on_activate`; optional `enabled`, `bind_enabled`, `primary` | Typed `Activate` event |
| `checkbox` | `label`, `on_toggle_changed`; optional `checked` or `bind_checked`, `enabled`, `bind_enabled` | Typed new boolean value |
| `toggle` | Same contract as `checkbox` | Alternate boolean-control appearance |
| `badge` | Exactly one of `text` or `bind_text`; optional `font_size`, `tone` | Non-interactive status label |
| `separator` | Common properties only | Non-interactive horizontal divider (`divider()` in Rust) |
| `spacer` | `flex` defaults1; optional fixed width/height with `flex: 0` | Invisible non-interactive main-axis space |

Container spacing also supports directional padding and horizontal/vertical gap;
see [spacing semantics](RUST_UI.md#directional-padding-and-gap). Insets reserve
left+right/top+bottom independently for measurement, clipping and input.

Use `align`: `start`, `center`, `end`, or `stretch` to control a container's cross
axis. Optional `flow` accepts `top_left`, `top`, `top_right`, `left`, `center`,
`right`, `bottom_left`, `bottom`, `bottom_right` to place content in both axes.
Flow is placement, not wrapping; see [stack semantics](RUST_UI.md#stack-direction-and-content-placement).
`flex` distributes remaining space; this constrained layout is not CSS.
Text/labels are bounded to 8 KiB each. Names use ASCII letters, numbers, `_` and
`-`, up to 128 bytes. Documents are limited to 256 KiB, 4096 nodes and depth 48.

Checkbox and toggle bindings are explicit: the event delivers the desired value
to Rust, and the host publishes the updated `bind_checked` value. A literal
`checked` value without a binding stays literal until the document is reloaded;
use a binding for application-owned, changing state.

Rust `.rounded(4)` / `.rounded(4.5)` / `.rounded(Rounding::Full)` map to the
same local radius semantics. Full rounding follows resize/DPI and hit geometry;
it does not round the rectangular clipping of descendant content.

## Style reference

Put presentation overrides in a node's `style` object:

```json
{
  "type": "text",
  "id": "section_heading",
  "text": "Build preferences",
  "style": {
    "font_family": "Avenir Next",
    "font_size": 20,
    "font_weight": 600,
    "font_style": "normal",
    "color": "#ECF1F8"
  }
}
```

| Property | Accepted value | Notes |
|---|---|---|
| `color` | `#RRGGBB` or `#RRGGBBAA` | Text/label color; inherits |
| `accent_color` | `#RRGGBB` or `#RRGGBBAA` | Local interaction accent; does not inherit |
| `font_family` | Nonempty name, at most 128 bytes, no control characters | System-installed font; inherits |
| `font_size` | `8`–`96` | Logical pixels; inherits |
| `font_weight` | `100`–`900`, in steps of `100` | Typical regular `400`, medium `500`, semibold `600`, bold `700`; inherits |
| `font_style` | `normal` or `italic` | Inherits |
| `background` | `#RRGGBB` or `#RRGGBBAA` | Component-local surface color |
| `border_color` | `#RRGGBB` or `#RRGGBBAA` | Component-local frame color |
| `border_width` | `0`–`16` | Logical pixels; local |
| `radius` | `0`–`256` or `"full"` | Logical pixels, or half minimum laid-out dimension; local |

Unknown keys, wrong types, invalid colors and out-of-range values are rejected.
Use a six-digit color for an opaque Solid surface. The alpha byte is last:
`#A7B8FA80` is approximately 50% opacity, not an ARGB color. Alpha blending is
not backdrop blur or a Glass implementation.

### Inheritance and precedence

```text
Container font/color
        |
        +--> child inherits unspecified typography + text color
        |          |
        |          +--> child's explicit style overrides inherited value
        |
        +--> backgrounds, borders, radii and interaction accents stay local
```

Set the family and base type size on the root, then override only headings or
specialized text. The compatible top-level `font_size` and `tone` properties on
Text/Badge remain available. `style.font_size` wins over top-level `font_size`;
resolved `style.color` (including inheritance) wins the semantic tone color. Styles do not remove focus,
hover, pressed or disabled behavior. Checked indicators and interactive focus
outlines use a component's local
`accent_color`, falling back to theme accents when omitted. Text `color` does not
recolor the indicator. A checkbox/toggle `background` override styles its row
surface, not its checked indicator. `accent_color` does not inherit: set it on each
interactive component that needs a custom accent. It also colors the control's
hover outline and supplies a primary button's base fill when no explicit
`background` is set. With an explicit button `background`, that background wins
for the base fill while `accent_color` still controls its focus ring. On boolean
controls, `border_color` also supplies the indicator outline; omit `border_width`
(or set it to zero) to avoid drawing a frame around the whole control row.

Fonts are resolved from the operating system; the toolkit does not download or
embed them. The native adapter understands `"system"` as its platform default.
Named fonts absent from a machine can fall back to another available face, so
font weights, metrics and screenshots may vary between platforms. A requested
weight/style is limited by what the installed family provides.

### Three buttons, one event contract

```json
{
  "type": "button",
  "id": "apply_button",
  "label": "Apply changes",
  "on_activate": "apply",
  "bind_enabled": "can_apply",
  "style": {
    "background": "#A7B8FA",
    "color": "#151B25",
    "border_color": "#A7B8FA",
    "accent_color": "#A7B8FA",
    "border_width": 1,
    "radius": 7,
    "font_size": 13,
    "font_weight": 600
  }
}
```

The examples use an accent-filled primary action, a framed slate secondary action
and a quieter panel-colored action. These are ordinary style overrides—not three
separate button implementations or special callback types.

## Boolean controls connected to Rust

```json
{
  "type": "toggle",
  "id": "preview_preference",
  "label": "Open preview after applying changes",
  "bind_checked": "open_preview",
  "on_toggle_changed": "preview_changed",
  "style": {
    "font_size": 13,
    "color": "#ECF1F8",
    "accent_color": "#A7B8FA"
  }
}
```

```rust
use volt_ui_kit::declarative::{Bindings, Handlers};

struct App { open_preview: bool }
let mut handlers = Handlers::default();
handlers.on_toggle_changed("preview_changed", |app: &mut App, checked| {
    app.open_preview = checked;
})?;
// In the native host's update closure, after successful dispatch:
let mut bindings = Bindings::default();
bindings.set_bool("open_preview", app.open_preview);
```

The sample above is an API excerpt: the host supplies `app` and wires `handlers`
and its update closure through `native::run`. The runnable examples show the
complete setup. Callback signatures and binding types are validated before
installation. Callbacks run synchronously; enqueue slow work in the application
rather than block the event loop. The workbench only updates memory; its controls
do not persist configuration or execute build commands.

## Example design tokens

`complete_app` intentionally uses opaque, restrained Solid surfaces:

| Role | Value |
|---|---|
| Canvas | `#151B25` |
| Panel | `#1C2533` |
| Inset activity surface | `#19212D` |
| Frame/divider | `#334154` |
| Enabled control outline | `#52647E` |
| Primary text | `#ECF1F8` |
| Secondary text | `#AAB8CA` |
| Action accent | `#A7B8FA` |

Avenir Next is the macOS-first example family: 25/700 for the window heading,
20/600 for section titles, 13/400 for body text and 11–12 for supporting notes.
Menlo is used only where source-path/code content benefits from it. The components
example also includes a Georgia italic specimen and a green (`#85C8B3`) checkbox
accent to demonstrate local font and interaction-color overrides. This is an
example palette and hierarchy, not a hard-coded global toolkit theme.

## Scope and compatibility

Editable styles and additional components are opt-in. Existing Volt widget and
Painter APIs are not renamed or replaced, and the default path gains no native
host/event loop. This slice does not provide a theme stylesheet, arbitrary
expressions, font downloads, text editing/IME, accessible platform adapters,
menus, scrolling, multi-window support or GPU Glass.

Mouse and keyboard interaction must be verified in a real native window; snapshot
mode verifies rendering only. System fonts also make screenshots unsuitable as
portable pixel-golden references without a controlled font fixture.

Generic native fonts select installed families instead of assuming cosmic-text defaults exist. On macOS the current preferred sans/serif/mono families are Avenir Next/Georgia/Menlo when installed; named families remain explicit and fallback stays host-dependent. Legacy Volt text adapters are unchanged.

## Rust effects and selected corners

The Rust API now supports arrays/slices of spacing selectors, `.rounded_on`
corner/side groups, shadows/glows, subtree opacity, interaction patches and
time-based appearance animation. Rust buttons default to2logical-pixel corners;
JSON/legacy buttons remain7px. See [full contracts](EFFECTS_AND_WINDOWING.md).
Child clips are rectangular; this is not backdrop blur or a native glass style.
