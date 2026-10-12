# Security and resource boundaries

This guide describes current input, rendering and asset constraints, not a
security certification or sandbox for application callbacks. Default/legacy
Painter behaviour and the embedded dependency footprint remain unchanged.

## Input and renderer checks

```text
Assistive request
  -> current tree / bounded payload / role / enabled / modal scope checks
  -> focus, selection and callback (invalid requests are no-ops)

Native paint commands
  -> explicit image admission: unique handles + bytes + compatible atlas packing
       | reject                       | accept
       v                              v
  software fallback           protect needed cache hits
                              -> upload misses -> encode -> buffer-budget gate
                                   |                         |
                                   | safe                    | detected overflow
                                   v                         v
                               GPU drawing              software fallback
```

### Security and resource hardening

- AccessKit rejects foreign tree IDs, text over 8192 bytes and non-finite or
  unrepresentable numeric payloads before cloning/converting them.
- Runtime validates the current target's role, enabled/mounted state and modal
  scope **before** changing focus, editor selection or invoking a callback.
  Text input rejects read-only fields, oversized sanitized text (including tab
  expansion), and control-only replacements. Slider actions cannot target editors
  or buttons. Existing password withholding remains intact.
- Explicit GPU image sources are admitted before any registry upload. Unique
  per-frame source bytes must fit **96 MiB**, dimensions must fit the device and
  Vello's 8192 atlas side, and images must actually pack. Pixel area alone could
  admit incompatible full-width/full-height strips and silently lose one image.
- Admission uses the same pinned `guillotiere 0.6.2` allocator already present
  transitively through Vello. It adds **zero new locked packages**; the default
  toolkit still depends only on `unicode-width`. CPU-native does not enable it.
- Registry eviction now has a hard limit, not temporary over-allocation. All
  needed existing textures are protected up front to avoid eviction/re-upload
  churn when the next required image occurs later in draw order.
- A rejected frame has fresh fallback statistics, uploads no new image textures,
  and renders in software. Subsequent admitted frames can use the GPU normally.

These are raw registered image byte limits, **not total VRAM limits**. Framebuffer,
Vello atlas/buffers, driver allocations, fonts and application-owned CPU assets
are separate. This library does not sandbox application callbacks or filesystem
access. Assets are explicitly loaded by the host; existing PNG/JPEG decoding caps
remain 16 MiB encoded, 4096 per dimension and 128 MiB decoder allocation.

### Rendering correctness and performance

- Incremental repaint and GPU opacity-layer bounds include the default stroke's
  maximum miter reach plus anti-aliasing margin, for paths and batched lines.
  An acute 12px join no longer leaves 14 stale pixels after a colour change;
  regression tests retain partial repaint and compare it with a full repaint.
- GPU glyph opacity uses `brush_alpha` with an opaque foreground brush: bitmap
  emoji fades correctly and outline text is not attenuated twice.
- GPU clip elision requires actual outline-ink bounds, not advance width alone.
  Italic overhang is clipped; bitmap/colour faces without outline-only proof keep
  their clips. Normalized ink metadata is cached at at most 4096 face/glyph keys,
  avoiding repeated font-table parsing without rasterizing ordinary vector text.
- `ButtonVariant::Primary` now applies its paired `@primary-content` label colour,
  like `.primary(true)`, so inherited text colour cannot erase contrast.
- Opaque image upload staging borrows the existing pixel buffer instead of
  cloning it. Translucent image conversion remains byte-for-byte equivalent.
  In a local release helper measurement, a 2048x2048 opaque image (16 MiB),
  30 iterations, took **2.279ms -> 1.109ms** on average; the temporary staging copy
  fell from **16 MiB -> zero**. This measures CPU staging, not GPU upload time or
  whole-app latency. A unit test asserts borrowed buffer identity and alpha parity.

## Files, dropped paths and bundled fonts

- Drop dispatch revalidates mounting, enabled bindings and active modal scope;
  cached file-hover state cannot bypass a newly opened modal.
- Extension filtering is convenience, not file-content validation or authorization.
  Hosts decide whether/how to open supplied paths; no file is read automatically.
- Native text-file helpers bound UTF-8 to 8 KiB, refuse nonregular/symlink targets
  and use explicit creation/replacement policy. New-file fallback uses exclusive
  create with Unix mode 0600 when hard links are unavailable.
- These helpers are not a filesystem sandbox or multi-process transaction manager:
  checks and rename have documented races, ACL/xattrs are not retained, and parent
  directories are not fsynced. Linkless fallback can leave a partial file if the
  process dies during writing. See [document files](DOCUMENT_FILES.md).
- Font registration accepts at most 64 files, 64 MiB per file and 256 MiB total;
  immutable bytes are shared across windows. Register before creating windows.
- SVG path import accepts bounded path data (1 MiB / 100,000 segments), validates
  finite coordinates, and does not execute scripts or load external SVG assets.
- Local bundle tooling sanitizes executable names and escapes plist values; it
  passes argument vectors to native tools, not shell-expanded input. See
  [packaging](MACOS_PACKAGING.md).

## Dependency maintenance

Run `cargo audit` against the current advisory database before releases. The
latest local audit on 2026-10-12 reported zero known vulnerability advisories,
with informational unmaintained warnings for the existing optional stacks:

| Package | Dependency route | Official advisory |
| --- | --- | --- |
| `paste 1.0.15` | Metal -> wgpu-hal, optional GPU/macOS | [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) |
| `rustybuzz 0.14.1` | cosmic-text, optional native | [RUSTSEC-2026-0206](https://rustsec.org/advisories/RUSTSEC-2026-0206.html) |
| `ttf-parser 0.20.0/0.21.1/0.25.1` | native font stack | [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html) |

These advisories are **unmaintained**, not reported exploitable vulnerabilities;
they have no patched versions. They are not suppressed. Replacing/forking the
upstream native font/Metal stack needs a separate compatibility/MSRV/visual plan,
not a blind lockfile update. Audit results are time-sensitive and do not prove
unknown flaws absent; no fuzzing, penetration test or supply-chain attestation is
claimed by this batch.

## Validation and remaining risks

Run the [validation gates](VALIDATION.md), including actual GPU tests and native
acceptance. Timing/staging optimizations are described in [performance](PERFORMANCE.md);
these do not establish whole-app latency or CPU/GPU utilization guarantees.

Late-generated colour glyphs are not completely atlas-packing-preflighted; dense
mixed emoji/image scenes and conservative fixed-buffer estimates still need stress
coverage. Real device loss, sustained image churn, cross-platform runtime and
live assistive technology remain separate acceptance work. Native glass switching
and generic in-app backdrop blur retain their [documented limits](SURFACES_THEMES_AND_SCROLLING.md).

Private compositor blur remains opt-in and is not an App Store-safe distribution
claim. Avoid broad dependency updates that silently raise the MSRV or invalidate
surface-alpha/font/renderer contracts. Toolkit checks do not authorize a Volt
vendor sync or replace terminal visual/performance verification.
