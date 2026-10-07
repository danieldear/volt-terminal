# Shell integration: prompt navigation (batch 3)

Themes are unchanged. This is the first shell-integration increment, **not full
Ghostty parity or completion of phase 3**.

## Try it

Rebuild/relaunch Volt. For Zsh or Bash, choose **View → Enable Shell Integration
(This Tab)**. This explicitly installs Volt's compiled-in hooks in the current
shell only; it does not read project scripts or edit startup files. Repeat setup
is safe. Run tasks after enabling it to get their actual running/exit status.

Alternatively, source the packaged hook **inside Volt**:

```sh
# zsh
source /Applications/Volt.app/Contents/Resources/shell-integration/volt.zsh
# bash
source /Applications/Volt.app/Contents/Resources/shell-integration/volt.bash
```

For a source build, use `scripts/shell-integration/volt.zsh` or `volt.bash`
from the checkout. For persistent setup, source the packaged hook at the end of
your startup file yourself.

Fish 4.9.3 emits OSC 133 markers natively. **Do not source a `volt.fish` hook;
there is none.** Start Fish inside Volt or configure it as Volt’s shell.

Zsh/Bash hooks are packaged in `Volt.app/Contents/Resources/shell-integration/`.
Nothing automatically edits `.zshrc`, `.bashrc`, or `config.fish`. Sourcing a Zsh/Bash hook
is idempotent and restricted to interactive shells with `TERM_PROGRAM=volt`.
Source at the end of the startup file yourself if persistent integration is wanted.
Do not enable several terminals' OSC 133 integrations simultaneously.

Run several commands until their prompts enter scrollback:

```
Cmd+Shift+Up    -> previous prompt above the viewport
Cmd+Shift+Down  -> next prompt, then back to live output
```

These shortcuts never move a shell/TUI editing cursor or send escape sequences
to Neovim. They do nothing in the alternate screen. Find, workspace search, and
the keyboard-focused workspace card retain their normal key ownership.

The configurable actions are `previous_prompt` and `next_prompt`, for example:

```toml
[[keybindings]]
key = "alt+pageup"
action = "previous_prompt"

[[keybindings]]
key = "alt+pagedown"
action = "next_prompt"
```

Custom bindings and explicit `unbind` override defaults. The default Super-based
shortcuts also exist on Linux; use custom bindings if the window manager owns them.

## Implemented behavior

- OSC 133 A records a main-screen prompt row. C starts command timing and updates
  busy state. D emits status and elapsed milliseconds. B has no side effects
  in this increment; no input text is captured or stored.
- Status absent from D is unknown (`-1` internally), not success. Malformed or
  out-of-range shell statuses are ignored. Duplicate C does not restart timing.
  A recovers a missing D; RIS clears timing. Alternate-screen OSC 133 is ignored.
- Up to 1024 sparse prompt anchors per grid. None allocated until the first A.
  Full-screen scrolling updates an eviction origin in constant time, with no
  per-character work and no new per-cell fields. Navigation examines at most
  1024 entries, only on an explicit action.
- History eviction and configured limit reductions retain correct coordinates.
  Destructive screen clears, line insert/delete, and partial scrolls invalidate
  live anchors conservatively while retaining unaffected history anchors.
- A real resize/reflow or clear-scrollback discards all anchors. A no-op resize
  does not. This is deliberately safer than keeping stale row coordinates.
  New prompts are recorded normally afterward.
- No command strings, output copies, remote paths, or secrets are collected by
  this integration. Existing OS-owned local CWD inheritance remains unchanged.

## Hook compatibility and known limits

Zsh uses additive `precmd`/`preexec` hooks without replacing PS1, ZLE widgets, or
existing hook functions. Bash preserves string and array PROMPT_COMMAND forms;
Bash before 5.1 executes only array index zero, so the adapter preserves that
legacy behavior. Bash 4.4+ uses PS0 for C; macOS Bash 3.2 receives prompt/status
marks but **cannot report command duration/start**. No DEBUG trap is installed.

Fish 4.9.3 was validated in a real isolated PTY and native Volt window. It
provides `A;click_events=1`, `B`, `C;cmdline_url=…`, and `D;<status>` itself.
Volt recognizes the extended A/C forms; adding a separate Fish hook would
duplicate markers and potentially break navigation. Other Fish versions require
their own compatibility check.

Prompt hooks which print text can place A above the visible prompt. Asynchronous
prompt redraws and nested/multiplexer shell integrations need further acceptance
testing. Neither OSC 133 nor its reported status is an authenticated boundary:
program output can send the same protocol. Nothing privileged is triggered by it.
The lower-level VTE parser's raw OSC buffering is outside this sparse-anchor cap.

Still pending in phase 3: command-output selection, retained anchors across
reflow, robust prompt-redraw ordering, and modern Bash runtime acceptance.
No automatic shell injection is attempted.

## Protocol and shell references

