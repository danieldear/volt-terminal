//! Bounded, in-memory iTerm2 OSC 1337 `File=...;inline=1:BASE64` PNGs.
//! Host registration, terminal mutations and GPU uploads deliberately live elsewhere.
//! See `docs/inline-images.md` for the intentionally strict protocol subset.

use std::{collections::VecDeque, io::Cursor, sync::Arc};

pub const MAX_ENCODED_BYTES: usize = 64 * 1024;
pub const MAX_PIXELS: usize = 1_000_000;
pub const MAX_RGBA_BYTES: usize = MAX_PIXELS * 4;
pub const MAX_IMAGES: usize = 32;
pub const MAX_RESIDENT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CELL_SPAN: u32 = 4096;
const MAX_PNG_BYTES: usize = MAX_ENCODED_BYTES / 4 * 3;
const DECODER_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    EncodedLimit,
    Header,
    Base64,
    SizeMismatch,
    UnsupportedPng,
    PixelLimit,
    Decode,
    Allocation,
    Placement,
    StorageLimit,
    IdExhausted,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "inline image: {self:?}")
    }
}
impl std::error::Error for Error {}

/// `None` means protocol `auto` (or omitted). Explicit values are terminal cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestedCells {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub preserve_aspect_ratio: bool,
}

/// Straight (not premultiplied) RGBA8, top-to-bottom rows, no color conversion.
/// Private fields prevent callers constructing inconsistent pixel buffers.
#[derive(Debug, Clone)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
    requested: RequestedCells,
}

impl RgbaImage {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    pub fn pixels(&self) -> Arc<[u8]> {
        Arc::clone(&self.rgba)
    }
    pub fn requested_cells(&self) -> RequestedCells {
        self.requested
    }

    /// Resolve requests using positive physical cell pixel dimensions.
    /// Aspect-preserving images fit within two explicit dimensions; with one
    /// explicit dimension the other is derived. Spans round up to whole cells.
    /// This only describes presentation; it does not move the terminal cursor.
    pub fn cell_extent(&self, cell_width: u32, cell_height: u32) -> Result<CellExtent, Error> {
        if cell_width == 0 || cell_height == 0 {
            return Err(Error::Placement);
        }
        let natural_w = f64::from(self.width);
        let natural_h = f64::from(self.height);
        let req = self.requested;
        let w = req.width.map(|v| f64::from(v) * f64::from(cell_width));
        let h = req.height.map(|v| f64::from(v) * f64::from(cell_height));
        let (pixel_w, pixel_h) = if req.preserve_aspect_ratio {
            let scale = match (w, h) {
                (Some(w), Some(h)) => (w / natural_w).min(h / natural_h),
                (Some(w), None) => w / natural_w,
                (None, Some(h)) => h / natural_h,
                (None, None) => 1.0,
            };
            (natural_w * scale, natural_h * scale)
        } else {
            (w.unwrap_or(natural_w), h.unwrap_or(natural_h))
        };
        let columns = (pixel_w / f64::from(cell_width)).ceil();
        let rows = (pixel_h / f64::from(cell_height)).ceil();
        if !(1.0..=f64::from(MAX_CELL_SPAN)).contains(&columns)
            || !(1.0..=f64::from(MAX_CELL_SPAN)).contains(&rows)
        {
            return Err(Error::Placement);
        }
        Ok(CellExtent {
            columns: columns as u32,
            rows: rows as u32,
        })
    }
}

fn number(bytes: &[u8], max: usize) -> Result<usize, Error> {
    if bytes.is_empty() {
        return Err(Error::Header);
    }
    bytes.iter().try_fold(0usize, |n, &b| {
        if !b.is_ascii_digit() {
            return Err(Error::Header);
        }
        let n = n
            .checked_mul(10)
            .and_then(|n| n.checked_add(usize::from(b - b'0')))
            .ok_or(Error::Header)?;
        if n > max {
            Err(Error::Header)
        } else {
            Ok(n)
        }
    })
}

fn dimension(bytes: &[u8]) -> Result<Option<u32>, Error> {
    if bytes == b"auto" {
        return Ok(None);
    }
    let n = number(bytes, MAX_CELL_SPAN as usize)?;
    if n == 0 {
        return Err(Error::Header);
    }
    Ok(Some(n as u32))
}

