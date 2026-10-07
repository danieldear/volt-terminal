# Settings implementation validation

2026-10-07; public working directory `terminal-public`, starting HEAD `71b4cc5`.
Pre-existing optimization and documentation changes were retained. No commit,
version bump, PR or public release was performed for this implementation.

## Verified

- 361 Rust tests passed; 4 existing tests ignored.
- Locked workspace Clippy/all targets with warnings denied passed.
- Rust 1.87 locked workspace check, rustfmt and whitespace checks passed.
- 24 Settings GPU cache/fresh/hide cases across four themes and three scales.
- Real installed Bash, Zsh and Fish startup tests: personal rc contents remain
  unchanged, login/non-login profile loading, quoted paths, failure-status OSC
  events and the status delivered to the prompt renderer.
- Native macOS isolated HOME/XDG smoke tests: first-run introduction, Next,
  live theme/font/size/line-height, Finish and a Meow tab, Cmd+comma reopening,
  cancel restoring saved appearance, external-edit refusal, Skip preserving the
  normal prompt, shortcut recording/saving/dispatch, and Cmd+S while editing.
- Final release-mode app bundle built at `target/macos/Volt.app`.
- Existing running Volt and personal config/startup files were not modified.

## Rendering comparison

Six alternating saved-executable pairs with Settings closed. Baseline is the
pre-Settings cached-rendering executable, **not** an old public release. The
benchmark measures warmed offscreen CPU build/submit time, not PTY throughput,
display latency, energy, input-to-photon or whole termbench timing.

| Workload | Before median CPU p50 ms | After | Change |
| --- | ---: | ---: | ---: |
| ascii-full | 0.905000 | 0.917625 | +1.40% |
| ascii-one-row | 0.562000 | 0.526312 | -6.35% |
| unicode-full | 0.557292 | 0.563480 | +1.11% |
| unicode-static | 0.356875 | 0.319958 | -10.34% |
| tasks-full | 1.007417 | 0.963271 | -4.38% |

Runs were noisy: individual paired changes varied widely in both directions.
The small positive medians must not be presented as zero regression, and the
negative medians must not be marketed as speedups. There was no consistent
per-pair loss or aggregate >5% loss in these trials. A matched visible-window
termbench comparison remains a release acceptance check.

Raw local runs and executable hashes are in ignored `target/perf-settings/`.
Settings has no periodic refresh timer and its UI/config/font discovery occurs
on interaction; normal output parsing does not call Meow or the settings model.

## Boundaries

See [Settings](settings.md) for Bash true-login/logout limitations, custom prompt
preservation and optional integration. The bundled Meow retains the primary
layout/signals, but not elapsed-command timing. Custom GPU-control accessibility
exposure and broader IME compatibility remain in the product-wide audit plan.
Passing a finite test matrix is not proof of universal compatibility/security.

## Footer icons

Apply/Finish use a vector checkmark, Cancel/Skip use a vector close icon, and
onboarding Next/Back use direction arrows. Icon-and-label groups are centered
without changing their hit rectangles. No dependency on Nerd Font glyphs.
`settingscheck` verifies contrasting icon pixels across four themes and three
DPI scales, plus cached/fresh frame parity and removal without ghosting. This
fixture is now included in the Linux software-GPU CI job.
