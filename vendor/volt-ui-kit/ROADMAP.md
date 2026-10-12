# Roadmap

Volt UI Kit is a Rust-first, renderer-independent toolkit with an optional native
host. It is not yet a production-certified cross-platform application framework.
Catalog is the current component acceptance app; older examples are regression
fixtures. See [developer documentation](docs/README.md).

## Delivered foundation

- Extracted Volt presentation/layout/paint APIs, static-dispatch `Painter`, SVG
  gallery and reproducible vendoring. Default dependency: `unicode-width` only.
- Rust declarations, typed callbacks, stable keys and validated whole-view
  recomposition; optional schema-1 JSON frontend with explicit reload.
- Stacks/layers, responsive rules, grids/wrapping/constraints, semantic themes,
  selective corners/spacing/borders, images, vector paths/icons and clipping.
- Editable fields, IME/clipboard/undo foundation, selection/value/date/form
  controls, bars/tabs/navigation, dialogs/disclosure/overlays and display/chart APIs.
- Keyed nested scrolling, focus reveal, fixed-row virtualization, tables, trees
  and pixel/ratio split panes; bounded worker messages and coalesced wakeups.
- Optional Vello/wgpu GPU drawing, vector text, resident images, conservative
  admission with GPU strips and software fallback; incremental software damage.
- Bounded appearance/slide/ambient animations, reduced motion and display pacing.
- Native accessibility tree/actions, macOS unified chrome, public Material and
  explicit experimental private compositor blur.
- Application scaffolding/dev runner, state-aware commands/close veto, native
  macOS menus/tray, file panels, bounded local notifications and local `.app` bundles.

## Next: application and interaction maturity

- Independent keyboard/screen-reader range-slider thumbs; dedicated result
  navigation for comboboxes/command palettes; deeper composite semantics.
- Human VoiceOver/Narrator/Orca, real IME/bidi, trackpad, resize/DPI and other-OS
  native runtime acceptance. Headless checks do not close these gates.
- Resolve and desktop-validate Material -> Compositor square external corners.
  Public Material remains the default; private compositor blur is opt-in.
- Profile native idle/scroll/animation/image churn, total memory/energy and device
  loss. Bound caches and intermediate targets without stale frames or broken clips.
- Fine-grained reconciliation/signals and cached/damaged GPU scenes; current
  callbacks rebuild the whole view and admitted Vello scenes normally redraw fully.
- Multi-window/state restoration/full OS termination, deeper navigation and
  richer notification settings/actions/foreground lifecycle.
- Dependency-stack maintenance without silently raising Rust 1.87, changing
  renderer contracts or altering the embedded/Volt dependency footprint.

## Later, explicitly deferred

- Generic in-app GPU backdrop blur with explicit sources, bounded cached targets,
  saturation/tint/opacity and alpha-correct clipping; native window blur is separate.
- Springs/keyframes/layout animation, touch/gesture scrolling, variable-height
  virtualization, toast timers, cascading submenus and interactive chart axes/tooltips.
- Theme-editor tooling; debugging/inspection and richer reload tooling.
- WebKit/maps/on-device AI/App Intents adapters after desktop-service acceptance.
- Mobile hosts: touch, lifecycle, IME, accessibility and packaging require dedicated
  platform work; portable declarations alone do not provide mobile support.
- A desktop-style panel/dock/launcher demo inside a normal app window. A real
  Wayland compositor/window manager is a separate project, not a demo setting.

## Shipping gates

Keep the [validation](docs/VALIDATION.md) and
[security/performance](docs/SECURITY_AND_PERFORMANCE.md) boundaries explicit.
Publishing/version policy, Developer ID/notarization/installers and other-platform
packaging remain release work. Volt is a separately vendored consumer: canonical
source changes do not imply a vendor sync, terminal performance or visual acceptance.
