//! Editable type/color/surface styles and stateful controls; no WYSIWYG required.
mod support;
use volt_ui_kit::declarative::{Bindings, HandlerError, Handlers};

struct Demo {
    notifications: bool,
    keyboard_hints: bool,
    approvals: bool,
    changes: usize,
    message: String,
}
impl Default for Demo {
    fn default() -> Self {
        Self {
            notifications: true,
            keyboard_hints: true,
            approvals: false,
            changes: 0,
            message: "Use a control, or edit the JSON and press F5.".into(),
        }
    }
}
impl Demo {
    fn changed(&mut self, label: &str, value: bool) {
        self.changes += 1;
        self.message = format!(
            "{label}: {}. Event {} received by Rust.",
            if value { "on" } else { "off" },
            self.changes
        );
    }
}
fn handlers() -> Result<Handlers<'static, Demo>, HandlerError> {
    let mut callbacks = Handlers::default();
    callbacks.on_toggle_changed("notifications_changed", |demo: &mut Demo, value| {
        demo.notifications = value;
        demo.changed("Notifications", value);
    })?;
    callbacks.on_toggle_changed("hints_changed", |demo: &mut Demo, value| {
        demo.keyboard_hints = value;
        demo.changed("Keyboard hints", value);
    })?;
    callbacks.on_toggle_changed("approval_changed", |demo: &mut Demo, value| {
        demo.approvals = value;
        demo.changed("Require approval", value);
    })?;
    callbacks.on_activate("primary", |demo: &mut Demo| {
        demo.changes += 1;
        demo.message = "Primary button activated. Your Rust callback owns the behavior.".into();
    })?;
    callbacks.on_activate("secondary", |demo: &mut Demo| {
        demo.changes += 1;
        demo.message = "Secondary button activated with the same typed event contract.".into();
    })?;
    callbacks.on_activate("quiet", |demo: &mut Demo| {
        demo.changes += 1;
        demo.message = "Quiet button activated. Styling does not change event behavior.".into();
    })?;
    Ok(callbacks)
}
fn update(demo: &Demo, bindings: &mut Bindings) {
    bindings.set_bool("notifications", demo.notifications);
    bindings.set_bool("keyboard_hints", demo.keyboard_hints);
    bindings.set_bool("approvals", demo.approvals);
    bindings.set_text("status", &demo.message);
    bindings.set_text("event_count", format!("{} events handled", demo.changes));
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::launch(
        "Volt UI Kit — Components & Styling",
        include_str!("ui/components.json"),
        Some("examples/ui/components.json"),
        Demo::default(),
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
    fn all_editable_components_connect_to_rust() {
        let mut demo = Demo::default();
        let mut callbacks = handlers().unwrap();
        let mut bindings = Bindings::default();
        update(&demo, &mut bindings);
        Runtime::new(
            Document::parse(include_str!("ui/components.json")).unwrap(),
            &callbacks,
            &bindings,
        )
        .unwrap();
        callbacks
            .dispatch("approval_changed", Event::ToggleChanged(true), &mut demo)
            .unwrap();
        callbacks
            .dispatch("primary", Event::Activate, &mut demo)
            .unwrap();
        update(&demo, &mut bindings);
        assert_eq!(bindings.get("approvals"), Some(&Value::Bool(true)));
        assert_eq!(demo.changes, 2);
    }
    #[test]
    fn component_keyboard_toggle_and_layout_at_two_scales() {
        let mut demo = Demo::default();
        let mut callbacks = handlers().unwrap();
        let mut bindings = Bindings::default();
        update(&demo, &mut bindings);
        let mut runtime = Runtime::new(
            Document::parse(include_str!("ui/components.json")).unwrap(),
            &callbacks,
            &bindings,
        )
        .unwrap();
        for scale in [1., 2.] {
            let viewport = Viewport::new(960. * scale, 760. * scale, scale).unwrap();
            let mut painter = RasterPainter::new(viewport);
            runtime.layout(viewport, &bindings, &mut painter).unwrap();
            assert_eq!(runtime.boxes().len(), 32);
            assert!(runtime
                .boxes()
                .iter()
                .any(|item| item.id == "italic_sample"));
            assert!(runtime
                .boxes()
                .iter()
                .any(|item| item.id == "disabled_control" && !item.enabled));
        }
        runtime.focus_next(false);
        assert_eq!(runtime.focused(), Some("notification_control"));
        runtime.key_down();
        runtime.key_up(&mut callbacks, &mut demo).unwrap();
        assert!(!demo.notifications);
        assert_eq!(demo.changes, 1);
    }
}
