# Inline PNG images

Inline PNG display is wired through core OSC events, a per-window off-thread
bounded decoder/store, and the renderer's shared-RGBA image API. Disabled by
default. Enable explicitly in config:

```toml
[inline_images]
enabled = true
```

## Actual cursor and layout behavior

Images overlay the terminal at the cursor position captured when the OSC is
parsed. They **do not advance the cursor, reserve cells, insert blank lines,
change wrapping, or prevent subsequent text from being covered**. Programs must
reserve their own space/move the cursor if needed. Explicit positive `width` and
`height` cell counts are supported, including non-aspect-preserving scaling.
`auto` uses natural pixel dimensions relative to physical cell size. Resolution
rounds cell pixel dimensions up; image spans round up to whole cells, so exact
aspect ratio is not guaranteed. Draws are clipped to the owning pane and scroll
with its retained-history anchor. Main and alternate screens are isolated.
Resize/reflow, RIS/reset, removed panes and recycled alternate screens discard
stale anchors; scrollback eviction removes fully evicted images. Alternate-screen/no-history scrolling and partial scroll
regions/insert-delete lines conservatively invalidate that screen's images rather
than attempting image-aware row mutations. Text erasure does not erase images: this is a strict overlay subset, not full iTerm2 compatibility.

At most one decoder runs per window, outside render and PTY locks. Incoming
requests while decoding are dropped, not queued. Core coalesces requests per
parse batch. One bounded completion slot prevents decoded-result backlogs.
Disabling during a decode discards its result and keeps the slot occupied until
completion. No image metadata, filename or payload is ever read from disk.

## Host flow (native window)

```text
PTY reader -> bounded parse queue -> fixed-capacity Parser
                                     |
                                     v
                           Performer OSC 1337 dispatch
                           captured pane screen/epoch + absolute row/column
                                     |
                                     v
                           CoreEvent::InlineImageRequested
                                     |
                           PTY event wake -> native winit window
                                     |
                       enabled + idle? -- no --> drop request
                                     |
                                    yes
                                     v
                       one background PNG decoder per window
                                     |
                       one completion slot + event-loop wake
                                     |
                       reconcile live panes/epochs/history
                       reject stale result -> bounded ImageStore
                                     |
                       active-tab visible images only
                       immutable Arc RGBA + physical rect/pane clip
                                     |
                       Renderer::set_inline_images -> atlas/draw
                       text < image < UI overlays
```

The window drains image events both during redraw and idle processing. Worker
completion wakes the event loop; installing a valid image requests a full redraw.
The empty host path returns before inspecting panes or constructing image packets.
Offscreen CPU images remain bounded in the store, while renderer packets/cache
are replaced with the currently visible set. This is source-verified integration,
not proof of native shell-to-window visual acceptance or full `imgcat` behavior.

## Accepted protocol

Input to `decode(&[u8])` is exactly the bytes after `1337;` and before BEL/ST:

```text
OSC 1337 ; File=inline=1;width=8;height=auto;preserveAspectRatio=1:BASE64 ST
           ^---------------------- decode input ---------------------^
```

- `File=` is case-sensitive. Semicolon-separated `key=value` headers precede the
  first colon. `inline=1` is mandatory. Order is flexible; duplicates, unknown
  keys, empty fields and whitespace are rejected.
- Optional `width`/`height`: `auto` or positive decimal **cell** counts, <=4096.
  Pixel/percentage suffixes are deliberately unsupported.
- `preserveAspectRatio=0|1`, default 1. Optional `size` must equal decoded PNG
  file byte count. Optional `name` must be nonempty canonical base64, <=1024
  encoded bytes; it is ignored metadata, NEVER a filesystem path.
- Canonical padded standard base64 only (no whitespace, URL alphabet, bad padding
  or nonzero unused bits). Payload bytes must be an actual PNG, not a path.
- Static, non-interlaced PNG only. RGB/RGBA/gray-alpha require 8-bit channels;
  grayscale/indexed accept 1/2/4/8-bit samples and are expanded to RGBA8.
  PLTE/tRNS and small uncompressed sRGB/gAMA/cHRM/pHYs chunks are allowed.
  All other chunks (including text, ICC profiles and APNG) are rejected. No
  16-bit PNG, JPEG, GIF, Kitty or sixel support is claimed.