Implementation checked against [iTerm2 shell protocol documentation](https://iterm2.com/documentation-escape-codes.html),
[Zsh hook documentation](https://zsh.sourceforge.io/Doc/Release/Functions.html),
[Bash interactive behavior](https://www.gnu.org/software/bash/manual/html_node/Interactive-Shell-Behavior),
and [Fish’s native OSC 133 support](https://fishshell.com/docs/current/relnotes.html).

## Validation

- Six core integration tests: navigation/eviction/limit reduction, alternate
  screen isolation, resize/clear/reset, destructive edits, fragmented sequences,
  duplicate starts and malformed status handling. Two bounded-anchor unit tests.
- Real isolated PTY tests for Zsh and macOS Bash 3.2: hooks still run, original
  failure status reaches both the existing prompt hook and the next command,
  double sourcing emits only one mark set, PS1 remains intact. Bash string and
  legacy array forms tested. Fish 4.9.3 PTY validation used `--no-config`
  and temporary XDG directories, with a simulated terminal capability handshake;
  its native C/D status 1 and A/B boundaries were observed without any Volt hook.
  No user startup files are read or modified.
- Run `PYTHONDONTWRITEBYTECODE=1 python3 scripts/validation/test_shell_hooks.py`.
- Parser throughput results, build, and native acceptance are recorded below
  after validation. Parser throughput is not full-app TermMark performance.

### Performance investigation

The first batch-3 build failed the gate: color-heavy parser cases lost 8–9%.
Moving OSC bookkeeping into a cold helper alone did not fix that loss. The
retained change separates SGR handling from the large cursor/edit CSI dispatch,
keeping identical parameter validation and SGR semantics while avoiding the
large unrelated dispatch prologue. Disassembly confirmed that the old CSI path
saved many registers before reaching color handling. This was a tested change,
not justification for accepting the failing build.

The three-repeat exploratory measurement improved color-heavy cases by
5.75–10.20% vs batch 2; ASCII cases ranged from -1.65% to +0.02%. Final acceptance
uses the seven-repeat runs below, not these exploratory numbers. Raw rejected
runs remain in `target/terminal-shell/performance*.json` (ignored build output).

### Final automated validation (2026-09-27)

- Workspace: 218 Rust tests passed; 4 opt-in tests ignored. Strict Clippy,
  formatting and Rust 1.87 workspace/all-target compilation passed.
- Seven alternating repetitions per workload, 256 MiB, 91 x 16 cells,
  ordinary and wide-seeded grids. Saved batch-2 baseline compared with the
  final build; no concurrent builds or GUI testing during measurements.

| Workload | Tracking off | Tracking on |
|---|---:|---:|
| fg (wide=False) | +8.06% | +8.09% |
| fg (wide=True) | +9.10% | +8.26% |
| fgbg (wide=False) | +11.20% | +9.78% |
| fgbg (wide=True) | +9.73% | +11.01% |
| long (wide=False) | +0.64% | -0.20% |
| long (wide=True) | +0.44% | -0.35% |
| many (wide=False) | -1.03% | -1.88% |
| many (wide=True) | -0.64% | -2.12% |

Positive is faster throughput. All cases passed the 5% gate. Tracking-on
uses `VOLT_BENCH_PROMPTS=1` to seed an anchor before the timed stream, keeping
constant-time eviction bookkeeping active even after that anchor is evicted.
No command/output feature was disabled for these measurements.
Raw reports: `target/terminal-shell/performance-final.json` and
`target/terminal-shell/performance-prompts.json`.

The small many-line regression is measured, not zero. This does not establish
whole-app speed, latency, power usage, or relative performance vs Alacritty.

### Native and GPU acceptance

- 132 GPU cases passed: 60 cursor-background, 27 terminal cached/full and
  linked/plain equality, 27 docked/floating card, 9 search, 9 link-preview bounds.
- Release app packaged and ad-hoc signature verified. Zsh/Bash hooks are bundled.
- Isolated native app: F7/F8 action overrides and default Cmd+Shift+Up/Down
  visibly navigated fixture prompt 35 -> 34 -> 33 -> live output.
- Find retained keyboard ownership when those shortcuts were pressed.
- Alternate-screen `read` test received an empty string after navigation shortcuts
  and Enter: no navigation key bytes leaked into the application.
- Built app reported `TERM_PROGRAM=volt`. Sourcing the Zsh hook and executing a
  30-line command created a real anchor; Cmd+Shift+Up brought the command prompt
  to the top, and Cmd+Shift+Down returned to live output.
- Nine Python validation tests passed, including Zsh/Bash hooks and native Fish PTY.
- Foreground live-prompt idle: 0.83% of one CPU core over
  30 seconds (1% gate passed), RSS 133056–133088 KiB.
  Only the isolated test instance was stopped afterward; user sessions/configs
  were not replaced.

### Fish 4.9.3 acceptance (2026-09-27)

- Parser regression test uses Fish’s actual A/B/C/D sequence forms, including
  `click_events=1` and `cmdline_url=false`, and verifies status 1 and navigation.
- In an isolated signed Volt build with Fish `--no-config --interactive --private`,
  a 40-line Fish command completed; F7 jumped to its prompt and F8 returned
  to live output. No default/login shell or user Fish config was changed.
- Because this adds no parser/render hot-path code (only test and documentation),
  the batch-3 throughput and GPU measurements above remain the applicable code
  measurements; no fresh TermMark claim is made.
- One initial isolated cold launch showed two prompt rows. A new tab and a
  second cold launch each showed one; this is an observed, non-reproduced startup
  anomaly, not a diagnosed Fish protocol or renderer defect. Recheck if seen
  during normal use.
