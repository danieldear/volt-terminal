//! Offscreen theme editor regression: opaque over the terminal, independent
//! of the theme being edited, cache-stable, clean hide, grid size unchanged.
use anyhow::Result;
use volt_config::{Color, Theme};
use volt_core::{cell::Cell, grid::Grid};
use volt_renderer::{
    theme_editor::{EditorRow, ThemeEditorView},
    Renderer,
};

fn draw(r: &mut Renderer, g: &Grid, theme: &Theme) {
    r.render_frame(
        g,
        theme,
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

fn view(theme: &Theme, naming: bool) -> ThemeEditorView {
    let row = |label: &str, color: Color| EditorRow {
        label: label.into(),
        color,
        text: color.to_hex(),
        valid: true,
    };
    let mut rows = vec![
        row("Background", theme.background),
        row("Foreground", theme.foreground),
        row("Cursor", theme.cursor),
        row("Cursor text", theme.cursor_text),
        row("Selection", theme.selection_bg),
        row("Selection text", theme.selection_fg),
    ];
    rows[1].text = "#a0b".into();
    rows[1].valid = false;
    ThemeEditorView {
        subtitle: "Nord · modified".into(),
        rows,
        ansi: theme.ansi,
        detail: row("ANSI 4 · blue", theme.ansi[4]),
        focus: 1,
        naming: naming.then(|| ("My very long theme name that scrolls".into(), 20)),
        save_label: "Save as…".into(),
        status: (!naming).then(|| ("Saved to themes/nord-custom.toml".into(), false)),
    }
}

/// RGBA rows of the panel interior, inset past the rounded corners.
fn panel_pixels(r: &Renderer, pixels: &[u8], w: u32) -> Vec<u8> {
    let l = r.theme_editor_layout().expect("editor fits");
    let inset = 16. * l.scale;
    let mut out = Vec::new();
    for y in (l.y + inset) as usize..(l.y + l.h - inset) as usize {
        let start = (y * w as usize + (l.x + inset) as usize) * 4;
        let end = (y * w as usize + (l.x + l.w - inset) as usize) * 4;
        out.extend_from_slice(&pixels[start..end]);
    }
    out
}

fn main() -> Result<()> {
    std::fs::create_dir_all("target/themecheck")?;
    let edited = Theme::by_name("nord");
    let other = Theme::by_name("gruvbox");
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
            draw(&mut r, &filled, &edited);
            let baseline = r.read_offscreen_rgba()?;

            for naming in [false, true] {
                r.theme_editor = Some(view(&edited, naming));
                anyhow::ensure!(r.grid_size() == size, "editor resized terminal grid");
                draw(&mut r, &empty, &edited);
                let over_empty = r.read_offscreen_rgba()?;
                draw(&mut r, &filled, &edited);
                let over_filled = r.read_offscreen_rgba()?;
                anyhow::ensure!(
                    panel_pixels(&r, &over_empty, w) == panel_pixels(&r, &over_filled, w),
                    "terminal text bleeds through the editor"
                );
                draw(&mut r, &filled, &edited);
                anyhow::ensure!(
                    over_filled == r.read_offscreen_rgba()?,
                    "editor cache changes pixels"
                );
                // The panel uses fixed colors; only its swatches show the theme.
                draw(&mut r, &filled, &other);
                anyhow::ensure!(
                    panel_pixels(&r, &over_filled, w)
                        == panel_pixels(&r, &r.read_offscreen_rgba()?, w),
                    "editor chrome depends on the window theme"
                );
                let f = std::fs::File::create(format!(
                    "target/themecheck/editor-{scale}-{height}-{}.png",
                    if naming { "naming" } else { "fields" }
                ))?;
                let mut png = png::Encoder::new(f, w, h);
                png.set_color(png::ColorType::Rgba);
                png.set_depth(png::BitDepth::Eight);
                png.write_header()?.write_image_data(&over_filled)?;
            }

            r.theme_editor = None;
            draw(&mut r, &filled, &edited);
            anyhow::ensure!(
                r.read_offscreen_rgba()? == baseline,
                "editor hide leaves stale pixels"
            );
            cases += 1;
        }
    }
    println!("PASS: {cases} theme editor GPU cases (opaque, theme-independent, cache, clean hide)");
    Ok(())
}
