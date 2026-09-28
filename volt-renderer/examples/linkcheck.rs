//! Real GPU proof that long link previews stay within the prompt's horizontal bounds.
use anyhow::Result;
use volt_config::Theme;
use volt_core::performer::Performer;
use volt_renderer::{PromptOverlay, Renderer};

fn main() -> Result<()> {
    let mut cases = 0;
    for scale in [1.0f32, 1.5, 2.0] {
        let (w, h) = ((800.0 * scale) as u32, (220.0 * scale) as u32);
        let mut r =
            pollster::block_on(Renderer::new_offscreen(w, h, 14.0, scale, "monospace", 1.2))?;
        r.custom_tab_bar = false;
        let (cols, rows) = r.grid_size();
        let p = Performer::new(cols, rows);
        let text = format!(
            "https://example.test/{}?query=日本語",
            "long/path/".repeat(150)
        );
        for cursor in [0, text.chars().count() / 2, text.chars().count()] {
            let draw = |r: &mut Renderer, text, cursor| {
                r.render_frame(
                    &p.grid,
                    &Theme::dark(),
                    &[],
                    false,
                    None,
                    false,
                    None,
                    &[],
                    None,
                    Some(PromptOverlay {
                        title: "Open link? Enter: open / Esc: cancel",
                        text,
                        read_only: true,
                        cursor,
                        match_count: 0,
                        matches_truncated: false,
                        current_match: 0,
                    }),
                    None,
                    false,
                );
            };
            draw(&mut r, "", 0);
            let blank = r.read_offscreen_rgba()?;
            draw(&mut r, &text, cursor);
            let visible = r.read_offscreen_rgba()?;
            std::fs::create_dir_all("target/terminal-links/previews")?;
            let file = std::fs::File::create(format!(
                "target/terminal-links/previews/link-{scale}-{cursor}.png"
            ))?;
            let mut encoder = png::Encoder::new(file, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header()?.write_image_data(&visible)?;
            let (left, right) = (
                (w as f32 - 420. * scale) / 2.,
                (w as f32 + 420. * scale) / 2.,
            );
            for y in 0..h as usize {
                for x in 0..w as usize {
                    if (x as f32) >= left && (x as f32) <= right {
                        continue;
                    }
                    let offset = (y * w as usize + x) * 4;
                    anyhow::ensure!(
                        blank[offset..offset + 4] == visible[offset..offset + 4],
                        "URL escaped dialog at {x},{y}"
                    );
                }
            }
            anyhow::ensure!(blank != visible, "URL preview invisible");
            cases += 1;
        }
    }
    println!("PASS: {cases} link-preview GPU bounds cases (1x/1.5x/2x, beginning/middle/end)");
    Ok(())
}
