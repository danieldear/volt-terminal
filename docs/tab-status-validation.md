# Tab status lights regression

Validated locally on macOS, 2026-10-07, in the `terminal-public` working tree.

## Fix

- Restore the neutral hollow circle for idle custom tabs. Previously the
  renderer intentionally emitted no shape for that state.
- Hide the custom strip with one tab, as requested. It appears at two or
  more tabs. Hidden tab, close and + controls have no invisible hit targets;
  the original top window-chrome spacing remains reserved for native controls.
- Preserve the green busy and red shell-reported failure lights, separately
  from user-selected accents. Successful completion returns to the idle ring.
- The ring uses constant-sample triangle geometry in the existing background
  batch. Its center is genuinely empty, preserving tint and transparency.
  No extra timers, process polling, glyph shaping or GPU pipelines were added
  for the lights. Single-tab rendering skips the entire strip as before.

## Checks

- 363 workspace Rust tests passed, four existing tests ignored.
- Locked Clippy/all targets with warnings denied, Rust 1.87 workspace check,
  rustfmt and whitespace checks passed.
- `cargo run --locked --release -p volt-renderer --example tabcheck`:
  existing accent/long-title fixtures plus 360 GPU status-transition checks.
  Three scales, five themes, active/inactive tinted tabs, two opacity values,
  single-tab suppression/three visible tabs, hollow centers, round shapes, exact status colors and repeat
  frames. Status changes cannot affect pixels outside the light's bounds.
  Native-tab mode remains unaffected.
- Isolated native app with temporary HOME/XDG and Zsh integration: gray ring
  with multiple tabs, green during `sleep`, red after Ctrl+C, gray after `true`,
  red after `false`, retained on the inactive tab after Cmd+T.
- Release bundle rebuilt at `target/macos/Volt.app`. No install, version bump,
  commit, push or release performed; existing unrelated changes were retained.

## Boundaries

Exit-code indicators require shell-reported results. Process-group fallback
can establish busy/idle, but cannot prove success/failure; that safety rule
is unchanged. Native macOS tabs do not use the custom GPU tab lights.
Matched visible-window termbench timing was not repeated for this small fix.

One parallel Settings test initially collided with another test's timestamp-
named temporary file. Its path now includes an atomic counter to ensure
isolation, without changing production Settings persistence.

## Single-tab follow-up

The user clarified that the single-tab strip must stay hidden. GPU checks now
compare all single-tab status states to a strip-free frame and verify the
three-to-one-to-three transition. Native tests confirm Cmd+T shows the strip
and Cmd+W hides it again, while multi-tab dragging remains functional.
