pub mod config;
pub mod keybindings;
pub mod preferences;
pub mod tasks;
pub mod theme;
pub mod themes;

pub use config::{
    config_dir, config_path, config_path_to_edit, sample_config_toml, set_theme_in_config_file,
    AiConfig, AppearanceConfig, Config, CursorStyle, EditorConfig,
};
pub use theme::{Color, Theme, BUILTIN_THEMES};
pub use themes::{ThemeEntry, ThemeRegistry, ThemeSource};
