# Advanced keyboard bindings

This engine is separate from the legacy `Chord`, `KeyBinding` and
`Bindings::compile/lookup` APIs, which remain unchanged. It emits typed existing
`Action` values only: it does not execute commands, parse configuration at runtime,
or write terminal input. Volt compiles it at config load and dispatches before ordinary terminal input.

## Example TOML

Add the following section to your Volt configuration. Prefix keys are consumed,
never replayed into the shell. Escape, timeout, focus loss, and IME composition
cancel an unfinished sequence. These are source changes pending a release.

```toml
[advanced_keybindings]
timeout_ms = 1000

[[advanced_keybindings.bindings]]
sequence = ["ctrl+k", "c"]
target = { kind = "action", value = "copy" }

[[advanced_keybindings.bindings]]
sequence = ["ctrl+p"]
target = { kind = "table", value = "pane" }

[[advanced_keybindings.tables]]
name = "pane"

[[advanced_keybindings.tables.bindings]]
sequence = ["h"]
target = { kind = "action", value = "focus_left" }

[[advanced_keybindings.tables.bindings]]
sequence = ["j", "j"]
target = { kind = "action", value = "focus_down" }
```

Validation runs during deserialization AND `AdvancedBindings::compile`.
Constraints: 1–8 chords per sequence, 256 bindings total, 16 tables,
1–60000 ms timeout, nonempty tables and unique case-sensitive ASCII names
(1–32 letters/digits/underscore/hyphen). Table references must exist. Duplicate
sequences and prefix ambiguity are rejected within each scope, in either order.
Sibling sequences with a shared incomplete prefix are valid. Escape is reserved,
including modified Escape. Unknown fields/actions/targets fail validation.
Modifiers match exactly: ctrl=1, alt=2, shift=4, super=8; unknown bits do not match.
Table-to-table references may cycle safely because activation replaces state;
there is no recursion or table stack.

## Host contract

Volt's host integration uses `volt_config::keybindings` and the internal
`volt_ui::keybindings` module. Configuration loading compiles the engine, and
the app dispatches eligible physical key presses before ordinary terminal input.
The UI module remains private; external crate consumers would need a public
module or re-export. The contract below describes the engine's integration API.

1. Compile on load with `AdvancedBindings::compile(&config)?`; construct
   `KeyboardDispatcher::new(compiled)`. Validate a replacement before calling
   `replace_bindings`; failed reloads can retain the old engine. Replacement resets
   any in-progress sequence/table.
2. Feed **physical key presses** to
   `dispatch(key, modifier_bits, event.repeat, Instant::now(), context)`.
   Release events must never advance this engine. Track/suppress releases and
   repeats for consumed presses in the host, especially after an action changes
   focus. Modifier-only events should update host modifier state, not be fed as
   sequence steps. Handle IME/composition using existing text-input paths.
3. `DispatchContext { focus_epoch, eligible }`: use an epoch that changes for
   pane/window/focus-owner changes. Set eligibility false for modal dialogs,
   text editing, IME composition, or window focus loss. Call `set_context`
   immediately on those transitions (including transitions with no key event).
   The next dispatch also checks context defensively. Context cancellation does
   not consume a new owner's key or replay the old prefix.
4. **Only `PassThrough`** may run legacy bindings/default shortcuts/PTY input.
   `Pending`, `Action`, `Consumed`, and `Cancelled` consume the current press.
   `Action::Unbind` is a consumed terminal leaf in this engine; it must not fall
   through. `Ignore` likewise consumes without host action. The host owns all
   actual action execution.
5. `deadline()` is O(1) and returns `None` while idle. Schedule the event loop's
   next wake only when present, composing it with other host deadlines. Call
   `expire(now)` at that wake; true means the prefix/table was discarded. No
   periodic idle polling is needed. Check again after each dispatch/context/reset.
6. `reset()` on app suspension or any other context invalidation not represented
   by the epoch. A dispatcher should be per independent keyboard focus stream.

## State and leakage policy

```text
                    prefix or table activation
 IDLE  ------------------------------------------------> PENDING
  |                                                        |
  | root miss: PassThrough                                 | valid continuation
  |                                                        v
  +<--- Action (consumed) <--- completed action <------ PENDING
  +<--- Cancelled (consumed) <--- mismatch / Escape / timeout
  +<--- reset / focus or modal change / replacement / expire
```

Tables are **exclusive and one-shot**. There is no fallback to root while a table
is active. An action leaves the table; entering another table replaces it.
Each accepted prefix/table transition renews the timeout. The exact deadline
is expired (`now >= deadline`). Repeated events never advance or renew pending
state; root single-step actions may repeat, while repeat events cannot start a
sequence/table. Escape takes precedence over repeat suppression while pending.

A mismatch consumes BOTH the accumulated prefix and the mismatching key; it is
not retried against the root and nothing is replayed to the PTY. Unidentified
physical keys also mismatch pending state. If a press discovers an expired
pending state before the timer was serviced, that press is consumed as
`Cancelled(Timeout)`. If the timer already expired the state via `expire`, later
presses start normally at the root. This deliberate strict policy prevents
accidental input leakage and is not a replay buffer.

Compilation creates bounded tries with exact `(KeyCode, u8)` edges. Dispatch does
one hash lookup per press (expected O(1)), allocates no memory and parses no strings.
Idle timeout/context/reset operations are O(1). Existing legacy lookup behavior
is unchanged. Unit tests cover validation, modifier matching, mismatches,
exclusive tables, timeout boundaries, Escape, reset/reload, focus/modal changes,
repeat suppression, and unidentified keys. Config and app wiring is present in
source; native keyboard/IME behavior still requires desktop validation.
