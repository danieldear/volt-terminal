# Conservative macOS output accessibility bridge

The bridge lives in `volt-ui/src/native_text.rs` and the pure
`volt-ui/src/accessibility.rs` snapshot module. Parent integration in `app.rs`
includes the AX render publication call, AX helpers/tests, cache invalidation
at output/input/scope/focus/manual-secure boundaries, and the requested Unix
clipboard sensitivity cfg correction. Other lifecycle features remain separately owned.
The snapshot module is nested under `native_text` using `#[path]`; no `lib.rs`
registration is required.

## Parent integration contract

```text
Parent resolves active pane + displayed viewport + privacy + modal scope
    |
    +-- unknown / noecho / manualsecure ------> redacted snapshot
    +-- known safe terminal -----------------> bounded displayed output
    +-- modal -------------------------------> named safe modal summary only
    |
    v
NativeTextInput::publish_accessibility(snapshot)
    |
    v
Per-view Context cache --> AX read-only getters (no PTY / Performer access)
```

On the main thread, call `invalidate_accessibility()` **before** changing the
active tab/pane, screen/viewport, modal scope, privacy state, or any identity
that makes the old snapshot stale. Publish a replacement only after resolving
all these facts. Installation starts redacted. There is no automatic expiry or
PTY echo detection: the parent is responsible for invalidation and current
privacy knowledge. Do not infer safety just because a secure flag is absent.
If multiple sensitivity signals exist, unknown/noecho/manualsecure dominates;
only positively established safety permits `Sensitivity::KnownSafe`.

## Lazy demand and publication

`NativeTextInput::accessibility_requested()` is an allocation-free two-second
TTL hint based on the last cached AX getter request (`Instant`, per view).
It starts false, is true strictly before two seconds have elapsed, and becomes
false at expiry. Getters record requests even if a contended cache fails closed;
role/element/action-list discovery also records interest. Getters return a cached snapshot immediately; first/expired queries clear the
old cache and return redacted until the next safe publication. They never build text.

Parent integration should check privacy/scope transitions unconditionally,
then check demand **before** formatting or copying displayed text:

```text
privacy / scope transition? --> invalidate immediately (regardless of TTL)
next loop/frame:
    requested == false --> skip AX text building, allocation, history scanning
    requested == true  --> resolve safe active viewport + build bounded snapshot
                       --> publish if ready
```

Publication compares the complete bounded snapshot and posts
`NSAccessibilityValueChangedNotification` once only when it changed. Identical
publishes are silent. The cache borrow is released before AppKit is called;
synchronous reentrant publications update the cache but coalesce into the
in-flight notification instead of recursively posting. Invalidation uses the
same changed-only path, independent of TTL, so unsafe old text is cleared even
without recent demand. Neither publication nor invalidation records a client
query; an actual getter triggered by AppKit during notification does.

**TTL is a demand heuristic, not client-presence detection.** A client can
subscribe to notifications and stop querying for more than two seconds. Such
a client may stop receiving live output updates until it queries again. Expired demand silently clears cached text on parent demand checks and on the
first renewed getter, with a freshness flag that also denies contended/reentrant
reads. Privacy/scope invalidation is still mandatory. For guaranteed continuous announcement, alternatives are an explicit
user-enabled accessibility/live-output mode, or a sticky per-view interest latch
reset on a defined session boundary. Both intentionally retain more update work
than this TTL policy. Do not use this hint to gate privacy enforcement.

The first query after installation or TTL expiry requests a redraw through a
weak reference to the owning window. It returns the old cache immediately;
the next render publishes the replacement. Renewed queries inside the TTL do
not repeatedly request redraws. No PTY, text construction, or history traversal
occurs in native getters.

### Public-repo render integration

`MainState::publish_accessibility_for_render` runs after rendering and reads only
the already displayed composite grid, cropping to the active pane rectangle.
Text extraction stops at the UTF-16/row limits before snapshot construction.
Inactive panes, tabs, and offscreen retained history are never serialized for AX.
Demand is checked before pane layout, cell extraction, or string allocation.
Privacy clearing is independent of demand.

**Strict echo gate:** publication uses the core owner's `Pty::echo_enabled()`
accessor (actual `ECHO`, including raw-mode no-echo). The demand check comes
before the syscall; no interest means no echo query or AX text allocation.
Modal summaries skip PTY inspection entirely. Unknown echo, actual no-echo,
manual-secure, OS secure-input, and read-only inspection panes redact. Hidden
or unfocused windows redact too. Native callbacks never inspect a PTY.

