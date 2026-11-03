# Bounded, on-demand clipboard backend

`volt-ui/src/clipboard.rs` provides synchronous `read(max_bytes) ->
std::io::Result<Vec<u8>>` and `write(bytes, max_bytes) -> std::io::Result<()>`.
The app host dispatches these calls from a worker, never the UI event loop.
Each window allows one operation in flight, with no queued retries. Explicit
copy actions and policy-authorized OSC52 requests snapshot their bytes; reads
retain the window/pane/PID and exact UI surface ownership fingerprint. Input,
focus loss, native menu actions, and modal/tab changes invalidate delivery,
even if the user switches back before completion. Privacy is checked again
before delivery: sensitivity must match the read-start snapshot. Explicit
single-line, control-free password paste is permitted in an unchanged protected
terminal; unsafe protected paste is blocked with a generic alert, never a
preview, regardless of the unsafe-paste confirmation setting. Read-only panes
still reject terminal paste, and protected terminals cannot issue OSC52 writes.
Closing the window discards results. Success clears only the
original unchanged terminal selection; failure preserves it. App payloads and
selection extraction are capped at 8 MiB; debug events redact clipboard bytes.
No dependency changes are needed:
`libc` is already a Unix dependency. Other operating systems return Unsupported.

## Runtime prerequisites and selection

Install utilities through your distribution's package manager, and launch Volt
inside the desktop session, with trusted executable search paths:

- **Wayland:** `wl-clipboard` provides `wl-paste`/`wl-copy`. A nonempty
  `WAYLAND_DISPLAY` enables this candidate. Session socket permissions and the
  compositor's clipboard support must permit access; `XDG_RUNTIME_DIR` usually
  comes from the session. No environment values are printed or dumped.
- **X11:** install `xclip`, or `xsel` as its fallback. A nonempty `DISPLAY` enables
  X11 candidates. The session must supply the necessary X server authorization.
- **macOS:** `/usr/bin/pbpaste` and `/usr/bin/pbcopy` are used directly.

Selection is Wayland first, then xclip, then xsel, but only for enabled display
candidates. Fallback occurs **only when an executable cannot be found**. A
permission failure, disconnected display, nonzero exit, limit violation, or
operation timeout is returned immediately, not masked with another utility.
With neither display variable set, Linux returns NotFound without launching a
process. Both variables set may permit XWayland fallback when wl-clipboard is
missing; this does not prove native/XWayland clipboard synchronization.

The regular CLIPBOARD selection is used, never PRIMARY. Wayland requests text
and uses `--no-newline`; copies declare `text/plain;charset=utf-8`. The API
transfers bytes without Rust-side decoding, trimming, or newline insertion,
but desktop text utilities/compositors may interpret encodings. This is not a
portable arbitrary-binary clipboard format guarantee.

Upstream references: [wl-clipboard](https://github.com/bugaevc/wl-clipboard),
[xclip](https://github.com/astrand/xclip), [xsel](https://github.com/kfish/xsel).

## Resource and privacy contract

- Only explicit API calls launch a utility. No clipboard watcher, idle polling,
  display probe subprocess, or background Volt task is installed.
- A single **2-second deadline** covers all candidates and subprocess read,
  write, and stdout-drain work. Pipes are nonblocking; the active operation
  checks its deadline between bounded 8 KiB chunks. Empty writes close stdin.
- Reads reject overflow rather than returning a truncated successful paste.
  Exactly-at-limit content is accepted only after EOF. Writes reject excessive
  input before spawning anything. Caller chooses `max_bytes`; do not use an
  effectively unlimited value for user-controlled clipboard content.
- The write child's stdout and every child's stderr go to the null device.
  Unneeded output cannot fill a pipe or grow a captured diagnostics buffer.
  Errors are static messages: no clipboard bytes, utility stderr, or credentials.
- Executables and fixed arguments are passed directly to `Command`, never to a
  shell. Clipboard data is sent only over stdin, not in arguments or environment.
- Errors kill the subprocess's isolated process group plus the direct child,
  close owned pipes, and attempt to reap for at most another **100 ms**. Normal
  OS scheduling, process creation, and pathological kernel stalls cannot be given
  a hard real-time bound; if SIGKILL cannot finish in this cleanup window, reaping
  is best-effort rather than an unbounded UI hang. No worker threads are leaked.
- Successful Linux copy utilities may daemonize a clipboard selection owner.
  That is necessary for clipboard persistence and is intentionally **not killed**
  on success. It serves selection requests; it is not a Volt idle poller.
  Failure may already have changed the system clipboard: writes are not atomic
  transactions and cannot roll back a compositor update.

## Automated evidence and remaining manual validation

Inline mock-executable tests exercise exact/zero/overflow limits, input
preflight, literal binary and shell-metacharacter stdin, missing executable
fallback, nonzero errors without payload diagnostics, display selection, stderr
flooding, early writer exit, stalled reads/writes, inherited stdout drains, and
kill/reap cleanup. Test-only fixture shell scripts do not evaluate clipboard
input. They do not read or modify the actual user's clipboard.

**Native Wayland and X11 runtime acceptance is still required.** Use only
disposable non-sensitive text:

1. On native Wayland, copy/paste multiline Unicode, trailing newlines, empty
   text, and near-cap text between Volt and another native application. Confirm
   no extra newline, no partial oversized paste, and persistence after copy
   utility exit. Test the compositor/seat actually used for release.
2. On X11, repeat with xclip; repeat in a controlled test installation where
   xclip is absent but xsel exists. Verify CLIPBOARD, not PRIMARY, is affected.
3. With both display types available, confirm native/XWayland bridging. With
   wl-clipboard absent, confirm X11 fallback is attempted only with DISPLAY.
4. Test no display, missing utilities, invalid session access, and deliberate
   stalls via a disposable mock utility. Confirm one bounded failure and no
   late paste/copy. Do not paste secrets into test fixtures.
5. Leave Volt idle and verify no new clipboard utility launches or periodic
   reads. After replacing the clipboard, check successful copy owners retire
   according to the utility/compositor's behavior.
6. On macOS, validate Unicode/newline and empty clipboard round trips with the
   native utilities. Mock tests alone are not native pasteboard acceptance.

No real clipboard access is performed by the automated suite. App model tests
cover stale ownership, focus loss, away-and-back invalidation, redacted events,
and bounded selection extraction. Validate UI responsiveness during a stalled
helper, repeated-click admission, and tab/modal/focus switching manually too.
Functional mock tests allow a 3-second startup/operation budget under build
load; timeout tests retain their explicit 200 ms deadline and strict elapsed
assertions. The production backend deadline remains 2 seconds.

## Desktop launcher

The source/release archive includes `assets/volt.desktop`, the actual Volt icon,
and `scripts/linux/install_desktop.py`. On Linux, install an already built binary
without root:

```sh
python3 scripts/linux/install_desktop.py ./volt
```

For a source build, pass `target/release/volt`. The helper copies the executable
to `~/.local/bin/volt`, installs the icon and an absolute-path launcher under
`$XDG_DATA_HOME` (default `~/.local/share`). `--dry-run` inspects destinations
without writing and works on macOS too. It never builds, executes the supplied
binary, adds shell startup hooks or runs privileged commands. The launcher
follows the [Desktop Entry Exec specification](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html).
Actual desktop launcher/compositor behavior still requires Linux testing.
