pub mod config;
pub mod theme;

pub use config::{
    config_dir, config_path, sample_config_toml, AiConfig, AppearanceConfig, Config, CursorStyle,
};
pub use theme::{Color, Theme};
