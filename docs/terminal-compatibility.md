# Terminal compatibility batch — v0.1.13 source

This work originated on `feature/terminal-compatibility`, based on reviewed main
`e088e4e` (v0.1.12). **The existing v0.1.12 release does not contain these changes.**
The user reports the draft worked on another system. Nothing here publishes it,
changes Apple secrets or modifies the standalone toolkit. Linux platform work
is explicitly deferred until a test system is available.

## Implemented

| Feature | Behavior / limits |
| --- | --- |
| Synchronized output | DECSET/DECRST 2026 and DECRQM query. One visible-cell snapshot per explicit batch, no duplicate scrollback. PTY parsing/replies continue. Repeated starts do not extend the one-second deadline. Release on end, timeout, reset or resize. Timeout scheduling is event-driven: no added performer lock/traversal when disabled. Other panes and overlays remain interactive. History browsing is independent of the held live frame. |
| Extended keyboard | Opt-in progressive flags, bounded main/alternate stacks, set/add/remove/push/pop/query. Character, navigation, F1–F35, keypad, modifier and common media key encoding; repeats/releases, shifted/available base-layout codes, associated text. IME/picker commits use pure text events when all-key reporting is requested; control characters are never embedded as associated text. Plain shell input uses the existing legacy encoder. Application shortcuts and local text fields own their input. Releases require the original pane/screen; focus loss drops stale keys. Winit does not supply reliable Caps/Num lock-state modifiers; they are not fabricated. Unmapped platform keys retain the legacy fallback. |
| Command output | OSC 133 C/D boundaries (or next A recovery), bounded sparse storage without command strings. `Cmd+Shift+O` selects the latest completed output, then `Cmd+C` copies. Custom action: `select_command_output`. Anchors are remapped by the text reflow algorithm, including an in-flight command start, pending-wrap starts and repeated bottom-edge reflow. Trimming/erasing invalidates unavailable ranges; alternate-screen apps cannot create main-shell anchors. Ordinary selections still clear on resize; this work preserves shell marks, not arbitrary selection endpoints. Soft-wrapped lines copy as logical text, preserving spaces at wrap boundaries. |
| Layout restoration | Settings → Advanced → Restore workspace layout, or `[session] restore = true`. Off by default. Saves logical window sizes, tab/split arrangement, ratios, active tab/pane, colors and OS-reported local CWDs. Saves on window close / event-loop exit, not during rendering. Missing directories use the normal launch directory. Custom tabs only; native macOS tabs are excluded. |
| Input composition | Explicitly enables winit IME handling and positions the native candidate window at the active pane's cursor. Final committed text keeps existing modal/read-only ownership. Dead keys are not converted into protocol escapes. Preedit text is not persisted or logged. |
| Performance gate | Saved pre-change parser executable plus alternating matched comparisons. `compare_termbench.py` checks exact build/config hashes, complete grid/font/scale/history/hooks/card context and at least three equal-count trials, retaining each workload and its range. A total win cannot hide a per-case regression. |

