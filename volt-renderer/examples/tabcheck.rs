//! Pixel regression checks for tab lights and single-tab suppression.
//! cargo run --release -p volt-renderer --example tabcheck
use anyhow::{ensure, Result};
use std::path::Path;
use volt_config::{Theme, BUILTIN_THEMES};
use volt_core::grid::Grid;
use volt_renderer::{tab_color::TabColor, Renderer, TabEntry, TabStatus};

fn draw(r: &mut Renderer, g: &Grid, theme: &Theme, tabs: &[TabEntry<'_>]) -> Result<Vec<u8>> {
    r.render_frame(
        g,
        theme,
        tabs,
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
    r.read_offscreen_rgba()
}

fn pixel(pixels: &[u8], width: u32, x: f32, y: f32) -> [u8; 4] {
    let offset = (y.floor() as usize * width as usize + x.floor() as usize) * 4;
    pixels[offset..offset + 4].try_into().unwrap()
}

fn png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let mut encoder = png::Encoder::new(std::fs::File::create(path)?, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

fn main() -> Result<()> {
    color_and_title_fixture()?;
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/tabcheck".into());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir)?;
    let mut checks = 0;
    for scale in [1.0, 1.5, 2.0] {
        let (width, height) = ((960.0 * scale) as u32, (200.0 * scale) as u32);
        let mut r = pollster::block_on(Renderer::new_offscreen(
            width,
            height,
            14.0,
            scale,
            "monospace",
            1.8,
        ))?;
        let (cols, rows) = r.grid_size();
        let g = Grid::new(cols, rows);
        for (id, _) in BUILTIN_THEMES {
            let theme = Theme::builtin(id).unwrap();
            for opacity in [0.85, 1.0] {
                r.set_background_opacity(opacity);
                for count in [1, 3] {
                    for active in [false, true] {
                        let mut tabs: Vec<_> = (0..count)
                            .map(|i| TabEntry {
                                title: ["billing-api", "tests", "zsh"][i],
                                color: Some(TabColor::Purple),
                                active: if active { i == 0 } else { i == 1 },
                                index: i + 1,
                                status: TabStatus::Idle,
                                pane_count: if i == 0 { 2 } else { 1 },
                            })
                            .collect();
                        if count == 1 {
                            let hidden = draw(&mut r, &g, &theme, &[])?;
                            for status in [TabStatus::Idle, TabStatus::Running, TabStatus::Failed] {
                                tabs[0].status = status;
                                ensure!(
                                    draw(&mut r, &g, &theme, &tabs)? == hidden,
                                    "single-tab strip should be hidden: {id}/{scale}/{status:?}"
                                );
                                checks += 1;
                            }
                            continue;
                        }
                        let cy = r.top_offset_for_tab_count(count) / 2.0;
                        let cx = 93.5 * scale; // 78 left pad + 12 inset + radius.
                        let idle = draw(&mut r, &g, &theme, &tabs)?;
                        let center = pixel(&idle, width, cx, cy);
                        let background = pixel(&idle, width, 88.0 * scale, cy);
                        ensure!(center == background, "idle center overwrote tab tint: {id}/{scale}/{count}/{active}/{opacity}");
                        let edge = pixel(&idle, width, 90.5 * scale, cy);
                        ensure!(
                            edge != center,
                            "idle ring missing: {id}/{scale}/{count}/{active}"
                        );
                        for status in [TabStatus::Running, TabStatus::Failed, TabStatus::Idle] {
                            tabs[0].status = status;
                            let result = draw(&mut r, &g, &theme, &tabs)?;
                            let repeated = draw(&mut r, &g, &theme, &tabs)?;
                            ensure!(result == repeated, "cached tab light mismatch");
                            if status == TabStatus::Idle {
                                ensure!(result == idle, "status transition left stale pixels");
                            } else {
                                let expected =
                                    theme.ansi[if status == TabStatus::Running { 2 } else { 1 }];
                                let actual = pixel(&result, width, cx, cy);
                                for (a, b) in
                                    actual.into_iter().zip([expected.r, expected.g, expected.b])
                                {
                                    ensure!(a.abs_diff(b) <= 1, "wrong tab light color: {status:?}/{id}/{scale}: {actual:?}");
                                }
                                let corner = pixel(&result, width, 90.0 * scale, cy - 3.0 * scale);
                                ensure!(corner == background, "tab light is not circular");
                            }
                            // Only the light may change; accents, titles, close and +
                            // buttons, other tabs and terminal content must be intact.
                            for (i, (a, b)) in
                                idle.chunks_exact(4).zip(result.chunks_exact(4)).enumerate()
                            {
                                if a != b {
                                    let x = (i % width as usize) as f32 + 0.5;
                                    let y = (i / width as usize) as f32 + 0.5;
                                    ensure!(
                                        x >= 90.0 * scale
                                            && x < 97.0 * scale
                                            && (y - cy).abs() <= 3.5 * scale,
                                        "status changed pixels outside its light: {x},{y}"
                                    );
                                }
                            }
                            checks += 1;
                        }
                    }
                }
            }
        }
        // Screenshot reference: idle ring, green busy, red failure; tab color
        // and active highlight remain independent of the command status.
        let mut tabs = [
            TabEntry {
                title: "billing-api",
                color: Some(TabColor::Red),
                active: false,
                index: 1,
                status: TabStatus::Idle,
                pane_count: 1,
            },
            TabEntry {
                title: "tests",
                color: Some(TabColor::Purple),
                active: true,
                index: 2,
                status: TabStatus::Running,
                pane_count: 2,
            },
            TabEntry {
                title: "zsh",
                color: None,
                active: false,
                index: 3,
                status: TabStatus::Failed,
                pane_count: 1,
            },
        ];
        let theme = Theme::dark();
        let pixels = draw(&mut r, &g, &theme, &tabs)?;
        png(
            &dir.join(format!("lights-{scale}.png")),
            width,
            height,
            &pixels,
        )?;
        let hidden = draw(&mut r, &g, &theme, &[])?;
        ensure!(
            draw(&mut r, &g, &theme, &tabs[..1])? == hidden,
            "closing back to one tab leaves stale strip pixels"
        );
        ensure!(
            draw(&mut r, &g, &theme, &tabs)? == pixels,
            "reopening tabs does not restore the strip"
        );
        // Native tabs must not acquire custom chrome accidentally.
        r.custom_tab_bar = false;
        let native = draw(&mut r, &g, &theme, &tabs)?;
        tabs[0].status = TabStatus::Running;
        ensure!(
            draw(&mut r, &g, &theme, &tabs)? == native,
            "native tabs acquired a custom light"
        );
    }
    println!("{checks} GPU tab-status checks passed (three scales, five themes, tint, opacity, single-tab suppression, multiple tabs and cached transitions).");
    Ok(())
}

fn fixture_pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * width + x) * 4) as usize;
    rgba[at..at + 4].try_into().unwrap()
}

