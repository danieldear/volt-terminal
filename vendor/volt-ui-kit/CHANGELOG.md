# Changelog

## Unreleased

## 0.4.0 (2026-10-12; not published to crates.io)

- Minor-version milestone for the expanded Rust component set, application
  foundation, native desktop integrations and Catalog. Existing default
  dependencies and opt-in native/GPU feature boundaries are unchanged.
  Private-compositor corner switching, generic in-app GPU backdrop blur,
  full composite keyboard/accessibility acceptance, multi-window lifecycle
  and signed distribution remain separate gates; see `docs/VALIDATION.md`.

- Harden the component-parity delivery after review: validate inactive
  responsive rules and disabled offsets, bound date/time and pagination
  arithmetic, keep number-input callbacks finite, revalidate file-drop scope
  and enablement, and retain private file permissions on linkless volumes.
  Align editable ink, caret/selection and IME geometry consistently; form
  labels name controls unless an explicit accessible name was supplied.
  Registered fonts now share immutable bytes across windows and have a
  256 MiB aggregate limit. Add review regressions and an acceptance receipt.

- `select` is styled as a field, matching text fields and `combobox`: a
  light filled trigger with a thin outline and an inner chevron (hover
  darkens the outline), and a card popup with quiet rows and a check mark on
  the chosen option instead of a primary fill. Behaviour, keys and
  accessibility are unchanged; `.background(..)` / `.border_color(..)` still
  override. The catalog Select page compares select and combobox.
- Catalog chrome uses the new components: `command_palette` for ⌘K search
  (grouped pages, fuzzy keywords), a `kbd` hint on the search button, an
  icon filter field with a clear action, a clickable `breadcrumb`, `toaster`
  confirmations, checked accent menu items, `form_field` sign-in with a
  password reveal, and a `command_palette` for application commands.

- Pixel-sized split panes: `split(a, b).first_size(px).max_first(px)` keeps
  the first pane at a fixed width as the window resizes and reports pixels
  from `on_resize`; `.handle_color(..)` styles the divider. A responsively
  hidden first pane collapses the split. `.fill_height()` joins
  `.fill_width()`. `.handle_style(SplitHandle::Reveal)` hides the divider
  until hover, drag or keyboard focus, then shows a rounded pill; the resize
  cursor holds for the whole drag. Divider drags patch the installed
  document and lay out once per frame: `on_resize` still fires on every move,
  but moves return `changed` (layout) rather than `activated` (recompose), and
  the host recomposes once on release. Layout is cheaper when widths change:
  text that fits unwrapped reuses its natural-width measurement (and draws
  from it when start-aligned), and the measure memo, style map and shaped-text
  cache are hashed rather than ordered by id/text strings (catalog landing:
  24.7 → 5.9 ms per drag frame at 2×; other pages 1–1.6 ms). The catalog sidebar is now resizable (200–420 px), and
  collapses into a drawer below 1024 px; the landing hero has compact,
  medium and full sizes; doc pages stop at a 1040 px column.

