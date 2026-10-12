# Original app-scale showcases

These examples demonstrate the existing Rust UI toolkit as complete, stateful
screen compositions. They use no webview, JSON UI, network service, downloaded
artwork or copied daisyUI/Codex assets. Their names, illustrations, conversations,
people, metrics and plans are original fictional/local demo content.

## Volt Atelier

```sh
cargo run --locked --features native-gpu --example showcase
# Opaque fallback, including software rendering
cargo run --locked --features native --example showcase
# Or open the same state/components from the existing Lab:
cargo run --locked --features native --example ui_lab -- --section showcase

# Direct screen and appearance selection
cargo run --locked --features native --example showcase -- --screen workspace
cargo run --locked --features native --example showcase -- --screen studio --theme light --palette tide
```

The `showcase` launcher defaults to 1280x900. `--size` overrides that default.
Use the **UI Lab** button to return to the toolkit workbench without losing local
counter, saved-project or task state. Lab navigation includes **Showcase** to return.

```text
+--------------------------------------------------------------------+
| volt / Atelier                                      UI Lab         |
| Discover / Workspace / Components        Midnight / Paper / System  |
+--------------------------------------------------------------------+
| Discover                 Workspace              Component studio   |
| Editorial typography     Activity bar chart     Live semantic theme |
| Original ribbon paths    Keyed task checklist   Button states       |
| Counter + callback       Project / avatars      Corners + clipping  |
| Rust composition sample  Calendar / player      Type + alerts       |
|                          Saved notice           Horizontal gallery |
+--------------------------------------------------------------------+
| Last Rust callback                                   Local demo    |
+--------------------------------------------------------------------+
```

### New component acceptance lives here

New demos and visual/interaction tests live in `examples/showcase/`, not the Lab
workbench. The old Lab remains a diagnostic/regression fixture. Activity and
Explorer use the same Atelier colors, SF/SF Mono, semantic text roles, corners and
responsive spacing as Discover/Workspace/Components.

```sh
cargo run --locked --features native-gpu --example showcase -- --screen activity
cargo run --locked --features native-gpu --example showcase -- --screen explorer
cargo run --locked --features native-gpu --example showcase -- --screen activity --theme light --palette tide
cargo run --locked --features native-gpu --example showcase -- --background opaque
cargo run --locked --features native-gpu --example showcase -- --background transparent --window-opacity 0.82
cargo run --locked --features native-gpu --example showcase -- --background blur --window-opacity 0.82
```

```text
Atelier / one shared design language
  Discover     editorial/vector composition
  Workspace    dashboard and existing controls
  Components   theme/corners/type/clipping/gallery
  Activity     cancellable worker + live click + menus + popover + wrapped notes
  Explorer     10,000-row viewport + selection + sortable table + tree + split
```

**GPU-enabled Showcase alone defaults to Blur with 0.82 root tint.** This smoke tint
is deliberate: dark text roles must stay readable even over a bright desktop.
The default embedded toolkit, Volt and software examples remain opaque. Explicit
CLI flags override the demo defaults. Lower opacity exposes more desktop but can
reduce contrast. `.background_opacity` affects surfaces only; text/controls are
not faded as a group. New cards use 0.88 surface tint; title/footer use 0.72.

The host requests real OS window material, not a wallpaper screenshot or a panel
shadow. Since 0.3.0, decorated macOS GPU windows use public AppKit
`NSVisualEffectView`; unavailable/borderless requests retain transparency without
blur. AppKit owns material appearance. This does not certify App Store packaging
or guarantee ChatGPT/Codex appearance parity. See
[window glass and platform limits](SURFACES_THEMES_AND_SCROLLING.md).
Windows/X11 do not gain blur from this winit API; Wayland needs a supporting KDE
blur protocol. GPU auto initialization failure logs opaque software fallback;
`VOLT_UI_RENDERER=gpu` fails rather than pretending GPU succeeded.

