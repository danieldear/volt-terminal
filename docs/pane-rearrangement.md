# Live pane rearrangement

These are **within one tab**, stable-pane-ID, topology-only operations. IDs from
another tab are not valid targets even if the numeric values happen to match;
the caller must select the correct tab/tree first.

## Public APIs

`PaneTree` exposes:

- `swap_panes(first_id, second_id)`
- `swap_active_pane(target_id)`
- `move_pane(source_id, target_id, direction, insert_before)`
- `move_active_pane(target_id, direction, insert_before)`

`TerminalTab` forwards the two active-pane methods. All return `RearrangeResult`:

- `Changed`: topology changed; recompute layout and request redraw.
- `Unchanged`: same valid source and target; no mutation or resize needed.
- `NotFound { pane_id }`: stale/invalid pane ID (including divider IDs); no mutation.
  Source validation precedes target validation. Same nonexistent IDs are invalid,
  not a successful no-op.

For move, `Vertical` means side-by-side and `Horizontal` means stacked.
`insert_before = true` places the source left/above; false places it right/below.
Swap exchanges **complete leaves including IDs**, not process contents between
fixed ID slots. Focus and selection therefore continue to identify the same shell.

## Topology

Move detaches only the source leaf, collapses its former parent, then splits the
destination leaf at ratio 0.5. The collapsed divider ID is reused for the new
split; all other divider IDs, directions and ratios remain intact. Pane count and
`next_id` remain unchanged. A sibling-to-sibling move is supported, as are moves
between differently nested branches. Root/parent ownership is reconstructed in
safe Rust: no PTY spawn, clone, close, sentinel terminal, or unsafe pointer move.

Example: move pane A below C (horizontal, insert_before=false):

```text
Before                         After
Vertical #10                   Vertical #10
+-- Horizontal #11             +-- B
|   +-- A                      +-- Horizontal #12
|   +-- B                          +-- Horizontal #11 (ratio 0.5)
+-- Horizontal #12                 |   +-- C
    +-- C                         |   +-- A
    +-- D                         +-- D
```

Subtree-aware layout clamps ensure every child has its full minimum size,
including nested dividers. Divider geometry and nested drag spans use the same
clamps. If the whole tree cannot fit, the existing active-only fallback applies.

## Integration boundary

- Configure `move_pane_left`, `move_pane_right`, `move_pane_up`, or
  `move_pane_down` in Settings → Shortcuts or `config.toml`. The app swaps the
  active pane with its spatial neighbor, retaining the same shell/PTY and focus.
  No shortcut is assigned by default; these do not interfere with terminal keys.
  Mouse dragging between panes and inter-tab transfers are not included.
- The topology primitives do not alter the renderer or spawn processes.
- Whole `TerminalPane` objects retain their PTY, performer/grid, receiver, titles,
  cwd, command/task state, scroll offset, and read-only setting. No terminal grid
  or PTY is resized by these primitives.
- `active_id` never changes on rearrangement, including generic moves of an
  inactive source. Tab title override and accent are untouched.
- Selection remains keyed by the original pane ID. The caller should retain it
  and recompute screen coordinates rather than attaching it to the old rectangle.
- There is no pane zoom field in the current `TerminalTab`/`PaneTree`. Any parent
  integration adding ID-based zoom should keep the zoomed ID unchanged: every
  pane survives. Avoid resizing hidden panes to zero or unzooming implicitly.
- On `Changed`, use the ordinary layout/PTY resize pipeline for the new rectangles;
  apply the existing zoom policy if any. Clear/rebase cached divider drag geometry:
  a moved divider ID survives but now identifies a different location.
- Existing `remove` behavior is unchanged: closing the active leaf focuses a
  survivor; closing an inactive leaf preserves focus; stale closes are harmless;
  the last leaf reports `RemovedLastLeaf` so the caller closes the tab.

## Tests

`PaneTree<P = TerminalPane>` / `PaneNode<P = TerminalPane>` allow pure tests to
exercise the production algorithms with non-Clone owned payloads. Tests cover all
four-pane source/target pairs, both axes/orders, sibling/cross-root moves, exact
swap rectangles and involution, ratio/divider preservation, active focus,
invalid/divider/same IDs, payload pointer identity and drop counts, close-after-
move semantics, single/two-leaf trees, grid/selection/title retention, and all
small window dimensions. No new test spawns a PTY or requires a window event loop.
