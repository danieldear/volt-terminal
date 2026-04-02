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
}

/// CPU-side glyph atlas: R8 single-channel bitmap, shelf packer
pub struct CpuAtlas {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // R8: one byte per pixel
    cache: HashMap<CacheKey, Option<AtlasRegion>>,
    shelf_x: u32,
    shelf_y: u32,
    shelf_h: u32,
    pub dirty: bool, // true when data changed since last GPU upload
}

impl CpuAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0u8; (width * height) as usize],
            cache: HashMap::new(),
            shelf_x: 0,
            shelf_y: 0,
            shelf_h: 0,
            dirty: false,
        }
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

        // Find shelf space — advance to next shelf if current row is full
        if self.shelf_x + w > self.width {
            self.shelf_y += self.shelf_h + 1;
            self.shelf_x = 0;
            self.shelf_h = 0;
        }
        // Atlas is full
        if self.shelf_y + h > self.height {
            self.cache.insert(key, None);
            return None;
        }

        // Copy glyph pixels into atlas (R8)
        match image.content {
            SwashContent::Mask => {
                for row in 0..h {
                    for col in 0..w {
                        let src = image.data[(row * w + col) as usize];
                        let dst = ((self.shelf_y + row) * self.width + self.shelf_x + col) as usize;
                        self.data[dst] = src;
                    }
                }
            }
            SwashContent::Color => {
                // RGBA — use alpha channel as coverage
                for row in 0..h {
                    for col in 0..w {
                        let src = image.data[((row * w + col) * 4 + 3) as usize];
                        let dst = ((self.shelf_y + row) * self.width + self.shelf_x + col) as usize;
                        self.data[dst] = src;
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
                        let dst = ((self.shelf_y + row) * self.width + self.shelf_x + col) as usize;
                        self.data[dst] = coverage;
                    }
                }
            }
        }

        let region = AtlasRegion {
            u0: self.shelf_x as f32 / self.width as f32,
            v0: self.shelf_y as f32 / self.height as f32,
            u1: (self.shelf_x + w) as f32 / self.width as f32,
            v1: (self.shelf_y + h) as f32 / self.height as f32,
            width: w,
            height: h,
            offset_x: image.placement.left,
            offset_y: image.placement.top,
        };

        self.shelf_x += w + 1;
        if h > self.shelf_h {
            self.shelf_h = h;
        }
        self.dirty = true;
        self.cache.insert(key, Some(region));
        Some(region)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};

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
