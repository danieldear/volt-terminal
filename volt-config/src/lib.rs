pub mod config;
pub mod keybindings;
pub mod theme;

pub use config::{
    config_dir, config_path, config_path_to_edit, sample_config_toml, AiConfig, AppearanceConfig,
    Config, CursorStyle,
};
pub use theme::{Color, Theme};
