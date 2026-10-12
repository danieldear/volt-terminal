# Real document files (Unreleased)

This opt-in slice adds **macOS file panels and a bounded notes workflow**, not a
complete editor engine or automatic persistence of arbitrary application state.
The developer owns paths, dirty state, saving policy and the UI. Default/core
`Painter` and Volt's separate vendored toolkit are unchanged.

```text
Rust button / Cmd+O / Cmd+S / native menu
                 |
          developer command
           /           \
     native panel    known document path
    OK / Cancel             |
           \                /
             one I/O worker
            read / write / sync
                    |
             bounded UiSender
                    |
          main-thread completion
         generation + saved snapshot
                    |
       clean / visible error / remain dirty
                    |
       recheck pending Open or Close policy
```

## Native panels

```rust,no_run
use volt_ui_kit::native::file_dialog::{self, FileDialogOptions};
// Call from a running native application's main-thread callback.
let selection = file_dialog::open_file(&FileDialogOptions::new("Open notes"))?;
if let Some(path) = selection {
    // Move the path to your background worker; never read on the UI thread.
    println!("Selected {}", path.display());
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

`save_file` accepts `.suggested_name("notes.txt")`; titles and suggested names
are bounded/validated before touching the OS. Cancel is `Ok(None)`. Unsupported
platforms/builds, wrong thread and invalid native responses return explicit
errors. `available()` reports the compiled adapter, not authorization.

Panels require `native-desktop` on macOS, and use public
[NSOpenPanel](https://developer.apple.com/documentation/appkit/nsopenpanel) /
[NSSavePanel](https://developer.apple.com/documentation/appkit/nssavepanel).
They are **synchronous application-modal panels** (`runModal`), not an async sheet
API. AppKit handles the native modal UI; toolkit worker-message delivery resumes
when the panel returns. The actual file I/O runs off the UI thread in Catalog.
No private API, AppleScript, GTK backend or new registry crate is introduced.
Only existing optional objc2 feature flags are expanded. Windows/Linux panel
adapters and sandbox security-scoped bookmark persistence remain pending.

## Bounded file helpers

```rust,no_run
use std::path::Path;
use volt_ui_kit::native::files::{read_text_file, save_text_file, SavePolicy};
// Run on a worker thread after your application obtains permission/path intent.
let path = Path::new("notes.txt");
let original = read_text_file(path)?;
save_text_file(path, "Updated note", SavePolicy::MatchContents(&original))?;
# Ok::<(), std::io::Error>(())
```

- UTF-8 regular files only, maximum **8192 bytes / 8 KiB**, matching the current
  lightweight text editor. Oversized/invalid UTF-8 reads fail without replacing
  the installed draft. Input is never silently truncated. This is not large-file
  editor support.
- `CreateNew` never overwrites an existing target. `MatchContents(original)`
  checks existing contents before writing and immediately before committing.
  `ApprovedReplacement` explicitly permits replacement; Catalog uses it only
  after a Save As panel selection, including AppKit's overwrite confirmation.
- Writes allocate a sibling using `create_new`, write/sync it, and then commit.
  New targets use a hard link so a concurrent existing name is not overwritten.
  Where hard links are unsupported (exFAT/FAT, many SMB/NFS shares) new targets
  fall back to an exclusive `create_new` write: still never overwriting, but not
  atomic if the process dies mid-write (a failed write removes its own file).
  Replacement uses `std::fs::rename`. Originals are not truncated in place, and
  failed commits clean up the temporary file on normal error/drop paths.
- Unix new temporary files use mode 0600. Existing permission bits are retained;
  read-only targets are rejected even with explicit replacement. Ownership,
  ACLs, extended attributes and hard-link identity are **not** preserved.
- **Not a filesystem sandbox or cross-process transaction.** Link/directory/
  device selections are rejected in ordinary checks, but malicious directory
  mutation/TOCTOU races are not eliminated. Do not use these helpers as a secure
  untrusted-path boundary. Matching does not establish an OS file lock.
- Parent directories are not fsynced, so this is not a power-loss durability
  guarantee. A killed process may leave a temporary file. No backup/recovery,
  auto-save, path watching, session restoration or format conversion is supplied.
- An in-flight filesystem call cannot be forcibly cancelled. Limit concurrent
  jobs and handle stalled/network filesystems in your own application policy.

## Catalog workflow

```sh
cargo run --release --features native-gpu,native-desktop --example catalog -- \
  --page documents
