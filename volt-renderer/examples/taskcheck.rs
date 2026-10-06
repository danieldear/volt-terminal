//! Offscreen task UI check: the task strip and the Add task form draw where
//! their hit-tests say, cover the terminal, follow the theme, and leave no
//! pixels behind when hidden. Writes PNGs to target/taskcheck/.
use anyhow::Result;
use volt_config::Theme;
use volt_core::{cell::Cell, grid::Grid};
use volt_renderer::{
    task_form::{FormField, TaskFormView},
    task_strip::{StripState, StripTask, TaskStripView},
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

fn strip() -> TaskStripView {
    let task = |name: &str, state| StripTask {
        name: name.into(),
        state,
    };
    TaskStripView {
        tasks: vec![
            task("Build", StripState::Succeeded),
            task("Test", StripState::Failed),
            task("Run", StripState::Running),
            task("Deploy staging", StripState::Idle),
        ],
        more: false,
        message: Some("Added \"Deploy staging\".".into()),
    }
}

fn form() -> TaskFormView {
    let field = |label: &str, value: &str, placeholder: &str, cursor| FormField {
        label: label.into(),
        value: value.into(),
        placeholder: placeholder.into(),
        cursor,
    };
    TaskFormView {
        title: "Add task".into(),
        project: "~/code/atlas/.volt/tasks.toml".into(),
        fields: vec![
            field("Name", "Deploy staging", "Build", None),
            field(
                "Command",
                "./scripts/deploy.sh --env staging",
                "cargo build --release",
                Some(34),
            ),
            field("Folder (inside the project)", "", "Project folder", None),
        ],
        confirm: true,
        focus: 1,
        status: None,
    }
}

fn png(path: &str, w: u32, h: u32, pixels: &[u8]) -> Result<()> {
    let f = std::fs::File::create(path)?;
    let mut e = png::Encoder::new(f, w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()?.write_image_data(pixels)?;
    Ok(())
}

fn region(pixels: &[u8], w: u32, r: [f32; 4]) -> Vec<u8> {
    let mut out = Vec::new();
    for y in r[1] as usize..(r[1] + r[3]) as usize {
        let start = (y * w as usize + r[0] as usize) * 4;
        out.extend_from_slice(&pixels[start..start + r[2] as usize * 4]);
    }
    out
}

fn main() -> Result<()> {
    std::fs::create_dir_all("target/taskcheck")?;
    let mut cases = 0;
    for scale in [1., 2.] {
        let (w, h) = ((1100. * scale) as u32, (720. * scale) as u32);
        let mut r = pollster::block_on(Renderer::new_offscreen(w, h, 14., scale, "SF Mono", 1.2))?;
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
        let theme = Theme::by_name("gruvbox");
        draw(&mut r, &filled, &theme);
        let baseline = r.read_offscreen_rgba()?;

        // Strip: draws where it hits, over the terminal, and nowhere when hidden.
        r.task_strip = Some(strip());
        let l = r.task_strip_layout().expect("strip fits");
        anyhow::ensure!(r.grid_size() == size, "strip resized the grid");
        draw(&mut r, &filled, &theme);
        let with_strip = r.read_offscreen_rgba()?;
        for b in &l.buttons {
            let inset = [
                b[0] + 30. * scale,
                b[1] + 6. * scale,
                b[2] - 36. * scale,
                b[3] - 12. * scale,
            ];
            anyhow::ensure!(
                region(&with_strip, w, inset) != region(&baseline, w, inset),
                "strip button not drawn where it is hit-tested"
            );
        }
        png(
            &format!("target/taskcheck/strip-{scale}.png"),
            w,
            h,
            &with_strip,
        )?;
        r.task_strip = None;
        draw(&mut r, &filled, &theme);
        anyhow::ensure!(
            r.read_offscreen_rgba()? == baseline,
            "strip leaves pixels when hidden"
        );

        // Form: opaque over the terminal, cache-stable, follows the theme.
        r.task_form = Some(form());
        let fl = r.task_form_layout().expect("form fits");
        let inner = [
            fl.x + 16. * scale,
            fl.y + 16. * scale,
            fl.w - 32. * scale,
            fl.h - 32. * scale,
        ];
        draw(&mut r, &empty, &theme);
        let over_empty = r.read_offscreen_rgba()?;
        draw(&mut r, &filled, &theme);
        let over_filled = r.read_offscreen_rgba()?;
        anyhow::ensure!(
            region(&over_empty, w, inner) == region(&over_filled, w, inner),
            "terminal shows through the form"
        );
        draw(&mut r, &filled, &theme);
        anyhow::ensure!(
            over_filled == r.read_offscreen_rgba()?,
            "form cache changes pixels"
        );
        png(
            &format!("target/taskcheck/form-{scale}.png"),
            w,
            h,
            &over_filled,
        )?;
        draw(&mut r, &filled, &Theme::by_name("nord"));
        anyhow::ensure!(
            region(&r.read_offscreen_rgba()?, w, inner) != region(&over_filled, w, inner),
            "form ignores the theme"
        );
        r.task_form = None;
        draw(&mut r, &filled, &theme);
        anyhow::ensure!(
            r.read_offscreen_rgba()? == baseline,
            "form leaves pixels when hidden"
        );
        cases += 1;
    }
    println!("PASS: {cases} task UI GPU cases (strip placement, form occlusion, cache, theme, clean hide)");
    Ok(())
}
