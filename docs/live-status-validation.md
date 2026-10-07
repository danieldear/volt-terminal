# Live status and throughput validation

This change restores status reporting without weakening task-file review or
executing detected commands automatically.

## Fixes

- Pure OSC command/title/cwd events now wake the UI, even without printable output.
- Command status events repaint tabs and task buttons even with cursor blinking off.
- Task results remain visible after another task starts, including reviewed run-once
  tasks. History is bounded and matched to the command and working directory;
  changing either hides the old result. It does not grant file-wide trust.
- Unknown/missing exit reports do not become success checkmarks.
- Tab status is green while running, red for a reported nonzero exit, blank when idle.
  The user-selected tab accent remains independent.
- A 250 ms native context check refreshes directory fallback labels and observes
  foreground jobs without shell hooks. It never guesses an exit code.
- View → Enable Shell Integration (This Tab) explicitly enables the packaged
  Zsh/Bash hooks. Bare source builds use the compiled-in copy. Startup files are
  not edited. Read-only, foreground-job, TUI and secure-input gates remain in place.

## Measured optimization trial

Apple M4 Pro; identical 91 × 16 PTYs; same TermMark executable; three alternating
before/after trials. These measurements include the production PTY pipeline and
parser, **not GPU rendering/presentation**. They compare the single-row rotation
and retained blank-template optimizations against v0.1.10's core.

| Case | Before median (s) | After median (s) | Elapsed change |
|---|---:|---:|---:|
| ManyLine | 7.6552 | 7.5927 | -0.8% |
| LongLine | 7.1875 | 6.9040 | -3.9% |
| FGPerChar | 2.4087 | 2.3823 | -1.1% |
| FGBGPerChar | 5.6509 | 5.5801 | -1.3% |
| TermMarkV2 Normal | 22.9023 | 22.3918 | -2.2% |

Seven alternating parser-only trials (256 MiB, ASCII and prior-wide-character
seeds) improved ManyLine throughput about 4.2–4.3%; other cases ranged from
−0.8% to +2.7%, within the 5% regression gate. A single 300-sample PTY round-trip
trial retained approximately 0.033 ms median latency; this does not measure display
latency. None of these figures establishes a full-app speed ranking or restores
an earlier benchmark taken with different window/font/grid dimensions.

## Runtime checks

In an isolated Zsh tab with cursor blinking disabled:

- Build showed a running dot, then ✓ with visible console output.
- Test showed its running dot while Build retained ✓, then an error result.
- An inactive tab retained its red error dot; an idle tab had no status ring.
- An unnamed tab followed `cd` without shell hooks; a foreground `sleep` showed
  green busy state through the native fallback.
- The new menu enabled hooks with a readable one-line source command.

Automated checks also cover source/exit tracking, history bounds, safe repeated
hook setup, native status semantics, directory fallback/custom-title precedence,
blank-template allocation reuse, and scroll-region equivalence. Renderer fixtures
check status colors, blank idle indicators, task UI and tab colors at 1×/2×.

Final local checks: 335 tests passed, four pre-existing tests ignored; strict
Clippy, Rust 1.87 workspace checks, formatting and diff checks passed. The
release-mode app bundle passed strict ad-hoc signature verification. A redacted
scan of the changed and new files found no credential matches; this is not a
whole-history security audit. These changes are not yet a published release.
