//! Real GPU pixel/CPU+GPU-completion regression harness. No window or compositor.
//! cargo run --release -p volt-renderer --example rendercheck -- <output-dir>
use anyhow::Result;
use std::{fs::File, path::Path, time::Instant};
use volt_config::{CursorStyle, Theme};
use volt_core::performer::Performer;
use volt_renderer::Renderer;

fn draw(renderer: &mut Renderer, p: &Performer, selected: bool) {
    renderer.render_frame(
        &p.grid,
        &Theme::dark(),
        &[],
        false,
        selected.then_some(((0, 0), (10, 0))),
        false,
        None,
        &[],
        None,
        None,
        None,
        false,
    );
}
fn feed(p: &mut Performer, text: &str) {
    vte::Parser::new().advance(p, text.as_bytes());
}
fn fixture(p: &mut Performer) {
    let lines = [
        "Volt renderer conformance: text, separators, underlines, wide/combining cells",
        "\x1b[30;104m NORMAL \x1b[94;40m\u{e0b0}\x1b[37;40m file.rs \x1b[37;40m\u{e0bc}\x1b[0m main",
        "\x1b[95m\u{e0b6}\x1b[30;105m folder \x1b[0;92m\u{e0b6}\x1b[30;102m 1/1 \x1b[0m",
        "\x1b[36m\u{e0b0}\u{e0b1}\u{e0b2}\u{e0b3}\u{e0b4}\u{e0b5}\u{e0b6}\u{e0b7}\u{e0b8}\u{e0b9}\u{e0ba}\u{e0bb}\u{e0bc}\u{e0bd}\u{e0be}\u{e0bf}\x1b[0m",
        "\x1b[4mUnderline, including spaces     \x1b[0m plain \x1b[1;3mBold italic\x1b[0m",
        "Styles: é \x1b[1mé BOLD\x1b[0m \x1b[3mé ITALIC\x1b[0m \x1b[1;3mé BOTH\x1b[0m",
        "Wide: 日本語 中文 | marks: x\u{0301}\u{0308} e\u{0301} | ZWJ: 👩\u{200d}💻 👍🏽 🇺🇸",
        "\x1b[32m████ ▀▀▀▀ ▄▄▄▄ ▌▌▌▌ ▐▐▐▐\x1b[0m",
        "Nerd glyphs: \u{f07b} \u{e62b} \u{e0a0} | arrows: → ← ↔",
    ];
    feed(p, &lines.join("\r\n"));
}
fn png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    let mut encoder = png::Encoder::new(File::create(path)?, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}
fn percentile(samples: &mut [f64], percentile: f64) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[((samples.len() - 1) as f64 * percentile).round() as usize]
}

