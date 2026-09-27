use std::ffi::OsStr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    #[serde(default = "default_font_family")]
    pub family: String,
    #[serde(default = "default_font_size")]
    pub size: f32,
}
fn default_font_family() -> String {
    "monospace".to_string()
}
fn default_font_size() -> f32 {
    14.0
}
impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: default_font_family(),
            size: default_font_size(),
        }
    }
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
fn default_shell_args() -> Vec<String> {
    vec!["-l".to_string()]
}
impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            program: default_shell(),
            args: default_shell_args(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    #[serde(default = "default_padding")]
    pub padding: u16,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "default_blur_amount")]
    pub blur_amount: f32,
    #[serde(default)]
    pub transparent: bool,
    #[serde(default)]
    pub blur: bool,
    #[serde(default)]
    pub cursor_style: CursorStyle,
    #[serde(default = "default_cursor_blink")]
    pub cursor_blink: bool,
    /// Use the native macOS window tab bar instead of the custom GPU-rendered one.
    /// Only has effect on macOS. Default: false.
    #[serde(default = "default_native_tabs")]
    pub native_tabs: bool,
    /// Alpha (opacity) of the idle pane divider line, 0.0–1.0. Default: 0.09.
    #[serde(default = "default_divider_opacity")]
    pub divider_opacity: f32,
}
fn default_padding() -> u16 {
    8
}
fn default_line_height() -> f32 {
    1.4
}
fn default_opacity() -> f32 {
    1.0
}
fn default_blur_amount() -> f32 {
    0.0
}
fn default_cursor_blink() -> bool {
    true
}
fn default_native_tabs() -> bool {
    false
}
fn default_divider_opacity() -> f32 {
    0.09
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CursorStyle {
    #[default]
    Block,
    Underline,
    #[serde(alias = "line")]
    Beam,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            padding: default_padding(),
            line_height: default_line_height(),
            opacity: default_opacity(),
            blur_amount: default_blur_amount(),
            transparent: false,
            blur: false,
            cursor_style: CursorStyle::Block,
            cursor_blink: default_cursor_blink(),
            native_tabs: default_native_tabs(),
            divider_opacity: default_divider_opacity(),
        }
    }
}

impl AppearanceConfig {
    pub fn effective_opacity(&self) -> f32 {
        let o = self.opacity.clamp(0.0, 1.0);
        if o < 1.0 {
            o
        } else if self.transparent {
            0.86
        } else {
            1.0
        }
    }

    pub fn transparent_enabled(&self) -> bool {
        self.transparent || self.opacity < 1.0
    }

    pub fn blur_enabled(&self) -> bool {
        self.blur || self.blur_amount > 0.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Number of lines to keep in the scrollback buffer per pane.
    /// Increase this for high-output commands like `adb logcat`.
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: usize,
}
fn default_scrollback_lines() -> usize {
    10_000
}
impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            scrollback_lines: default_scrollback_lines(),
        }
    }
}

/// Workspace inspector placement. Floating never reserves terminal cells.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceLayout {
    #[default]
    Docked,
    Floating,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub layout: WorkspaceLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub font: FontConfig,
    #[serde(default)]
    pub shell: ShellConfig,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub appearance: AppearanceConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub workspace: WorkspaceConfig,
}
fn default_theme() -> String {
    "catppuccin".to_string()
}

/// Configuration for the AI Chat Panel feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// Base URL of an OpenAI-compatible API endpoint.
    /// Example: "https://api.openai.com/v1"
    #[serde(default = "default_ai_endpoint")]
    pub endpoint: String,
    /// API key (Bearer token). Empty string disables authentication.
    #[serde(default)]
    pub api_key: String,
    /// Model name to request.
    #[serde(default = "default_ai_model")]
    pub model: String,
    /// Number of terminal scrollback lines to inject as context with each query.
    #[serde(default = "default_ai_context_lines")]
    pub context_lines: usize,
}

fn default_ai_endpoint() -> String {
    "https://api.openai.com/v1".to_string()
}
fn default_ai_model() -> String {
    "gpt-4o-mini".to_string()
}
fn default_ai_context_lines() -> usize {
    50
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            endpoint: default_ai_endpoint(),
            api_key: String::new(),
            model: default_ai_model(),
            context_lines: default_ai_context_lines(),
        }
    }
}

fn is_value_boundary(ch: char) -> bool {
    matches!(ch, '=' | ',' | '[' | '{' | '(' | ':')
}

fn should_prefix_leading_zero(last_non_ws: Option<char>, prev_non_ws: Option<char>) -> bool {
    match last_non_ws {
        None => true,
        Some(ch) if is_value_boundary(ch) => true,
        Some('+' | '-') => match prev_non_ws {
            None => true,
            Some(prev) if is_value_boundary(prev) => true,
            _ => false,
        },
        _ => false,
    }
}

