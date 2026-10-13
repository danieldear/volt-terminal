# JSON interfaces (optional)

Most applications declare their interface in Rust. The `declarative` feature
also accepts the same kind of tree as a JSON document, which you can edit and
reload (F5) without rebuilding. Rust still owns all behaviour: the document
names events and bindings, and your Rust code handles them.

```text
JSON document ──parse + validate──▶ tree ◀── Rust handlers and bindings
                                      │
                             layout · input · paint
```

Run the examples and edit their files under `examples/ui/` while they run:

```sh
cargo run --features native --example complete_app
cargo run --features native --example components
```

## Documents

```json
{"schema_version": 1, "root": {"type": "column", "id": "root", "padding": 24, "gap": 16, "children": [
  {"type": "text", "id": "status", "bind_text": "status", "font_size": 20},
  {"type": "button", "id": "save", "label": "Save", "on_activate": "save",
   "bind_enabled": "can_save", "primary": true}
]}}
```

| Type | Properties (besides `id`, `width`, `height`, `flex`, `style`) |
| --- | --- |
| `column`, `row`, `panel` | `children`, `padding`, `gap`, `align`, `flow`, directional `padding_top` … `gap_vertical` |
| `stack` | as above, plus `direction`: `vertical` or `horizontal` |
| `zstack` | as `column`; children overlap |
| `text`, `badge` | `text` or `bind_text`; `font_size`, `tone` (`body`, `muted`, `accent`, `title`) |
| `button` | `label`, `on_activate`; `enabled` or `bind_enabled`; `primary` |
| `checkbox`, `toggle` | `label`, `on_toggle_changed`; `checked` or `bind_checked`; `enabled` |
| `separator`, `spacer` | layout only |

Every node needs a unique `id` (1–128 ASCII letters, digits, `_` or `-`).
Unknown properties and wrong types are errors, reported with a line and column
or the node's path. Documents are limited to 256 KiB, 4,096 nodes and depth 48.
The JSON format covers the core layout and controls; the larger components
(tables, menus, charts, …) are Rust-only.

## Styles

Any node can carry a `style` object:

```json
{"type": "text", "id": "heading", "text": "Build preferences",
 "style": {"font_family": "Avenir Next", "font_size": 20, "font_weight": 600, "color": "#ECF1F8"}}
```

Supported keys: `color`, `accent_color`, `background`, `border_color` (hex
colours or `@role` theme tokens such as `@primary/15`), `font_family`,
`font_size` (8–96), `font_weight` (100–900), `font_style` (`normal` or
`italic`), `border_width` (0–16) and `radius` (0–256 or `"full"`). Typography
and text colour inherit; the rest apply to the node alone.

## Connecting Rust

```rust
use volt_ui_kit::declarative::{Bindings, Event, Handlers};
#[derive(Default)]
struct App { saves: usize }
let mut handlers = Handlers::default();
handlers.on_activate("save", |app: &mut App| app.saves += 1)?;

let mut values = Bindings::default();
values.set_text("status", "Ready");
values.set_bool("can_save", true);

let mut app = App::default();
handlers.dispatch("save", Event::Activate, &mut app)?;
assert_eq!(app.saves, 1);
# Ok::<(), volt_ui_kit::declarative::HandlerError>(())
```

`native::run(options, source, state, handlers, update)` opens a window for a
document. After each event, `update` writes your state into the bindings
(`set_text`, `set_bool`), which the document reads through `bind_text`,
`bind_checked` and `bind_enabled`. Handler names and payload types are checked
before anything runs.

Press F5 to reload the file. An invalid edit keeps the last valid interface and
prints the error; handlers are registered once and survive reloads.
