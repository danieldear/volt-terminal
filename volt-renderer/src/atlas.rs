use cosmic_text::{CacheKey, FontSystem, SwashCache, SwashContent};
use std::collections::HashMap;

/// UV region in the atlas texture (normalised 0.0–1.0), plus placement info
#[derive(Debug, Clone, Copy)]
pub struct AtlasRegion {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub width: u32,
    pub height: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub is_color: bool,
}

/// CPU-side glyph atlas: RGBA bitmap, shelf packer
pub struct CpuAtlas {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGBA: white RGB for monochrome coverage, actual RGB for emoji
    cache: HashMap<CacheKey, Option<AtlasRegion>>,
    shelf_x: u32,
    shelf_y: u32,
    shelf_h: u32,
    pub dirty: bool,
    dirty_rect: Option<(u32, u32, u32, u32)>, // x, y, right, bottom
    symbol_cache: HashMap<(char, u32, u32), AtlasRegion>,
    pub full: bool,
}

impl CpuAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0u8; (width * height * 4) as usize],
            cache: HashMap::new(),
            shelf_x: 0,
            shelf_y: 0,
            shelf_h: 0,
            dirty: false,
            dirty_rect: None,
            symbol_cache: HashMap::new(),
            full: false,
        }
    }

    /// Shelf allocation is transactional: an oversized glyph must not corrupt
    /// the shelf or write past the bitmap. Exhaustion is retried on a new frame.
    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w == 0 || h == 0 || w > self.width || h > self.height {
            return None;
        }
        let (x, y, shelf_h) = if self.shelf_x + w > self.width {
            (0, self.shelf_y + self.shelf_h + 1, 0)
        } else {
            (self.shelf_x, self.shelf_y, self.shelf_h)
        };
        if y + h > self.height {
            self.full = true;
            return None;
        }
        self.shelf_x = x + w + 1;
        self.shelf_y = y;
        self.shelf_h = shelf_h.max(h);
        Some((x, y))
    }

    fn mark_dirty(&mut self, x: u32, y: u32, w: u32, h: u32) {
        self.dirty = true;
        self.dirty_rect = Some(match self.dirty_rect {
            Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x + w), d.max(y + h)),
            None => (x, y, x + w, y + h),
        });
    }

    pub fn take_dirty_rect(&mut self) -> Option<(u32, u32, u32, u32)> {
        self.dirty = false;
        self.dirty_rect
            .take()
            .map(|(x, y, r, b)| (x, y, r - x, b - y))
    }

    pub fn get_cell_symbol(&mut self, c: char, width: u32, height: u32) -> Option<AtlasRegion> {
        if let Some(&region) = self.symbol_cache.get(&(c, width, height)) {
            return Some(region);
        }
        let (x, y) = self.allocate(width, height)?;
        let mask = crate::symbols::rasterize(c, width, height);
        for row in 0..height {
            for col in 0..width {
                let dst = (((y + row) * self.width + x + col) * 4) as usize;
                self.data[dst..dst + 4].copy_from_slice(&[
                    255,
                    255,
                    255,
                    mask[(row * width + col) as usize],
                ]);
            }
        }
        let region = AtlasRegion {
            u0: x as f32 / self.width as f32,
            v0: y as f32 / self.height as f32,
            u1: (x + width) as f32 / self.width as f32,
            v1: (y + height) as f32 / self.height as f32,
            width,
            height,
            offset_x: 0,
            offset_y: 0,
            is_color: false,
        };
        self.mark_dirty(x, y, width, height);
        self.symbol_cache.insert((c, width, height), region);
        Some(region)
    }

    /// Get or rasterize a glyph. Returns `None` if the glyph has no visible pixels
    /// (e.g. space) or if the atlas is full.
    pub fn get_or_rasterize(
        &mut self,
        key: CacheKey,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
    ) -> Option<AtlasRegion> {
        if let Some(&region) = self.cache.get(&key) {
            return region;
        }

        let image = swash_cache.get_image_uncached(font_system, key)?;

        let w = image.placement.width;
        let h = image.placement.height;

        if w == 0 || h == 0 {
            self.cache.insert(key, None);
            return None;
        }

        let (x, y) = self.allocate(w, h)?;
        // Preserve color glyphs rather than reducing emoji to silhouettes.
        match image.content {
            SwashContent::Mask => {
                for row in 0..h {
                    for col in 0..w {
                        let src = image.data[(row * w + col) as usize];
                        let dst = (((y + row) * self.width + x + col) * 4) as usize;
                        self.data[dst..dst + 4].copy_from_slice(&[255, 255, 255, src]);
                    }
                }
            }
            SwashContent::Color => {
                // RGBA — retain intrinsic color, including alpha.
                for row in 0..h {
                    for col in 0..w {
                        let src = ((row * w + col) * 4) as usize;
                        let dst = (((y + row) * self.width + x + col) * 4) as usize;
                        self.data[dst..dst + 4].copy_from_slice(&image.data[src..src + 4]);
                    }
                }
            }
            SwashContent::SubpixelMask => {
                // Use the strongest sub-channel coverage to avoid thinning
                // edges when collapsing LCD subpixel masks to grayscale alpha.
                for row in 0..h {
                    for col in 0..w {
                        let base = ((row * w + col) * 3) as usize;
                        let coverage = image.data[base]
                            .max(image.data[base + 1])
                            .max(image.data[base + 2]);
                        let dst = (((y + row) * self.width + x + col) * 4) as usize;
                        self.data[dst..dst + 4].copy_from_slice(&[255, 255, 255, coverage]);
                    }
                }
            }
        }

        let region = AtlasRegion {
            u0: x as f32 / self.width as f32,
            v0: y as f32 / self.height as f32,
            u1: (x + w) as f32 / self.width as f32,
            v1: (y + h) as f32 / self.height as f32,
            width: w,
            height: h,
            offset_x: image.placement.left,
            offset_y: image.placement.top,
            is_color: image.content == SwashContent::Color,
        };

        self.mark_dirty(x, y, w, h);
        self.cache.insert(key, Some(region));
        Some(region)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};

    #[test]
    fn oversized_symbol_cannot_corrupt_the_atlas() {
        let mut atlas = CpuAtlas::new(16, 16);
        assert!(atlas.get_cell_symbol('\u{2588}', 17, 4).is_none());
        assert!(atlas.data.iter().all(|&v| v == 0));
        let r = atlas.get_cell_symbol('\u{2588}', 4, 4).unwrap();
        assert_eq!((r.u0, r.v0), (0.0, 0.0));
        assert_eq!(atlas.take_dirty_rect(), Some((0, 0, 4, 4)));
        assert_eq!(atlas.take_dirty_rect(), None);
        atlas.get_cell_symbol('\u{2588}', 4, 4).unwrap();
        assert_eq!(atlas.take_dirty_rect(), None);
    }

    #[test]
    fn dirty_upload_is_bounded_and_full_atlas_is_reported() {
        let mut atlas = CpuAtlas::new(8, 8);
        atlas.get_cell_symbol('\u{2588}', 8, 8).unwrap();
        assert_eq!(atlas.take_dirty_rect(), Some((0, 0, 8, 8)));
        assert!(atlas.get_cell_symbol('\u{2580}', 8, 8).is_none());
        assert!(atlas.full);
        assert_eq!(atlas.data.len(), 8 * 8 * 4);
        assert!(atlas.data.iter().all(|&v| v == 255));
    }

    #[test]
    fn test_atlas_does_not_panic() {
        let mut font_system = FontSystem::new();
        let mut swash_cache = SwashCache::new();
        let mut atlas = CpuAtlas::new(512, 512);

        // Shape a character to get a real CacheKey
        let mut buf = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
        buf.set_size(&mut font_system, 100.0, 50.0);
        buf.set_text(&mut font_system, "A", Attrs::new(), Shaping::Basic);
        buf.shape_until_scroll(&mut font_system, false);

        for run in buf.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let result =
                    atlas.get_or_rasterize(physical.cache_key, &mut font_system, &mut swash_cache);
                // Just assert it doesn't panic — result may be None if no font available
                let _ = result;
            }
        }
    }

    #[test]
    fn test_atlas_caches_result() {
        let mut font_system = FontSystem::new();
        let mut swash_cache = SwashCache::new();
        let mut atlas = CpuAtlas::new(512, 512);

        let mut buf = Buffer::new(&mut font_system, Metrics::new(14.0, 20.0));
        buf.set_size(&mut font_system, 100.0, 50.0);
        buf.set_text(&mut font_system, "B", Attrs::new(), Shaping::Basic);
        buf.shape_until_scroll(&mut font_system, false);

        for run in buf.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let first =
                    atlas.get_or_rasterize(physical.cache_key, &mut font_system, &mut swash_cache);
                atlas.dirty = false; // reset dirty flag
                let second =
                    atlas.get_or_rasterize(physical.cache_key, &mut font_system, &mut swash_cache);
                // Second call should hit cache, not mark dirty again
                assert!(!atlas.dirty);
                // Both calls return the same result
                assert_eq!(first.is_some(), second.is_some());
            }
        }
    }
}