Protocol references: [synchronized output](https://contour-terminal.org/vt-extensions/synchronized-output/)
and [extended keyboard specification](https://sw.kovidgoyal.net/kitty/keyboard-protocol/).
These are protocol references, not product comparisons.

## Layout security and compatibility

Layout is `layout.json` in Volt's configuration directory. It has a versioned,
strict schema, maximum 128 KiB, 8 windows, 16 tabs per window, 16 panes per tab,
maximum depth 8 and validated ratios/sizes. Reads reject non-regular files and
symlinks; writes use an exclusive private (0600) temporary file, fsync and atomic
rename. Corrupt/unsupported state falls back to a normal window.

It stores **no commands, terminal contents, shell arguments, environment,
passwords, process IDs, task results, automatic OSC titles or custom titles**.
It opens fresh shells using the user's existing shell configuration; normal
user shell startup files still run. It does not resume processes. Disabling
restoration leaves the old file unused; delete it to remove saved directory names.
Only known schema version 1 is accepted; future schemas need explicit migrations.

## Accessibility audit: not complete screen-reader support

The terminal and most controls are GPU-drawn, not a native accessibility tree.
There is no semantic role/value/action bridge for these controls or bounded
accessible terminal text. Therefore VoiceOver navigation cannot be called
complete, and keyboard navigation alone does not establish accessibility.
Adding such a bridge safely requires scoped native implementation, explicit
focus/action ownership and no hidden-input exposure, followed by VoiceOver
acceptance. **This remains an open P1 item.** The IME fixes above do not claim
full multilingual runtime acceptance.

## Acceptance remaining before release

- Native Neovim/tmux/fzf synchronized drawing and keyboard negotiation;
  composition/dead keys with extended mode both off/on; Emoji & Symbols;
  read-only and password prompts. No trace may log typed secrets.
- Cmd+Shift+O output selection/copy with Bash/Zsh/Fish, Unicode, wrapping,
  resize and multiple panes; hook-disabled behavior is a no-op, not guessing.
- Enable layout restoration, arrange two windows/tabs/nested splits/colors,
  close/quit/relaunch; verify active panes and missing-directory fallback;
  disabling restoration must open an ordinary fresh window.
- At least three matched visible-window termbench trials per build, preferably
  five, for closed/floating/docked cards; idle CPU/RSS and typing during output.
  Parser/offscreen numbers are not display/PTY/input-latency measurements.
- VoiceOver semantic bridge and acceptance are outstanding. Linux work is
  deliberately deferred, not claimed complete.

## Validation evidence — 2026-10-08

- Full workspace unit/integration/doc tests: **398 passed, 4 intentionally ignored**.
  Python validation suite: **34 passed**. Formatting, whitespace checks,
  all-target Clippy with warnings denied and Rust **1.87** all-target checks passed.
- GPU fixtures: **24 settings**, **2 task-strip** and **360 tab-status** cases passed.
  These are offscreen fixtures, not native window interaction or accessibility evidence.
- The release-mode macOS bundle builds and passes strict local signature verification.
  This **new local bundle is ad-hoc signed**, not the notarized v0.1.12 release.
  The draft and release/version metadata were not changed or published.
- The pinned toolkit snapshot matches its own recorded file hashes and was not changed.
  It is older than the currently evolving standalone project: its canonical sync check
  reports a difference. Updating that dependency is a separate reviewed operation.
- Fresh `cargo audit` reported no blocking vulnerability, but **three unmaintained
  dependency warnings** remain: rustybuzz 0.12.1 and ttf-parser 0.20.0/0.25.1.
  These are maintenance debt, not proof of an exploitable issue or proof of security.
  Font/shaping dependency migration needs its own compatibility/performance tests.

The native Mac was locked when interaction testing was attempted. The isolated
smoke-test app could not produce a valid visible-frame test; it was stopped.
User configuration and the installed application were not overwritten. Native
acceptance and full-app termbench must be completed on an unlocked machine.

### Matched microbenchmarks (not full-app TermMark)

Baseline commit: `e088e4e8c2a609388eb93acda588c87a7cb39102`.
Environment: Apple M4 Pro (14 logical CPUs), macOS 27.0.1, Rust 1.94.0.
The offscreen fixture uses 1800×1520 pixels, a 200×60 grid, monospace 14-point
font, scale 1, line height 1.2, 30 warmup and 300 measured frames per workload.
Both executables were built in release mode. Each comparison used seven
alternating repetitions after warmup, with no concurrent build/GPU-fixture run.
Final parser throughput changes were between **−0.38% and +0.58%** across eight
64 MiB cases (91 columns, 16 rows; scalar and wide-seeded variants).

| Offscreen workload | CPU median change | CPU p95 change | GPU completion median change |
| --- | ---: | ---: | ---: |
| Full ASCII | +0.01% | +0.27% | +0.02% |
| One-row ASCII | +3.11% | +1.08% | +1.27% |
| Task strip | −10.46% | −0.72% | −5.71% |
| Full Unicode | −10.16% | −2.79% | −5.24% |
| Static Unicode | +2.39% | +0.47% | +2.85% |

The 5% median gates passed. **These are noisy measurements, not speedup claims.**
An initial comparison against an inherited executable showed +8% full-ASCII CPU
median; its exact source provenance was not established. Rebuilding the baseline
from the recorded commit removed that median failure. The first exact-baseline
run still showed +5.70% one-row CPU p95; the final rerun measured +1.08%.
Retain the tail observation and recheck it, rather than conclude zero regression.
Full-app throughput, typing latency and visible presentation remain unmeasured.

Final executable SHA-256:

- Parser baseline: `5dab3ee32b4105f19e562c8b16c5d3f85a1cf636d7c192c8e9000c6a369577fd`
- Parser new: `19d391278107053156acd9ac69f883dcfd8bbceca5df650221c88f005e4c585a`
- Renderer exact baseline: `f7d16ac14bb301980e8d58aaaa5d3f22258e588578b920d178102b38c9d48a23`
- Renderer new: `acf6225996e64f6b802e93b611f09e7a10014f2d61b6cf879fa603d99d8b6c82`

For visible-window comparisons, record matching context and at least three
complete trials per build, then run:

```sh
python3 scripts/validation/compare_termbench.py before.json after.json
```

The report schema is documented in the script. Configurations are identified
by hash, not uploaded; the checker never collects shell text or credentials.
Supplying a JSON report does not by itself prove that visible frames occurred.