The visible-row factory is bounded (maximum 96 mounted rows), and updates its
controlled range on wheel/scrollbar/resize. Lists, progress, editors and split
panes use `.fill_width()` to follow their parent allocation instead of retaining
fixed-width builder defaults. SF Mono is reserved for identifiers and diagnostics.
Snapshots validate drawing only, not OS backdrop blur or physical interaction.

### Design system

Midnight base `#17181F`, graphite surfaces `#21232D`, paper text `#F4F2ED`,
iris `#BEB0F4`, apricot `#F5B895`, and mint `#9EE0C5` establish the dark direction.
The paper theme changes both surface and foreground roles; it does not just swap
the background. Iris/Tide/Ember select paired primary colors independently of
Lab's blue/violet/coral accent. Vector art intentionally keeps its illustration
palette. All UI text uses semantic foreground roles.

The interface uses SF with deliberate 10/13/19/34/52 logical-pixel sizes and
300/400/500/600/700 weights. SF Mono marks code, technical labels and aligned
numeric values. Containers use 20/28px corners, controls 10px, pills full rounding,
and selected-corner previews. Surfaces remain calm: no indiscriminate glow or
shadow on every card. Motion answers hover/focus and a dismissible notice.

The adaptive hero, workspace columns and studio columns stack below 1024 logical
pixels. Navigation and the gallery scroll horizontally; page bodies scroll
vertically and use distinct screen keys. Font size itself is not breakpoint-aware.
The calendar, focus player, task checklist, saved flag and plan controls dispatch
real Rust callbacks. The player is **visual only**, with no audio file or playback.
The pricing preview does **not** charge, subscribe or contact a service.

### Manual interaction check

1. Increment the welcome counter and toggle the saved sample.
2. Switch to Workspace: change period, complete tasks, select a day, save a project.
3. Dismiss the saved notice; play/pause and change the visual track preview.
4. Switch to Components: choose palettes, every radius (2px/12px/24px/Full), clipping
   and scrollbar visibility. After selecting Full, increment the counter and switch
   theme/screens; updates must continue without a rejected-tree error. Full uses
   `Rounding::Full`, not a large numeric radius sentinel.
5. Confirm the counter/saved values remain shared. Disabled action never fires.
6. Scroll the gallery to its last card, including with hidden indicators.
7. In Activity: start, click during progress, cancel/restart; open menus with arrows, Escape popover, edit a wrapped paragraph. Right-click Local workspace.
8. In Explorer: scroll/select records, drag the scrollbar, sort, expand Sources and resize the divider using pointer/keyboard.
9. Resize below/above 1024px, compare Midnight/Paper, then test Tab and Space.
10. Return to UI Lab and back. Reset keeps current screen/palette but resets demos.

## Volt Desk

An original desktop workbench inspired by the spatial structure of the Codex app,
not a Codex clone, OpenAI client or working agent service.

```sh
cargo run --locked --features native --example desk
cargo run --locked --features native --example desk -- --theme light
cargo run --locked --features native --example desk -- --compact --size 400x850
cargo run --locked --features native --example desk -- --no-inspector
```

```text
+--------------------------------------------------------------------+
| Sidebar toggle        Desk                         Theme / Details  |
+------------------+--------------------------------+----------------+
| Conversations    | Active conversation            | Session info   |
| New conversation | User / local assistant messages| Source labels  |
|                  | Rust composition preview       | UI layers      |
| Project          | Independent vertical scrolling | Save locally   |
|                  |                                |                |
| Local workspace  | Editable composer / Next / Send|                |
+------------------+--------------------------------+----------------+
| Local callback result                                              |
+--------------------------------------------------------------------+
```

Default launch size is 1360x900, with a 224px sidebar and 248px inspector. Those
panes can be hidden/restored from the title bar. Use `--compact` for narrow windows;
this example does **not** automatically collapse panes on resize. Conversation
width reflows when a pane is hidden; the composer remains outside the scroll area.
Independent scrolling keeps long messages and sidebar/inspector content reachable.
The empty pane-placement overlay explicitly uses `.scroll_passthrough(true)` on
its layout box and spacer, so it does not intercept conversation wheel events;
its sidebar/inspector descendants remain normal interactive scroll targets.

