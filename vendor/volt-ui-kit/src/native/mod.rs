//! Optional native host: software rasterization or opt-in Vello GPU drawing.
//! Native backdrop blur is an OS request, not a widget-renderer blur simulation.
mod accessibility;
mod application;
pub use application::{Application, RendererPreference};
mod commands;
pub use commands::{
    ApplicationRequest, CloseDecision, CloseReason, Command, CommandDispatch, CommandInfo,
    CommandRegistry, Shortcut,
};
mod assets;
pub mod file_dialog;
pub mod files;
#[cfg(target_os = "macos")]
mod macos_chrome;
#[cfg(all(target_os = "macos", feature = "native-gpu"))]
mod macos_material;
pub mod notifications;
mod raster;
pub mod system;
pub use assets::{decode_image, load_image};
#[cfg(feature = "native-gpu")]
mod gpu;
#[cfg(feature = "native-gpu")]
pub use gpu::GpuOffscreenRenderer;
#[cfg(feature = "native-gpu")]
mod gpu_scene;
#[cfg(all(test, feature = "native-gpu"))]
mod gpu_tests;
mod messages;
pub use messages::{UiSendError, UiSender};
mod window;
pub use raster::{register_font, Damage, RasterPainter};
pub use window::{
    load_document, run, run_ui, run_ui_with_messages, run_ui_with_messages_and_theme,
    run_ui_with_options, run_ui_with_theme, run_with_options, NativeOptions, SurfaceOptions,
    WindowBackground, WindowChrome, WindowOptions,
};
