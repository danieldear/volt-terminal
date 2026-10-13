# Workspace UI

The workspace card is a compact workspace inspector, not a list of terminal
lines. Discovery and actions remain in Volt; Volt UI Kit supplies the shared
icons, buttons, palettes and rounded-surface drawing primitives. The host owns
workspace-specific composition and input so the same data/actions can later
serve a larger Workspace / Tasks / AI sidebar.

## Content contract

- A section is an expandable header with one icon and one click target.
- An action is one item, regardless of the number of visual lines its name uses.
- Metadata is one complete value, such as a path, upstream or PR title. Its icon
  appears once, at the beginning, not on every continuation line.
- Paragraphs have no item icon or click target.
- Code previews are one inset, read-only block with preserved line breaks.
- Notices are compact information banners, never simulated file items.

The content model does not pre-wrap or truncate paths, commands, branch names or
configuration previews. Rendering wraps within the available width, preferring
word/path boundaries. A full-width long status moves below its label. Only the
small card title/subtitle use bounded previews; section content remains readable.
Project languages/frameworks appear in the subtitle instead of individual
"Detected" file rows. The repository/project root is explicitly identified.

## Geometry and input

Items have variable heights. One layout supplies drawing, click targets,
keyboard reveal and content scrolling. Scrolling uses logical pixels, so a long
paragraph/code block can be traversed without becoming many item rows. Body
triangles and glyph UVs are clipped to the content rectangle: header/footer
controls cannot be covered or clicked through scrolled text.

Pin/dock, float/drag, minimize, refresh and close retain the existing contracts.
A floating card reserves no terminal columns; a docked card does. Minimize keeps
the terminal grid and vertical prompt position stable. Keyboard navigation skips
non-actions and brings a focused action into view.

Layout and shaped geometry are cached. Hover does not rewrap content. With the
card hidden, no card layout or drawing is performed. There is no extra timer,
GPU host/device, discovery loop or runtime dependency. Whole-app performance
still needs a comparable terminal benchmark; UI fixtures are not throughput
benchmarks.

## Theme editor

The editor now offers a saturation/value surface, hue strip, RGB sliders and
preset color swatches. Select a base or ANSI slot, then pick visually. Hex input
remains available for precision. The current slot/color is shown beside the
picker, with live terminal preview.

Wide windows show color slots and picker side by side. Narrow windows retain the
original panel width: choosing a color opens the picker inside the panel, and
Back returns to the color slots. Pointer capture ends on release, leaving the
window, losing focus, changing the slot, or starting theme naming. Arrow keys
adjust the last focused picker control.

Visual edits change only the working theme. Save/Save as remain explicit, Revert
restores the saved copy, and Cancel restores the pre-edit theme. Color picking
never writes a file by itself.

## Local macOS build and installation

```sh
# Build and replace the local Volt app, then open the installed copy.
bash scripts/macos/build_app.sh --release --install --open
```

The installed app is `~/Applications/Volt.app`. It uses the normal Volt
configuration, and must be closed before replacement. The existing build helper
backs up the previous installed bundle under `target/macos/install-backups`.
This is an ad-hoc signed local test build, not a notarized public release.
Omitted shell arguments use the normal login-shell default; explicit `args = []`
still requests a non-login shell.

## Automated validation

```sh
RUST_MIN_STACK=33554432 cargo test --locked --workspace --features volt/ui-gpu-prototype
cargo clippy --locked --workspace --all-targets --features volt/ui-gpu-prototype -- -D warnings
cargo run --locked --release -p volt-renderer --example cardcheck
cargo run --locked --release -p volt-renderer --example themecheck
```

The UI fixtures exercise cached pixels, clipping, real hit targets, clean hide,
wide/compact color picking, and 1× / 1.5× / 2× backing scales. Native drag, click,
keyboard and theme restoration checks are additional acceptance gates.
