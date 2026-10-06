# Volt v0.1.9

## Highlights

- Unified terminal and workspace search, with configurable editor commands.
- Flat native-feeling tab styling with a soft active pill and colored tabs.
- Project task buttons, an Add Task form, and saved tasks in the workspace card.
- An updated product website and documentation.

## Task reliability and security

- Saved tasks run visibly in the selected terminal pane, with keyboard focus
  returned to the console. Known foreground jobs, full-screen apps, read-only
  panes, and detected hidden-input prompts block task injection.
- Trusted tasks run directly unless configured to ask each time. Untrusted
  tasks offer a compact, read-only **Run once** confirmation instead of opening
  the entire workspace card. Confirmation never grants file-wide trust.
- Confirmation is invalidated if the task file or selected pane changes.
  Task-file trust compares the exact reviewed bytes against the current file.
- Task loading rejects oversized files, symlinks, and non-regular files without
  blocking on FIFOs. PTY write errors are reported rather than treated as a
  successful dispatch.
- Detected commands are explicit import suggestions, separate from saved tasks.
  Import saves the chosen command safely; it does not execute it.

## Interface fixes

- Task forms fit the default Retina window size, and first-task creation remains
  accessible from the compact strip and native View menu.
- Busy/error notices are compact, non-actionable banners rather than file rows.
- Command confirmations cannot be obscured by workspace cards or task buttons.
- Pressing Enter in a shell without OSC command hooks no longer permanently
  marks that pane busy. Foreground-job checks do not require those hooks.

## Distribution

The macOS archive is **ad-hoc signed**, not Developer ID signed or notarized.
Linux and macOS artifacts and SHA-256 checksums are published by the release
workflow. Download only from this repository's release page and follow the
[distribution guidance](distribution.md).

Automated validation includes Rust tests, Clippy, Rust 1.87 compatibility,
validation-script tests, renderer fixtures, dependency auditing, and secret
scanning. This is not a claim that every platform/UI interaction or performance
scenario has been exhaustively verified.
