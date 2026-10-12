# Volt UI Kit integration

Volt consumes a pinned, hash-inventoried copy of the separate Volt UI Kit
project. Do not edit files under `vendor/volt-ui-kit` directly: import upstream
changes with the toolkit's `scripts/sync_into_volt.py` and validate the resulting
`SOURCE.json` inventory.

## Current integration

The toolkit snapshot is version **0.4.0**, pinned to merged upstream commit
`5547694aea097aea57d8e32ec7e8a42315924a2e` (2026-10-12). Settings, onboarding, workspace cards,
task controls, search and tab layout continue to use its shared paint/layout
recipes. Boolean Settings rows now use the toolkit's declarative **Toggle**
control: rounded track, sliding-position knob and an explicit On/Off label.
Boolean rows place the setting label on the left and the state/switch at the
right edge, vertically centered; long labels reserve room for that control.
Volt still owns the real settings draft, hit testing, keyboard handling,
preview, Apply and Cancel; toggling a switch does not write configuration.

The renderer enables `rust-ui` (which includes `declarative`). The toolkit's
`native`, `native-desktop`, `native-notifications` and `native-gpu` hosts are
**not** enabled. Closed select triggers and opacity/divider sliders now paint
with the actual Rust component implementations from 0.4.0.
Volt retains its own winit window, wgpu 29 renderer, glyph atlas and cosmic-text
font system. Optional native dependencies can appear in Cargo.lock without
being built into Volt.

```text
Volt events -> Settings draft -> visible typed control state
                                    |
                          Settings geometry cache
                                    |
                          Volt UI Kit component paint
                                    |
                      existing Volt GPU buffers/atlas
```

The host bridge is deliberately limited to closed controls and bounded labels.
Volt owns dropdown option lists, keyboard selection/type-ahead, custom font/shell
values, numeric/text editing, shortcut recording and slider pointer capture.
Selection popups remain host-painted so arbitrary installed-font lists do not
inherit the toolkit select component's 128-option limit. Switches stay at the
right edge; numbers/text use editable fields, choices use dropdowns, scalar
opacities use sliders, actions use buttons and explanatory rows remain inert.
It is not a general declarative-document renderer: arbitrary text, clipping,
images, effects and toolkit input dispatch require additional adapters. Its
no-op toolkit handler is never dispatched; actual input uses Volt's existing
Settings row handlers.

The 0.4.0 import captures 206 files from a clean archive of the merged commit,
not a mutable working tree. `SOURCE.json` inventories every imported file; the
validation test also checks that manifest, package and consumer versions match.
The toolkit repository itself is unchanged. This supersedes the initial 179-file
0.2.0 development snapshot from working-tree base commit `419725f`.

## Migration sequence

1. **Integrated:** 0.4.0 shared snapshot, switches, select triggers and sliders;
   all Settings/onboarding rows expose semantic control types rather than generic
   minus/plus widgets. Existing transaction/input/renderer paths remain authoritative.
2. **Next:** general toolkit text-editor/input and popup adapters, with explicit
   clipping, selection, IME, clipboard and accessibility contracts. Current
   text/numeric fields and option lists deliberately keep the validated host input.
3. **Later:** compose complete settings/card/search surfaces with toolkit layout
   and reusable components, after each surface matches current behavior.
4. **Separate decision:** evaluate effects or a different GPU backend. There is
   no reason to replace the terminal renderer merely to adopt new UI controls.

This is incremental adoption, not a complete port of the toolkit's gallery or
its native application host. Theme colors remain Volt's current theme colors.

## Acceptance checks

