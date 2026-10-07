//! Custom tab color and narrow-title GPU smoke fixture.
//! cargo run --release -p volt-renderer --example tabcheck
use anyhow::{ensure, Result};
use volt_config::Theme;
use volt_core::grid::Grid;
use volt_renderer::{tab_color::TabColor, Renderer, TabEntry, TabStatus};

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * width + x) * 4) as usize;
    rgba[at..at + 4].try_into().unwrap()
}

fn main() -> Result<()> {
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
    let red = pixel(&rgba, width, left_pad + 1, 18);
    let blue = pixel(&rgba, width, left_pad + tab_w + tab_gap + 1, 18);
    ensure!(red[0] > red[2], "red accent missing: {red:?}");
    ensure!(blue[2] > blue[0], "blue accent missing: {blue:?}");

    let red_status = pixel(&rgba, width, left_pad + 15, 18);
    let green_status = pixel(&rgba, width, left_pad + tab_w + tab_gap + 15, 18);
    let idle_x = left_pad + 2 * (tab_w + tab_gap) + 15;
    let idle_status = pixel(&rgba, width, idle_x, 18);
    let idle_background = pixel(&rgba, width, idle_x, 11);
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
        "idle tab still draws a ring: {idle_status:?}"
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
