//! Actual offscreen GPU readback: RGBA, alpha, clipped UVs, layering, reset and exhaustion.
use anyhow::{ensure, Result};
use std::sync::Arc;
use volt_config::Theme;
use volt_core::{cell::Cell, grid::Grid};
use volt_renderer::{workspace_card::WorkspaceCard, InlineImage, Renderer};

fn draw(r: &mut Renderer, g: &Grid) {
    r.render_frame(
        g,
        &Theme::dark(),
        &[],
        false,
        None,
        false,
        None,
        &[],
        None,
        None,
        None,
        false,
    );
}
fn pixel(p: &[u8], x: usize, y: usize) -> [u8; 4] {
    p[(y * 640 + x) * 4..(y * 640 + x) * 4 + 4]
        .try_into()
        .unwrap()
}
fn main() -> Result<()> {
    let mut r = pollster::block_on(Renderer::new_offscreen(640, 480, 14., 1., "SF Mono", 1.2))?;
    let (cols, rows) = r.grid_size();
    let empty = Grid::new(cols, rows);
    let mut filled = Grid::new(cols, rows);
    for y in 0..rows {
        for x in 0..cols {
            let mut c = Cell::default();
            c.set_char('#');
            filled.put_char(x, y, c);
        }
    }
    draw(&mut r, &filled);
    let baseline = r.read_offscreen_rgba()?;
    // Four distinct texels; right half clips away the red and blue left texels.
    let image = InlineImage {
        id: 1,
        pixels: Arc::from([
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ]),
        width: 2,
        height: 2,
        rect: [80., 80., 160., 160.],
        clip: [160., 80., 80., 160.],
    };
    r.set_inline_images(vec![image.clone()]);
    draw(&mut r, &filled);
    let p = r.read_offscreen_rgba()?;
    ensure!(
        pixel(&p, 180, 100) == [0, 255, 0, 255],
        "clipped UV top-right is wrong"
    );
    ensure!(
        pixel(&p, 180, 200) == [255, 255, 0, 255],
        "clipped UV bottom-right is wrong"
    );
    ensure!(
        pixel(&p, 100, 100) == pixel(&baseline, 100, 100),
        "image leaked outside pane clip"
    );
    // Opaque image must completely occlude the populated terminal.
    draw(&mut r, &empty);
    let q = r.read_offscreen_rgba()?;
    for y in 80..240 {
        for x in 160..240 {
            ensure!(
                pixel(&p, x, y) == pixel(&q, x, y),
                "terminal text appears above image"
            );
        }
    }
    draw(&mut r, &empty);
    ensure!(
        q == r.read_offscreen_rgba()?,
        "cached upload changes pixels"
    );
    r.update_scale(1., 14.);
    draw(&mut r, &empty);
    ensure!(
        q == r.read_offscreen_rgba()?,
        "atlas reset leaves stale image UVs"
    );
    // The same immutable ID/Arc can move and shrink without reuploading RGBA.
    let moved = InlineImage {
        rect: [300., 80., 80., 80.],
        clip: [300., 80., 80., 80.],
        ..image.clone()
    };
    r.set_inline_images(vec![moved]);
    draw(&mut r, &empty);
    let moved_pixels = r.read_offscreen_rgba()?;
    ensure!(
        pixel(&moved_pixels, 310, 90) == [255, 0, 0, 255],
        "cached image movement lost source texel"
    );
    r.set_inline_images(vec![]);
    draw(&mut r, &filled);
    ensure!(
        baseline == r.read_offscreen_rgba()?,
        "image hide leaves stale pixels"
    );
    // Straight alpha: transparent blue preserves destination, half-red blends.
    draw(&mut r, &empty);
    let bg = r.read_offscreen_rgba()?;
    let alpha = InlineImage {
        id: 2,
        pixels: Arc::from([255, 0, 0, 128, 0, 0, 255, 0]),
        width: 2,
        height: 1,
        rect: [80., 80., 160., 80.],
        clip: [80., 80., 160., 80.],
    };
    r.set_inline_images(vec![alpha]);
    draw(&mut r, &empty);
    let p = r.read_offscreen_rgba()?;
    ensure!(
        pixel(&p, 200, 100) == pixel(&bg, 200, 100),
        "transparent RGBA changed background"
    );
    let dst = pixel(&bg, 100, 100);
    let actual = pixel(&p, 100, 100);
    for i in 0..3 {
        let src = if i == 0 { 255. } else { 0. };
        let expected = (src * (128. / 255.) + dst[i] as f32 * (127. / 255.)).round() as i16;
        ensure!(
            (actual[i] as i16 - expected).abs() <= 1,
            "incorrect straight-alpha blend"
        );
    }
    // Cards are above images, including above their opaque source RGB.
    r.workspace_card = Some(WorkspaceCard {
        title: "OVER IMAGE".into(),
        floating: true,
        position: Some([160., 80.]),
        ..Default::default()
    });
    r.set_inline_images(vec![]);
    draw(&mut r, &empty);
    let overlay = r.read_offscreen_rgba()?;
    let cover = InlineImage {
        id: 3,
        pixels: Arc::from([0, 255, 0, 255]),
        width: 1,
        height: 1,
        rect: [0., 0., 640., 480.],
        clip: [0., 0., 640., 480.],
    };
    r.set_inline_images(vec![cover]);
    draw(&mut r, &filled);
    let composed = r.read_offscreen_rgba()?;
    // Resolve physical layout, including terminal top offset.
    let layout = r.workspace_card_layout().expect("floating card layout");
    let x = (layout.x + 60.) as usize;
    let y = (layout.y + 30.) as usize;
    ensure!(
        pixel(&overlay, x, y) == pixel(&composed, x, y),
        "overlay is below image"
    );
    r.workspace_card = None;
    let too_wide = InlineImage {
        id: 4,
        pixels: Arc::from(vec![255; 2049 * 4]),
        width: 2049,
        height: 1,
        rect: [0., 0., 100., 100.],
        clip: [0., 0., 100., 100.],
    };
    r.set_inline_images(vec![too_wide]);
    draw(&mut r, &empty);
    ensure!(
        r.inline_image_atlas_misses() == 1,
        "atlas exhaustion not reported"
    );
    let exhausted = r.read_offscreen_rgba()?;
    draw(&mut r, &empty);
    ensure!(
        exhausted == r.read_offscreen_rgba()? && r.inline_image_atlas_misses() == 1,
        "negative cache is unstable"
    );
    println!("inlineimagecheck: RGBA, alpha, pane clipping/UVs, cache/move/reset, layering, hide and exhaustion passed");
    Ok(())
}
