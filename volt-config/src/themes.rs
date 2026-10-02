//! User themes on disk (`<config dir>/themes/<id>.toml`) and the combined
//! built-in + user list that the theme chooser and editor work from.
//!
//! Reading is defensive in the same way as the rest of the config: a bounded
//! number of files, a size cap, regular files only (symlinks are skipped), and
//! a strict schema. A file that fails any check is skipped and reported in
//! `problems()`, never half-loaded.
//!
//! File format:
//!
//! ```toml
//! name = "Midnight Clay"
//! background = "#1a1715"
//! foreground = "#e8ddd3"
//! cursor = "#d97757"
//! cursor_text = "#1a1715"
//! selection_background = "#4a3a32"
//! selection_foreground = "#f2ebe4"
//! # black, red, green, yellow, blue, magenta, cyan, white, then bright variants
//! ansi = ["#2b2522", "#e06c5a", …16 colors…]
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::theme::{Color, Theme, BUILTIN_THEMES};

pub const MAX_THEME_FILES: usize = 256;
pub const MAX_THEME_FILE_BYTES: u64 = 64 * 1024;
pub const MAX_THEME_NAME_CHARS: usize = 64;
const MAX_ID_CHARS: usize = 64;
/// Slugs are truncated so a `-custom` / `-NN` uniqueness suffix still fits.
const MAX_SLUG_CHARS: usize = 48;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    name: String,
    background: Color,
    foreground: Color,
    cursor: Color,
    cursor_text: Color,
    selection_background: Color,
    selection_foreground: Color,
    ansi: [Color; 16],
}

impl ThemeFile {
    fn into_theme(self) -> Theme {
        Theme {
            background: self.background,
            foreground: self.foreground,
            cursor: self.cursor,
            cursor_text: self.cursor_text,
            selection_bg: self.selection_background,
            selection_fg: self.selection_foreground,
            ansi: self.ansi,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeSource {
    BuiltIn,
    File(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ThemeEntry {
    /// The value written as `theme = "<id>"` in the config file.
    pub id: String,
    pub name: String,
    pub source: ThemeSource,
    pub theme: Theme,
}

impl ThemeEntry {
    pub fn is_builtin(&self) -> bool {
        self.source == ThemeSource::BuiltIn
    }
}

/// Every theme Volt can show: the built-ins first, then user files sorted
/// by id. Rebuilt from disk on startup, on settings reload, and after a save.
#[derive(Debug, Clone)]
pub struct ThemeRegistry {
    entries: Vec<ThemeEntry>,
    problems: Vec<String>,
}

pub fn themes_dir() -> Option<PathBuf> {
    crate::config::config_dir().map(|d| d.join("themes"))
}

pub fn is_builtin_id(id: &str) -> bool {
    BUILTIN_THEMES.iter().any(|(builtin, _)| *builtin == id)
}

/// Ids are lowercase ASCII slugs, so they are always safe as a file stem and
/// as a TOML string written without escaping.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_CHARS
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Derive a file id from a display name: ASCII letters and digits are kept
/// (lowercased), every other run of characters becomes one `-`.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if slug.len() >= MAX_SLUG_CHARS {
                break;
            }
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        "theme".to_string()
    } else {
        slug
    }
}

/// Trim and validate a display name for saving.
pub fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Theme name can't be empty".into());
    }
    if name.chars().count() > MAX_THEME_NAME_CHARS {
        return Err(format!(
            "Theme name is longer than {MAX_THEME_NAME_CHARS} characters"
        ));
    }
    if name.chars().any(char::is_control) {
        return Err("Theme name can't contain control characters".into());
    }
    Ok(name.to_string())
}

impl ThemeRegistry {
    pub fn load() -> Self {
        Self::load_from(themes_dir().as_deref())
    }

