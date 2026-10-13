//! Inspector GPU smoke test: geometry reservation, cache equivalence, clean hide.
//! cargo run --release -p volt-renderer --example cardcheck
use anyhow::Result;
use volt_config::Theme;
use volt_core::{cell::Cell, grid::Grid};
use volt_renderer::{
    workspace_card::{CardIcon, CardRow, CardTone, WorkspaceCard},
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
    std::fs::create_dir_all("target/cardcheck")?;
    let mut cases = 0;
    for scale in [1.0, 1.5, 2.0] {
        for height in [1.0, 1.2, 1.8] {
            let (w, h) = ((1000. * scale) as u32, (650. * scale) as u32);
            let mut r =
                pollster::block_on(Renderer::new_offscreen(w, h, 14., scale, "SF Mono", height))?;
            let (cols, rows) = r.grid_size();
            let g = Grid::new(cols, rows);
            draw(&mut r, &g);
            let baseline = r.read_offscreen_rgba()?;
            r.workspace_card = Some(WorkspaceCard {
                title: "Volt workspace".into(),
                status: "Changed".into(),
                tone: CardTone::Amber,
                subtitle: "Visual regression fixture".into(),
                rows: vec![
                    CardRow {
                        kind: volt_renderer::workspace_card::BlockKind::Notice,
                        label:
                            "A command is running. Wait for it to finish before starting a task."
                                .into(),
                        detail: String::new(),
                        icon: CardIcon::Notice,
                        action: None,
                        expanded: false,
                        section: false,
                        tone: CardTone::Amber,
                        diff: None,
                    },
                    CardRow {
                        kind: volt_renderer::workspace_card::BlockKind::Section,
                        label: "Changes".into(),
                        detail: "3".into(),
                        icon: CardIcon::Branch,
                        action: Some(0),
                        expanded: false,
                        section: true,
                        tone: CardTone::Purple,
                        diff: Some((42, 7)),
                    },
                    CardRow {
                        kind: volt_renderer::workspace_card::BlockKind::Section,
                        label: "Agent".into(),
                        detail: "Not connected".into(),
                        icon: CardIcon::Agent,
                        action: Some(2),
                        expanded: false,
                        section: true,
                        tone: Default::default(),
                        diff: None,
                    },
                    CardRow {
                        kind: volt_renderer::workspace_card::BlockKind::Section,
                        label: "Context".into(),
                        detail: String::new(),
                        icon: CardIcon::File,
                        action: Some(3),
                        expanded: false,
                        section: true,
                        tone: Default::default(),
                        diff: None,
                    },
                    CardRow {
                        kind: volt_renderer::workspace_card::BlockKind::Section,
                        label: "Local models".into(),
                        detail: String::new(),
                        icon: CardIcon::Server,
                        action: Some(5),
                        expanded: false,
                        section: true,
                        tone: Default::default(),
                        diff: None,
                    },
                ],
                ..Default::default()
            });
            let (ncols, nrows) = r.grid_size();
            anyhow::ensure!(
                ncols < cols && nrows == rows,
                "panel must only reserve horizontal space"
            );
            let l = r.workspace_card_layout().unwrap();
            anyhow::ensure!(
                ncols as f32 * r.cell_width + r.padding * scale < l.x,
                "terminal overlaps inspector"
            );
            let expanded_top = r.content_top_offset_for_tab_count(1);
            let mut slim = Grid::new(ncols, nrows);
            for (row, text) in [
                "neo terminal main > cd ../other",
                "neo other > prompt stays here",
            ]
            .iter()
            .enumerate()
            {
                for (col, c) in text.chars().enumerate() {
                    let mut cell = Cell::default();
                    cell.set_char(c);
                    slim.put_char(col, row, cell);
                }
            }
            draw(&mut r, &slim);
            let first = r.read_offscreen_rgba()?;
            draw(&mut r, &slim);
            anyhow::ensure!(
                first == r.read_offscreen_rgba()?,
                "cached inspector pixels differ"
            );
            let file =
                std::fs::File::create(format!("target/cardcheck/card-{scale}-{height}.png"))?;
            let mut png = png::Encoder::new(file, w, h);
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header()?.write_image_data(&first)?;
            r.workspace_card.as_mut().unwrap().minimized = true;
            let (mini_cols, mini_rows) = r.grid_size();
            anyhow::ensure!(
                mini_cols == ncols && mini_rows == nrows,
                "collapse must not resize or reflow the terminal"
            );
            let compact = r.workspace_card_layout().unwrap();
            anyhow::ensure!(
                (compact.x, compact.y) == (l.x, l.y),
                "collapse moves card anchor"
            );
            anyhow::ensure!(
                r.content_top_offset_for_tab_count(1) == expanded_top,
                "collapse moves prompt vertically"
            );
            draw(&mut r, &slim);
            let minimized = r.read_offscreen_rgba()?;
            draw(&mut r, &slim);
            anyhow::ensure!(
                minimized == r.read_offscreen_rgba()?,
                "minimized cache differs"
            );
            let terminal_edge = ((w as f32 - r.workspace_card_reserved_width()) as usize) * 4;
            for (before, after) in first
                .chunks_exact(w as usize * 4)
                .zip(minimized.chunks_exact(w as usize * 4))
            {
                anyhow::ensure!(
                    before[..terminal_edge] == after[..terminal_edge],
                    "collapse changes terminal pixels"
                );
            }
            let file =
                std::fs::File::create(format!("target/cardcheck/compact-{scale}-{height}.png"))?;
            let mut png = png::Encoder::new(file, w, h);
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header()?.write_image_data(&minimized)?;
            r.workspace_card.as_mut().unwrap().minimized = false;
            let prior_layout = r.workspace_card_layout().unwrap();
            r.workspace_card.as_mut().unwrap().hover = Some(100);
            let hover_layout = r.workspace_card_layout().unwrap();
            anyhow::ensure!(
                std::sync::Arc::ptr_eq(&prior_layout, &hover_layout),
                "hover unnecessarily rewraps workspace content"
            );
            r.workspace_card.as_mut().unwrap().hover = None;
            r.workspace_card.as_mut().unwrap().floating = true;
            r.workspace_card.as_mut().unwrap().position = Some([120., 60.]);
            anyhow::ensure!(
                r.grid_size() == (cols, rows),
                "floating must preserve full TUI grid"
            );
            draw(&mut r, &g);
            let empty_overlay = r.read_offscreen_rgba()?;
            let mut filled = Grid::new(cols, rows);
            for row in 0..rows {
                for col in 0..cols {
                    let mut c = Cell::default();
                    c.set_char('#');
                    filled.put_char(col, row, c);
                }
            }
            draw(&mut r, &filled);
            let over_text = r.read_offscreen_rgba()?;
            let f = r.workspace_card_layout().unwrap();
            for y in (f.y + 24. * scale) as usize..(f.y + f.h - 24. * scale) as usize {
                let start = (y * w as usize + (f.x + 24. * scale) as usize) * 4;
                let end = (y * w as usize + (f.x + f.w - 24. * scale) as usize) * 4;
                anyhow::ensure!(
                    empty_overlay[start..end] == over_text[start..end],
                    "terminal glyphs bleed through floating card"
                );
            }
            let file =
                std::fs::File::create(format!("target/cardcheck/floating-{scale}-{height}.png"))?;
            let mut png = png::Encoder::new(file, w, h);
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header()?.write_image_data(&over_text)?;
            r.workspace_card.as_mut().unwrap().position = Some([350., 140.]);
            draw(&mut r, &filled);
            let moved = r.read_offscreen_rgba()?;
            draw(&mut r, &filled);
            anyhow::ensure!(
                moved == r.read_offscreen_rgba()?,
                "drag cache leaves artifacts"
            );
            r.workspace_card.as_mut().unwrap().minimized = true;
            anyhow::ensure!(
                r.grid_size() == (cols, rows),
                "floating collapse resizes TUI"
            );
            cases += 1;
            r.workspace_card.as_mut().unwrap().minimized = false;
            let mut code = CardRow::new(
                &format!("{}SAFE_END", "preview line 日本\n".repeat(80)),
                "",
                CardIcon::File,
                None,
                false,
                false,
            );
            code.kind = volt_renderer::workspace_card::BlockKind::Code;
            r.workspace_card.as_mut().unwrap().rows = vec![
                CardRow::new("Project", "", CardIcon::Folder, Some(5), true, true),
                CardRow::new(
                    "Repository root\n/Users/neo/workspace/rust/terminal-public",
                    "",
                    CardIcon::Folder,
                    None,
                    false,
                    false,
                ),
                code,
                CardRow::new("Add task", "", CardIcon::Plus, Some(1202), false, false),
            ];
            r.workspace_card.as_mut().unwrap().scroll = usize::MAX;
            let end_layout = r.workspace_card_layout().unwrap();
            anyhow::ensure!(
                end_layout.blocks.len() == 4,
                "wrapped content turned into extra items"
            );
            let action = &end_layout.blocks[3];
            let action_rect = end_layout.block_rect(action);
            anyhow::ensure!(
                end_layout.hit(
                    action_rect[0] + 10. * scale,
                    action_rect[1] + action_rect[3] * 0.5,
                    r.workspace_card.as_ref().unwrap()
                ) == Some(1202),
                "scrolled click hits wrong action"
            );
            draw(&mut r, &filled);
            let scrolled = r.read_offscreen_rgba()?;
            draw(&mut r, &filled);
            anyhow::ensure!(
                scrolled == r.read_offscreen_rgba()?,
                "scrolled cache differs"
            );
            let file =
                std::fs::File::create(format!("target/cardcheck/blocks-{scale}-{height}.png"))?;
            let mut png = png::Encoder::new(file, w, h);
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header()?.write_image_data(&scrolled)?;
            cases += 1;
            r.workspace_card = None;
            draw(&mut r, &g);
            anyhow::ensure!(
                r.read_offscreen_rgba()? == baseline,
                "hiding leaves stale card pixels"
            );
            anyhow::ensure!(
                r.grid_size() == (cols, rows),
                "hide must restore grid dimensions"
            );
            cases += 2;
        }
    }
    println!(
        "{cases} docked/floating inspector GPU cases passed (1x/1.5x/2x, line height 1/1.2/1.8)"
    );
    Ok(())
}