/// A non-block cursor may only change pixels inside its stroke, never the
/// background of the whole cell (including its alpha or selection highlight).
fn cursor_background_checks() -> Result<usize> {
    let mut tested = 0;
    for scale in [1.0f32, 1.5, 2.0] {
        let w = (480.0 * scale) as u32;
        let h = (160.0 * scale) as u32;
        let mut renderer =
            pollster::block_on(Renderer::new_offscreen(w, h, 14.0, scale, "monospace", 1.8))?;
        renderer.custom_tab_bar = false;
        for style in [CursorStyle::Underline, CursorStyle::Beam] {
            renderer.cursor_style = style;
            for opacity in [0.85, 1.0] {
                renderer.set_background_opacity(opacity);
                for background in ["default", "custom", "reverse", "selection", "search"] {
                    let (cols, rows) = renderer.grid_size();
                    let mut p = Performer::new(cols, rows);
                    feed(&mut p, "\x1b[2;3H");
                    match background {
                        "custom" => feed(&mut p, "\x1b[44m \x1b[0m\x1b[2;3H"),
                        "reverse" => feed(&mut p, "\x1b[31;7m \x1b[0m\x1b[2;3H"),
                        _ => {}
                    }
                    let selection = (background == "selection").then_some(((2, 1), (2, 1)));
                    let search = (background == "search").then_some(((2, 1), (2, 1)));
                    let draw = |renderer: &mut Renderer, visible| {
                        renderer.render_frame(
                            &p.grid,
                            &Theme::dark(),
                            &[],
                            visible,
                            selection,
                            false,
                            None,
                            &[],
                            search,
                            None,
                            None,
                            false,
                        );
                    };
                    renderer.set_row_cache_enabled(false);
                    draw(&mut renderer, false);
                    let hidden = renderer.read_offscreen_rgba()?;
                    draw(&mut renderer, true);
                    let visible = renderer.read_offscreen_rgba()?;
                    let pad = renderer.padding * scale;
                    let x0 = (pad + 2.0 * renderer.cell_width).round();
                    let x1 = (pad + 3.0 * renderer.cell_width).round();
                    let y0 = (pad + renderer.cell_height).round();
                    let y1 = (pad + 2.0 * renderer.cell_height).round();
                    let stroke_h = (1.5 * scale).max(2.0).min(y1 - y0);
                    let stroke_w = (1.2 * scale).max(1.5).min(x1 - x0);
                    let mut changed = 0;
                    let hidden_pixels: &[[u8; 4]] = bytemuck::cast_slice(&hidden);
                    let visible_pixels: &[[u8; 4]] = bytemuck::cast_slice(&visible);
                    for (i, (a, b)) in hidden_pixels.iter().zip(visible_pixels).enumerate() {
                        if a == b {
                            continue;
                        }
                        changed += 1;
                        let x = (i % w as usize) as f32 + 0.5;
                        let y = (i / w as usize) as f32 + 0.5;
                        let inside = match style {
                            CursorStyle::Underline => {
                                x >= x0 && x < x1 && y >= y1 - stroke_h && y < y1
                            }
                            CursorStyle::Beam => x >= x0 && x < x0 + stroke_w && y >= y0 && y < y1,
                            CursorStyle::Block => unreachable!(),
                        };
                        anyhow::ensure!(inside,
                            "cursor changed background outside stroke: {style:?}/{background}/opacity={opacity}/scale={scale} at {x},{y}: {a:?} -> {b:?}");
                    }
                    anyhow::ensure!(changed > 0, "cursor stroke was not rendered: {style:?}/{background}/opacity={opacity}/scale={scale}");
                    renderer.set_row_cache_enabled(true);
                    for _ in 0..3 {
                        draw(&mut renderer, true);
                    }
                    anyhow::ensure!(
                        renderer.last_frame_reused_rows == rows,
                        "cursor cache not exercised"
                    );
                    anyhow::ensure!(
                        renderer.read_offscreen_rgba()? == visible,
                        "cached cursor mismatch"
                    );
                    draw(&mut renderer, false);
                    anyhow::ensure!(
                        renderer.read_offscreen_rgba()? == hidden,
                        "cursor blink left stale background"
                    );
                    tested += 1;
                }
            }
        }
    }
    Ok(tested)
}
fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/rendercheck".into());
    let dir = Path::new(&output);
    std::fs::create_dir_all(dir)?;
    let mut tested = 0;
    if !std::env::args().any(|arg| arg == "--bench-only") {
        println!(
            "{} cursor-background GPU cases passed",
            cursor_background_checks()?
        );
        for family in ["monospace", "SF Mono", "JetBrainsMono Nerd Font Mono"] {
            for scale in [1.0f32, 1.5, 2.0] {
                for height in [1.0f32, 1.2, 1.8] {
                    let w = (1000.0 * scale) as u32;
                    let h = (300.0 * scale) as u32;
                    let mut renderer = pollster::block_on(Renderer::new_offscreen(
                        w, h, 14.0, scale, family, height,
                    ))?;
                    if family != "monospace"
                        && !renderer
                            .list_monospace_families()
                            .iter()
                            .any(|f| f.eq_ignore_ascii_case(family))
                    {
                        println!("SKIP unavailable font: {family}");
                        break;
                    }
                    renderer.custom_tab_bar = false;
                    let (cols, rows) = renderer.grid_size();
                    let mut p = Performer::new(cols, rows);
                    fixture(&mut p);
                    draw(&mut renderer, &p, false);
                    draw(&mut renderer, &p, false);
                    draw(&mut renderer, &p, false); // reuse cached row geometry
                    anyhow::ensure!(
                        renderer.last_frame_reused_rows == rows,
                        "cache fixture did not exercise row reuse"
                    );
                    let cached = renderer.read_offscreen_rgba()?;
                    renderer.set_row_cache_enabled(false);
                    draw(&mut renderer, &p, false);
                    let full = renderer.read_offscreen_rgba()?;
                    anyhow::ensure!(
                        cached == full,
                        "cached/full pixel mismatch: {family}/{scale}/{height}"
                    );
                    png(
                        &dir.join(format!("{}-{scale}-{height}.png", family.replace(' ', "_"))),
                        w,
                        h,
                        &full,
                    )?;
                    // Link IDs must not change glyph choice, procedural cell symbols,
                    // emoji shaping, spacing or cached geometry at any font/scale.
                    let mut linked = Performer::new(cols, rows);
                    feed(&mut linked, "\x1b]8;;https://example.test/render-proof\x07");
                    fixture(&mut linked);
                    feed(&mut linked, "\x1b]8;;\x07");
                    draw(&mut renderer, &linked, false);
                    anyhow::ensure!(
                        full == renderer.read_offscreen_rgba()?,
                        "linked/plain pixel mismatch: {family}/{scale}/{height}"
                    );
                    renderer.set_row_cache_enabled(true);
                    draw(&mut renderer, &linked, false);
                    draw(&mut renderer, &linked, false);
                    anyhow::ensure!(
                        full == renderer.read_offscreen_rgba()?,
                        "linked cache mismatch"
                    );

                    // Test row replacement, selection and atlas UV reuse, not only static output.
                    renderer.set_row_cache_enabled(true);
                    draw(&mut renderer, &p, false);
                    draw(&mut renderer, &p, false);
                    feed(&mut p, "\x1b[1;1Hchanged x\u{0308} 日本");
                    draw(&mut renderer, &p, false);
                    anyhow::ensure!(
                        renderer.last_frame_reused_rows == rows - 1,
                        "dirty-row fixture did not reuse unaffected rows"
                    );
                    let dirty = renderer.read_offscreen_rgba()?;
                    renderer.set_row_cache_enabled(false);
                    draw(&mut renderer, &p, false);
                    anyhow::ensure!(
                        dirty == renderer.read_offscreen_rgba()?,
                        "dirty-row pixel mismatch"
                    );
                    renderer.set_row_cache_enabled(true);
                    draw(&mut renderer, &p, false);
                    draw(&mut renderer, &p, false);
                    draw(&mut renderer, &p, true);
                    let changed = renderer.read_offscreen_rgba()?;
                    renderer.set_row_cache_enabled(false);
                    draw(&mut renderer, &p, true);
                    anyhow::ensure!(
                        changed == renderer.read_offscreen_rgba()?,
                        "changed-row pixel mismatch"
                    );
                    // Font/scale updates must invalidate ASCII and extended
                    // shape caches as well as atlas-dependent geometry.
                    renderer.update_scale(scale, 15.0);
                    draw(&mut renderer, &p, true);
                    renderer.update_scale(scale, 14.0);
                    draw(&mut renderer, &p, true);
                    anyhow::ensure!(
                        changed == renderer.read_offscreen_rgba()?,
                        "font-size cache roundtrip mismatch"
                    );
                    tested += 1;
                }
            }
        }
        println!("{tested} font/scale/line-height GPU fixtures passed cached/full and linked/plain pixel equality");
    }

    if std::env::args().any(|arg| arg == "--fixtures-only") {
        return Ok(());
    }
    // Independent renderers retain warm caches. Alternate their execution order
    // each frame to reduce thermal/load bias from testing one variant first.
    let mut renderers = Vec::new();
    for cache in [false, true] {
        let mut renderer = pollster::block_on(Renderer::new_offscreen(
            1800,
            1520,
            14.0,
            1.0,
            "monospace",
            1.2,
        ))?;
        renderer.custom_tab_bar = false;
        renderer.set_row_cache_enabled(cache);
        renderers.push(renderer);
    }
    let mut p = Performer::new(200, 60);
    for r in 0..60 {
        feed(
            &mut p,
            &format!(
                "\x1b[{};1H{}",
                r + 1,
                "0123456789 abcdefghijklmnopqrstuvwxyz ".repeat(5)
            ),
        );
    }
    let mut csv = String::from(
        "workload,row_cache,cpu_p50_ms,cpu_p95_ms,completion_p50_ms,completion_p95_ms\n",
    );
    for workload in ["static", "one-row", "full"] {
        let mut samples = [Vec::new(), Vec::new()];
        let mut cpu_samples = [Vec::new(), Vec::new()];
        for i in 0..330 {
            if workload != "static" {
                let rows = if workload == "full" { 60 } else { 1 };
                for row in 0..rows {
                    feed(&mut p, &format!("\x1b[{};1Hframe {i:06}", row + 1));
                }
            }
            for variant in if i % 2 == 0 { [0, 1] } else { [1, 0] } {
                let renderer = &mut renderers[variant];
                let start = Instant::now();
                draw(renderer, &p, false);
                let cpu_ms = start.elapsed().as_secs_f64() * 1000.0;
                renderer.wait_for_gpu()?;
                if i >= 30 {
                    samples[variant].push(start.elapsed().as_secs_f64() * 1000.0);
                    cpu_samples[variant].push(cpu_ms);
                }
            }
        }
        for variant in 0..2 {
            let cache = variant == 1;
            let cpu_p50 = percentile(&mut cpu_samples[variant], 0.5);
            let cpu_p95 = percentile(&mut cpu_samples[variant], 0.95);
            let p50 = percentile(&mut samples[variant], 0.5);
            let p95 = percentile(&mut samples[variant], 0.95);
            println!("CPU+GPU completion (not display latency): {workload} cache={cache} p50={p50:.3}ms p95={p95:.3}ms");
            println!("  CPU build+submit: p50={cpu_p50:.3}ms p95={cpu_p95:.3}ms");
            csv.push_str(&format!(
                "{workload},{cache},{cpu_p50:.6},{cpu_p95:.6},{p50:.6},{p95:.6}\n"
            ));
        }
    }
    std::fs::write(dir.join("performance.csv"), csv)?;
    Ok(())
}