- Tailwind / shadcn parity (see `docs/COMPONENTS.md`). All new APIs are
  Rust-only; the schema-1 JSON format is unchanged.
  - Layout and style utilities: `grid(..).columns(n)` / `.min_column_width`
    with `.column_span`, `.wrap(true)`, `.justify(Justify::…)`, min/max
    sizes, `.width_fraction`, `.aspect_ratio`, responsive `.visible` /
    width / font size, `.text_align`, `.line_height`, `.max_lines` /
    `.truncate`, `.uppercase`, `.underline`, `.strikethrough`, single-side
    and dashed/dotted borders, linear/radial gradients, `.offset`,
    `.cursor(Cursor::…)`, `.scroll_to`, design `tokens`, `Ambient::pulse`,
    slide transitions, `Easing::CubicBezier`, animation delays, disabled and
    focus-visible styles, `native::register_font`, `VectorPath::from_svg`.
  - Icons: `icon(Icon::…)`, 65 original theme-aware glyphs (`ALL_ICONS`).
  - Date and time: `CalendarDate`, `calendar`, `date_picker`,
    `date_range_picker`, `time_picker`.
  - Form inputs: `radio_group`, `number_input`, `combobox`, `tag_input`,
    `range_slider`, `color_picker` (`Hsl`), `otp_input`, `rating`,
    `form_field`, text-field adornments (`.leading_icon`,
    `.trailing_icon`, `.trailing_action`) and `drop_zone` with native file
    drag and drop.
  - Overlays and disclosure: `collapsible`, `accordion`, `drawer`,
    `toaster` (`Toast`), `command_palette`, `hover_card`; menu rows gain
    `.checked`, `.shortcut`, `.icon`, and `menu_submenu` drill-downs.
  - Display: `avatar` / `avatar_group`, `alert` (`Severity`), `skeleton`,
    `kbd`, `breadcrumb`, `pagination`, `steps`, `timeline`, `empty_state`,
    `stat`, `sparkline`, `bar_chart`, `line_chart`, `carousel`, `image_view`.
  - Runtime: two-thumb sliders, `Event::FilesDropped` /
    `Handlers::on_files_dropped` with `Runtime::file_hover` / `drop_files`,
    `Popover::keep_anchor_interactive`, `Button::label_inset`, and keyboard
    menu navigation over decorated (icon/shortcut) rows.
  - Catalog: Utilities, Icons, Date & time, Form inputs, Overlays, Data
    display and Charts pages.
  - Upgrade notes: `Event` and `EventKind` gain a `FilesDropped` variant
    (exhaustive matches need an arm). The prelude now exports `avatar`,
    `stat`, `steps` and other component names; a local function with the same
    name imported by glob becomes ambiguous — import it explicitly.

- Animation frames are paced by the display. GPU windows (FIFO presentation)
  request the next frame after each present and let vsync set the rate —
  120 Hz on ProMotion, measured 601 frames in 5 s — instead of a fixed
  16.7 ms timer re-armed after each frame, which drifted to ~48 fps and beat
  against vsync (judder). Software windows tick at the monitor's refresh
  interval on a drift-free cadence. Refresh is rechecked each second while
  animating (windows can move between displays).

- `Ambient` paint-time motion: `.ambient(Ambient::float(x, y))` drifts a
  subtree on a slow ellipse and `.ambient(Ambient::spin())` rotates a canvas,
  with `period_ms` and `phase`. Applied at paint only: no recomposition,
  layout or hit-target movement; frames continue only while an ambient node
  is visible, and reduced motion stops them.
- `.letter_spacing(em)` tracking for text (Rust-only style; schema 1 unchanged).
- Catalog landing hero: typography-led on the page's own background (light or
  dark): a custom vector VOLT wordmark, a sans + italic-serif headline, square
  letter-spaced mono buttons, two hairline orbits with one slow accent arc,
  and three small floating cards built from real components. Letter-spaced
  mono navigation with an inverted active chip; a swaying capability ticker.