Cache invalidation also runs before input/focus/scope events, menu/settings/
native-input/quick-terminal actions, visibility hiding, manual-secure changes,
full semantic redraw requests, PTY wakes and displayed output updates. Idle
parent checks clear expired demand without posting a self-renewing notification.
A renewed getter cannot resurrect the previous cached value before the next
render. Echo is sampled asynchronously on interested frames: a process-only
termios transition with no delivered app event cannot be detected synchronously
by a cached getter. That limitation must be exercised in native acceptance;
these getters are not an atomic oracle for live PTY privacy.

Safe modal summaries are integrated: settings, theme editor, task form, prompts,
workspace search, and focused workspace panel publish only static public names
and constant summaries. User-entered values, paths, commands, and background
terminal output are excluded. Full modal controls remain unimplemented.
These invalidation hooks are deliberately conservative; full actionable-control
lifecycle behavior remains a separate follow-up.

Use `native_text::accessibility::{AccessibilitySnapshot, Sensitivity, TextRange}`:

- `AccessibilitySnapshot::terminal_output(displayed_text, sensitivity, selection)`
  accepts only active displayed output, not hidden panes or offscreen history.
- `AccessibilitySnapshot::modal_summary(safe_name, safe_summary)` replaces the
  entire terminal snapshot with explicitly public read-only modal text. Never
  append background terminal output, typed secrets, or sensitive form values.
  Empty/whitespace names fail closed. A modal's safe summary is independent of
  background PTY echo state; this constructor is an explicit parent trust boundary.
- Selection is optional and snapshot-relative **UTF-16**, not grid columns or
  retained-history coordinates. Offscreen, stale, overflowed, or surrogate-split
  ranges are rejected, not clipped. Rectangular/discontinuous selection should
  be omitted unless it can truthfully be represented as one contiguous range.

Output is a prefix capped at 32,768 UTF-16 units and 256 newline-delimited rows;
truncation never splits a Unicode scalar. Labels are separately capped at 256
UTF-16 units and one row. Newlines belong to the preceding line; a trailing
newline introduces an empty final row. EOF resolves to the last row. Empty
output has no line. Invalid substrings return nil; absent selection/invalid
line ranges use NSNotFound. No selection returns empty selected text. Visible
range is exactly the published string, not the entire scrollback.

The native role is `AXStaticText`, not a terminal text editor. AX edit/action
selectors are denied; no editing setters or command handlers are installed.
Existing NSTextInputClient/IME behavior, chrome hit-tests, original-class Drop
restoration, and the later-subclass guard are unchanged. No ivars are added.
The implementation does not claim full NSAccessibilityNavigableStaticText
protocol conformance: range geometry is not implemented in this slice.

## Remaining work

- Native runtime acceptance of asynchronous echo/privacy/scope transitions and
  TTL renewal. Strict echo-state publication is now wired.
- Selection/layout-specific AX notifications and any further rate-limit policy
  beyond changed-only value notifications.
- Range geometry, navigation enhancements, and actual VoiceOver/Accessibility
  Inspector acceptance on macOS (compilation and pure tests are not acceptance).
- Full controls/action AX: tabs, panes, split controls, menus, buttons, modal
  focus containment, modal form controls, focus restoration, and semantic actions.
  A read-only modal summary is **not** an accessible interactive modal UI.
- Cross-platform accessibility is not provided by this macOS-only bridge.

Selectors were checked against the installed AppKit SDK and Apple's
[static text documentation](https://developer.apple.com/documentation/appkit/nsaccessibilitystatictext)
and [navigable static text documentation](https://developer.apple.com/documentation/appkit/nsaccessibilitynavigablestatictext).

Pure tests can run without AppKit or the full workspace:

```sh
rustc --edition 2021 --test volt-ui/src/accessibility.rs -o /tmp/volt-ax-tests
/tmp/volt-ax-tests
```

## Synthetic native ABI smoke example

`volt-ui/examples/native_accessibility_check.rs` installs the real bridge on
hidden winit views without a shell, PTY, or user content. It dispatches actual
Objective-C selectors for role/label/value, UTF-16 counts/selection/ranges/lines,
surrogate/overflow rejection, redaction, safe modal replacement, expiry renewal,
and both original-class restoration and the later-subclass guard.

```sh
cargo run --locked -p volt-ui --example native_accessibility_check
```

The parent runs this native smoke when ready. Building it is not a runtime pass.
Full controls/actions, VoiceOver, AX-client-observed notifications, and modal
focus/range geometry are still follow-ups.
