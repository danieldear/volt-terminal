# Native Emoji & Symbols input (macOS)

## Scope

The system picker sends text through AppKit, not necessarily through a physical
key. Volt previously consumed only `KeyboardInput`. Also, the pinned winit 0.30.13
implementation only emits `Ime::Commit` from `insertText:replacementRange:` when
there is marked text. Direct, unmarked text can therefore be lost.

`volt-ui/src/native_text.rs` installs a no-ivar subclass on each Volt NSView. It
forwards out-of-band, unmarked native insertions to the originating window's
`VoltEvent::NativeText`. Both NSString and NSAttributedString inputs are copied
as complete UTF-8. The legacy single-argument selector is supported as well.

Keyboard-triggered `insertText` and marked composition stay with winit. Volt does
not globally swizzle AppKit, change IME enablement, read the clipboard, or turn
text insertion into a shell command. The owning guard retains the window and
restores its original class on teardown. No processing is added to PTY output or
rendering loops, and event-loop proxies are cloned only during installation.

Committed text goes to the active search/Find/rename field or writable terminal
pane. A focused non-editable inspector or read-only terminal does not leak it to
the PTY. The `Ime::Commit` event is also routed through the same handler.

This is not a full IME/preedit implementation or an emoji rendering/font change.

## Automated validation

Run from the repository root:

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo run --locked -p volt-ui --example native_input_check
```

The native smoke check opens three invisible winit windows (no shell or PTY),
dispatches 30 real Objective-C insertions through both selectors, releases the
source strings before processing the events, checks the target window and exact
text, and verifies class restoration. It also dispatches six ordinary native
keyboard events with IME both enabled and disabled and checks that they are not
duplicated as native commits. Samples include faces, modifiers, ZWJ sequences,
flags, and explicit emoji presentation.

Unit tests check routing/read-only guards and string conversion, including
embedded NUL preservation. The workspace suite passed 178 tests (3 existing
ignored benchmarks); strict Clippy passed.

## Visual acceptance still required

Native callback dispatch is verified; an actual click in the system picker is
**not yet verified**. The computer-use tool returned a zero-sized/invisible
CharacterPalette popup and selection attempts produced no native insertion
callback. This is not evidence that a real user's picker selection succeeds.

Build a separate app with `bash scripts/macos/build_app.sh --release`, or open
the prepared `target/inspector-preview/VoltEmojiReady.app` (not installed over the
user's app). Check:

1. At a shell prompt, Edit > Emoji & Symbols > select an emoji. It should appear
   once; ordinary typing, Backspace and Enter should continue to work.
2. Repeat in `nvim` insert mode and in Volt's Find/search text fields.
3. A read-only terminal pane should reject the insertion.
4. Repeat after closing/opening windows and switching tabs/panes.

Do not confuse this acceptance check with the independent color-font/VS16
rendering issue, which is deferred.