fn headers(bytes: &[u8]) -> Result<(RequestedCells, Option<usize>), Error> {
    let bytes = bytes.strip_prefix(b"File=").ok_or(Error::Header)?;
    let mut seen = 0u8;
    let mut requested = RequestedCells {
        width: None,
        height: None,
        preserve_aspect_ratio: true,
    };
    let mut size = None;
    for field in bytes.split(|&b| b == b';') {
        let eq = field.iter().position(|&b| b == b'=').ok_or(Error::Header)?;
        let (key, value) = (&field[..eq], &field[eq + 1..]);
        let bit = match key {
            b"inline" => {
                if value != b"1" {
                    return Err(Error::Header);
                }
                1
            }
            b"width" => {
                requested.width = dimension(value)?;
                2
            }
            b"height" => {
                requested.height = dimension(value)?;
                4
            }
            b"preserveAspectRatio" => {
                requested.preserve_aspect_ratio = match value {
                    b"0" => false,
                    b"1" => true,
                    _ => return Err(Error::Header),
                };
                8
            }
            b"size" => {
                size = Some(number(value, MAX_PNG_BYTES)?);
                16
            }
            b"name" => {
                // Metadata only. Never used as a path or a filename.
                if value.len() > 1024 || value.is_empty() {
                    return Err(Error::Header);
                }
                let _ = base64(value)?;
                32
            }
            _ => return Err(Error::Header),
        };
        if seen & bit != 0 {
            return Err(Error::Header);
        }
        seen |= bit;
    }
    if seen & 1 == 0 {
        return Err(Error::Header);
    }
    Ok((requested, size))
}

fn sextet(b: u8) -> Result<u8, Error> {
    match b {
        b'A'..=b'Z' => Ok(b - b'A'),
        b'a'..=b'z' => Ok(b - b'a' + 26),
        b'0'..=b'9' => Ok(b - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(Error::Base64),
    }
}

/// Canonical standard alphabet, mandatory padding, no whitespace or URL alphabet.
fn base64(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    if bytes.len() > MAX_ENCODED_BYTES {
        return Err(Error::EncodedLimit);
    }
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return Err(Error::Base64);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(bytes.len() / 4 * 3)
        .map_err(|_| Error::Allocation)?;
    for (i, q) in bytes.chunks_exact(4).enumerate() {
        let a = sextet(q[0])?;
        let b = sextet(q[1])?;
        let last = i == bytes.len() / 4 - 1;
        out.push(a << 2 | b >> 4);
        if q[2] == b'=' {
            if !last || q[3] != b'=' || b & 15 != 0 {
                return Err(Error::Base64);
            }
        } else {
            let c = sextet(q[2])?;
            out.push(b << 4 | c >> 2);
            if q[3] == b'=' {
                if !last || c & 3 != 0 {
                    return Err(Error::Base64);
                }
            } else {
                out.push(c << 6 | sextet(q[3])?);
            }
        }
    }
    Ok(out)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Preflight BEFORE the PNG decoder can allocate any scanlines or inflate data.
/// Compressed ancillary metadata and APNG are excluded, preventing metadata bombs.
fn preflight(bytes: &[u8]) -> Result<(u32, u32, usize), Error> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(Error::UnsupportedPng);
    }
    let mut offset = 8usize;
    let mut dimensions = None;
    let mut idat = false;
    let mut idat_ended = false;
    let mut palette = false;
    let mut palette_entries = 0;
    let mut depth = 0;
    let mut transparency = false;
    let mut color = 0;
    while offset < bytes.len() {
        let chunk = bytes.get(offset..).ok_or(Error::Decode)?;
        if chunk.len() < 12 {
            return Err(Error::Decode);
        }
        let length = be32(chunk) as usize;
        let end = length.checked_add(12).ok_or(Error::Decode)?;
        if end > chunk.len() {
            return Err(Error::Decode);
        }
        let kind = &chunk[4..8];
        let data = &chunk[8..8 + length];
        if crc32(&chunk[4..8 + length]) != be32(&chunk[8 + length..end]) {
            return Err(Error::Decode);
        }
        if dimensions.is_none() && kind != b"IHDR" {
            return Err(Error::Decode);
        }
        if idat && kind != b"IDAT" {
            idat_ended = true;
        }
        match kind {
            b"IHDR" => {
                if dimensions.is_some() || length != 13 {
                    return Err(Error::Decode);
                }
                let (w, h) = (be32(data), be32(&data[4..]));
                let pixels = u64::from(w) * u64::from(h);
                if w == 0 || h == 0 || pixels > MAX_PIXELS as u64 {
                    return Err(Error::PixelLimit);
                }
                color = data[9];
                depth = data[8];
                let supported_depth = match color {
                    0 | 3 => matches!(data[8], 1 | 2 | 4 | 8),
                    2 | 4 | 6 => data[8] == 8,
                    _ => false,
                };
                if !supported_depth || data[10] != 0 || data[11] != 0 || data[12] != 0 {
                    return Err(Error::UnsupportedPng);
                }
                dimensions = Some((w, h, pixels as usize * 4));
            }
            b"PLTE" => {
                if palette
                    || idat
                    || transparency
                    || matches!(color, 0 | 4)
                    || length == 0
                    || length > 768
                    || length % 3 != 0
                {
                    return Err(Error::Decode);
                }
                palette = true;
                palette_entries = length / 3;
                if color == 3 && palette_entries > (1usize << depth) {
                    return Err(Error::Decode);
                }
            }
            b"tRNS" => {
                if transparency || idat {
                    return Err(Error::Decode);
                }
                let valid = match color {
                    0 => length == 2,
                    2 => length == 6,
                    3 => palette && length > 0 && length <= palette_entries,
                    _ => false,
                };
                if !valid {
                    return Err(Error::Decode);
                }
                transparency = true;
            }
            b"IDAT" => {
                if idat_ended || (color == 3 && !palette) {
                    return Err(Error::Decode);
                }
                idat = true;
            }
            b"IEND" => {
                if length != 0 || !idat || offset + end != bytes.len() {
                    return Err(Error::Decode);
                }
                return dimensions.ok_or(Error::Decode);
            }
            // Small, uncompressed metadata only; no profiles/text/external references.
            b"sRGB" if length == 1 && !idat => {}
            b"gAMA" if length == 4 && !idat => {}
            b"cHRM" if length == 32 && !idat => {}
            b"pHYs" if length == 9 && !idat => {}
            _ => return Err(Error::UnsupportedPng),
        }
        offset += end;
    }
    Err(Error::Decode)
}

