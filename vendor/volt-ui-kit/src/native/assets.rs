//! Explicit, bounded asset decoding. Store handles in application state, not views.
use crate::graphics::Image;
use std::{io::Read, path::Path};
/// Decode PNG/JPEG bytes (16 MiB encoded, 16 MP, bounded decoder allocation).
pub fn decode_image(bytes: &[u8]) -> Result<Image, Box<dyn std::error::Error>> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("encoded image exceeds 16 MiB".into());
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err("only PNG and JPEG assets are supported".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode()?.into_rgba8();
    Ok(Image::from_rgba(
        decoded.width(),
        decoded.height(),
        decoded.into_raw(),
    )?)
}
/// No implicit loads, remote URLs or file watching; clone the returned handle.
pub fn load_image(path: impl AsRef<Path>) -> Result<Image, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    decode_image(&bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_decode_roundtrips_png_and_rejects_unknown_or_corrupt_input() {
        let mut p = tiny_skia::Pixmap::new(2, 1).unwrap();
        p.fill(tiny_skia::Color::from_rgba(1., 0., 0., 0.5).unwrap());
        let image = decode_image(&p.encode_png().unwrap()).unwrap();
        assert_eq!([image.width(), image.height()], [2, 1]);
        assert_eq!(image.pixels(), p.data());
        assert!(decode_image(b"not an image").is_err());
        assert!(decode_image(b"\xff\xd8\xff").is_err());
        assert!(decode_image(&vec![0; 16 * 1024 * 1024 + 1]).is_err());
    }
}