    /// Built-ins plus every valid theme file in `dir` (if it exists).
    pub fn load_from(dir: Option<&Path>) -> Self {
        let mut entries: Vec<ThemeEntry> = BUILTIN_THEMES
            .iter()
            .map(|(id, name)| ThemeEntry {
                id: (*id).to_string(),
                name: (*name).to_string(),
                source: ThemeSource::BuiltIn,
                theme: Theme::builtin(id).expect("every BUILTIN_THEMES id resolves"),
            })
            .collect();
        let mut problems = Vec::new();
        if let Some(dir) = dir {
            load_dir(dir, &mut entries, &mut problems);
        }
        Self { entries, problems }
    }

    pub fn entries(&self) -> &[ThemeEntry] {
        &self.entries
    }

    /// Files that were skipped, with a short reason each.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    pub fn get(&self, id: &str) -> Option<&ThemeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The theme for a config id. Unknown ids fall back to Catppuccin, the
    /// same behaviour an unknown name has always had.
    pub fn resolve(&self, id: &str) -> Theme {
        self.get(id)
            .map(|e| e.theme.clone())
            .unwrap_or_else(Theme::dark)
    }
}

fn load_dir(dir: &Path, entries: &mut Vec<ThemeEntry>, problems: &mut Vec<String>) {
    let read = match std::fs::read_dir(dir) {
        Ok(read) => read,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            problems.push(format!("{}: {err}", dir.display()));
            return;
        }
    };
    let mut paths: Vec<PathBuf> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();
    if paths.len() > MAX_THEME_FILES {
        problems.push(format!(
            "{}: only the first {MAX_THEME_FILES} theme files are loaded",
            dir.display()
        ));
        paths.truncate(MAX_THEME_FILES);
    }
    for path in paths {
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match load_file(&path) {
            Ok((id, file)) => entries.push(ThemeEntry {
                id,
                name: file.name.clone(),
                source: ThemeSource::File(path),
                theme: file.into_theme(),
            }),
            Err(reason) => problems.push(format!("themes/{label}: {reason}")),
        }
    }
}

fn load_file(path: &Path) -> Result<(String, ThemeFile), String> {
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("file name isn't valid UTF-8")?
        .to_string();
    if !is_valid_id(&id) {
        return Err("file name must be lowercase letters, digits and dashes".into());
    }
    if is_builtin_id(&id) {
        return Err(format!(
            "\"{id}\" is a built-in theme name; rename the file"
        ));
    }
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a regular file (symlinks are skipped)".into());
    }
    if meta.len() > MAX_THEME_FILE_BYTES {
        return Err(format!("larger than {} KiB", MAX_THEME_FILE_BYTES / 1024));
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let file: ThemeFile = toml::from_str(&text).map_err(|e| e.message().to_string())?;
    validate_name(&file.name)?;
    Ok((id, file))
}

/// Write `theme` to `<dir>/<id>.toml` and return the id.
///
/// With `overwrite = Some(id)` that user theme file is replaced (built-in ids
/// are refused). Otherwise a new id is derived from `name` that collides with
/// neither a built-in nor an existing file. The write goes to a temporary
/// file first and is renamed into place, so a crash can't leave a truncated
/// theme behind.
pub fn save_theme(
    dir: &Path,
    name: &str,
    theme: &Theme,
    overwrite: Option<&str>,
) -> Result<String, String> {
    let name = validate_name(name)?;
    let id = match overwrite {
        Some(id) if is_valid_id(id) && !is_builtin_id(id) => id.to_string(),
        Some(id) => return Err(format!("\"{id}\" can't be overwritten")),
        None => unique_id(dir, &slugify(&name)),
    };
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let target = dir.join(format!("{id}.toml"));
    // The loader sorts and reads only the first MAX_THEME_FILES files. Refuse
    // a save that would produce a theme the user cannot select after reload.
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    files.retain(|path| path.extension().is_some_and(|ext| ext == "toml"));
    files.sort();
    if overwrite.is_some() {
        let position = files.iter().position(|path| path == &target);
        if position.is_none_or(|position| position >= MAX_THEME_FILES) {
            return Err(format!(
                "themes/{id}.toml is outside the first {MAX_THEME_FILES} theme files"
            ));
        }
        if !std::fs::symlink_metadata(&target).is_ok_and(|meta| meta.is_file()) {
            return Err(format!("themes/{id}.toml is not a regular theme file"));
        }
    } else if files.len() >= MAX_THEME_FILES {
        return Err(format!(
            "the themes folder already has {MAX_THEME_FILES} theme files"
        ));
    }
    let tmp = dir.join(format!(".{id}.toml.tmp"));
    std::fs::write(&tmp, render_theme_file(&name, theme))
        .and_then(|()| std::fs::rename(&tmp, &target))
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{}: {e}", target.display())
        })?;
    Ok(id)
}

