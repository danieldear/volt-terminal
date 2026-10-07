//! Warm-cache CPU build/submit benchmark, not display latency or PTY throughput.
//! Run saved executables in alternating order; do not time concurrent builds.
use anyhow::Result;
use std::time::Instant;
use volt_config::Theme;
use volt_core::performer::Performer;
use volt_renderer::Renderer;
fn percentile(samples: &mut [f64], percent: f64) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[((samples.len() - 1) as f64 * percent).round() as usize]
}
fn main() -> Result<()> {
    let theme = Theme::dark();
    let mut renderer = pollster::block_on(Renderer::new_offscreen(
        1800,
        1520,
        14.0,
        1.0,
        "monospace",
        1.2,
    ))?;
    renderer.custom_tab_bar = false;
    for workload in [
        "ascii-full",
        "ascii-one-row",
        "unicode-full",
        "unicode-static",
        "tasks-full",
    ] {
        renderer.task_strip = (workload == "tasks-full").then(|| {
            use volt_renderer::task_strip::{StripState, StripTask, TaskStripView};
            TaskStripView {
                tasks: ["Build", "Test", "Run", "Deploy"]
                    .into_iter()
                    .map(|name| StripTask {
                        name: name.into(),
                        state: StripState::Running,
                    })
                    .collect(),
                more: false,
                message: None,
            }
        });
        let mut p = Performer::new(200, 60);
        let mut parser = vte::Parser::new();
        let text = if workload.starts_with("unicode") {
            "é ẍ 👩‍💻 👍🏽 日本 \x1b[1mé ẍ\x1b[0m \x1b[3mé ẍ\x1b[0m \x1b[1;3mé ẍ\x1b[0m ".repeat(6)
        } else {
            "\x1b[38;5;120m0123456789 \x1b[48;5;52mabcdefghijklmnopqrstuvwxyz \x1b[0m".repeat(5)
        };
        for row in 0..60 {
            parser.advance(&mut p, format!("\x1b[{};1H{}", row + 1, text).as_bytes());
        }
        let mut cpu = Vec::new();
        let mut completion = Vec::new();
        for frame in 0..330 {
            let changed = match workload {
                "ascii-one-row" => 1,
                "unicode-static" => 0,
                _ => 60,
            };
            for row in 0..changed {
                parser.advance(&mut p, format!("\x1b[{};1H{frame:06}", row + 1).as_bytes());
            }
            let start = Instant::now();
            renderer.render_frame(
                &p.grid,
                &theme,
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
            let cpu_ms = start.elapsed().as_secs_f64() * 1000.0;
            renderer.wait_for_gpu()?;
            if frame >= 30 {
                cpu.push(cpu_ms);
                completion.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        }
        let cpu_p50 = percentile(&mut cpu, 0.5);
        let cpu_p95 = percentile(&mut cpu, 0.95);
        let completion_p50 = percentile(&mut completion, 0.5);
        let completion_p95 = percentile(&mut completion, 0.95);
        println!("{{\"workload\":\"{workload}\",\"frames\":300,\"cpu_p50_ms\":{cpu_p50:.6},\"cpu_p95_ms\":{cpu_p95:.6},\"completion_p50_ms\":{completion_p50:.6},\"completion_p95_ms\":{completion_p95:.6}}}");
    }
    Ok(())
}
