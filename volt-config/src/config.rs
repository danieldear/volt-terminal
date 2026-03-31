use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    #[serde(default = "default_font_family")]
    pub family: String,
    #[serde(default = "default_font_size")]
    pub size: f32,
}
fn default_font_family() -> String { "monospace".to_string() }
fn default_font_size() -> f32 { 14.0 }
impl Default for FontConfig {
    fn default() -> Self { Self { family: default_font_family(), size: default_font_size() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    #[serde(default = "default_shell")]
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}
fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}
impl Default for ShellConfig {
    fn default() -> Self { Self { program: default_shell(), args: vec![] } }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub font: FontConfig,
    #[serde(default)]
    pub shell: ShellConfig,
    #[serde(default = "default_theme")]
    pub theme: String,
}
fn default_theme() -> String { "catppuccin".to_string() }

impl Config {
    pub fn load() -> Self {
        let path = dirs::config_dir()
            .map(|d| d.join("volt").join("config.toml"));
        if let Some(path) = path {
            if let Ok(s) = std::fs::read_to_string(path) {
                if let Ok(c) = toml::from_str(&s) {
                    return c;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let Some(dir) = dirs::config_dir().map(|d| d.join("volt")) else { return };
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = std::fs::write(path, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let c = Config::default();
        assert_eq!(c.font.size, 14.0);
        assert!(!c.shell.program.is_empty());
    }

    #[test]
    fn test_config_from_toml() {
        let toml = r#"
            [font]
            family = "JetBrains Mono"
            size = 16.0
            [shell]
            program = "/bin/bash"
        "#;
        let c: Config = toml::from_str(toml).unwrap();
        assert_eq!(c.font.family, "JetBrains Mono");
        assert_eq!(c.font.size, 16.0);
        assert_eq!(c.shell.program, "/bin/bash");
    }

    #[test]
    fn test_config_roundtrip() {
        let mut c = Config::default();
        c.theme = "tokyo-night".to_string();
        c.font.size = 18.0;
        let s = toml::to_string_pretty(&c).unwrap();
        let c2: Config = toml::from_str(&s).unwrap();
        assert_eq!(c2.theme, "tokyo-night");
        assert_eq!(c2.font.size, 18.0);
    }
}
