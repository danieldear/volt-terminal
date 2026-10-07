//! Settings visual/cache regression check. No display or user config required.
use anyhow::Result;
use volt_config::Theme;
use volt_core::grid::Grid;
use volt_renderer::{
    settings::{SettingsRow, SettingsView},
    Renderer,
};
fn draw(r: &mut Renderer, g: &Grid, t: &Theme) {
    r.render_frame(
        g,
        t,
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
fn view(onboarding: bool, section: usize) -> SettingsView {
    let sections = if onboarding {
        vec!["Welcome", "Appearance", "Workflow", "Shell & prompt"]
    } else {
        vec![
            "Appearance",
            "Shell & prompt",
            "Terminal",
            "Workspace",
            "Tasks",
            "Shortcuts",
            "Security",
            "Advanced",
        ]
    };
    SettingsView {
        title: if onboarding {
            "Make Volt yours"
        } else {
            "Settings"
        }
        .into(),
        subtitle: if onboarding {
            "Step 2 of 4 • skip anytime"
        } else {
            "Live appearance preview • Apply saves • Esc cancels"
        }
        .into(),
        sections: sections.iter().map(|s| s.to_string()).collect(),
        section,
        rows: vec![
            SettingsRow {
                label: "Theme".into(),
                value: "gruvbox".into(),
                hint: "Use − / + or arrows to select a theme".into(),
                editing: false,
                adjustable: true,
                actionable: true,
                caret: None,
                selected: false,
            },
            SettingsRow {
                label: "Font family".into(),
                value: "SF Mono".into(),
                hint: "Installed fonts; Enter to type a family name".into(),
                editing: true,
                adjustable: true,
                actionable: true,
                caret: Some(3),
                selected: true,
            },
            SettingsRow {
                label: "Font size".into(),
                value: "14".into(),
                hint: "6–72 points • changes preview live".into(),
                editing: false,
                adjustable: true,
                actionable: true,
                caret: None,
                selected: false,
            },
            SettingsRow {
                label: "Background blur".into(),
                value: "Off".into(),
                hint: "Platform compositor support varies".into(),
                editing: false,
                adjustable: false,
                actionable: true,
                caret: None,
                selected: false,
            },
        ],
        focus: 1,
        status: String::new(),
        onboarding,
        preview: true,
    }
}
fn png(path: &str, w: u32, h: u32, bytes: &[u8]) -> Result<()> {
    let mut e = png::Encoder::new(std::fs::File::create(path)?, w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()?.write_image_data(bytes)?;
    Ok(())
}
fn main() -> Result<()> {
    std::fs::create_dir_all("target/settingscheck")?;
    let mut cases = 0;
    for scale in [1., 1.5, 2.] {
        let (w, h) = ((850. * scale) as u32, (600. * scale) as u32);
        let mut r = pollster::block_on(Renderer::new_offscreen(w, h, 14., scale, "SF Mono", 1.2))?;
        let (cols, rows) = r.grid_size();
        let g = Grid::new(cols, rows);
        for name in ["gruvbox", "catppuccin", "dracula", "light"] {
            let t = Theme::by_name(name);
            for onboarding in [false, true] {
                r.settings = None;
                draw(&mut r, &g, &t);
                let baseline = r.read_offscreen_rgba()?;
                r.settings = Some(view(onboarding, 1));
                let l = r.settings_layout().expect("fits");
                r.settings.as_mut().unwrap().rows.truncate(l.rows);
                draw(&mut r, &g, &t);
                let first = r.read_offscreen_rgba()?;
                for (i, label) in [
                    (0, if onboarding { "Next" } else { "Apply" }),
                    (1, if onboarding { "Skip" } else { "Cancel" }),
                ] {
                    let rect = l.footer(i);
                    let group = (22. + label.len() as f32 * 12. * 0.6) * scale;
                    let x = rect[0] + (rect[2] - group) * 0.5;
                    let y = rect[1] + (rect[3] - 16. * scale) * 0.5;
                    let px = |x: u32, y: u32| {
                        &first[((y * w + x) * 4) as usize..((y * w + x) * 4 + 3) as usize]
                    };
                    let background = px(
                        (rect[0] + 4. * scale) as u32,
                        (rect[1] + rect[3] / 2.) as u32,
                    );
                    let mut contrast = 0;
                    for py in y.ceil() as u32..(y + 16. * scale).floor() as u32 {
                        for px_x in x.ceil() as u32..(x + 16. * scale).floor() as u32 {
                            if px(px_x, py)
                                .iter()
                                .zip(background)
                                .any(|(a, b)| a.abs_diff(*b) > 35)
                            {
                                contrast += 1;
                            }
                        }
                    }
                    anyhow::ensure!(
                        contrast > 5,
                        "missing footer icon: {label}, {name}, DPI {scale}"
                    );
                }
                draw(&mut r, &g, &t);
                anyhow::ensure!(
                    first == r.read_offscreen_rgba()?,
                    "cached settings pixels changed"
                );
                r.update_scale(scale, 14.);
                draw(&mut r, &g, &t);
                anyhow::ensure!(
                    first == r.read_offscreen_rgba()?,
                    "fresh settings pixels differ"
                );
                anyhow::ensure!(first != baseline, "Settings missing");
                if name == "gruvbox" && scale == 1. {
                    png(
                        &format!(
                            "target/settingscheck/{}.png",
                            if onboarding { "onboarding" } else { "settings" }
                        ),
                        w,
                        h,
                        &first,
                    )?;
                }
                r.settings = None;
                draw(&mut r, &g, &t);
                anyhow::ensure!(
                    baseline == r.read_offscreen_rgba()?,
                    "settings leave ghosts"
                );
                cases += 1;
            }
        }
    }
    println!("{cases} settings GPU cache/hide/theme/DPI cases passed");
    Ok(())
}