fn unique_id(dir: &Path, slug: &str) -> String {
    let base = if is_builtin_id(slug) {
        format!("{slug}-custom")
    } else {
        slug.to_string()
    };
    let free = |id: &str| std::fs::symlink_metadata(dir.join(format!("{id}.toml"))).is_err();
    if free(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| free(id))
        .expect("an unused numeric suffix always exists")
}

/// Hand-written rather than `toml::to_string` so the file keeps a readable
/// layout and comments for anyone editing it by hand.
fn render_theme_file(name: &str, theme: &Theme) -> String {
    let quoted_name = toml::Value::String(name.to_string()).to_string();
    let ansi = |range: std::ops::Range<usize>| {
        theme.ansi[range]
            .iter()
            .map(|c| format!("\"{}\"", c.to_hex()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "# Volt theme. Edit by hand or with Volt ▸ Theme ▸ Customize Theme…,\n\
         # then select it from Volt ▸ Theme.\n\
         name = {quoted_name}\n\
         \n\
         background = \"{}\"\n\
         foreground = \"{}\"\n\
         cursor = \"{}\"\n\
         cursor_text = \"{}\"\n\
         selection_background = \"{}\"\n\
         selection_foreground = \"{}\"\n\
         \n\
         # black, red, green, yellow, blue, magenta, cyan, white — then the bright variants\n\
         ansi = [\n  {},\n  {},\n]\n",
        theme.background.to_hex(),
        theme.foreground.to_hex(),
        theme.cursor.to_hex(),
        theme.cursor_text.to_hex(),
        theme.selection_bg.to_hex(),
        theme.selection_fg.to_hex(),
        ansi(0..8),
        ansi(8..16),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_dir() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "volt-themes-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn sample_theme() -> Theme {
        let mut t = Theme::nord();
        t.background = Color::rgb(0x1a, 0x17, 0x15);
        t.ansi[4] = Color::rgb(0x7f, 0xa3, 0xc7);
        t
    }

    #[test]
    fn builtins_come_first_and_resolve() {
        let reg = ThemeRegistry::load_from(None);
        let ids: Vec<_> = reg.entries().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            ["catppuccin", "tokyo-night", "gruvbox", "nord", "dracula"]
        );
        assert_eq!(reg.resolve("gruvbox"), Theme::gruvbox());
        assert_eq!(reg.resolve("missing"), Theme::dark());
        assert!(reg.problems().is_empty());
    }

    #[test]
    fn save_then_load_round_trips_every_color_and_the_name() {
        let dir = temp_dir();
        let theme = sample_theme();
        let id = save_theme(&dir, "Midnight \"Clay\" ✨", &theme, None).unwrap();
        assert_eq!(id, "midnight-clay");
        let reg = ThemeRegistry::load_from(Some(&dir));
        let entry = reg.get(&id).unwrap();
        assert_eq!(entry.theme, theme);
        assert_eq!(entry.name, "Midnight \"Clay\" ✨");
        assert!(!entry.is_builtin());
        assert!(reg.problems().is_empty(), "{:?}", reg.problems());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn new_saves_never_collide_with_builtins_or_existing_files() {
        let dir = temp_dir();
        let t = sample_theme();
        assert_eq!(save_theme(&dir, "Nord", &t, None).unwrap(), "nord-custom");
        assert_eq!(save_theme(&dir, "Nord", &t, None).unwrap(), "nord-custom-2");
        assert_eq!(save_theme(&dir, "Mine", &t, None).unwrap(), "mine");
        assert_eq!(save_theme(&dir, "mine!", &t, None).unwrap(), "mine-2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn overwrite_replaces_a_user_theme_but_never_a_builtin() {
        let dir = temp_dir();
        let mut t = sample_theme();
        let id = save_theme(&dir, "Mine", &t, None).unwrap();
        t.cursor = Color::rgb(1, 2, 3);
        assert_eq!(save_theme(&dir, "Mine v2", &t, Some(&id)).unwrap(), id);
        let reg = ThemeRegistry::load_from(Some(&dir));
        assert_eq!(reg.get(&id).unwrap().theme.cursor, Color::rgb(1, 2, 3));
        assert_eq!(reg.get(&id).unwrap().name, "Mine v2");
        assert!(save_theme(&dir, "x", &t, Some("dracula")).is_err());
        assert!(save_theme(&dir, "x", &t, Some("../escape")).is_err());
        assert!(!dir.join(".mine.toml.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_files_are_skipped_and_reported_not_half_loaded() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let good = render_theme_file("Good", &sample_theme());
        std::fs::write(dir.join("good.toml"), &good).unwrap();
        std::fs::write(dir.join("nord.toml"), &good).unwrap();
        std::fs::write(dir.join("Bad Name.toml"), &good).unwrap();
        std::fs::write(
            dir.join("short-ansi.toml"),
            good.replace(", \"#eceff4\"", ""),
        )
        .unwrap();
        std::fs::write(dir.join("bad-hex.toml"), good.replace("#1a1715", "#1a17")).unwrap();
        std::fs::write(dir.join("extra.toml"), format!("{good}\nfont = \"x\"\n")).unwrap();
        std::fs::write(
            dir.join("huge.toml"),
            "#".repeat(MAX_THEME_FILE_BYTES as usize + 1),
        )
        .unwrap();
        std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("good.toml"), dir.join("link.toml")).unwrap();

        let reg = ThemeRegistry::load_from(Some(&dir));
        let user: Vec<_> = reg
            .entries()
            .iter()
            .filter(|e| !e.is_builtin())
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(user, ["good"]);
        assert_eq!(
            reg.resolve("nord"),
            Theme::nord(),
            "built-in can't be shadowed"
        );
        let expected_problems = if cfg!(unix) { 7 } else { 6 };
        assert_eq!(
            reg.problems().len(),
            expected_problems,
            "{:?}",
            reg.problems()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn slugs_ids_and_names_are_validated() {
        assert_eq!(slugify("Tokyo Night (mine)"), "tokyo-night-mine");
        assert_eq!(slugify("✨✨"), "theme");
        assert!(slugify(&"a".repeat(200)).len() <= MAX_SLUG_CHARS);
        assert!(is_valid_id("tokyo-night-2"));
        for bad in ["", "-a", "a-", "A", "a b", "../x", "a.b"] {
            assert!(!is_valid_id(bad), "{bad:?}");
        }
        assert!(validate_name("  ").is_err());
        assert!(validate_name("a\nb").is_err());
        assert!(validate_name(&"n".repeat(MAX_THEME_NAME_CHARS + 1)).is_err());
        assert_eq!(validate_name("  Mine ").unwrap(), "Mine");
    }

    #[test]
    fn saves_do_not_succeed_outside_the_loader_limit() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let theme = sample_theme();
        std::fs::write(dir.join("mine.toml"), render_theme_file("Mine", &theme)).unwrap();
        for i in 1..MAX_THEME_FILES {
            std::fs::write(dir.join(format!("z{i:03}.toml")), "# counted by loader\n").unwrap();
        }
        assert!(save_theme(&dir, "Another", &theme, None).is_err());
        assert!(!dir.join("another.toml").exists());
        assert_eq!(
            save_theme(&dir, "Mine", &theme, Some("mine")).unwrap(),
            "mine"
        );
        let reg = ThemeRegistry::load_from(Some(&dir));
        assert!(reg.get("mine").is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}
