#![cfg(feature = "native")]
use volt_ui_kit::native::{Application, RendererPreference, WindowBackground};
use volt_ui_kit::ui::prelude::*;
#[test]
fn application_configuration_is_additive_and_developer_owned() {
    let app = Application::new("Inbox", (), |_| text("Mail").into())
        .renderer(RendererPreference::Software)
        .background(WindowBackground::Blur { opacity: 0.65 });
    assert_eq!(
        app.options().background,
        WindowBackground::Blur { opacity: 0.65 }
    );
    assert_eq!(app.renderer_preference(), RendererPreference::Software);
    assert!(app.validate().is_ok());
    assert_eq!(
        Application::new("App", (), |_| text("Hi").into())
            .options()
            .theme,
        ThemeMode::System
    );
}
#[test]
fn invalid_application_configuration_fails_before_creating_a_window() {
    let app = Application::new("App", (), |_| text("Hi").into())
        .background(WindowBackground::Transparent { opacity: f32::NAN });
    assert!(app.validate().is_err());
    for title in ["", " \t", "bad\nname"] {
        assert!(Application::new(title, (), |_| text("Hi").into())
            .validate()
            .is_err());
    }
    for size in [(0, 400), (400, 0), (16385, 400)] {
        assert!(Application::new("App", (), |_| text("Hi").into())
            .size(size.0, size.1)
            .validate()
            .is_err());
    }
}

#[test]
fn required_renderer_and_unsupported_services_report_missing_capabilities() {
    let app = Application::new("App", (), |_| text("Hi").into())
        .renderer(RendererPreference::GpuRequired);
    assert_eq!(app.validate().is_ok(), cfg!(feature = "native-gpu"));
    let app = Application::new("App", (), |_| text("Hi").into()).desktop(
        volt_ui_kit::native::system::DesktopConfig::new("App"),
        |_, _| {},
    );
    assert_eq!(
        app.validate().is_ok(),
        cfg!(all(target_os = "macos", feature = "native-desktop"))
    );
}
