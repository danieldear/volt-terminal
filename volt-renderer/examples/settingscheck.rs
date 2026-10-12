//! Settings visual/cache regression check. No display or user config required.
use anyhow::Result;
use volt_config::Theme;
use volt_core::grid::Grid;
use volt_renderer::{
    settings::{SettingsControl, SettingsDropdown, SettingsRow, SettingsView},
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
                r.settings_toggles = r
                    .settings
                    .as_ref()
                    .unwrap()
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(i, _)| if i == 3 { Some(false) } else { None })
                    .collect();
                r.settings_controls = vec![
                    SettingsControl::Choice,
                    SettingsControl::Choice,
                    SettingsControl::Number,
                    SettingsControl::Switch(false),
                ];
                r.settings_controls
                    .truncate(r.settings.as_ref().unwrap().rows.len());
                r.settings_dropdown = None;
                draw(&mut r, &g, &t);
                let first = r.read_offscreen_rgba()?;
                let field = volt_renderer::settings::control_rect(&l, 0);
                let sample = |x: u32, y: u32| {
                    &first[((y * w + x) * 4) as usize..((y * w + x) * 4 + 3) as usize]
                };
                let base = sample(
                    (field[0] + 4. * scale) as u32,
                    (field[1] + field[3] * 0.5) as u32,
                );
                let mut ink = 0;
                for y in (field[1] + 6. * scale).ceil() as u32
                    ..(field[1] + field[3] - 6. * scale).floor() as u32
                {
                    for x in (field[0] + field[2] - 32. * scale).ceil() as u32
                        ..(field[0] + field[2] - 9. * scale).floor() as u32
                    {
                        if sample(x, y)
                            .iter()
                            .zip(base)
                            .any(|(a, b)| a.abs_diff(*b) > 30)
                        {
                            ink += 1;
                        }
                    }
                }
                anyhow::ensure!(
                    ink > 3,
                    "missing toolkit select chevron, {name}, DPI {scale}"
                );
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
                if r.settings_toggles.len() > 3 {
                    // Same view: typed switch state must be part of cache identity.
                    r.settings_toggles[3] = Some(true);
                    r.settings_controls[3] = SettingsControl::Switch(true);
                    draw(&mut r, &g, &t);
                    let enabled = r.read_offscreen_rgba()?;
                    anyhow::ensure!(enabled != first, "toggle state did not repaint");
                    let row = l.row(3);
                    for (pixel, (off, on)) in first
                        .chunks_exact(4)
                        .zip(enabled.chunks_exact(4))
                        .enumerate()
                    {
                        if off != on {
                            let x = (pixel as u32 % w) as f32;
                            let y = (pixel as u32 / w) as f32;
                            anyhow::ensure!(
                                x >= row[0] + row[2] - 110. * scale
                                    && x < row[0] + row[2]
                                    && y >= row[1]
                                    && y < row[1] + row[3],
                                "toggle state change escaped right-hand row control"
                            );
                        }
                    }
                    draw(&mut r, &g, &t);
                    anyhow::ensure!(enabled == r.read_offscreen_rgba()?, "toggle cache differs");
                    if name == "gruvbox" && scale == 1. && !onboarding {
                        png(
                            "target/settingscheck/settings-toggle-on.png",
                            w,
                            h,
                            &enabled,
                        )?;
                    }
                    r.settings_toggles[3] = Some(false);
                    r.settings_controls[3] = SettingsControl::Switch(false);
                    draw(&mut r, &g, &t);
                    anyhow::ensure!(first == r.read_offscreen_rgba()?, "toggle leaves ghosts");
                }
                r.settings_dropdown = Some(SettingsDropdown {
                    row: 0,
                    items: vec![
                        "Gruvbox".into(),
                        "Catppuccin".into(),
                        "Nord".into(),
                        "Tokyo Night".into(),
                        "Dracula".into(),
                    ],
                    selected: 1,
                    current: 0,
                    offset: 0,
                });
                draw(&mut r, &g, &t);
                let menu = r.read_offscreen_rgba()?;
                anyhow::ensure!(menu != first, "dropdown missing");
                draw(&mut r, &g, &t);
                anyhow::ensure!(menu == r.read_offscreen_rgba()?, "dropdown cache changed");
                r.update_scale(scale, 14.);
                draw(&mut r, &g, &t);
                anyhow::ensure!(
                    menu == r.read_offscreen_rgba()?,
                    "dropdown fresh/cache mismatch"
                );
                if name == "gruvbox" && scale == 1. && !onboarding {
                    png("target/settingscheck/dropdown.png", w, h, &menu)?;
                }
                r.settings_dropdown = None;
                draw(&mut r, &g, &t);
                anyhow::ensure!(
                    first == r.read_offscreen_rgba()?,
                    "dropdown left stale glyphs"
                );
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
    // Complete component mix at all supported backing scales.
    for scale in [1., 1.5, 2.] {
        let (w, h) = ((1000. * scale) as u32, (740. * scale) as u32);
        let mut r = pollster::block_on(Renderer::new_offscreen(w, h, 14., scale, "SF Mono", 1.2))?;
        let (cols, rows) = r.grid_size();
        let g = Grid::new(cols, rows);
        let t = Theme::by_name("gruvbox");
        let mut v = view(false, 0);
        let template = v.rows[0].clone();
        v.rows = [
            ("Theme", "gruvbox"),
            ("Font family", "SF Mono"),
            ("Font size", "14"),
            ("Line height", "1.2"),
            ("Padding", "12"),
            ("Opacity", "0.85"),
            ("Background blur", "On"),
        ]
        .into_iter()
        .map(|(label, value)| SettingsRow {
            label: label.into(),
            value: value.into(),
            adjustable: false,
            editing: false,
            caret: None,
            selected: false,
            ..template.clone()
        })
        .collect();
        r.settings = Some(v);
        r.settings_controls = vec![
            SettingsControl::Choice,
            SettingsControl::Choice,
            SettingsControl::Number,
            SettingsControl::Number,
            SettingsControl::Number,
            SettingsControl::Slider {
                value: 0.85,
                min: 0.1,
                max: 1.,
            },
            SettingsControl::Switch(true),
        ];
        draw(&mut r, &g, &t);
        let first = r.read_offscreen_rgba()?;
        draw(&mut r, &g, &t);
        anyhow::ensure!(first == r.read_offscreen_rgba()?, "component cache changed");
        if scale == 1. {
            png("target/settingscheck/appearance-controls.png", w, h, &first)?;
        }
        let v = r.settings.as_mut().unwrap();
        v.rows[0].label = "Shell arguments".into();
        v.rows[0].value = "[\"-l\"]".into();
        v.rows[1].label = "New tab".into();
        v.rows[1].value = "Cmd+T".into();
        v.rows[2].label = "Edit theme colors".into();
        v.rows[2].value = "Customize".into();
        r.settings_controls[0] = SettingsControl::Text;
        r.settings_controls[1] = SettingsControl::Shortcut;
        r.settings_controls[2] = SettingsControl::Action("Customize…".into());
        draw(&mut r, &g, &t);
        let mixed = r.read_offscreen_rgba()?;
        anyhow::ensure!(mixed != first, "component types did not invalidate cache");
        draw(&mut r, &g, &t);
        anyhow::ensure!(
            mixed == r.read_offscreen_rgba()?,
            "mixed components cache changed"
        );
        if scale == 1. {
            png("target/settingscheck/mixed-controls.png", w, h, &mixed)?;
        }
        cases += 1;
    }
    println!("{cases} settings GPU cache/hide/theme/DPI cases passed");
    Ok(())
}
