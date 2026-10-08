//! Volt's desktop visual language as an embeddable, renderer-independent toolkit.
//! Models, pointer geometry, vector icons and widget drawing contain no terminal
//! state, file access, shell commands, window lifecycle or background timers.
//! Implement [`paint::Painter`] to draw into an existing application's backend.
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]
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

pub mod geometry;
pub mod panel;
pub mod surface;
pub use geometry::{HitMap, HitTarget, Rect};
pub use panel::{BackdropRequest, Panel};
pub use surface::{SurfaceStyle, Tint, TintPicker};