fn color_and_title_fixture() -> Result<()> {
    let (width, height, scale) = (760, 300, 1.0);
    let mut renderer = pollster::block_on(Renderer::new_offscreen(
        width, height, 14.0, scale, "SF Mono", 1.4,
    ))?;
    let (cols, rows) = renderer.grid_size_for_tab_count(3);
    let grid = Grid::new(cols, rows);
    let tabs = [
        TabEntry {
            title: "api-server-with-a-very-long-title",
            color: Some(TabColor::Red),
            active: false,
            index: 1,
            status: TabStatus::Failed,
            pane_count: 1,
        },
        TabEntry {
            title: "integration-tests-with-a-very-long-title",
            color: Some(TabColor::Blue),
            active: true,
            index: 2,
            status: TabStatus::Running,
            pane_count: 1,
        },
        TabEntry {
            title: "plain",
            color: None,
            active: false,
            index: 3,
            status: TabStatus::Idle,
            pane_count: 1,
        },
    ];
    renderer.render_frame(
        &grid,
        &Theme::dark(),
        &tabs,
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
    let rgba = renderer.read_offscreen_rgba()?;
    let left_pad = 78u32;
    let tab_gap = 4u32;
    let plus_x = width - 10 - 24;
    let tab_area_w = plus_x - left_pad - 10;
    let tab_w = ((tab_area_w - 2 * tab_gap) / 3).clamp(100, 220);
    let red = fixture_pixel(&rgba, width, left_pad + 1, 18);
    let blue = fixture_pixel(&rgba, width, left_pad + tab_w + tab_gap + 1, 18);
    ensure!(red[0] > red[2], "red accent missing: {red:?}");
    ensure!(blue[2] > blue[0], "blue accent missing: {blue:?}");

    let red_status = fixture_pixel(&rgba, width, left_pad + 15, 18);
    let green_status = fixture_pixel(&rgba, width, left_pad + tab_w + tab_gap + 15, 18);
    let idle_x = left_pad + 2 * (tab_w + tab_gap) + 15;
    let idle_status = fixture_pixel(&rgba, width, idle_x, 18);
    let idle_background = fixture_pixel(&rgba, width, idle_x, 11);
    ensure!(
        red_status[0] > red_status[1],
        "failed status isn't red: {red_status:?}"
    );
    ensure!(
        green_status[1] > green_status[0],
        "running status isn't green: {green_status:?}"
    );
    ensure!(
        idle_status == idle_background,
        "idle ring center is not hollow: {idle_status:?}"
    );

    std::fs::create_dir_all("target/tabcheck")?;
    let file = std::fs::File::create("target/tabcheck/tab-colors.png")?;
    let mut png = png::Encoder::new(file, width, height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&rgba)?;
    // Retina: the same tabs at 2x, for reviewing shapes at their real size.
    let (w2, h2) = (width * 2, height * 2);
    let mut retina =
        pollster::block_on(Renderer::new_offscreen(w2, h2, 14.0, 2.0, "SF Mono", 1.4))?;
    let (cols, rows) = retina.grid_size_for_tab_count(3);
    retina.render_frame(
        &Grid::new(cols, rows),
        &Theme::dark(),
        &tabs,
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
    let rgba2 = retina.read_offscreen_rgba()?;
    let file = std::fs::File::create("target/tabcheck/tab-colors-2x.png")?;
    let mut png = png::Encoder::new(file, w2, h2);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&rgba2)?;
    println!("tab color + title fixture: target/tabcheck/tab-colors.png (+ -2x)");
    Ok(())
}
