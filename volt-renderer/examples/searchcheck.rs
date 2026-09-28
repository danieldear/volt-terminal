//! Offscreen search compositor regression: full grid, opaque layering, cache, hide.
use anyhow::Result;
use volt_config::Theme;
use volt_core::{cell::Cell, grid::Grid};
use volt_renderer::{
    search_palette::{SearchRow, SearchView},
    workspace_card::WorkspaceCard,
    Renderer,
};
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
fn main() -> Result<()> {
    std::fs::create_dir_all("target/searchcheck")?;
    let mut cases = 0;
    for scale in [1., 1.5, 2.] {
        for height in [1., 1.2, 1.8] {
            let (w, h) = ((1050. * scale) as u32, (720. * scale) as u32);
            let mut r =
                pollster::block_on(Renderer::new_offscreen(w, h, 14., scale, "SF Mono", height))?;
            let size = r.grid_size();
            let empty = Grid::new(size.0, size.1);
            let mut filled = Grid::new(size.0, size.1);
            for y in 0..size.1 {
                for x in 0..size.0 {
                    let mut c = Cell::default();
                    c.set_char('#');
                    filled.put_char(x, y, c);
                }
            }
            draw(&mut r, &filled);
            let baseline = r.read_offscreen_rgba()?;
            r.search_palette = Some(SearchView {
                query: "workspace".into(),
                query_selected: false,
                cursor: 9,
                scope: 2,
                selected: 0,
                rows: vec![
                    SearchRow {
                        kind: "FILE".into(),
                        title: "src/workspace_search.rs".into(),
                        detail: "Saved file · read-only preview".into(),
                    },
                    SearchRow {
                        kind: "TEXT".into(),
                        title: "fn search_workspace()".into(),
                        detail: "src/main.rs:42".into(),
                    },
                ],
                preview: vec![
                    "  42  fn search_workspace() {".into(),
                    "  43      // Local, cancellable search".into(),
                    "  44  }".into(),
                ],
                status: "2 results · local only".into(),
                root: "/example/project".into(),
                action: "Enter: read-only preview".into(),
            });
            anyhow::ensure!(r.grid_size() == size, "search resized terminal grid");
            draw(&mut r, &empty);
            let overlay = r.read_offscreen_rgba()?;
            r.workspace_card = Some(WorkspaceCard {
                title: "UNDERLYING INSPECTOR TEXT".into(),
                floating: true,
                position: Some([200., 60.]),
                ..Default::default()
            });
            draw(&mut r, &filled);
            let composed = r.read_offscreen_rgba()?;
            let l = r.search_layout().unwrap();
            for y in (l.y + 24. * scale) as usize..(l.y + l.h - 24. * scale) as usize {
                let start = (y * w as usize + (l.x + 24. * scale) as usize) * 4;
                let end = (y * w as usize + (l.x + l.w - 24. * scale) as usize) * 4;
                anyhow::ensure!(
                    overlay[start..end] == composed[start..end],
                    "terminal or inspector bleeds through search"
                );
            }
            draw(&mut r, &filled);
            anyhow::ensure!(
                composed == r.read_offscreen_rgba()?,
                "search cache changes pixels"
            );
            let f =
                std::fs::File::create(format!("target/searchcheck/search-{scale}-{height}.png"))?;
            let mut png = png::Encoder::new(f, w, h);
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header()?.write_image_data(&composed)?;
            r.search_palette = None;
            r.workspace_card = None;
            draw(&mut r, &filled);
            anyhow::ensure!(
                r.read_offscreen_rgba()? == baseline,
                "search hide leaves stale pixels"
            );
            let mut prompt_pixels = Vec::new();
            for (i, grid) in [&empty, &filled].into_iter().enumerate() {
                r.render_frame(
                    grid,
                    &Theme::dark(),
                    &[],
                    false,
                    None,
                    false,
                    None,
                    &[],
                    None,
                    Some(volt_renderer::renderer::PromptOverlay {
                        title: "Find",
                        read_only: false,
                        text: "needle",
                        cursor: 6,
                        match_count: 1,
                        matches_truncated: false,
                        current_match: 0,
                    }),
                    None,
                    false,
                );
                let pixels = r.read_offscreen_rgba()?;
                if i == 0 {
                    prompt_pixels = pixels;
                } else {
                    let x = (w as f32 - 420. * scale) / 2.;
                    for y in (44. * scale) as usize..(108. * scale) as usize {
                        let start = (y * w as usize + (x + 4. * scale) as usize) * 4;
                        let end = (y * w as usize + (x + 416. * scale) as usize) * 4;
                        anyhow::ensure!(
                            prompt_pixels[start..end] == pixels[start..end],
                            "terminal bleeds through Find prompt"
                        );
                    }
                }
            }
            cases += 1;
        }
    }
    println!("PASS: {cases} search GPU cases (grid, two underlying layers, cache, clean hide, Find handoff)");
    Ok(())
}
