# UI toolkit extraction

Canonical project: sibling `volt-ui-kit`, version 0.1.0 (local, unreleased).
Volt dependency: `vendor/volt-ui-kit`, a reproducible snapshot with `SOURCE.json`.

## Boundary

```text
Volt state / trusted actions / native input
                  |
Compatibility exports + cached geometry
                  |
volt-ui-kit models / layout / widgets / icons
                  |
Renderer's Painter adapter
                  |
Existing wgpu device / buffers / cosmic-text / glyph atlas
```

Moved: card, Settings/onboarding, task strip/form and search presentation and
paint recipes; vector primitives/icons; tab geometry and accent colors.
A reusable generic Button now paints Settings footer controls.

Stayed in Volt: terminal parser/grid/PTY, command/task lifecycle and trust,
Git/project detection, persistent preferences, native input/AppKit, theme
editor's terminal-specific color model and native window dragging.

## Updating the snapshot

From the canonical toolkit:

```sh
python3 scripts/sync_into_volt.py --volt ../terminal-public
python3 scripts/sync_into_volt.py --volt ../terminal-public --check
```

Do not make independent edits in the vendor directory. Volt retains a normal
self-contained checkout, lockfile and CI; it does not require a sibling folder
or a new unpublished GitHub repository to build. The validation suite checks
the manifest inventory and hashes. A future published version can replace the
vendored path dependency.

## Verification

- Toolkit builds independently with only `unicode-width`; no Volt dependencies.
- Toolkit unit/doc tests, Clippy, MSRV 1.87 and Cargo package verification.
- Volt workspace tests, Clippy and MSRV; macOS release bundle rebuilt.
- All **48 baseline PNGs** from Settings, task, search and card GPU checks matched
  their pre-extraction SHA-256 hashes exactly.
- 360 GPU tab rendering/status checks pass; shared tab hit-testing tests moved
  with their implementation into the toolkit.
- Earlier uncommitted shortcut/task-message fixes retained byte-for-byte.
- Static component gallery generated at dark/light themes and 1x/2x scale.

Renderer caches, backend resources and frame scheduling are unchanged by this
extraction. No extra rendering loop, GPU device, command execution or recurring
redraw timer was introduced. Warm-cache microbenchmarks are diagnostic and can
be noisy; matching-window/config termbench remains the end-to-end performance
gate. This document does not claim a new benchmark record or automatic full
framework/platform/accessibility readiness.

### Warm-cache renderer comparison

Saved pre/post extraction executables, 12 runs in a repeating A/B/B/A order
(six runs per executable), 300 measured frames per workload after warm-up.
Median of each run's CPU p50, milliseconds:

| Workload | Before | After |
| --- | ---: | ---: |
| ASCII full | 1.229 | 1.220 |
| ASCII one-row | 0.848 | 0.735 |
| Unicode full | 0.886 | 0.925 |
| Unicode static | 0.339 | 0.323 |
| Tasks full | 1.448 | 1.474 |

The observed ranges overlap broadly. This did not reveal a consistent slowdown;
small changes in either direction are not a demonstrated speedup or an
end-to-end terminal performance guarantee. These are renderer CPU submission
measurements, **not** termbench timings or display latency.

## Glass Browser inputs

The canonical toolkit also includes an optional general-purpose Panel, smoked/frosted surface tokens, host-fed brightness hysteresis and clipped/rounded topmost hit routing. These are independently implemented design inputs from Glass Browser, not a copied Vello/CEF backend. Volt continues using its current opaque widgets, renderer and caches; no backdrop sampling or blur pass is enabled. The gallery labels its surface examples as opaque fallbacks. A real glass compositor and accessibility preference plumbing remain separate future work.