/// Input is the bytes AFTER `1337;`, without OSC prefix or BEL/ST terminator.
/// The entire parameters+payload input must fit the event's 64 KiB bound.
/// No disk, network, filesystem names or other graphics protocols are consulted.
pub fn decode(parameters: &[u8]) -> Result<RgbaImage, Error> {
    if parameters.len() > MAX_ENCODED_BYTES {
        return Err(Error::EncodedLimit);
    }
    let colon = parameters
        .iter()
        .position(|&b| b == b':')
        .ok_or(Error::Header)?;
    let (requested, declared_size) = headers(&parameters[..colon])?;
    let bytes = base64(&parameters[colon + 1..])?;
    if declared_size.is_some_and(|size| size != bytes.len()) {
        return Err(Error::SizeMismatch);
    }
    let (width, height, rgba_bytes) = preflight(&bytes)?;
    let mut decoder = png::Decoder::new(Cursor::new(&bytes));
    decoder.set_limits(png::Limits {
        bytes: DECODER_BYTES,
    });
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|_| Error::Decode)?;
    let buffer_size = reader.output_buffer_size();
    if buffer_size > rgba_bytes || rgba_bytes > MAX_RGBA_BYTES {
        return Err(Error::PixelLimit);
    }
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(buffer_size)
        .map_err(|_| Error::Allocation)?;
    frame.resize(buffer_size, 0);
    let info = reader.next_frame(&mut frame).map_err(|_| Error::Decode)?;
    if info.width != width || info.height != height || info.bit_depth != png::BitDepth::Eight {
        return Err(Error::Decode);
    }
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return Err(Error::Decode),
    };
    let pixels = rgba_bytes / 4;
    if info.buffer_size() != pixels * channels {
        return Err(Error::Decode);
    }
    let rgba = if channels == 4 {
        frame
    } else {
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(rgba_bytes)
            .map_err(|_| Error::Allocation)?;
        for pixel in frame[..info.buffer_size()].chunks_exact(channels) {
            match channels {
                1 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 255]),
                2 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]),
                3 => rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]),
                _ => unreachable!(),
            }
        }
        rgba
    };
    Ok(RgbaImage {
        width,
        height,
        rgba: rgba.into(),
        requested,
    })
}

