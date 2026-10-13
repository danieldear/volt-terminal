//! Opt-in Vello UI surface probe. Does not construct Volt's terminal renderer,
//! load user configuration, start a PTY, or persist settings.
use anyhow::{anyhow, ensure, Result};
use std::{path::Path, time::Instant};
use volt_ui_kit::{
    declarative::{Bindings, PaintBatch, Runtime},
    native::{Application, GpuOffscreenRenderer, RasterPainter, RendererPreference},
    ui::{column, prelude::*, row},
    Painter, Rect, Theme, Viewport,
};

type Batch = PaintBatch<<RasterPainter as Painter>::Shape, <RasterPainter as Painter>::Glyph>;
#[derive(Clone)]
struct Draft {
    blur: bool,
    opacity: f32,
    choice: u8,
    select_open: bool,
    applied: bool,
    updates: usize,
    theme: Theme,
}
impl Draft {
    fn new(theme: Theme, blur: bool) -> Self {
        Self {
            blur,
            opacity: 0.85,
            choice: 0,
            select_open: false,
            applied: false,
            updates: 0,
            theme,
        }
    }
}
fn hex(c: [f32; 4]) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c[0] * 255.) as u8,
        (c[1] * 255.) as u8,
        (c[2] * 255.) as u8
    )
}
fn view(s: &Draft) -> Element<Draft> {
    let c = volt_ui_kit::style::Colors::from_theme(&s.theme);
    column(vec![
        text("Volt · UI renderer prototype")
            .font_size(23.)
            .font_weight(600)
            .into(),
        text("Isolated preview — no terminal or configuration changes")
            .font_size(12.)
            .color(hex(c.muted))
            .into(),
        separator().background(hex(c.border)).into(),
        row(vec![
            text("Theme").flex(1.).into(),
            select([select_item(0u8, "Gruvbox"), select_item(1u8, "Catppuccin")])
                .key("theme")
                .selected(s.choice)
                .open(s.select_open)
                .width(240.)
                .height(36.)
                .background(hex(c.panel))
                .border_color(hex(c.border))
                .rounded(6.)
                .on_select(|s: &mut Draft, value| {
                    s.choice = value;
                    s.theme = palette(if value == 0 { "gruvbox" } else { "catppuccin" });
                    s.select_open = false;
                    s.updates += 1;
                })
                .on_open_change(|s: &mut Draft, open| {
                    s.select_open = open;
                })
                .into(),
        ])
        .height(44.)
        .gap(16., Axis::Both)
        .into(),
        row(vec![
            text("Background blur").flex(1.).into(),
            text(if s.blur { "On" } else { "Off" }).width(40.).into(),
            toggle("", s.blur)
                .key("blur")
                .width(44.)
                .height(22.)
                .accent_color(hex(c.accent))
                .on_change(|s: &mut Draft, value| {
                    s.blur = value;
                    s.updates += 1;
                })
                .into(),
        ])
        .height(44.)
        .gap(12., Axis::Both)
        .into(),
        row(vec![
            text("Opacity").flex(1.).into(),
            slider(s.opacity)
                .key("opacity")
                .range(0.25..=1.)
                .step(0.01)
                .width(200.)
                .height(32.)
                .accent_color(hex(c.accent))
                .on_change(|s: &mut Draft, value| {
                    s.opacity = value;
                    s.updates += 1;
                })
                .into(),
            text(format!("{:.0}%", s.opacity * 100.)).width(48.).into(),
        ])
        .height(44.)
        .gap(12., Axis::Both)
        .into(),
        text("Unicode: e\u{301} · 日本 · 👩‍💻 · 👍🏽")
            .key("unicode")
            .font_size(15.)
            .into(),
        row(vec![
            button("Build")
                .key("task")
                .enabled(!s.applied)
                .on_click(|s: &mut Draft| {
                    s.updates += 1;
                })
                .into(),
            text("+278").color(hex(s.theme.ansi[2])).into(),
            text("−14").color(hex(s.theme.ansi[1])).into(),
        ])
        .height(36.)
        .gap(16., Axis::Both)
        .into(),
        text(format!(
            "{} preview updates · {}",
            s.updates,
            if s.applied {
                "Applied (preview only)"
            } else {
                "Draft"
            }
        ))
        .key("status")
        .font_size(12.)
        .color(hex(c.muted))
        .into(),
        row(vec![
            button("Reset preview")
                .key("reset")
                .on_click(|s: &mut Draft| {
                    *s = Draft::new(palette("gruvbox"), false);
                })
                .into(),
            button("Apply preview")
                .key("apply")
                .background(hex(c.accent))
                .color(hex(c.panel))
                .on_click(|s: &mut Draft| {
                    s.applied = true;
                    s.updates += 1;
                })
                .into(),
        ])
        .height(40.)
        .gap(12., Axis::Both)
        .into(),
    ])
    .padding(24., Edge::All)
    .gap(14., Axis::Both)
    .font_family("SF Mono")
    .font_size(14.)
    .color(hex(c.text))
    .background(hex(c.panel))
    .border_color(hex(c.border))
    .border_width(1.)
    .rounded(14.)
    .into()
}
fn scene(p: &mut RasterPainter, draft: &Draft, viewport: Viewport) -> Result<Vec<Batch>> {
    let (doc, handlers) = view(draft).build()?.into_parts();
    let bindings = Bindings::default();
    let mut runtime = Runtime::new(doc, &handlers, &bindings)?;
    runtime.set_reduced_motion(true);
    runtime.layout(viewport, &bindings, p)?;
    Ok(runtime.paint(p, &draft.theme))
}
fn palette(name: &str) -> Theme {
    let t = volt_config::Theme::by_name(name);
    Theme {
        background: t.background.to_f32(),
        foreground: t.foreground.to_f32(),
        ansi: t.ansi.map(|c| c.to_f32()),
    }
}
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}
// Vello compute antialiasing can vary by one 8-bit quantization step across
// submissions. Keep that explicit, while rejecting larger changes/stale content.
fn maximum_delta(a: &[u8], b: &[u8]) -> Result<u8> {
    ensure!(a.len() == b.len(), "surface dimensions changed");
    Ok(a.iter()
        .zip(b)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0))
}
fn probe(out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let start = Instant::now();
    let mut gpu = GpuOffscreenRenderer::new().map_err(|e| anyhow!(e))?;
    let startup_ms = start.elapsed().as_secs_f64() * 1000.;
    println!("Vello UI probe: {}", gpu.adapter_name);
    let mut results = vec![];
    for scale in [1., 1.5, 2.] {
        let vp = Viewport::new(680. * scale, 580. * scale, scale).unwrap();
        let mut p = RasterPainter::new(vp);
        for name in ["gruvbox", "catppuccin", "dracula", "light"] {
            let t = palette(name);
            let mut states = vec![];
            for checked in [false, true] {
                let draft = Draft::new(t.clone(), checked);
                let start = Instant::now();
                let batches = scene(&mut p, &draft, vp)?;
                let compose_ms = start.elapsed().as_secs_f64() * 1000.;
                ensure!(batches.iter().any(|b| !b.glyphs.is_empty()), "missing text");
                let first = gpu
                    .render(&mut p, &batches, [0.; 4])
                    .map_err(|e| anyhow!(e))?;
                ensure!(
                    gpu.last_over_budget().is_none(),
                    "scene fell back to software"
                );
                ensure!(
                    gpu.framebuffer_upload_bytes() == 0,
                    "CPU framebuffer upload"
                );
                let sw = p.rasterize(&batches, [0.; 4]).map_err(|e| anyhow!(e))?;
                let mean_channel_diff = first
                    .data()
                    .iter()
                    .zip(sw.data())
                    .map(|(a, b)| a.abs_diff(*b) as f64)
                    .sum::<f64>()
                    / first.data().len() as f64;
                let id = format!("{name}-{scale}-{checked}");
                first.save_png(out.join(format!("{id}-gpu.png")))?;
                sw.save_png(out.join(format!("{id}-software.png")))?;
                let mut ms = vec![];
                let mut static_max_delta = 0;
                for _ in 0..7 {
                    let start = Instant::now();
                    let next = gpu
                        .render(&mut p, &batches, [0.; 4])
                        .map_err(|e| anyhow!(e))?;
                    ms.push(start.elapsed().as_secs_f64() * 1000.);
                    let delta = maximum_delta(next.data(), first.data())?;
                    static_max_delta = static_max_delta.max(delta);
                    ensure!(
                        delta <= 1,
                        "static frame changed beyond 1-LSB GPU quantization: {id}: {delta}"
                    );
                    ensure!(gpu.last_over_budget().is_none(), "warm scene fell back");
                    ensure!(gpu.resource_bytes() == 0, "warm glyph/image cache rebuilt");
                }
                // Render a clipped scene; all pixels outside its half-open clip
                // must remain transparent. This checks glyph and shape clipping.
                let mut clipped = batches.clone();
                let clip = Rect::new(24. * scale, 24. * scale, 320. * scale, 280. * scale).unwrap();
                for b in &mut clipped {
                    b.clip = clip;
                }
                let image = gpu
                    .render(&mut p, &clipped, [0.; 4])
                    .map_err(|e| anyhow!(e))?;
                let [x, y, w, h] = clip.bounds();
                for py in 0..image.height() {
                    for px in 0..image.width() {
                        if (px as f32) < x
                            || (px as f32) >= x + w
                            || (py as f32) < y
                            || (py as f32) >= y + h
                        {
                            ensure!(
                                image.pixel(px, py).unwrap().alpha() == 0,
                                "clip escaped: {id}"
                            );
                        }
                    }
                }
                let restored = gpu
                    .render(&mut p, &batches, [0.; 4])
                    .map_err(|e| anyhow!(e))?;
                ensure!(
                    maximum_delta(restored.data(), first.data())? <= 1,
                    "clip restore stale: {id}"
                );
                states.push(first.data().to_vec());
                results.push(serde_json::json!({"theme":name,"scale":scale,"checked":checked,"compose_ms":compose_ms,
                    "warm_render_readback_median_ms":median(&mut ms),"software_mean_channel_difference":mean_channel_diff,"static_max_channel_delta":static_max_delta,
                    "gpu_fallback":gpu.last_over_budget(),"framebuffer_upload_bytes":gpu.framebuffer_upload_bytes()}));
            }
            ensure!(states[0] != states[1], "toggle state invisible");
            // Returning to the original state must restore exactly, even after
            // another state, another clip and prior palette used the device.
            let b = scene(&mut p, &Draft::new(t.clone(), false), vp)?;
            let off = gpu.render(&mut p, &b, [0.; 4]).map_err(|e| anyhow!(e))?;
            ensure!(
                maximum_delta(off.data(), &states[0])? <= 1,
                "toggle restore stale"
            );
            let mut expanded = Draft::new(t.clone(), false);
            expanded.select_open = true;
            let popup = scene(&mut p, &expanded, vp)?;
            let popup_image = gpu
                .render(&mut p, &popup, [0.; 4])
                .map_err(|e| anyhow!(e))?;
            ensure!(popup_image.data() != off.data(), "dropdown did not open");
            let closed = gpu.render(&mut p, &b, [0.; 4]).map_err(|e| anyhow!(e))?;
            ensure!(
                maximum_delta(closed.data(), off.data())? <= 1,
                "dropdown restore stale"
            );
            let empty = gpu.render(&mut p, &[], [0.; 4]).map_err(|e| anyhow!(e))?;
            ensure!(
                empty.data().iter().all(|v| *v == 0),
                "hide left stale pixels"
            );
        }
    }
    let report = serde_json::json!({"adapter":gpu.adapter_name,"startup_ms":startup_ms,"cases":results,
        "boundary":"Offscreen UI prototype, warm timing includes GPU completion and full readback; not CPU time, terminal throughput, or an in-window compositor comparison."});
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    println!(
        "24 theme/DPI/state cases passed; clip/hide/restore and warm resource reuse checked. {}",
        out.display()
    );
    Ok(())
}
fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("--interactive") => Application::new(
            "Volt UI GPU Prototype",
            Draft::new(palette("gruvbox"), false),
            view,
        )
        .size(680, 580)
        .renderer(RendererPreference::GpuRequired)
        .run()
        .map_err(|e| anyhow!(e.to_string())),
        None => probe(Path::new("target/uigpucheck")),
        _ => Err(anyhow!(
            "Use no arguments for GPU checks, or --interactive for an isolated preview."
        )),
    }
}
