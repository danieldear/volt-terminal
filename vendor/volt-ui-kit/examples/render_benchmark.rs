//! Fixed pre-styling scene for reproducible CPU rendering measurements.
mod support;
use volt_ui_kit::declarative::Handlers;
#[derive(Default)]
struct Project {
    milestones: usize,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !std::env::args().any(|arg| arg == "--bench") {
        println!("Run with --bench COUNT [--scale 2]. This fixed scene measures rendering, not the component showcase.");
        return Ok(());
    }
    let mut handlers = Handlers::default();
    handlers.on_activate("increment", |project: &mut Project| {
        project.milestones += 1;
    })?;
    handlers.on_activate("reset", |project: &mut Project| {
        project.milestones = 0;
    })?;
    support::launch(
        "Volt UI Kit — Rendering Benchmark",
        include_str!("ui/render_benchmark.json"),
        Some("examples/ui/render_benchmark.json"),
        Project::default(),
        handlers,
        |project, bindings| {
            bindings.set_text(
                "status",
                format!("{} milestones completed", project.milestones),
            );
            bindings.set_bool("can_reset", project.milestones > 0);
        },
    )
}