/// Host-issued screen/epoch identity. Change on reset/reflow/alternate-screen reuse.
/// Absolute retained-history rows MUST NOT be viewport-relative or recycled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub screen: u64,
    pub row: u64,
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellExtent {
    pub columns: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageId(pub u64);

#[derive(Debug)]
pub struct StoredImage {
    id: ImageId,
    anchor: Anchor,
    extent: CellExtent,
    image: RgbaImage,
}

impl StoredImage {
    pub fn id(&self) -> ImageId {
        self.id
    }
    pub fn anchor(&self) -> Anchor {
        self.anchor
    }
    pub fn extent(&self) -> CellExtent {
        self.extent
    }
    pub fn image(&self) -> &RgbaImage {
        &self.image
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub screen: u64,
    pub top_row: u64,
    pub left_column: u32,
    pub columns: u32,
    pub rows: u32,
}

/// Clipped cell rectangle. Source offsets are relative to the full image extent;
/// renderer UVs are offset/extent and (offset+span)/extent. This avoids uploading
/// duplicate cropped buffers and preserves the original anchor across scrolling.
#[derive(Debug)]
pub struct VisibleImage<'a> {
    pub stored: &'a StoredImage,
    pub viewport_column: u32,
    pub viewport_row: u32,
    pub source_column: u32,
    pub source_row: u32,
    pub columns: u32,
    pub rows: u32,
}

/// FIFO eviction (not access-order LRU). Immutable RGBA allocation length is charged.
/// Shared renderer uploads may temporarily retain an evicted image until next sync.
/// GPU residency is separately owned/bounded by the host renderer.
#[derive(Debug)]
pub struct ImageStore {
    images: VecDeque<StoredImage>,
    resident: usize,
    max_count: usize,
    max_bytes: usize,
    next_id: u64,
}

impl Default for ImageStore {
    fn default() -> Self {
        Self::new(MAX_IMAGES, MAX_RESIDENT_BYTES)
    }
}

impl ImageStore {
    /// Custom limits can only tighten the hard caps. Zero disables storage.
    pub fn new(max_count: usize, max_bytes: usize) -> Self {
        Self {
            images: VecDeque::new(),
            resident: 0,
            max_count: max_count.min(MAX_IMAGES),
            max_bytes: max_bytes.min(MAX_RESIDENT_BYTES),
            next_id: 1,
        }
    }
    pub fn len(&self) -> usize {
        self.images.len()
    }
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
    pub fn resident_bytes(&self) -> usize {
        self.resident
    }
    pub fn get(&self, id: ImageId) -> Option<&StoredImage> {
        self.images.iter().find(|image| image.id == id)
    }
    pub fn insert(
        &mut self,
        image: RgbaImage,
        anchor: Anchor,
        extent: CellExtent,
    ) -> Result<ImageId, Error> {
        if extent.columns == 0
            || extent.rows == 0
            || extent.columns > MAX_CELL_SPAN
            || extent.rows > MAX_CELL_SPAN
            || anchor.row.checked_add(u64::from(extent.rows)).is_none()
            || anchor.column.checked_add(extent.columns).is_none()
        {
            return Err(Error::Placement);
        }
        let bytes = image.rgba.len();
        if self.max_count == 0 || bytes > self.max_bytes {
            return Err(Error::StorageLimit);
        }
        let next = self.next_id.checked_add(1).ok_or(Error::IdExhausted)?;
        // Reserve before eviction so allocation failures leave existing entries intact.
        self.images.try_reserve(1).map_err(|_| Error::Allocation)?;
        while self.images.len() >= self.max_count || self.resident > self.max_bytes - bytes {
            let old = self
                .images
                .pop_front()
                .expect("nonempty under eviction conditions");
            self.resident -= old.image.rgba.len();
        }
        let id = ImageId(self.next_id);
        self.next_id = next;
        self.resident += bytes;
        self.images.push_back(StoredImage {
            id,
            anchor,
            extent,
            image,
        });
        Ok(id)
    }

    /// Cull only images fully above the retained-history boundary on this screen.
    /// Partially retained images remain and are clipped by `visible`.
    pub fn cull_before(&mut self, screen: u64, oldest_row: u64) {
        self.images.retain(|image| {
            image.anchor.screen != screen
                || image.anchor.row + u64::from(image.extent.rows) > oldest_row
        });
        self.recount();
    }
    pub fn remove_screen(&mut self, screen: u64) {
        self.images.retain(|image| image.anchor.screen != screen);
        self.recount();
    }
    pub fn clear(&mut self) {
        self.images.clear();
        self.resident = 0;
        // IDs are never reused, including across clear/reset.
    }
    fn recount(&mut self) {
        self.resident = self.images.iter().map(|image| image.image.rgba.len()).sum();
    }
    pub fn visible(&self, viewport: Viewport) -> impl Iterator<Item = VisibleImage<'_>> {
        self.images.iter().filter_map(move |stored| {
            if stored.anchor.screen != viewport.screen {
                return None;
            }
            let left = u64::from(stored.anchor.column).max(u64::from(viewport.left_column));
            let right = (u64::from(stored.anchor.column) + u64::from(stored.extent.columns))
                .min(u64::from(viewport.left_column) + u64::from(viewport.columns));
            let top = stored.anchor.row.max(viewport.top_row);
            let bottom = (stored.anchor.row + u64::from(stored.extent.rows))
                .min(viewport.top_row.saturating_add(u64::from(viewport.rows)));
            if left >= right || top >= bottom {
                return None;
            }
            Some(VisibleImage {
                stored,
                viewport_column: (left - u64::from(viewport.left_column)) as u32,
                viewport_row: (top - viewport.top_row) as u32,
                source_column: (left - u64::from(stored.anchor.column)) as u32,
                source_row: (top - stored.anchor.row) as u32,
                columns: (right - left) as u32,
                rows: (bottom - top) as u32,
            })
        })
    }
}