```

The new **Documents** page is separate from the earlier Application lifecycle
page, whose example draft still saves in memory only. Both are guarded across
navigation. If both drafts are dirty, each policy is rechecked in sequence; the
real-file save does not silently discard the memory example.

Documents offers Open, Save, Save As, SF Mono editing, status/error text and a
dirty badge. Cmd+O opens; Cmd+S saves the active page's draft; Cmd+Shift+S saves
the notes document as another file. An owned macOS File menu exposes the same
commands. Custom palettes remain developer-owned UI, not a mandatory shell.

One file worker runs at a time. The editor is read-only during the operation,
completion retries channel backpressure on the worker (never the UI thread),
and late/stale generations are ignored. Only the actually written snapshot is
marked saved; a newer Rust-state edit stays dirty. Save failure retains the draft
and pending confirmation. Save-panel cancellation clears pending Open/Close,
not the draft. Open while dirty asks Save/Discard/Cancel first.

Closing while an operation is busy is vetoed. Cancel cancels the pending action,
not a write already in progress. Only successful completion requests closing,
and the host rechecks all guards. Force Quit, Dock/system termination and the
`cargo volt dev` rebuild/restart runner still do **not** promise unsaved-document
protection. Full OS termination and state-preserving reload remain future work.

## Reliability and native receipt (2026-10-11)

- Catalog Application lifecycle includes a non-idempotent **Count command
  execution** witness (Cmd+Shift+P). Four discrete native shortcut presses
  yielded exactly 4, one File menu invocation yielded 5, one declarative Run
  button yielded 6. Pressing the shortcut in a modal palette left the count at 6.
- The host shortcut test passes 50 repeat-flagged events without extra callback
  executions and rejects missing Shift. This does not certify AppKit's physical
  held-key/native-menu auto-repeat, alternate layouts, real IME or VoiceOver.
- Real native Open loaded a disposable text fixture. Edit/Cmd+S wrote the exact
  text, verified by filesystem readback. An externally modified fixture caused a
  visible conflict, preserved disk contents and retained the unsaved draft.
- Save As wrote a separate fixture; invalid UTF-8 Open retained that loaded file.
  Native Save As cancellation and dirty-Open cancellation preserved edits.
  Save from native-close confirmation wrote the final snapshot, then exited
  with wrapper status 0. No `.volt-save-*` leftovers remained in that test folder.
- Retained native logs report Vello / Apple M4 Pro / Metal at 2880x1840, with
  zero framebuffer uploads. No new GPU panic/validation failure was observed.
  This is not utilization/power profiling or acceptance of the original
  larger-display resize/fullscreen driver issue.
- Logs, disposable bundle and fixtures are local/ignored under
  `work/documents-2026-10-11/`; no user documents were used. New visual demos are
  Catalog-only. This document workflow is included in v0.4.0.

Local app bundles and notifications are implemented in separate slices; signed
distribution, deeper navigation and multi-window remain application-foundation
work, not completed by this document workflow.

### Automated gates

Final all-feature/all-target suite: **483 passed, 13 ignored**. Separate doctests:
28 passed, 1 ignored; default suite: 40 passed, 1 ignored. Strict full-feature
Clippy and native-only library/test Clippy passed. Rust 1.87 all-feature/all-target
and Linux x86_64 all-feature/all-target checks passed (Linux runtime is not
accepted). Catalog's ignored hardware test explicitly passed all **39 pages**
at 1x/2x on Metal. Release Catalog and local default package verification passed.
Cargo.lock is unchanged; the default dependency tree still contains only
`unicode-width`. No registry publication or Volt snapshot sync was performed.
