# Developer documentation

Start with [Rust-first UI](RUST_UI.md), [application development](APPLICATION_DEVELOPMENT.md),
and the [Catalog](CATALOG.md). Historical implementation plans, agent handoffs and
per-session review/verification notes are not part of this documentation set;
their history remains available in Git.

## Architecture and authoring

- [Architecture and Volt compatibility](ARCHITECTURE.md)
- [Rust declarations, state, callbacks and keys](RUST_UI.md)
- [Reusable components and layout utilities](COMPONENTS.md)
- [Menus, overlays, data views and panes](EXTENDED_COMPONENTS.md)
- [Responsive and layered layouts](RESPONSIVE_UI.md)
- [Styling and optional JSON styles](STYLING.md)
- [Optional schema-1 JSON frontend](DECLARATIVE_UI.md)

## Surfaces and rendering

- [Themes, images, clipping, scrolling and window glass](SURFACES_THEMES_AND_SCROLLING.md)
- [Effects, motion and window chrome](EFFECTS_AND_WINDOWING.md)
- [GPU rendering, admission and dependency policy](GPU_RENDERING.md)
- [Performance measurement](PERFORMANCE.md)

## Applications and platform services

- [Project creation and application configuration](APPLICATION_DEVELOPMENT.md)
- [Bounded background messages](APP_RUNTIME.md)
- [Application commands and guarded closing](APPLICATION_COMMANDS.md)
- [Document files and native file panels](DOCUMENT_FILES.md)
- [macOS tray and local notifications](DESKTOP_NOTIFICATIONS.md)
- [macOS application packaging](MACOS_PACKAGING.md)

## Examples and contribution

- [Catalog: current component acceptance app](CATALOG.md)
- [Existing app-scale showcases](SHOWCASE_APPS.md)
- [Runnable examples and CLI flags](../examples/README.md)
- [Security and resource boundaries](SECURITY_AND_PERFORMANCE.md)
- [Validation and platform acceptance](VALIDATION.md)
- [Roadmap](../ROADMAP.md)

**Implemented** means an API/source exists. **Runtime accepted** requires device
and host evidence. Compilation, readback tests and screenshots are not substitutes
for every platform, assistive-technology or desktop-compositor acceptance gate.
