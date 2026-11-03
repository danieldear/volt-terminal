# Product-readiness implementation batch

Source work against released **v0.1.16**, 2026-10-14. This is not a release
announcement or a claim that every native/platform acceptance gate passed.
The terminal's existing renderer, PTYs, custom tabs, task execution guards and
user configuration remain in place; the canonical UI Kit is not modified.

| Priority | Capability | Implementation / safety boundary | Acceptance still required |
| --- | --- | --- | --- |
| P0 | Unsafe-paste review | Multiline unbracketed input, control bytes, invalid UTF-8 and bidi controls require review by default. Cancel is initially focused. Approval is single-use, expires and is pane-bound. | Real shell, bracketed editor, password prompt and focus-switch checks. |
| P0 | Clipboard permissions | OSC 52 writes: Ask (default), Deny or explicit Allow. No clipboard reads or query responses. Bounded, UTF-8 writes; focused-pane/privacy checks. | Native copy/paste and request dialog smoke tests. |
| P1 | Keyboard workflows | Validated multi-key sequences and one-shot key tables; prefixes are never replayed into the shell. | Physical-key/layout, IME, modal and repeat tests on the desktop. |
| P1 | Pane movement | Configurable directional actions swap whole neighboring pane payloads, retaining live PTYs and process state. | Run a job in each pane and verify output/focus/resize after moves. Inter-tab transfer is not included. |
| P1 | Accessibility | Conservative, demand-gated macOS read-only output snapshots, UTF-16 ranges and redaction. No hidden-pane or background-modal output. | VoiceOver acceptance; virtual actionable controls for Settings, tasks, search and workspace remain a distinct follow-up. This is not full accessibility. |
| P1 | IME compatibility | Preserve existing native composition/emoji routing; unfinished key sequences cancel during composition. | Japanese/Chinese/Korean IMEs, dead keys, emoji and marked-text focus transitions. |
| P2 | Linux integration | On-demand Wayland/X11 text clipboard backend plus a user-level desktop installer and real app icon. | Actual Wayland/X11 compositors, selection ownership, packaged launcher and desktop scaling. No Linux runtime acceptance is claimed on macOS. |
| P3 | Inline images | Opt-in, bounded in-memory OSC 1337 PNG subset with GPU clipping/cache. No filesystem paths, network fetches or animation. | Real terminal scroll/reset/alternate-screen/resize acceptance. This is not Kitty/sixel or complete imgcat compatibility. |
| P3 | Quick terminal | Opt-in macOS global shortcut and persistent hide/show window using existing terminal UI. | Shortcut conflicts, Secure Event Input, monitor/Space/fullscreen and background activation. Linux global shortcuts remain unimplemented. |

## Performance gates

- Idle advanced-key dispatch has no timer; only an armed sequence adds a deadline.
- Clipboard work is explicit, not a watcher; no clipboard payloads are logged.
- Accessibility snapshot formatting is only requested for recent AX interest.
- Empty image state adds no image decoding, uploads or per-cell rendering work.
- Parser and offscreen GPU comparisons are **not** whole-app termbench or latency
  measurements. Visible-window comparisons must use the same grid, font, line
  height, backing scale, scrollback and card state, without overlapping builds.
- Keep per-workload results. Do not hide a regression behind an aggregate win.

## Local testing

Use the existing app build helper, not a new development app or a second
installed bundle:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
RUST_MIN_STACK=33554432 cargo test --locked --workspace
python3 -m unittest discover -s scripts/validation -p 'test_*.py'
scripts/macos/build_app.sh --release --install --open
```

The helper refuses to replace a running installed app. Close Volt deliberately
only after its jobs are safe. Changes to source do not update an already running
app or previously downloaded release.
