# macOS custom-tab dragging regression

Validated locally on macOS, 2026-10-07, in `terminal-public`.

## Cause and fix

Full-size content was drawn **behind** the transparent native titlebar.
AppKit hit-tested that titlebar first, so clicking or dragging most of a
custom tab never reached Volt's existing tab-drag handler. Clicks near the
bottom edge of the tab did reach it; this was reproduced before the fix.

The initial fix raised the Metal content view above the titlebar. Although
hit-tests passed, this could cover/dim the native traffic lights: passing
mouse input through does not also make a GPU layer visually transparent.
That drawing-order change has been removed.

Custom-tab mode now installs a nonopaque, non-drawing **public NSView** input
router above the existing siblings. Original content/titlebar ordering stays
unchanged. Only custom chrome hits outside the native left reserve are sent
to the content view; real native controls elsewhere are also preserved.
`mouseDownCanMoveWindow` is disabled for custom content: Volt explicitly starts
native window dragging only for empty chrome. Native-tab mode installs no
router and retains its original view order and drag policy.

Do not subclass AppKit's private frame to override its hit-test: this was
rejected during runtime validation because it broke native title updates on
the tested macOS release. The router subclasses only public NSView, follows
parent resizing, and is removed/released before the retained Window dies.

No ivars or global method swizzles were introduced; the retained-window guard
and subclass restoration are unchanged. No PTY/parser/render-loop work,
polling or animation timer was added. Reordering still moves whole tabs and
preserves the active tab, its panes, colors and terminal state.

## Verified

- 363 workspace Rust tests passed; four existing tests ignored.
- Locked Clippy/all targets with warnings denied and Rust 1.87 check passed.
- `native_input_check`: real AppKit tab-center hit routing; all three native
  buttons remain visible, alpha 1, and receive their own hit-tests; every
  original sibling keeps its position; frame class is unchanged; changing
  window titles still works; router is removed on drop. Native mode unchanged.
  Thirty out-of-band plain/attributed insertions, six nonduplicated keyboard
  events, three window identities and content-class restoration still pass.
- The existing 360 GPU tab-status checks and long-title/accent fixtures passed.
- Isolated visible app after this correction: upper tab-center clicks, normal
  left-button drags in both directions, active-output preservation, 2→1→2 tab
  visibility, empty-chrome double-click zoom/restore, and reordering after
  resize. Native fullscreen button worked; reordering also worked fullscreen;
  View → Toggle Full Screen restored the window. No modifier is needed.
- The screen-capture system badge covers the traffic-light area during CUA
  screenshots. Therefore button layering/visibility/alpha are AppKit assertions,
  not a claim that screenshot pixels verified all three colors. An ordinary
  non-captured window remains the final visual check for native button colors.
- Release-mode app bundle rebuilt at `target/macos/Volt.app`.

The user's installed/running Volt and personal configuration were not changed.
No commit, version bump, push or release was requested or performed. Whole
termbench throughput and multi-monitor behavior were not retested for this
input-routing fix. The router is only consulted by AppKit hit-testing; no
PTY parsing, rendering timer or GPU drawing path was added.
