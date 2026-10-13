//! Native desktop interfaces in plain Rust, drawn on the GPU.
//!
//! Write a function from your application state to a tree of elements. The kit
//! lays it out, draws it with Vello on Metal, DX12 or Vulkan (or on the CPU),
//! handles input, focus and accessibility, and calls your closures when
//! something happens.
//!
//! | Feature | Adds |
//! | --- | --- |
//! | `native` | A window host with software drawing, text, images, clipboard, IME and accessibility |
//! | `native-gpu` | GPU drawing with Vello (implies `native`) |
//! | `native-desktop` | macOS menus, tray and file panels |
//! | `native-notifications` | macOS local notifications |
//! | `rust-ui` | The `ui` builders without a window, for custom hosts |
//! | `declarative` | The validated tree, runtime and optional JSON frontend |
//! | `dev-tools` | The `cargo volt` command |
//!
//! With no features, the crate is a small set of drawing recipes and geometry:
//! implement [`paint::Painter`] to draw them with an existing renderer.
//!
#![cfg_attr(
    feature = "native",
    doc = "Start with the [`guide`], beginning at [`guide::getting_started`]."
)]
#![cfg_attr(
    not(feature = "native"),
    doc = "The guides are in the repository's `docs/` folder, and in the `guide` module when built with the `native` feature."
)]
#![forbid(unsafe_code)]
pub mod card_paint;
pub mod form_paint;
pub mod paint;
pub mod search_paint;
pub mod search_palette;
pub mod settings;
pub mod settings_paint;
pub mod strip_paint;
pub mod style;
pub mod svg;
pub mod tab_color;
pub mod tab_layout;
pub mod task_form;
pub mod task_strip;
pub mod workspace_card;

pub use paint::{Painter, Viewport};
pub use style::{Colors, Theme};
pub use workspace_card::{CardIcon, CardTone};

pub mod button;
pub use button::{Button, ButtonColors};

#[cfg(feature = "declarative")]
mod fasthash;
pub mod geometry;
pub mod graphics;
pub mod theme;
pub use theme::{contrast_ratio, hex, parse_hex, ColorToken, Palette, ThemeMode, UiTheme};
pub mod panel;
pub mod surface;
pub use geometry::{HitMap, HitTarget, Rect};
pub use panel::{BackdropRequest, Panel};
pub use surface::{SurfaceStyle, Tint, TintPicker};

/// Opt-in validated document tree, bindings and interaction runtime; no event loop.
#[cfg(feature = "declarative")]
pub mod declarative;

/// Primary Rust-first declarative composition API; opt-in and host-independent.
#[cfg(feature = "rust-ui")]
pub mod ui;

/// Optional standalone software-rendered desktop host. Never used by legacy consumers.
#[cfg(feature = "native")]
pub mod native;

/// The guides from the repository's `docs/` folder. Every Rust example in
/// them is compiled and run as a doctest.
#[cfg(feature = "native")]
pub mod guide {
    #[doc = include_str!("../docs/getting-started.md")]
    pub mod getting_started {}
    #[doc = include_str!("../docs/building-uis.md")]
    pub mod building_uis {}
    #[doc = include_str!("../docs/components.md")]
    pub mod components {}
    #[doc = include_str!("../docs/styling-and-themes.md")]
    pub mod styling_and_themes {}
    #[doc = include_str!("../docs/windows-and-rendering.md")]
    pub mod windows_and_rendering {}
    #[doc = include_str!("../docs/app-services.md")]
    pub mod app_services {}
    #[doc = include_str!("../docs/json-ui.md")]
    pub mod json_ui {}
    #[doc = include_str!("../docs/architecture.md")]
    pub mod architecture {}
}