- All PNG chunk boundaries and CRCs, IHDR, IEND, no trailing data, IDAT ordering
  and key palette/transparency ordering are preflight checked. The decoder then
  validates/inflates the image. Output is straight RGBA8 with no color management.

## Limits / allocation policy

The entire reconstructed header+base64 event and decoder input is <=64KiB.
All three workspace VTE dependencies disable default features (`std`), and
`volt-core::parser::Parser` wraps `vte::Parser<65552>::new_with_size()`.
OSC assembly therefore uses a fixed 65,552-byte buffer, including unterminated
input; it does not grow an OSC heap `Vec`. Performer rejects dispatch when the
sum of parameter byte lengths reaches that capacity, and image reconstruction
independently rejects header+base64 beyond 64KiB.

VTE also caps OSC parameters at 16. The raw-length sum alone cannot detect bytes
hidden after that parameter table fills; a fail-closed saturated-parameter guard
is required before claiming every truncated OSC is rejected. This does not undo
the fixed-memory bound. Base64 output
is <=48KiB. IHDR dimensions are checked before constructing the PNG decoder:
nonzero, <=1,000,000 pixels, <=4,000,000 bytes RGBA. No scanline buffer is allocated before
this check. PNG internal decoding has an 8MiB allocation limit; expanded frame
buffer and final RGBA buffer are independently bounded. Compressed metadata is
rejected before decoding. These are component limits, not an exact whole-process
RSS bound; transient decode memory includes encoded input, PNG bytes, decoder
working memory and up to two frame buffers. Decoder implementation allocation
failure is not guaranteed recoverable; explicit model allocations are fallible.

`ImageStore` caps live count at 32 and immutable RGBA allocation length at 16MiB. Custom limits
only tighten these ceilings. Oversized single insertions fail without evicting
existing entries. Valid insertions evict oldest first until both limits fit.
Metadata is bounded by count; GPU textures require their own host-side limits.
Decode serially/backpressure events; unbounded pending decoded images would
circumvent store limits.

## Public API / host contract

- `decode(parameters) -> Result<RgbaImage, Error>`; image accessors return pixel
  width/height, borrowed RGBA, shared `Arc<[u8]>` pixels and `RequestedCells`.
- `RgbaImage::cell_extent(cell_pixel_width, cell_pixel_height)` resolves auto /
  aspect fitting into `CellExtent`. Dimensions round up to cells; the renderer
  maps the image into the resulting cell rectangle (cell rounding can slightly
  alter aspect ratio). Both cell pixel dimensions must be positive.
- `ImageStore::{new, insert, get, visible, cull_before, remove_screen, clear}`;
  `insert` takes an owned image, `Anchor` and resolved `CellExtent`, returning a
  monotonic `ImageId`. IDs do not reset on clear; use one store per upload-ID
  namespace or qualify IDs with store identity.
- Anchors are `(screen epoch, absolute retained-history row, column)`, never a
  viewport-relative row. Host changes epoch on destructive reset, incompatible
  reflow/resize or alternate-screen recycling, and removes the old epoch. A
  normal viewport scroll changes neither epoch nor anchor. A single bounded store per window gives globally unique image IDs across panes.
- `visible(Viewport)` yields borrowed `StoredImage` plus clipped destination
  cell position/span and source cell offsets. Upload the full RGBA once per ID;
  source UV bounds are offsets divided by full extent. Offscreen images stay in
  storage until history culling or FIFO eviction. `cull_before` drops only images
  fully above the oldest retained row. History-boundary clipping is the host's
  viewport responsibility. Clip draw calls to the pane.
- Host drops GPU uploads whose IDs are no longer returned by `get`, including
  clear/screen removal/culling/eviction; no automatic GPU lifetime is implied.

PNG dependency: `png = "0.17"` (PNG-only; canonical base64 is implemented
locally). Unit tests cover malformed/unsupported input, bomb limits, pixel order,
shared pixel ownership, FIFO limits, clipping, stale completion, screen isolation
and decode backpressure. Core integration tests cover captured absolute anchors,
coalescing, payload bounds, cursor preservation, OSC52 coexistence and epochs.
Native terminal end-to-end visual acceptance remains a separate manual gate;
offscreen renderer checks do not prove shell/application interoperability.