fn normalize_leading_dot_float_literals(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut in_comment = false;
    let mut in_basic_string = false;
    let mut in_literal_string = false;
    let mut escaped = false;
    let mut last_non_ws: Option<char> = None;
    let mut prev_non_ws: Option<char> = None;

    for (i, ch) in chars.iter().copied().enumerate() {
        if in_comment {
            out.push(ch);
            if ch == '\n' {
                in_comment = false;
            }
            continue;
        }

        if in_basic_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_basic_string = false;
            }
            continue;
        }

        if in_literal_string {
            out.push(ch);
            if ch == '\'' {
                in_literal_string = false;
            }
            continue;
        }

        if ch == '#' {
            in_comment = true;
            out.push(ch);
            prev_non_ws = last_non_ws;
            last_non_ws = Some(ch);
            continue;
        }
        if ch == '"' {
            in_basic_string = true;
            out.push(ch);
            prev_non_ws = last_non_ws;
            last_non_ws = Some(ch);
            continue;
        }
        if ch == '\'' {
            in_literal_string = true;
            out.push(ch);
            prev_non_ws = last_non_ws;
            last_non_ws = Some(ch);
            continue;
        }

        if ch == '.'
            && chars.get(i + 1).is_some_and(|next| next.is_ascii_digit())
            && should_prefix_leading_zero(last_non_ws, prev_non_ws)
        {
            out.push('0');
            prev_non_ws = last_non_ws;
            last_non_ws = Some('0');
        }

        out.push(ch);
        if !ch.is_whitespace() {
            prev_non_ws = last_non_ws;
            last_non_ws = Some(ch);
        }
    }

    out
}

fn parse_config_with_compat(input: &str) -> Result<Config, toml::de::Error> {
    match toml::from_str(input) {
        Ok(cfg) => Ok(cfg),
        Err(primary_err) => {
            let normalized = normalize_leading_dot_float_literals(input);
            if normalized == input {
                Err(primary_err)
            } else {
                toml::from_str(&normalized).map_err(|_| primary_err)
            }
        }
    }
}

fn resolve_config_dir(
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
    platform_config_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(xdg) = xdg_config_home {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("volt"));
        }
    }

    if let Some(home) = home {
        if !home.is_empty() {
            return Some(PathBuf::from(home).join(".config").join("volt"));
        }
    }

    platform_config_dir.map(|d| d.join("volt"))
}

pub fn config_dir() -> Option<PathBuf> {
    let xdg = std::env::var_os("XDG_CONFIG_HOME");
    let home = std::env::var_os("HOME");
    resolve_config_dir(xdg.as_deref(), home.as_deref(), dirs::config_dir())
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.toml"))
}

/// Open the same existing config that `load_with_diagnostics` prefers. When a
/// legacy file is still in use, opening Settings must not create a new default
/// primary file that silently shadows the user's old settings on reload.
pub fn config_path_to_edit() -> Option<PathBuf> {
    select_config_path_to_edit(config_path(), legacy_config_path())
}

fn select_config_path_to_edit(
    primary: Option<PathBuf>,
    legacy: Option<PathBuf>,
) -> Option<PathBuf> {
    match (primary, legacy) {
        (Some(primary), Some(legacy)) if !primary.exists() && legacy.exists() => Some(legacy),
        (Some(primary), _) => Some(primary),
        (None, legacy) => legacy,
    }
}

fn legacy_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("volt").join("config.toml"))
}

/// Returns a fully-commented sample config for first-time setup.
pub fn sample_config_toml() -> String {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    format!(
        r#"# Volt Terminal — Configuration
# Edit this file, save, then press Cmd+Shift+R in Volt to reload.
# All values shown are the defaults.

# ── Theme ──────────────────────────────────────────────────────────────────────
# Options: catppuccin  tokyo-night  gruvbox  nord  dracula
theme = "catppuccin"

# ── Font ───────────────────────────────────────────────────────────────────────
[font]
# Font family name exactly as shown in Font Book (macOS) or fc-list (Linux).
# For Starship / powerline icons set a Nerd Font variant, e.g.:
#   "JetBrainsMono Nerd Font Mono"
#   "SFMono Nerd Font"
#   "FiraCode Nerd Font Mono"
#   "Hack Nerd Font Mono"
family = "monospace"

# Font size in logical points (Retina / HiDPI scaling is automatic).
size = 14.0

# ── Shell ──────────────────────────────────────────────────────────────────────
[shell]
# Path to the shell binary.
program = "{shell}"

# Shell startup arguments.
# "-l" starts a login shell — needed for $PATH, Starship, nvm, homebrew, etc.
args = ["-l"]

# ── Appearance ─────────────────────────────────────────────────────────────────
[appearance]
# Inner padding around the terminal grid, in logical pixels.
padding = 8

# Line height multiplier.
# 1.0 = tight  |  1.2 = snug  |  1.4 = comfortable  |  1.6 = airy
line_height = 1.4

# Window background opacity.
# 1.0 = fully opaque, 0.0 = fully transparent
opacity = 1.0

# Blur amount. 0.0 disables blur.
# (macOS compositor blur is currently enabled/disabled by this value.)
blur_amount = 0.0

# macOS visual effects (window compositor effects)
# Compatibility booleans from older config versions still work.
transparent = false
blur = false

# Cursor style: "block", "underline", or "beam" (alias: "line")
cursor_style = "block"

# Blink cursor when focused
cursor_blink = true

# Use the native macOS window tab bar (macOS only). Default is false to keep
# the Ghostty-style custom curved tab strip integrated into the title area.
# When true, each new tab opens as a native OS window grouped in the system tab strip.
# native_tabs = false

# Opacity of the idle pane-split divider line (0.0 = invisible, 1.0 = opaque).
# The hover and drag states scale from this value automatically.
# divider_opacity = 0.09

# ── Terminal ────────────────────────────────────────────────────────────────────
[workspace]
# "docked" reserves space; "floating" overlays and can be dragged by its header.
layout = "docked"

[terminal]
# Number of lines retained in the scrollback buffer per pane.
# Increase this for high-output commands such as `adb logcat`.
# Memory usage is roughly: scrollback_lines × terminal_cols × 24 bytes.
scrollback_lines = 10000
"#,
        shell = shell
    )
}

