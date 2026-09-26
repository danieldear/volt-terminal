//! Cell-sized Powerline masks, independent of font metrics.
//! Codepoint names: ryanoasis/nerd-fonts glyphnames.json (Powerline/Powerline Extra).
//! Text and other Nerd Font icons still use the configured font/fallback.
pub fn is_cell_symbol(c: char) -> bool {
    matches!(
        c as u32,
        0xe0b0..=0xe0bf | 0x2580 | 0x2584 | 0x2588 | 0x258c | 0x2590
    )
}

pub fn rasterize(c: char, width: u32, height: u32) -> Vec<u8> {
    let mut data = vec![0; width as usize * height as usize];
    let stroke = 1.0 / width.min(height).max(1) as f32;
    for y in 0..height {
        for x in 0..width {
            let mut covered = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = (x as f32 + (sx as f32 + 0.5) / 4.0) / width as f32;
                    let py = (y as f32 + (sy as f32 + 0.5) / 4.0) / height as f32;
                    let edge = 1.0 - (2.0 * py - 1.0).abs();
                    let inside = match c as u32 {
                        0xe0b0 => px <= edge,
                        0xe0b1 => (px - edge).abs() <= stroke,
                        0xe0b2 => 1.0 - px <= edge,
                        0xe0b3 => (1.0 - px - edge).abs() <= stroke,
                        0xe0b4 => px * px + (2.0 * py - 1.0).powi(2) <= 1.0,
                        0xe0b5 => {
                            (px * px + (2.0 * py - 1.0).powi(2)).sqrt().sub(1.0).abs() <= stroke
                        }
                        0xe0b6 => (1.0 - px).powi(2) + (2.0 * py - 1.0).powi(2) <= 1.0,
                        0xe0b7 => {
                            ((1.0 - px).powi(2) + (2.0 * py - 1.0).powi(2))
                                .sqrt()
                                .sub(1.0)
                                .abs()
                                <= stroke
                        }
                        0xe0b8 => px <= py,
                        0xe0ba => px >= 1.0 - py,
                        0xe0bc => px <= 1.0 - py,
                        0xe0be => px >= py,
                        0xe0b9 | 0xe0bf => (px - py).abs() <= stroke,
                        0xe0bb | 0xe0bd => (px + py - 1.0).abs() <= stroke,
                        0x2580 => py < 0.5,
                        0x2584 => py >= 0.5,
                        0x2588 => true,
                        0x258c => px < 0.5,
                        0x2590 => px >= 0.5,
                        _ => false,
                    };
                    covered += u32::from(inside);
                }
            }
            data[(y * width + x) as usize] = ((covered * 255 + 8) / 16) as u8;
        }
    }
    data
}
use std::ops::Sub;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn powerline_masks_are_cell_sized_and_mirrored_at_all_line_heights() {
        for scale in [1.0f32, 1.5, 2.0] {
            for line_height in [1.0f32, 1.2, 1.8] {
                let w = (9.0 * scale).round() as u32;
                let h = (14.0 * scale * line_height).round() as u32;
                for (a, b) in [
                    ('\u{e0b0}', '\u{e0b2}'),
                    ('\u{e0b4}', '\u{e0b6}'),
                    ('\u{e0b8}', '\u{e0ba}'),
                ] {
                    let left = rasterize(a, w, h);
                    let right = rasterize(b, w, h);
                    assert_eq!(left.len(), (w * h) as usize);
                    for y in 0..h {
                        for x in 0..w {
                            assert_eq!(
                                left[(y * w + x) as usize],
                                right[(y * w + w - 1 - x) as usize]
                            );
                        }
                    }
                    assert!(left.contains(&255));
                }
                assert!(rasterize('\u{2588}', w, h).iter().all(|&v| v == 255));
            }
        }
    }
}
