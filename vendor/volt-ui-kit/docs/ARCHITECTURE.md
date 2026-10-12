# Architecture and compatibility contract

```text
Legacy host -> existing models/layout/paint -----------+
                                                      |
Optional JSON -> parser -> validated shared tree         |
Rust view + builders --> validated shared tree         |
                             |                        |
                 layout/text/focus/semantics           |
                             |                        |
                       component recipes -------------+
                                                      v
                                               Painter adapter
                                             /       |       \
                                          Volt      SVG    future GPU
```

Rust-first builders/direct callbacks and optional JSON both lower to the validated
shared layout/input runtime behind optional features. `run_ui` composes initially
and after actual callbacks; hover/resize/redraw do not run a view body. Successful
installs replace tree/handlers together and preserve exact-kind keyed focus;
invalid views keep prior UI but cannot roll back application state. See
[Rust UI contracts](RUST_UI.md). Broader editing controls, fine-grained signals
and platform accessibility remain future work. Legacy callers need not create a tree.

Responsive layout rules resolve once per invalidated layout from physical width /
scale. The resolved main axis, padding and gap share the node's property snapshot
used by intrinsic measurement and placement, so hit and paint boxes agree.
Stack direction does not change component identity; fixed Row/Column aliases
retain defaults. ZStack measures per-axis maxima and places each subtree in the
same padded content area, preserving tree paint/reverse interactive-hit order.
See [responsive semantics](RESPONSIVE_UI.md); there are no container-query cycles
or structural tree rebuilds on resize in this first slice.

## Appearance and window integration

```text
Rust tree + stable keys --> validated appearance / layout
         |                              |
         |                              v
         +--> direct callbacks     keyed motion tracks
                 |                      |
                 v                      v
          compose after action   sample on absolute clock
                                        |
                           paint commands + subtree layers
                                        |
                           native raster / host presentation
```

Motion is presentation state, not application state: ticks never invoke callbacks,
the view body or layout. Appearance tweens can be interrupted; removed fade nodes
are paint-only snapshots and cannot receive input. Exits retain tree paint order
and ancestor opacity. Corner geometry is shared by fill, border and hit testing.
The native adapter caches bounded shadow coverage and RGB tints separately from
animated alpha, and reuses transparent group buffers. See
[effects contracts and budgets](EFFECTS_AND_WINDOWING.md).

Legacy `NativeOptions` and run entry points keep their defaults. Additive window
options opt into integrated macOS chrome or borderless mode; declared header
dragging always yields to interactive controls. Native motion uses active-only
timer pacing, then returns to idle `Wait`; this is not display synchronization.
Reduced motion is explicit, not automatic OS accessibility detection. Live
platform window/input acceptance remains separate from headless raster tests.

## Ownership

- Application: domain state, persistence, command execution, async tasks.
- Toolkit: presentation, typed events, validated connections and component semantics.
- Host: window/event loop, input, clipboard/IME, GPU/text resources, platform services.
- Documents: data only. No imports of arbitrary files, eval, shell, or scripting.

## Compatibility

Do not remove exports, alter existing fields, add required Painter methods, or
change legacy geometry/text/material defaults. New APIs live behind explicit
features/modules. Default builds add no dependency, timer, device, event loop,
or initialization. Callback boxing belongs only to the opted-in registry; Painter
remains statically dispatched. Heavy adapters may eventually live in separate
crates, but no duplicate canonical toolkit source should be created.

Volt stays untouched until integration is authorized. Canonical changes make the
vendored snapshot stale by design until explicit synchronization. Local toolkit
tests alone cannot establish Volt compatibility or benchmark parity.

## Visual and performance contract

Theme controls semantic colors/typography/spacing; material controls Solid/Glass.
Both materials share layout and hit geometry. Cache measured text and geometry
against content, style, constraints and scale. Backdrop effects additionally key
against changed backdrop and effect parameters. No opaque-path blur work.
Reduced transparency selects opaque surfaces; reduced motion prevents animations.
No periodic idle redraw. Host queues expensive work and explicitly invalidates UI.
