//! Real Volt renderer + optional cached Vello Settings layer; no native window,
//! user configuration, PTY or background tasks are touched.
use anyhow::{ensure, Result};
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
fn settings(checked: bool) -> SettingsView {
    let row = |label: &str, value: &str| SettingsRow {
        label: label.into(),
        value: value.into(),
        hint: String::new(),
        editing: false,
        adjustable: false,
        actionable: true,
        caret: None,
        selected: false,
    };
    SettingsView {
        title: "Settings".into(),
        subtitle: "Apply saves • Esc cancels".into(),
        sections: [
            "Appearance",
            "Shell & prompt",
            "Terminal",
            "Workspace",
            "Tasks",
            "Shortcuts",
            "Security",
            "Advanced",
        ]
        .map(String::from)
        .to_vec(),
        section: 0,
        rows: vec![
            row("Theme", "gruvbox"),
            row("Font family", "SF Mono"),
            row("Font size", "14"),
            row("Background blur", if checked { "On" } else { "Off" }),
        ],
        focus: 0,
        status: String::new(),
        onboarding: false,
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
    std::fs::create_dir_all("target/settingsgpucheck")?;
    let mut results = vec![];
    for scale in [1., 1.5, 2.] {
        let (w, h) = ((850. * scale) as u32, (600. * scale) as u32);
        let mut r = pollster::block_on(Renderer::new_offscreen(w, h, 14., scale, "SF Mono", 1.2))?;
        let (cols, rows) = r.grid_size();
        let g = Grid::new(cols, rows);
        for name in ["gruvbox", "catppuccin", "dracula", "light"] {
            let t = Theme::by_name(name);
            r.settings = None;
            r.set_settings_gpu_enabled(true);
            draw(&mut r, &g, &t);
            let terminal = r.read_offscreen_rgba()?;
            let before = r.settings_gpu_stats();
            for _ in 0..3 {
                draw(&mut r, &g, &t);
            }
            ensure!(
                r.settings_gpu_stats().rebuilds == before.rebuilds,
                "hidden GPU work"
            );
            ensure!(
                r.settings_gpu_stats().uploaded_bytes == before.uploaded_bytes,
                "hidden upload"
            );
            for checked in [false, true] {
                r.settings = Some(settings(checked));
                r.settings_controls = vec![
                    SettingsControl::Choice,
                    SettingsControl::Choice,
                    SettingsControl::Number,
                    SettingsControl::Switch(checked),
                ];
                r.set_settings_gpu_enabled(false);
                draw(&mut r, &g, &t);
                let legacy = r.read_offscreen_rgba()?;
                r.set_settings_gpu_enabled(true);
                let before = r.settings_gpu_stats();
                draw(&mut r, &g, &t);
                let upgraded = r.read_offscreen_rgba()?;
                let after = r.settings_gpu_stats();
                ensure!(
                    after.rebuilds == before.rebuilds + 1 && after.fallbacks == before.fallbacks,
                    "Vello layer not active: {:?}",
                    after
                );
                ensure!(
                    after.uploaded_bytes - before.uploaded_bytes == u64::from(w) * u64::from(h) * 4,
                    "incorrect transfer accounting"
                );
                for _ in 0..5 {
                    draw(&mut r, &g, &t);
                    ensure!(
                        r.read_offscreen_rgba()? == upgraded,
                        "cached layer pixels changed"
                    );
                }
                ensure!(
                    r.settings_gpu_stats().uploaded_bytes == after.uploaded_bytes
                        && r.settings_gpu_stats().rebuilds == after.rebuilds,
                    "static UI rerender/upload"
                );
                ensure!(
                    after.gpu_initializations == 1 && after.pipeline_creations == 1,
                    "GPU device/pipeline recreated for Settings changes"
                );
                let l = r.settings_layout().unwrap();
                let i =
                    (((l.y + 130. * scale) as u32 * w + (l.x + 140. * scale) as u32) * 4) as usize;
                for channel in 0..4 {
                    ensure!(
                        legacy[i + channel].abs_diff(upgraded[i + channel]) <= 3,
                        "panel/scrim color changed"
                    );
                }
                let diff = legacy
                    .iter()
                    .zip(&upgraded)
                    .map(|(a, b)| a.abs_diff(*b) as f64)
                    .sum::<f64>()
                    / legacy.len() as f64;
                let id = format!("{name}-{scale}-{checked}");
                png(
                    &format!("target/settingsgpucheck/{id}-legacy.png"),
                    w,
                    h,
                    &legacy,
                )?;
                png(
                    &format!("target/settingsgpucheck/{id}-vello.png"),
                    w,
                    h,
                    &upgraded,
                )?;
                results.push(serde_json::json!({"theme":name,"scale":scale,"checked":checked,"mean_channel_difference":diff,"uploaded_bytes":after.uploaded_bytes-before.uploaded_bytes}));
            }
            // Host dropdown, hide/reopen and resize invalidation remain real Volt paths.
            r.settings_dropdown = Some(SettingsDropdown {
                row: 0,
                items: vec!["gruvbox".into(), "catppuccin".into()],
                selected: 1,
                current: 0,
                offset: 0,
            });
            let old = r.settings_gpu_stats();
            draw(&mut r, &g, &t);
            ensure!(
                r.settings_gpu_stats().rebuilds == old.rebuilds + 1,
                "dropdown not invalidated"
            );
            r.settings_dropdown = None;
            draw(&mut r, &g, &t);
            r.resize(w + 64, h + 32);
            let old = r.settings_gpu_stats();
            draw(&mut r, &g, &t);
            ensure!(
                r.settings_gpu_stats().rebuilds == old.rebuilds + 1,
                "resize not invalidated"
            );
            r.resize(300, 240);
            let old = r.settings_gpu_stats();
            draw(&mut r, &g, &t);
            ensure!(
                r.settings_gpu_stats().rebuilds == old.rebuilds,
                "too-small layout rebuilt GPU"
            );
            r.resize(w, h);
            draw(&mut r, &g, &t);
            ensure!(
                r.settings_gpu_stats().rebuilds == old.rebuilds + 1,
                "resize restore stale cache"
            );
            r.update_scale(scale, 15.);
            let old = r.settings_gpu_stats();
            draw(&mut r, &g, &t);
            ensure!(
                r.settings_gpu_stats().rebuilds == old.rebuilds + 1,
                "font reset not invalidated"
            );
            r.update_scale(scale, 14.);
            draw(&mut r, &g, &t);
            r.settings = None;
            draw(&mut r, &g, &t);
            let old = r.settings_gpu_stats();
            r.settings = Some(settings(true));
            draw(&mut r, &g, &t);
            ensure!(
                r.settings_gpu_stats().rebuilds == old.rebuilds + 1,
                "reopen did not rebuild dropped texture"
            );
            r.settings = None;
            draw(&mut r, &g, &t);
            ensure!(
                r.read_offscreen_rgba()? == terminal,
                "hide left UI over terminal"
            );
        }
    }
    std::fs::write(
        "target/settingsgpucheck/report.json",
        serde_json::to_vec_pretty(&results)?,
    )?;
    println!("24 real Volt Settings layer cases passed: cache, hidden-path, transfer accounting, restore and bounded palette samples. Visual rasterizer differences reported separately.");
    Ok(())
}