/// Per-window host state. There is no pending queue: requests while decoding
/// are dropped, and the single completion channel cannot retain a backlog.
#[derive(Default)]
pub(crate) struct Host {
    pub store: ImageStore,
    screens: std::collections::HashMap<(usize, bool, u64), u64>,
    next_screen: u64,
    // Retain identity allocations while anchored images exist, preventing
    // pointer reuse if a pane is removed/replaced before the next reconciliation.
    owners:
        std::collections::HashMap<usize, Arc<std::sync::Mutex<volt_core::performer::Performer>>>,
    completion: Option<std::sync::mpsc::Receiver<(Anchor, Result<RgbaImage, Error>)>>,
}

impl Host {
    pub fn idle(&self) -> bool {
        self.completion.is_none()
    }
    pub fn empty(&self) -> bool {
        self.store.is_empty() && self.completion.is_none() && self.screens.is_empty()
    }
    pub fn screen(&self, key: (usize, bool, u64)) -> Option<u64> {
        self.screens.get(&key).copied()
    }
    pub fn request(
        &mut self,
        owner: Arc<std::sync::Mutex<volt_core::performer::Performer>>,
        alternate_screen: bool,
        epoch: u64,
        row: u64,
        column: u32,
        parameters: Vec<u8>,
        wake: impl FnOnce() + Send + 'static,
    ) {
        if !self.idle() || parameters.len() > MAX_ENCODED_BYTES {
            return;
        }
        let owner_id = Arc::as_ptr(&owner) as usize;
        let key = (owner_id, alternate_screen, epoch);
        self.owners.insert(owner_id, owner);
        let screen = match self.screen(key) {
            Some(screen) => screen,
            None => {
                let Some(next) = self.next_screen.checked_add(1) else {
                    return;
                };
                self.next_screen = next;
                self.screens.insert(key, next);
                next
            }
        };
        let anchor = Anchor {
            screen,
            row,
            column,
        };
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("volt-inline-png".into())
            .spawn(move || {
                let _ = tx.send((anchor, decode(&parameters)));
                wake();
            }) {
            Ok(_) => self.completion = Some(rx),
            Err(_) => { /* failed spawning: drop request, never block the UI */ }
        }
    }
    pub fn reconcile(&mut self, live: &std::collections::HashMap<(usize, bool, u64), u64>) {
        self.screens.retain(|key, screen| {
            if let Some(oldest) = live.get(key) {
                self.store.cull_before(*screen, *oldest);
                true
            } else {
                self.store.remove_screen(*screen);
                false
            }
        });
        if self.store.is_empty() && self.idle() {
            self.screens.clear();
        }
        self.owners
            .retain(|owner, _| self.screens.keys().any(|key| key.0 == *owner));
    }
    pub fn poll(&mut self, cell_width: u32, cell_height: u32, enabled: bool) -> bool {
        let Some(rx) = &self.completion else {
            return false;
        };
        match rx.try_recv() {
            Ok((anchor, result)) => {
                self.completion = None;
                if enabled && self.screens.values().any(|&screen| screen == anchor.screen) {
                    if let Ok(image) = result {
                        if let Ok(extent) = image.cell_extent(cell_width, cell_height) {
                            return self.store.insert(image, anchor, extent).is_ok();
                        }
                    }
                }
                false
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.completion = None;
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
        }
    }
    pub fn disable(&mut self) {
        self.store.clear();
        self.screens.clear();
        self.owners.clear();
        // Keep the in-flight receiver until completion so re-enabling cannot
        // start a second concurrent decoder.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode64(bytes: &[u8]) -> Vec<u8> {
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        for q in bytes.chunks(3) {
            let a = q[0];
            let b = q.get(1).copied().unwrap_or(0);
            let c = q.get(2).copied().unwrap_or(0);
            out.extend_from_slice(&[
                alphabet[(a >> 2) as usize],
                alphabet[((a & 3) << 4 | b >> 4) as usize],
                if q.len() > 1 {
                    alphabet[((b & 15) << 2 | c >> 6) as usize]
                } else {
                    b'='
                },
                if q.len() > 2 {
                    alphabet[(c & 63) as usize]
                } else {
                    b'='
                },
            ]);
        }
        out
    }
    fn png_bytes(color: png::ColorType, pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(pixels).unwrap();
        }
        bytes
    }
    fn parameters(header: &[u8], bytes: &[u8]) -> Vec<u8> {
        let mut input = header.to_vec();
        input.push(b':');
        input.extend_from_slice(&encode64(bytes));
        input
    }
    fn tiny() -> RgbaImage {
        decode(&parameters(
            b"File=inline=1",
            &png_bytes(png::ColorType::Rgba, &[1, 2, 3, 4], 1, 1),
        ))
        .unwrap()
    }
    fn anchor(row: u64) -> Anchor {
        Anchor {
            screen: 7,
            row,
            column: 2,
        }
    }
    fn extent() -> CellExtent {
        CellExtent {
            columns: 3,
            rows: 4,
        }
    }