impl Config {
    pub fn load_with_diagnostics() -> (Self, Option<String>) {
        let mut paths = Vec::new();
        if let Some(path) = config_path() {
            paths.push(path);
        }
        if let Some(path) = legacy_config_path() {
            if !paths.iter().any(|candidate| candidate == &path) {
                paths.push(path);
            }
        }

        let mut first_error: Option<String> = None;
        for path in paths {
            match std::fs::read_to_string(&path) {
                Ok(s) => match parse_config_with_compat(&s) {
                    Ok(c) => return (c, None),
                    Err(err) => {
                        eprintln!("volt-config: failed to parse {}: {err}", path.display());
                        if first_error.is_none() {
                            first_error =
                                Some(format!("Config parse error in {}: {err}", path.display()));
                        }
                    }
                },
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    eprintln!("volt-config: failed to read {}: {err}", path.display());
                    if first_error.is_none() {
                        first_error =
                            Some(format!("Config read error in {}: {err}", path.display()));
                    }
                }
            }
        }

        (Self::default(), first_error)
    }

    pub fn load() -> Self {
        Self::load_with_diagnostics().0
    }

    pub fn save(&self) {
        let Some(dir) = config_dir() else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        let path = config_path().unwrap_or_else(|| dir.join("config.toml"));
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = std::fs::write(path, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_prefers_existing_legacy_config_over_creating_shadow_file() {
        let dir = std::env::temp_dir().join(format!(
            "volt-config-legacy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let primary = dir.join("preferred.toml");
        let legacy = dir.join("legacy.toml");
        std::fs::write(&legacy, "theme = \"dracula\"\n").unwrap();
        assert_eq!(
            select_config_path_to_edit(Some(primary.clone()), Some(legacy.clone())),
            Some(legacy.clone())
        );
        std::fs::write(&primary, "theme = \"nord\"\n").unwrap();
        assert_eq!(
            select_config_path_to_edit(Some(primary.clone()), Some(legacy)),
            Some(primary)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

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
        let mut c = Config {
            theme: "tokyo-night".to_string(),
            ..Config::default()
        };
        c.font.size = 18.0;
        let s = toml::to_string_pretty(&c).unwrap();
        let c2: Config = toml::from_str(&s).unwrap();
        assert_eq!(c2.theme, "tokyo-night");
        assert_eq!(c2.font.size, 18.0);
    }

    #[test]
    fn test_resolve_config_dir_prefers_xdg() {
        let dir = resolve_config_dir(
            Some(OsStr::new("/tmp/xdg")),
            Some(OsStr::new("/tmp/home")),
            Some(PathBuf::from("/tmp/platform")),
        )
        .unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/xdg/volt"));
    }

    #[test]
    fn test_resolve_config_dir_uses_home_when_xdg_missing() {
        let dir = resolve_config_dir(
            None,
            Some(OsStr::new("/tmp/home")),
            Some(PathBuf::from("/tmp/platform")),
        )
        .unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/home/.config/volt"));
    }

    #[test]
    fn test_resolve_config_dir_falls_back_to_platform() {
        let dir = resolve_config_dir(None, None, Some(PathBuf::from("/tmp/platform"))).unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/platform/volt"));
    }

    #[test]
    fn test_config_from_toml_accepts_leading_dot_floats() {
        let toml = r#"
            [appearance]
            opacity = .90
            blur_amount = -.25
        "#;
        let c = parse_config_with_compat(toml).unwrap();
        assert!((c.appearance.opacity - 0.90).abs() < f32::EPSILON);
        assert!((c.appearance.blur_amount + 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize_leading_dot_float_literals_ignores_strings_and_comments() {
        let raw = r#"
            [appearance]
            # keep .90 unchanged in comment
            note = ".90 should stay as text"
            opacity = .90
        "#;
        let normalized = normalize_leading_dot_float_literals(raw);
        assert!(normalized.contains("opacity = 0.90"));
        assert!(normalized.contains("# keep .90 unchanged in comment"));
        assert!(normalized.contains(r#"note = ".90 should stay as text""#));
    }
}