- Ambient drift is clipped like a paint-only transform (by clipping ancestors,
  not the parent's padding box), so drifting content never loses an edge.

- Font fallback lists: `.font_family("New York Extra Large, Charter, serif")`
  picks the first installed family, like CSS; aliases (`SF`, `SF Mono`) and
  generic names in a list always resolve. Installed families are indexed once.
- Catalog typography: New York display serif with italic accents for the
  landing hero, section headings, numerals and page titles; numbered SF Mono
  eyebrows; JetBrains Mono for code. Each has fallbacks for other machines.

- Smooth scrolling: a scroll moves boxes without resizing them, so the
  runtime now shifts the scrolled subtree in place and recomputes its clips,
  instead of a full layout per wheel event (catalog landing at 2x: 4.3 ms →
  0.19 ms per scroll frame). Open overlays and virtual lists still take the
  full layout. Resolved styles are also cached across layouts until the
  document or viewport width changes. Tests prove the shifted result equals a
  full layout, including nested scrollers and real catalog pages.
- Catalog landing: hero and product shot no longer use rounded clip layers,
  which nested five deep and spilled Vello's blend stack on most scroll
  positions (up to 3 GPU strips per frame); now no spill.
- Shared Rust example harness: `--bench-scroll PX` times scroll frames.

- Shaped-text cache raised from 256 to 2048 layouts. A page whose labels
  (measured at several widths) exceeded 256 made the LRU miss on every
  lookup, reshaping all text every frame: the catalog landing page took
  ~38 ms of layout per click and 89k misses in 87 frames; now ~4.8 ms.

- GPU strip rendering: frames too large for Vello 0.6's fixed per-pass
  buffers (5K/6K/8K Retina windows exceed the tile-command buffer on its
  per-tile reservation alone) render in up to 16 horizontal strips on the GPU
  instead of every frame falling back to software (6K: ~280 ms software →
  ~27 ms GPU including readback). Budgets are tracked per tile row; frames
  whose strips still overflow, or that exceed scene-wide buffers, keep the
  explicit software fallback. `VOLT_UI_TRACE` reports `bands=`.
- New-file saves fall back to an exclusive create where hard links are
  unsupported (exFAT, many network shares); never overwrites either way.
- `cargo volt bundle` names the executable after the app (ASCII-safe,
  `VoltApp` fallback) instead of always `VoltApp`.
- Catalog landing page redesigned as an editorial tour: aurora hero, a live
  miniature app, numbers, features, code with a live counter, live theming,
  the component index and a closing call to action.

- Keep Retina opacity-group overflow fallback bounded with tiled replay; preserve alpha instead of failing again in software. Reuse a hidden native material underlay on blur-backend switches, refresh native frame/shadow, and prepare backdrops before window visibility. Desktop-level Material-to-Compositor corner validation remains pending.

- Separate Catalog background opacity from accent tint; add live blur toggle and
  public-material/explicit compositor selection. Add state-derived native
  backgrounds and root-only theme color overrides. Raw compositor blur uses a
  private macOS API, remains opt-in and checks Reduce Transparency on redraw.
  No dependency or legacy default changes; in-app GPU backdrop blur is pending.

- Fix native Metal window presentation with pinned wgpu 26: compensate for its
  misadvertised straight-alpha mode, restoring translucent sidebar fills,
  badge/selection backgrounds and antialiased text/borders. No tint, material,
  font-family, public default or dependency changes. Add backend-policy and
  real-GPU presentation-pixel regressions, SF face/weight checks and opt-in
  surface/font diagnostics; see `docs/GPU_RENDERING.md`.

- Generated apps now use Catalog-style unified macOS chrome instead of a separate
  native title strip. All five templates retain native traffic lights, include an
  editable 60pt drag header with safe button clearance, SF and semantic colors.
  Other platforms keep native chrome. Existing application defaults/projects,
  blur policy and dependencies are unchanged.

- Add independent optional `native-notifications` with feature-pruned safe
  macOS UserNotifications bindings: explicit alert-only permission, bounded silent
  local requests, one-shot delay, stable IDs and removal. Main-thread/.app checks
  and accepted-not-displayed semantics are explicit; no automatic permission.
  Settings/action/foreground delegates and other OS adapters remain unsupported.
- Catalog Desktop services adds notification/status/error witnesses and guarded
  shared tray commands. Native permission, actual display (human screenshot),
  counted tray delivery (human click + CUA witness) and delivered removal (human
  disappearance confirmation) accepted on the test Mac; pending-only cancellation
  and native tray enablement/modal acceptance remain separate.
  Add a default-off 60-second cancellation-test option; retain captured-delay
  receipts and busy/modal regression coverage without new host API or crates.
  One new pinned optional framework crate; Rust 1.87/default/Volt contracts
  retained. See `docs/DESKTOP_NOTIFICATIONS.md`.

- Extend optional `cargo volt` tooling with macOS `.app` packaging for prebuilt
  executables: explicit metadata, bounded flat resources/ICNS/privacy descriptions,
  no overwrite, Mach-O/system-library/deployment checks and local ad-hoc signing.
  No new crates; Developer ID/notarization, installers and nested code are not
  supplied or claimed. See `docs/MACOS_PACKAGING.md`.

- Add optional public macOS Open/Save panels and explicit unsupported/main-thread
  errors, using existing native-desktop AppKit features (no new registry crate).
- Add bounded UTF-8 file helpers: private sibling saves, no-clobber new targets,
  explicit replacement and ordinary external-edit checks. This is not a file
  sandbox, cross-process transaction or full power-loss durability guarantee.
- Catalog Documents adds real async Open/Save/Save As, failed/cancelled-operation
  draft retention and save-before-close. Current lightweight editor limit is
  8 KiB; no silent truncation. The memory-only lifecycle demo stays separate.
- Catalog adds a counted command witness; host repeat events are tested as inert.
  Discrete native shortcut/menu/button executions and modal suppression were
  accepted; real held-key AppKit repeat/IME/VoiceOver remain separate gates.

- Add bounded state-aware `CommandRegistry`, logical Cmd/Ctrl shortcuts and
  metadata for developer-owned palettes/buttons; unknown/disabled IDs are inert.
- Add `Application::on_close_request` and a separate bounded application-request
  queue. Dirty documents can remain open for Save/Discard/Cancel; confirmed
  requests recheck the guard. No forced OS/Dock quit protection or persistence
  is claimed; the standalone host is still single-window.
- Owned macOS menus/tray reflect registered command enablement; native logical
  accelerators and guarded Quit/Cmd+Q use the same dispatcher/close policy.
- Catalog adds an Application lifecycle page and in-memory draft/command palette.
  UI Lab/older Showcase and Volt's vendored copy are unchanged.
- See `docs/APPLICATION_COMMANDS.md` for APIs, limits and platform boundaries.

## 0.3.0 (2026-10-11; not published to crates.io)

- Application foundation: opt-in `Application`, independent sidebar/rail/bottom
  navigation, local project templates and rebuild/restart tooling, plus optional
  macOS application/tray menus. The full lifecycle and desktop-service roadmap
  remains incomplete; see `docs/APPLICATION_DEVELOPMENT.md`.
- macOS GPU blur now requests a public `NSVisualEffectView` behind the winit
  content view, without changing native controls, the content view parent or
  delegates. Decorated windows on macOS 10.14+ only; unavailable/borderless
  requests explicitly retain transparency without blur. AppKit owns appearance
  and material accessibility behavior. No App Store certification is claimed.
- Correct Vello 0.6 GPU admission: reserve **64**, not 2, command words per
  viewport tile and include 256-word spill allocation chunks. Oversized scenes
  fall back before GPU submission. Native diagnostics now report framebuffer
  uploads for software fallback instead of incorrectly reporting zero.
- Surface retries use bounded event-loop backoff (16–256ms), not immediate
  redraw loops. This does not claim to eliminate upstream driver deadlocks.
- Relay retains at most 16 threads/16 messages per thread; prompts/replies,
  steps/files and rich-text blocks are bounded. Oversized prompts keep their
  draft, stale/cancelled runs cannot mutate newer runs, and file counts saturate.
- Catalog adds a live follow-end log and Enter/Shift+Enter composer demo;
  UI Lab and older showcase designs are unchanged.
- New `relay` example: a coding-agent desktop client with a frosted thread
  sidebar, streaming transcript (collapsible work steps, file-change cards,
  code blocks), composer with permission/model/effort menus, inspector card,
  thread actions popover and settings. A local scripted worker streams replies
  through bounded messages, with stop, retry and stale-run rejection.
- Catalog: frosted full-height sidebar with the content on a solid sheet that
  floats inside the material (inset 8px, rounded 12px), so it never meets the
  window's rounded corners. Blur is the default with `native-gpu`;
  `--background opaque` restores a solid window and software drawing stays
  opaque. The window clears fully transparent and the view paints the tint, so
  a sidebar slider (Catalog) or Settings slider (Relay) adjusts it live, 10–100%.
- Checked switch knobs and checkbox ticks contrast with the accent fill they
  sit on; a white accent (zinc, dark) previously hid the knob.
- Window framebuffers up to 64 MP are accepted (was 16 MP, below a 6K Retina
  display's ~20 MP, which made maximised windows exit). Shadows over the
  software blur budget are skipped (`effects_skipped`) instead of failing the
  frame.
- `.anchor_end(true)` keeps a vertical scroll view at its end while the reader
  is there (chat transcripts, logs); scrolling up releases it.
- `text_area(...).submit_on_enter(true)`: Enter submits and Shift+Enter inserts
  a newline, as chat composers do. Cmd/Ctrl+Enter still submits either way.
- GPU: axis-aligned lines (borders, dividers) are filled as exact rectangles.
  Stroked, a long horizontal border on the surface's top edge filled its whole
  first 16-pixel tile row, hiding a bordered sheet's background there.


## 0.2.0 (unreleased)

- macOS overlay chrome uses public AppKit unified toolbar geometry, giving native
  traffic lights more top/leading clearance. Catalog keeps them over its sidebar's
  drag strip. New `CompactOverlay` preserves the earlier short
  titlebar; standard Native/default chrome is unchanged. Native controls are not
  resized or repainted when switching appearance.

- Catalog data views: working rounded (12px default) and square table previews,
  continuous rounded frames, matching header/last-row focus contours without
  an internal gutter, dividers, column alignment and two-axis overflow;
  tables expose cell/header ancestry, indices and sort direction. Trees use vector
  chevrons, inset selection rows and logical accessible child relationships.

- Catalog is now the primary component demo: named controls, semantic selection,
  mounted accessibility focus/scroll reveal, scoped scroll actions, opt-in window
  alpha/blur, reliable worker completion and launch/state regression tests.
  Rust-only `.accessible_label(...)` and composite roles/states preserve schema1
  and legacy Painter/default dependencies. See `docs/CATALOG.md` for limits.

- Security/performance hardening: validate accessibility targets and values
  before side effects; enforce a hard 96 MiB GPU image registry budget and
  packing admission before uploads; borrow opaque image staging pixels and
  cache outline-ink bounds. Fix miter damage, GPU colour-glyph alpha/italic
  clipping and primary-variant contrast. See `docs/SECURITY_AND_PERFORMANCE.md`
  for measured results, advisory warnings and remaining limitations.

- Components: `ButtonVariant`/`ControlSize` presets, `.disclosure(open)` and
  `.label_align`, token opacity (`ColorToken::alpha`, `"@role/NN"`), readable
  primary labels under inherited colours, 6px default corners, padded panels,
  seamed joined bars, cleaner tables/trees and specific tree errors. New
  `specimen` example shows every default.
- Accessibility: AccessKit screen-reader tree and actions for native windows
  (roles, labels, values, ranges, states, focus; modal scoping; passwords never
  exposed), built on a toolkit-neutral runtime snapshot.
- Software renderer damage tracking: only changed regions are repainted and
  presented; incremental frames match full repaints pixel for pixel.
- GPU: Vello vector text, GPU-resident images, estimates for every Vello fixed
  buffer with per-frame software fallback for detected overflows, device-loss
  recovery. Benchmarks time settled frames. Windows added to CI.
- Document limits raised to 4096 nodes and depth 48.

- Bounded typed native worker messages with coalesced winit wakeups, batched
  UI-thread state updates, reentrant-producer fairness and close/cancel contracts.
- Nested focus reveal, captured draggable scrollbars and opt-in multiline soft
  wrap with shared paint/caret/selection metrics. Physical IME/bidi and OS
  accessibility acceptance remain separate gates.
- Typed in-window menus/context actions, tooltip/popover, progress/loading,
  fixed-row virtualization, sortable tables, expandable trees and split panes.
- Showcase Activity/Explorer are the new polished component acceptance surface;
  legacy Lab routes remain fixtures. GPU Showcase requests a smoke-tinted blurred
  window by default; opaque override, default Volt APIs/dependencies unchanged.
  Document the pinned winit macOS private-blur-API release limitation explicitly.
- Additive `.fill_width()` clears fixed component defaults for parent allocation;
  virtual rows stretch with their list and menu styling reaches the actual trigger.

- Replace the optional native GPU texture presenter with Vello 0.6/wgpu 26 GPU
  scene drawing for opaque/alpha windows: paths, glyph/image textures, nested
  clips, opacity and analytic shadows. Explicit auto/gpu/software policy,
  cached resources, diagnostic boundaries and actual hardware readback tests.
  Prune unused backend features while retaining Rust 1.87 and default embedded
  dependencies. No full CPU framebuffer raster/upload on the native GPU path.

- Rust-only reusable ButtonBar/Toolbar, typed SegmentedControl/TabBar and Tabs
  composites, with controlled selection, active-page-only callbacks, joined
  Full/selected outer corners and disabled-item validation. Additive group-scoped
  Left/Right/Home/End selection preserves ordinary button/legacy JSON behavior.
  UI Lab covers these controls; Atelier navigation now uses the typed tab bar.
- Controlled TextField/TextArea with grapheme editing, bounded undo/redo, selection,
  clipped viewport and IME; lazy native text clipboard, read-only/password protection.
  Typed Slider/Select and controlled modal/popup portals with input isolation and
  focus trapping/restoration. UI Lab exercises them together; Desk has real editing.
  Preserve Northbound contributor app. OS accessibility and mobile remain pending.

- Canonical `ui_lab` app with persistent demo state, section navigation, appearance
  controls, text/color specimens, ellipse/path clipping, horizontal/both-axis/nested
  scroll views, launch flags and headless section snapshots. Future feature demos
  extend this one workbench; focused examples remain regression fixtures.
- Additive `.scrollbars(Scrollbars::Hidden)` indicator policy. Hiding indicators
  leaves scroll input, offsets, clipping and callbacks intact.
- Fix natural scroll sizing through nested automatic containers while preserving
  bounded flex/spacers inside fixed-size panels, and round natural badge widths
  outward to prevent fractional-width last-glyph wrapping/clipping.

Extracted Volt presentation models, layouts, icon paths and paint recipes.

- Glass Browser-inspired optional surface tokens, adaptive tint, generic Panel and rounded/clipped layered pointer routing. Existing Volt visuals remain unchanged.

- Optional schema1 JSON documents, bounded validation, nested constrained layout,
  text/boolean bindings, button focus/capture and transactional explicit reload.
- Optional winit/softbuffer native Solid host with tiny-skia/cosmic-text adapter.
- Progressive native examples plus optional editable JSON files and usage documentation.
- Existing widget/Painter implementations and default dependencies preserved.

- Additive declarative styles: inheritable font family/size/weight/italic and text
  color; independent surface/border colors, widths and radii.
- Stateful Checkbox/Toggle, Badge and Separator; styled workbench/components demos.
- Optimized local dev CPU rendering, native border strokes, cached surface dimensions,
  opt-in stage/input tracing and reproducible CPU benchmark example.

- Primary `rust-ui` authoring: typed component builders/layout macros, direct owned
  Rust callbacks/functions, ordinary if/loops/reusable components, no JSON bridge.
- `native::run_ui`: action-only whole-tree composition and atomic tree/handler
  installation; exact-kind keyed focus, cancelled transient input and rejected-view
  recovery. Existing JSON run and input bool APIs retained.
- Counter/settings/keyed-list Rust examples, design docs and callback compile-fail
  tests. Native generic fonts choose installed sans/serif/monospace families.

- Additive container directional padding (horizontal/vertical/top/right/bottom/left)
  and horizontal/vertical inter-child gaps, with matching optional JSON fields,
  shared asymmetric layout/measurement/hit clipping and bounded validation.

- Correct in-development Rust UI spacing to `.padding(value, Edge)` / `.gap(value, Axis)`;
  unchanged Volt legacy widgets/Painter and JSON scalar fields.
- Numeric/full `.rounded` with layout-resolved radius and shared paint/hit geometry;
  nine `.flow(Flow)` content placements, divider alias and flexible/fixed spacers.
- HStack/VStack function/macro aliases preserve Row/Column behavior; new `rust_layout`
  interactive example and placement/rounding/spacer/compile-fail regressions.

- Typed responsive minimum-width values for Stack direction and container spacing,
  using logical viewport units with named/custom thresholds and bounded validation.
- Stack/ZStack functions/macros: adaptive orientation versus independent ordered
  layers; resize keeps nodes/handlers rather than recomposing view bodies.
- `rust_responsive` example, size-selectable Rust snapshot/benchmark/window CLI,
  threshold/DPI/focus/overlap regressions and responsive design documentation.

- Rust padding/gap accept enum/array/slice/vector selections; independent corner
  radius side/corner groups preserve uniform rounded APIs and shared hit geometry.
- New Rust buttons use 2 logical-pixel corners; legacy Button/JSON stay unchanged.
- Opt-in shadow/glow, subtree group opacity and hover/press/focus appearance;
  keyed interruptible color/border/opacity animation and paint-only enter/exit fade.
- Bounded cached native approximate-Gaussian masks and reusable opacity layers;
  exit ancestor/descendant opacity, sibling order and reinsertion continuity.
- Additive Native/Overlay/Borderless window options and safe header drag regions;
  active-only timers, focus/occlusion settling and explicit reduced-motion policy.
- rust_effects example and native pixel/latency/drag/compatibility regressions;
  no required Painter methods, new default dependencies or Volt vendor edits.

### Semantic surfaces and scrolling

- Fixed glow/shadow overflow through tight ordinary layout parents.
- Added semantic palettes, Light/Dark/System mode, custom palettes/global accents,
  and state-derived theme callbacks without changing legacy options structs.
- Added shared bounded PNG/JPEG image handles, fit/position/opacity, retained vector
  canvas commands, and explicit rounded/circle/path clips with input routing.
- Added opt-in Auto/Vertical/Horizontal/Both scrolling, keyed offsets, wheel/trackpad
  routing, nested edge bubbling, resize clamps and overlay position indicators.
- Added optional `native-gpu` alpha presenter and native blur request with logged
  opaque fallback. Real compositor/device acceptance remains separate.
- Added surface/scroll/transparent examples and regression tests.

### Original app-scale showcases

- Added Volt Atelier Welcome/Workspace/Component Studio screens to the shared UI
  Lab state, with a thin `showcase` launcher, responsive columns, real callbacks,
  live semantic palettes, typography, vectors, clips and horizontal gallery.
- Added original Volt Desk three-pane desktop composition with keyed local
  conversations, fixed sample composer, independently scrolling panes and compact
  launch mode. Still not an AI client; the later input batch adds an editable composer.
- Added installed-font SF/SF Pro and SF Mono aliases in the optional native painter;
  preserve old system/generic font behavior and never bundle Apple fonts.
- Added opt-in wheel-target passthrough for empty decorative overlay layout boxes;
  descendants still receive input, and existing routing defaults remain unchanged.
- Added font/glyph identity, geometry, callback, scroll, DPI and bounded-state
  regressions plus app-scale design and manual acceptance documentation.
- Fixed Atelier's Full corner selector: retain `Rounding::Full` rather than an
  invalid 999px sentinel, so selecting it no longer rejects later UI updates.
  Added all-preset rendering and pointer/keyboard/recomposition regressions.

### Application development first slice (unreleased)

- Additive standalone Application API with explicit renderer/theme/window policy,
  existing typed messages, and GPU-required recovery behavior.
- Independent typed destination navigation in Catalog; no mandatory shell.
- Opt-in cargo-volt local project templates and rebuild/restart development runner.
- Optional macOS application menu/tray menus with validated command delivery;
  default core/native dependency graphs preserved. Modern notifications and
  web/maps/AI/intents remain pending, not unsupported-operation success stubs.