- `RUST_MIN_STACK=33554432 cargo test --locked --workspace`
- `cargo test --locked --workspace --exclude volt-ui-kit` (ordinary host test stacks)
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo +1.87 check --locked --workspace`
- Python validation tests, including the independent vendored hash inventory
- `settingscheck`: switch state changes, cache hits/rebuilds, hide/show, footer
  icons, four themes and backing scales 1 / 1.5 / 2
- Existing card, task, search and tab GPU fixtures
- Alternating before/after release `cachebench` runs; this measures offscreen
  rendering, **not** native input latency or PTY/TermMark throughput
- Manual Settings interaction and full-app termbench on the target Mac remain
  separate runtime checks; offscreen rendering cannot substitute for these

Controls are composed only when the Settings geometry cache invalidates. The
ordinary terminal rendering path does not instantiate a toolkit runtime or
parse a UI document per frame.

## Upstream integration caveats — 0.4.0

The merged source has two relevant integration issues, preserved transparently:

- `--features declarative` alone fails to compile: `Runtime` initializes
  `slider_lower` and `file_hover` without the `rust-ui` guards used on their
  declarations. Volt uses `rust-ui` for its new controls, so its active feature
  graph compiles. The upstream source is not silently patched.
- The Rust UI negative depth-limit fixture overflows an ordinary test thread's
  stack in debug mode. The toolkit-inclusive test suite uses a 32 MiB **test-only**
  `RUST_MIN_STACK`, and CI/release/signing validation applies it only to the
  `cargo test` invocation. Host tests pass separately with ordinary stacks.
  This does not set the installed app's environment or enlarge production threads;
  it is a test workaround, not a fix for the upstream deep-tree issue. Volt's
  mounted component trees are fixed and shallow, not arbitrary user documents.

## Recorded validation — 2026-10-10

- 429 Rust tests/doc-tests and 34 Python validation tests passed.
- Strict Clippy (workspace/all targets), formatting and Rust 1.87 workspace
  checks passed for Volt's enabled feature graph.
- 422 GPU fixture cases passed: 24 settings, 27 cards, 9 search, 2 task UI and
  360 tab status cases. Settings fixtures cover both switch states and returning
  to the original cached image without stale pixels.
- The newly built macOS app was tested interactively: click and Return toggle
  the workspace switch; Cancel followed by reopening restores the saved state.
  The test did not Apply changes to the user's configuration.
- The local release-mode app bundle built successfully and its ad-hoc signature
  verified. This is a development build, not a newly published/notarized release.

Rendering comparison used saved executables from baseline `d2adeb3` and the
upgraded renderer. Both seven-pair batches were retained, giving 14 alternating
before/after pairs; the executable hashes match between batches.

| Workload | Before CPU median (ms) | After CPU median (ms) | CPU change | GPU completion p95 change |
| --- | ---: | ---: | ---: | ---: |
| ASCII full redraw | 1.020 | 1.004 | -1.5% | +24.8% |
| ASCII one-row change | 0.674 | 0.591 | -12.3% | -9.3% |
| Task buttons / full redraw | 1.131 | 1.107 | -2.1% | +12.6% |
| Unicode full redraw | 0.683 | 0.687 | +0.6% | +6.0% |
| Unicode static | 0.401 | 0.353 | -11.9% | -5.9% |

**Performance acceptance remains open.** Other toolkit Relay snapshot renders
were observed during both batches. The first batch passed the script's 5% CPU
median budget; the second exceeded it for Unicode full redraw (+5.6%). Aggregate
CPU medians are within budget, but the inconsistent individual batches and GPU
tails must not be described as a clean no-regression result. Concurrent work
may explain the variance; that is an inference, not a proven cause. Repeat on
a quiet machine and run full-app termbench with identical configuration and
window/grid size before release. These fixtures also do not benchmark Settings
rebuild cost, display latency or assistive-technology behavior.

## Recorded validation — 2026-10-12 (0.4.0)

- 707 Rust tests/doc-tests passed with the toolkit test-stack allowance; host
  tests also passed separately without it. Four illustrative toolkit doc-tests
  are intentionally ignored by upstream.
- 34 Python validation tests, strict Clippy, formatting and Rust 1.87 checks
  passed. The 206-file snapshot matches the pinned clean upstream archive.
- 425 GPU cases passed: 27 Settings/component cases, 27 cards, 9 search,
  2 task UI and 360 tab cases. Select chevron pixels are explicitly checked
  across the theme/DPI matrix; cached/fresh and popup hide/show images match.
- Live Mac QA verified dropdown opening and keyboard selection, theme and
  numeric-value previews, slider dragging, and cancellation restoring the
  existing preferences. The final rebuild also verified visible chevrons and
  keeping the checkmark on the chosen option independently of hover focus.
  No test changes were Applied to the user's configuration.
- Local release-mode bundle built and its ad-hoc signature verified. Volt's
  application version remains 0.1.12; nothing was pushed, tagged or released.

### Rendering comparison (acceptance not passed)

Baseline is the saved pre-upgrade 0.2.0 executable with the same existing Volt
UI work, not the earlier committed `d2adeb3` binary. Both five-pair alternating
batches were retained. Combined results across all ten before/after pairs:

| Workload | Before CPU median (ms) | After CPU median (ms) | CPU change | GPU completion p95 change |
| --- | ---: | ---: | ---: | ---: |
| ascii-full | 1.419 | 1.412 | -0.5% | -1.2% |
| ascii-one-row | 1.014 | 0.944 | -6.8% | -1.9% |
| tasks-full | 1.577 | 1.586 | +0.6% | +0.8% |
| unicode-full | 1.068 | 1.262 | +18.2% | +4.0% |
| unicode-static | 0.728 | 0.565 | -22.4% | -9.1% |

**Do not call this performance-neutral.** Both individual batches exceeded the
5% CPU budget for `unicode-full` (+18.4% and +18.0%). The second also exceeded
it for `ascii-full` (+7.1%). No terminal hot-path algorithm or rendering backend
was intentionally changed, and the imported legacy paint recipes are identical
to the previous snapshot. That does not prove binary-level performance is
unchanged. Other desktop/catalog activity was present, but its causal role is
unproven. Whole-app TermMark was not run. Performance diagnosis and a controlled
full-app comparison remain release gates; this build is for functional testing.

### Release preparation recheck — 2026-10-12

A third five-pair run against the saved pre-upgrade binary again failed the 5%
CPU median budget: Unicode full redraw +18.6%, task buttons +11.9%,
Unicode static +8.9%, ASCII one-row +7.8%, ASCII full +1.0%.
The application source version is now 0.1.13. These results reinforce the open
performance gate; functional checks and signing do not waive it.

Release preparation also passed the dependency audit (with informational
unmaintained-package warnings). Secret scanning found two false positives in
verified inventory checksums; the detector retains default rules and exempts
only those exact lines in that exact manifest. Independent inventory tests
verify the hashes, and synthetic API keys in/outside the manifest are rejected.