    #[test]
    fn tiny_png_and_pixel_formats() {
        for (color, pixels, rgba) in [
            (png::ColorType::Rgba, vec![1, 2, 3, 4], vec![1, 2, 3, 4]),
            (png::ColorType::Rgb, vec![1, 2, 3], vec![1, 2, 3, 255]),
            (png::ColorType::Grayscale, vec![9], vec![9, 9, 9, 255]),
            (
                png::ColorType::GrayscaleAlpha,
                vec![9, 10],
                vec![9, 9, 9, 10],
            ),
        ] {
            let bytes = png_bytes(color, &pixels, 1, 1);
            let header = format!(
                "File=name=dGlueS5wbmc=;size={};width=2;height=3;preserveAspectRatio=0;inline=1",
                bytes.len()
            );
            let image = decode(&parameters(header.as_bytes(), &bytes)).unwrap();
            assert_eq!((image.width(), image.height()), (1, 1));
            assert_eq!(image.rgba(), rgba);
            assert_eq!(
                image.requested_cells(),
                RequestedCells {
                    width: Some(2),
                    height: Some(3),
                    preserve_aspect_ratio: false
                }
            );
            assert_eq!(
                image.cell_extent(8, 16).unwrap(),
                CellExtent {
                    columns: 2,
                    rows: 3
                }
            );
        }
    }
    #[test]
    fn malformed_headers() {
        for header in [
            "File=",
            "File=inline=0",
            "File=width=1",
            "File=inline=1;inline=1",
            "File=inline=1;",
            "File=inline=1;width=0",
            "File=inline=1;width=2px",
            "File=inline=1;height=50%",
            "File=inline=1;unknown=1",
            "File=inline=1;name=***=",
            "File=inline=1;width=4097",
            "File=inline=1;preserveAspectRatio=2",
            "File=inline=1;size=999999999999999999999",
            "File=inline=1;width=+1",
            "file=inline=1",
            "File=inline=1;name=",
            "File=inline=1;width= 1",
        ] {
            assert!(
                decode(&parameters(header.as_bytes(), b"x")).is_err(),
                "{header}"
            );
        }
    }
    #[test]
    fn malformed_payload_and_encoded_bombs() {
        for payload in [
            "", "A", "AAAAA", "AA=A", "AB==", "AAB=", "AA==AAAA", "AA\n=", "____",
        ] {
            let input = format!("File=inline=1:{payload}");
            assert_eq!(
                decode(input.as_bytes()).unwrap_err(),
                Error::Base64,
                "{payload}"
            );
        }
        assert_eq!(
            decode(&vec![b'A'; MAX_ENCODED_BYTES + 1]).unwrap_err(),
            Error::EncodedLimit
        );
        assert!(decode(&parameters(b"File=inline=1", b"not a png")).is_err());
        let bytes = png_bytes(png::ColorType::Rgba, &[0; 4], 1, 1);
        assert_eq!(
            decode(&parameters(b"File=inline=1;size=1", &bytes)).unwrap_err(),
            Error::SizeMismatch
        );
        let mut truncated = bytes.clone();
        truncated.pop();
        assert!(decode(&parameters(b"File=inline=1", &truncated)).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode(&parameters(b"File=inline=1", &trailing)).is_err());
        let mut corrupt = bytes;
        corrupt[29] ^= 1;
        assert_eq!(
            decode(&parameters(b"File=inline=1", &corrupt)).unwrap_err(),
            Error::Decode
        );
    }
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        out.extend_from_slice(&crc32(&out[4..]).to_be_bytes());
        out
    }
    #[test]
    fn ihdr_pixel_bombs_are_rejected_before_inflation() {
        for (w, h) in [(0u32, 1u32), (1025, 1024), (u32::MAX, u32::MAX)] {
            let mut ihdr = Vec::new();
            ihdr.extend_from_slice(&w.to_be_bytes());
            ihdr.extend_from_slice(&h.to_be_bytes());
            ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
            let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
            bytes.extend(chunk(b"IHDR", &ihdr));
            assert_eq!(
                decode(&parameters(b"File=inline=1", &bytes)).unwrap_err(),
                Error::PixelLimit
            );
        }
        let bytes = png_bytes(png::ColorType::Rgba, &[0; 4], 1, 1);
        for kind in [b"zTXt", b"iCCP", b"acTL"] {
            let mut bomb = bytes[..33].to_vec();
            bomb.extend(chunk(kind, &[0; 10]));
            bomb.extend_from_slice(&bytes[33..]);
            assert_eq!(
                decode(&parameters(b"File=inline=1", &bomb)).unwrap_err(),
                Error::UnsupportedPng
            );
        }
    }
    #[test]
    fn exact_pixel_bound_and_dimension_math() {
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1000u32.to_be_bytes());
        ihdr.extend_from_slice(&1000u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend(chunk(b"IHDR", &ihdr));
        bytes.extend(chunk(b"IDAT", &[]));
        bytes.extend(chunk(b"IEND", &[]));
        assert_eq!(preflight(&bytes).unwrap(), (1000, 1000, MAX_RGBA_BYTES));
        let mut image = tiny();
        assert_eq!(
            image.cell_extent(8, 16).unwrap(),
            CellExtent {
                columns: 1,
                rows: 1
            }
        );
        image.requested.width = Some(2);
        assert_eq!(
            image.cell_extent(8, 16).unwrap(),
            CellExtent {
                columns: 2,
                rows: 1
            }
        );
        image.requested.height = Some(3);
        assert_eq!(
            image.cell_extent(8, 16).unwrap(),
            CellExtent {
                columns: 2,
                rows: 1
            }
        );
        assert_eq!(image.cell_extent(0, 16), Err(Error::Placement));
        image.requested.width = Some(MAX_CELL_SPAN);
        image.requested.height = None;
        assert_eq!(image.cell_extent(u32::MAX, 1), Err(Error::Placement));
    }
    #[test]
    fn decoded_rgba_byte_bound_and_pixel_order() {
        let pixels = vec![0; MAX_RGBA_BYTES];
        let bytes = png_bytes(png::ColorType::Rgba, &pixels, 1000, 1000);
        let image = decode(&parameters(b"File=inline=1", &bytes)).unwrap();
        assert_eq!(image.rgba().len(), MAX_RGBA_BYTES);
        assert_eq!((image.width(), image.height()), (1000, 1000));
        let bytes = png_bytes(png::ColorType::Rgba, &[1, 2, 3, 4, 5, 6, 7, 8], 2, 1);
        let image = decode(&parameters(b"File=inline=1", &bytes)).unwrap();
        assert_eq!(image.rgba(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }
    #[test]
    fn hard_count_cap_and_id_exhaustion() {
        let mut store = ImageStore::new(usize::MAX, usize::MAX);
        for row in 0..=MAX_IMAGES as u64 {
            store.insert(tiny(), anchor(row), extent()).unwrap();
        }
        assert_eq!(store.len(), MAX_IMAGES);
        assert_eq!(store.resident_bytes(), MAX_IMAGES * 4);
        store.next_id = u64::MAX;
        assert_eq!(
            store.insert(tiny(), anchor(0), extent()),
            Err(Error::IdExhausted)
        );
        assert_eq!(store.len(), MAX_IMAGES);
    }
    #[test]
    fn storage_count_bytes_and_atomic_rejection() {
        let mut store = ImageStore::new(2, 8);
        let first = store.insert(tiny(), anchor(10), extent()).unwrap();
        store.insert(tiny(), anchor(11), extent()).unwrap();
        assert_eq!(store.resident_bytes(), 8);
        let third = store.insert(tiny(), anchor(12), extent()).unwrap();
        assert_eq!(store.len(), 2);
        assert!(store.get(first).is_none());
        assert!(store.get(third).is_some());
        assert_eq!(
            store.insert(tiny(), anchor(u64::MAX), extent()),
            Err(Error::Placement)
        );
        assert_eq!(store.len(), 2);
        let mut bytes = ImageStore::new(32, 4);
        let id = bytes.insert(tiny(), anchor(0), extent()).unwrap();
        bytes.insert(tiny(), anchor(1), extent()).unwrap();
        assert!(bytes.get(id).is_none());
        assert_eq!(bytes.resident_bytes(), 4);
        assert_eq!(
            ImageStore::new(0, 4).insert(tiny(), anchor(0), extent()),
            Err(Error::StorageLimit)
        );
        assert_eq!(
            ImageStore::new(1, 3).insert(tiny(), anchor(0), extent()),
            Err(Error::StorageLimit)
        );
        store.clear();
        assert_eq!(store.resident_bytes(), 0);
        assert!(store.insert(tiny(), anchor(0), extent()).unwrap().0 > third.0);
    }
    #[test]
    fn stable_anchors_clip_cull_and_screen_isolation() {
        let mut store = ImageStore::default();
        let id = store.insert(tiny(), anchor(100), extent()).unwrap();
        let viewport = Viewport {
            screen: 7,
            top_row: 102,
            left_column: 3,
            columns: 1,
            rows: 2,
        };
        let visible = store.visible(viewport).next().unwrap();
        assert_eq!((visible.viewport_row, visible.viewport_column), (0, 0));
        assert_eq!(
            (
                visible.source_row,
                visible.source_column,
                visible.rows,
                visible.columns
            ),
            (2, 1, 2, 1)
        );
        assert_eq!(store.get(id).unwrap().anchor(), anchor(100));
        assert_eq!(
            store
                .visible(Viewport {
                    screen: 8,
                    ..viewport
                })
                .count(),
            0
        );
        assert_eq!(
            store
                .visible(Viewport {
                    rows: 0,
                    ..viewport
                })
                .count(),
            0
        );
        store.cull_before(8, 104);
        assert_eq!(store.len(), 1);
        store.cull_before(7, 103);
        assert_eq!(store.len(), 1);
        store.cull_before(7, 104);
        assert!(store.is_empty());
        assert_eq!(store.resident_bytes(), 0);
        store.insert(tiny(), anchor(0), extent()).unwrap();
        store.remove_screen(7);
        assert!(store.is_empty());
    }
    #[test]
    fn rgba_uploads_share_one_immutable_allocation() {
        let image = tiny();
        assert!(Arc::ptr_eq(&image.pixels(), &image.pixels()));
        let clone = image.clone();
        assert!(Arc::ptr_eq(&image.pixels(), &clone.pixels()));
    }

    #[test]
    fn host_serializes_decode_and_drops_stale_completions() {
        let owner = Arc::new(std::sync::Mutex::new(volt_core::performer::Performer::new(
            80, 24,
        )));
        let owner_id = Arc::as_ptr(&owner) as usize;
        let payload = parameters(
            b"File=inline=1;width=2;height=3;preserveAspectRatio=0",
            &png_bytes(png::ColorType::Rgba, &[1, 2, 3, 255], 1, 1),
        );
        let mut host = Host::default();
        let (wake_tx, wake_rx) = std::sync::mpsc::channel();
        host.request(owner.clone(), false, 1, 12, 3, payload.clone(), move || {
            wake_tx.send(()).unwrap();
        });
        assert!(!host.idle());
        host.request(owner.clone(), true, 1, 0, 0, payload.clone(), || {
            panic!("second decode started")
        });
        assert_eq!(host.screens.len(), 1);
        wake_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let live = std::collections::HashMap::from([((owner_id, false, 1), 0)]);
        host.reconcile(&live);
        assert!(host.poll(8, 16, true));
        assert!(host.idle());
        assert_eq!(host.store.len(), 1);
        let image = host.store.images.front().unwrap();
        assert_eq!((image.anchor.row, image.anchor.column), (12, 3));
        assert_eq!(
            image.extent,
            CellExtent {
                columns: 2,
                rows: 3
            }
        );
        host.reconcile(&std::collections::HashMap::from([(
            (owner_id, false, 1),
            15,
        )]));
        assert!(host.store.is_empty());
        let (wake_tx, wake_rx) = std::sync::mpsc::channel();
        host.request(owner, false, 1, 0, 0, payload, move || {
            wake_tx.send(()).unwrap();
        });
        host.reconcile(&std::collections::HashMap::new()); // removed pane/reset
        wake_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(!host.poll(8, 16, true));
        assert!(host.store.is_empty());
    }

    #[test]
    fn disabling_does_not_release_decode_slot_early() {
        let mut host = Host::default();
        let (_tx, rx) = std::sync::mpsc::sync_channel(1);
        host.completion = Some(rx);
        host.disable();
        assert!(!host.idle());
        assert!(!host.poll(8, 16, false));
        drop(_tx);
        assert!(!host.poll(8, 16, false));
        assert!(host.idle());
    }
    #[test]
    fn empty_host_fast_path_has_no_screens_pixels_or_decoder() {
        let mut host = Host::default();
        assert!(host.empty());
        assert!(host.idle());
        assert_eq!(host.store.resident_bytes(), 0);
        assert!(host.owners.is_empty());
        assert!(host.screens.is_empty());
        // MainState returns immediately on this gate without pane locks,
        // packet construction, decoding or renderer image calls.
        host.disable();
        assert!(host.empty());
        let owner = Arc::new(std::sync::Mutex::new(volt_core::performer::Performer::new(
            8, 2,
        )));
        host.request(
            owner,
            false,
            1,
            0,
            0,
            vec![0; MAX_ENCODED_BYTES + 1],
            || panic!("decoder started"),
        );
        assert!(host.empty());
    }
}
