//! A Solid workbench: editable presentation, typed Rust state and event handlers.
mod support;
use volt_ui_kit::declarative::{Bindings, HandlerError, Handlers};

struct Project {
    auto_save: bool,
    open_preview: bool,
    include_docs: bool,
    strict_checks: bool,
    dirty: bool,
    applied: usize,
    previews: usize,
    message: String,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            auto_save: true,
            open_preview: true,
            include_docs: true,
            strict_checks: true,
            dirty: false,
            applied: 0,
            previews: 0,
            message: "Ready. Change a preference to enable Apply changes.".into(),
        }
    }
}
impl Project {
    fn changed(&mut self, label: &str, value: bool) {
        self.dirty = true;
        self.message = format!(
            "{label} {}. Changes stay in memory until applied.",
            if value { "enabled" } else { "disabled" }
        );
    }
}
fn handlers() -> Result<Handlers<'static, Project>, HandlerError> {
    let mut handlers = Handlers::default();
    handlers.on_toggle_changed("auto_save_changed", |project: &mut Project, value| {
        project.auto_save = value;
        project.changed("Auto-save preference", value);
    })?;
    handlers.on_toggle_changed("preview_changed", |project: &mut Project, value| {
        project.open_preview = value;
        project.changed("Open preview", value);
    })?;
    handlers.on_toggle_changed("docs_changed", |project: &mut Project, value| {
        project.include_docs = value;
        project.changed("Include documentation", value);
    })?;
    handlers.on_toggle_changed("checks_changed", |project: &mut Project, value| {
        project.strict_checks = value;
        project.changed("Strict checks", value);
    })?;
    handlers.on_activate("apply", |project: &mut Project| {
        project.applied += 1;
        project.dirty = false;
        project.message = format!(
            "Configuration {} applied to Rust state. No files were written.",
            project.applied
        );
        println!("{}", project.message);
    })?;
    handlers.on_activate("defaults", |project: &mut Project| {
        project.auto_save = true;
        project.open_preview = true;
        project.include_docs = true;
        project.strict_checks = true;
        project.dirty = true;
        project.message = "Defaults restored. Apply changes when ready.".into();
    })?;
    handlers.on_activate("preview", |project: &mut Project| {
        project.previews += 1;
        project.message = format!(
            "Preview request {} handled in Rust. No shell commands executed.",
            project.previews
        );
        println!("{}", project.message);
    })?;
    Ok(handlers)
}
fn update(project: &Project, bindings: &mut Bindings) {
    bindings.set_bool("auto_save", project.auto_save);
    bindings.set_bool("open_preview", project.open_preview);
    bindings.set_bool("include_docs", project.include_docs);
    bindings.set_bool("strict_checks", project.strict_checks);
    bindings.set_bool("can_apply", project.dirty);
    bindings.set_text("status", &project.message);
    bindings.set_text(
        "config_state",
        if project.dirty {
            "Unsaved changes"
        } else {
            "Up to date"
        },
    );
    bindings.set_text(
        "applied_count",
        format!("{} configurations applied", project.applied),
    );
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::launch(
        "Volt UI Kit — Workbench",
        include_str!("ui/complete_app.json"),
        Some("examples/ui/complete_app.json"),
        Project::default(),
        handlers()?,
        update,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit::{
        declarative::{Document, Event, Runtime, Value},
        native::RasterPainter,
        Viewport,
    };
    #[test]
    fn document_and_rust_controls_stay_connected() {
        let mut project = Project::default();
        let mut callbacks = handlers().unwrap();
        let mut bindings = Bindings::default();
        update(&project, &mut bindings);
        Runtime::new(
            Document::parse(include_str!("ui/complete_app.json")).unwrap(),
            &callbacks,
            &bindings,
        )
        .unwrap();
        callbacks
            .dispatch("docs_changed", Event::ToggleChanged(false), &mut project)
            .unwrap();
        update(&project, &mut bindings);
        assert_eq!(bindings.get("include_docs"), Some(&Value::Bool(false)));
        assert_eq!(bindings.get("can_apply"), Some(&Value::Bool(true)));
        callbacks
            .dispatch("apply", Event::Activate, &mut project)
            .unwrap();
        assert!(!project.dirty);
        assert_eq!(project.applied, 1);
        callbacks
            .dispatch("defaults", Event::Activate, &mut project)
            .unwrap();
        assert!(project.include_docs && project.dirty);
    }
    #[test]
    fn workbench_keyboard_toggle_and_layout_at_two_scales() {
        let mut project = Project::default();
        let mut callbacks = handlers().unwrap();
        let mut bindings = Bindings::default();
        update(&project, &mut bindings);
        let mut runtime = Runtime::new(
            Document::parse(include_str!("ui/complete_app.json")).unwrap(),
            &callbacks,
            &bindings,
        )
        .unwrap();
        for scale in [1., 2.] {
            let viewport = Viewport::new(960. * scale, 760. * scale, scale).unwrap();
            let mut painter = RasterPainter::new(viewport);
            runtime.layout(viewport, &bindings, &mut painter).unwrap();
            assert_eq!(runtime.boxes().len(), 40);
            for id in [
                "auto_save_control",
                "preview_control",
                "docs_control",
                "checks_control",
                "apply_button",
                "feedback_message",
            ] {
                let item = runtime.boxes().iter().find(|item| item.id == id).unwrap();
                let [x, y, width, height] = item.bounds.bounds();
                assert!(width > 0. && height > 0.);
                assert!(item.clip.contains(x + width * 0.5, y + height * 0.5));
            }
        }
        runtime.focus_next(false);
        assert_eq!(runtime.focused(), Some("auto_save_control"));
        runtime.key_down();
        runtime.key_up(&mut callbacks, &mut project).unwrap();
        assert!(!project.auto_save && project.dirty);
    }
}
