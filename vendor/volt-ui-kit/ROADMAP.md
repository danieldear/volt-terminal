# Roadmap

## Delivered: toolkit extraction

- Terminal-independent presentation/layout models and vector paint recipes.
- Generic button shared with Volt; application-specific widget compatibility.
- Existing-host renderer adapter and static SVG backend.
- Component gallery, unit tests and reproducible vendoring.
- No additional GPU device, event loop or periodic redraw timer in Volt.
- Glass-inspired generic Panel, smoked/frosted tokens, host-fed tint hysteresis.
- Rounded/clipped topmost hit map with explicit background blockers.

## Next: polish and generalization (high priority)

- Replace compatibility-specific labels/action IDs with application-supplied metadata.
- General-purpose Input, Checkbox, Tabs and selectable List APIs (Panel delivered).
- Measured typography, clipping, long text, narrow-window and DPI improvements.
- Shared keyboard focus model, pointer capture and predictable disabled/hover states.
- Validate each change in the gallery and Volt; retain screenshot and performance gates.

## Before calling this a complete framework (required)

- Native standalone winit/wgpu host with text shaping, IME, clipboard and undo/redo.
- Accessibility semantic tree, actions and platform adapters.
- Nested layout, constraints, scroll containers and virtualization.
- Rich Unicode/multiline text editing and input method composition.
- Platform-specific menus/dialogs/window lifecycle and multi-window validation.
- Stable API, documentation, packaging, published crate and compatibility policy.
- macOS/Linux/Windows runtime and assistive-technology validation.

## Later possibilities

- Animation with reduced-motion support and explicit scheduling.
- Optional GPU glass compositor: cached downsampled/separable blur, host-supplied backdrop, consistent color space, reduced-transparency fallback; no blur work for opaque widgets.
- Additional renderer backends without changing widget APIs.
- Rich tables, tree views and inspector-oriented controls.
- Theme editor as a generic color/palette component.

No claim of automatic accessibility, full cross-platform framework readiness,
or terminal benchmark parity is made by the initial extraction.