The composer is now a real controlled **editable text area**. Send adds your
edited text and a fixed local response; blank drafts cannot send. Next prompt
loads a sample into the editable draft. Cmd/Ctrl+Enter sends; ordinary Enter
inserts a new line. The draft is retained for repeatable local testing. New conversations
and messages have stable IDs; at most six conversations and sixteen messages per
conversation are kept in memory. Switch conversations, edit/send drafts, hide/show
the code preview, save the local flag, and compare light/dark. No model is invoked,
no tools execute and no workspace files are changed by these actions.

## SF and SF Mono: installed fonts, not bundled assets

The native `RasterPainter` supports additive family aliases:

- `SF` / `SF Pro`: prefer installed **SF Pro Text** below 24 logical px and
  **SF Pro Display** at/above 24px, then SF Pro or the macOS system SF family.
- `SF Mono`: prefer installed SF Mono, then the macOS system SF Mono family.
- On non-macOS or when an Apple face is absent: **sans-serif / monospace**.
- `system`, existing generic families and literal family names keep their previous
  behavior. In particular, this does not silently change legacy Avenir/Menlo choices.

The renderer uses its existing font database discovery. There is no font download,
private-path probing, font modification or redistribution. On this development Mac,
actual shaped glyph font IDs were verified as SF Pro Text / SF Pro Display / SF Mono,
not merely labeled SF while drawing another font. Other machines may fall back;
missing glyphs and weights still follow the text backend's fallback rules. This is
not AppKit's complete automatic optical-sizing/tracking behavior. Variable system
font availability and weight fidelity should be checked on your target Mac.

`RasterPainter::resolved_font_family(requested, physical_size)` reports the family
selected for an alias, accounting for the viewport scale. It does not report
per-glyph fallback. On macOS, users can install Apple's developer fonts themselves
if needed, subject to Apple's terms. Do not add Apple font files to this repository
or package them for other platforms. See [Apple's font overview](https://developer.apple.com/documentation/technologyoverviews/fonts)
and [SF font information and terms](https://developer.apple.com/fonts/).

## Reproducible checks

```sh
cargo test --locked --features native --example ui_lab --example desk
cargo test --locked --features native --lib sf_alias_tests

cargo run --locked --features native --example showcase -- --screen welcome --snapshot work/atelier-welcome.png
cargo run --locked --features native --example showcase -- --screen studio --theme light --size 400x850 --scale 2 --snapshot work/atelier-studio-compact.png
cargo run --locked --features native --example desk -- --snapshot work/desk.png
cargo run --locked --features native --example desk -- --compact --size 400x850 --scale 2 --snapshot work/desk-compact.png
```

Headless tests cover composition, pointer callbacks, scroll reachability, stable
keys, bounded state, pane geometry, compact/full layouts, 1x/2x DPI, semantic
appearance and real SF font selection. Snapshots use the actual native painter,
not an HTML mockup: software with `native`, Vello GPU readback with `native-gpu`
(unless `VOLT_UI_RENDERER=software`). See [GPU drawing](GPU_RENDERING.md). Headless results alone do **not** prove OS events or compositor
behavior.

On 2026-10-08, native macOS QA windows were also opened: Atelier navigation,
live palette/counter/paper-theme callbacks, and Desk Send sample, conversation
wheel scrolling, theme switching and inspector collapse were visually verified.
After fixing the Full preset, native clicks also verified Full rounding followed
by further counter updates and a switch to Paper without freezing the previous UI.
These are smoke tests, not exhaustive device acceptance. Trackpad momentum,
all keyboard/window gestures, transparency/blur, accessibility integration and
cross-platform font availability still require their own acceptance checks.

## Northbound (preserved contributor example)

`cargo run --locked --features native --example northbound`

The original Norway-inspired trip-planning sample has Route, Days, Journal and
Settings screens, generated Rust art, editable search/journal fields, sliders,
selection popups and controlled reservation/reset dialogs. Places, prices and
journal entries are fictional; no reservation or external service is contacted.
Its existing five tests pass alongside the new component/runtime tests. This
integration preserves the contributor's design rather than replacing it.
