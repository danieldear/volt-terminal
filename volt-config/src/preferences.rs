//! Comment-preserving settings transactions. Read only when opening/saving UI.
use crate::config::{config_write_target, write_config_atomically, Config};
use std::{
    io,
    io::Read,
    path::{Path, PathBuf},
};
const MAX_BYTES: u64 = 1024 * 1024;

pub struct Preferences {
    path: PathBuf,
    target: PathBuf,
    original: Option<String>,
    pub initial: Config,
}
fn read(path: &Path) -> io::Result<Option<String>> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err(io::Error::other(
            "Settings must be a regular file smaller than 1 MiB",
        ));
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NONBLOCK);
    }
    let file = opts.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Settings is not a regular file"));
    }
    let mut s = String::new();
    file.take(MAX_BYTES + 1).read_to_string(&mut s)?;
    if s.len() as u64 > MAX_BYTES {
        return Err(io::Error::other("Settings file is too large"));
    }
    Ok(Some(s))
}
impl Preferences {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let target = config_write_target(&path)?;
        let original = read(&target)?;
        let initial = original.as_deref().map(crate::config::parse_config_with_compat)
            .transpose().map_err(|_| io::Error::other("Settings contains invalid TOML. Open config to repair it; it will not be overwritten."))?
            .unwrap_or_default();
        Ok(Self {
            path,
            target,
            original,
            initial,
        })
    }
    pub fn is_new(&self) -> bool {
        self.original.is_none()
    }
    pub fn save(&mut self, draft: &Config) -> io::Result<()> {
        validate(draft).map_err(io::Error::other)?;
        if config_write_target(&self.path)? != self.target || read(&self.target)? != self.original {
            return Err(io::Error::other(
                "Settings changed outside Volt. Close and reopen Settings before saving.",
            ));
        }
        let text = self.original.as_deref().unwrap_or("");
        let mut doc: toml_edit::Document = match text.parse() {
            Ok(doc) => doc,
            Err(_) => crate::config::normalize_leading_dot_float_literals(text)
                .parse()
                .map_err(|_| io::Error::other("Invalid settings document"))?,
        };
        let before = toml::Value::try_from(&self.initial).map_err(io::Error::other)?;
        let after = toml::Value::try_from(draft).map_err(io::Error::other)?;
        let a = after
            .as_table()
            .ok_or_else(|| io::Error::other("Invalid settings"))?;
        let b = before
            .as_table()
            .ok_or_else(|| io::Error::other("Invalid settings"))?;
        for (key, value) in a {
            if self.original.is_some() && b.get(key) == Some(value) {
                continue;
            }
            // No UI exposes credentials. Never reconstruct [ai] or unknown keys.
            if key == "ai" {
                if b.get(key) == Some(value) {
                    continue;
                }
                return Err(io::Error::other("AI credentials cannot be edited here"));
            }
            if let Some(table) = value.as_table() {
                for (field, value) in table {
                    if self.original.is_some()
                        && b.get(key).and_then(|v| v.get(field)) == Some(value)
                    {
                        continue;
                    }
                    let item = item(value)?;
                    if !doc.contains_key(key) {
                        doc[key] = toml_edit::Item::Table(toml_edit::Table::new());
                    }
                    let dest = &mut doc[key][field];
                    replace(dest, item);
                }
            } else {
                replace(&mut doc[key], item(value)?);
            }
        }
        let updated = doc.to_string();
        let parsed: Config =
            toml::from_str(&updated).map_err(|_| io::Error::other("Invalid resulting settings"))?;
        validate(&parsed).map_err(io::Error::other)?;
        // Recheck immediately before replacement; no silent clobber of edits
        // made while validation was running. Atomic rename prevents torn writes.
        if read(&self.target)? != self.original {
            return Err(io::Error::other(
                "Settings changed while saving; reopen Settings",
            ));
        }
        write_config_atomically(&self.target, &updated)?;
        self.original = Some(updated);
        self.initial = draft.clone();
        Ok(())
    }
}
fn replace(dest: &mut toml_edit::Item, mut new: toml_edit::Item) {
    if let (Some(old), Some(value)) = (dest.as_value(), new.as_value_mut()) {
        *value.decor_mut() = old.decor().clone();
    }
    *dest = new;
}
fn item(value: &toml::Value) -> io::Result<toml_edit::Item> {
    let text = format!("value = {value}");
    // Display for tables isn't a TOML value; serialization handles arrays of tables.
    let text = if value.is_array()
        && value
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.is_table()))
    {
        let mut map = toml::map::Map::new();
        map.insert("value".into(), value.clone());
        toml::to_string(&map).map_err(io::Error::other)?
    } else {
        text
    };
    let mut d: toml_edit::Document = text.parse().map_err(io::Error::other)?;
    d.remove("value")
        .ok_or_else(|| io::Error::other("Invalid setting value"))
}
pub fn validate(c: &Config) -> Result<(), String> {
    for (name, n, min, max) in [
        ("Font size", c.font.size, 6., 72.),
        ("Line height", c.appearance.line_height, 1., 3.),
        ("Opacity", c.appearance.opacity, 0.1, 1.),
        ("Blur", c.appearance.blur_amount, 0., 100.),
        ("Divider opacity", c.appearance.divider_opacity, 0., 1.),
    ] {
        if !n.is_finite() || !(min..=max).contains(&n) {
            return Err(format!("{name} must be between {min} and {max}"));
        }
    }
    if c.font.family.trim().is_empty()
        || c.font.family.len() > 256
        || c.font.family.chars().any(char::is_control)
    {
        return Err("Choose a valid font family".into());
    }
    if c.appearance.padding > 100 || !(1..=1_000_000).contains(&c.terminal.scrollback_lines) {
        return Err("Padding must be 0–100; scrollback 1–1000000".into());
    }
    if c.shell.program.trim().is_empty()
        || c.shell.program.chars().any(char::is_control)
        || c.shell.args.iter().any(|a| a.chars().any(char::is_control))
    {
        return Err("Shell must be an executable, not a command string".into());
    }
    if (c.shell.integration || c.shell.prompt == crate::config::PromptMode::Meow)
        && !matches!(
            Path::new(&c.shell.program)
                .file_name()
                .and_then(|n| n.to_str()),
            Some("bash" | "zsh" | "fish")
        )
    {
        return Err("Automatic shell setup supports Bash, Zsh and Fish".into());
    }
    if (c.shell.integration || c.shell.prompt == crate::config::PromptMode::Meow)
        && c.shell
            .args
            .iter()
            .any(|a| !matches!(a.as_str(), "-l" | "--login" | "-i" | "--interactive"))
    {
        return Err("Bundled shell setup requires login/interactive arguments only".into());
    }
    if c.keybindings.len() > 128 {
        return Err("At most 128 shortcuts are supported".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> PathBuf {
        // SystemTime on macOS can return the same tick in parallel tests.
        // Keep each transaction isolated even when those calls coincide.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "volt-preferences-{}-{}-{}.toml",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn preserves_unknown_values_comments_and_credentials() {
        let p = path();
        let text="# heading\ntheme = 'nord' # keep\n[font]\nsize = 14 # size\n[ai]\napi_key = 'secret'\n[unknown]\nvalue = 'untouched'\n";
        std::fs::write(&p, text).unwrap();
        let mut tx = Preferences::open(p.clone()).unwrap();
        let mut c = tx.initial.clone();
        c.font.size = 17.;
        c.theme = "dracula".into();
        tx.save(&c).unwrap();
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(
            s.contains("# heading")
                && s.contains("# keep")
                && s.contains("# size")
                && s.contains("api_key = 'secret'")
                && s.contains("value = 'untouched'")
        );
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn refuses_external_edits_and_invalid_values() {
        let p = path();
        let mut tx = Preferences::open(p.clone()).unwrap();
        let mut c = tx.initial.clone();
        c.font.size = f32::NAN;
        assert!(tx.save(&c).is_err());
        c.font.size = 18.;
        std::fs::write(&p, "theme='nord'\n").unwrap();
        assert!(tx.save(&c).is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "theme='nord'\n");
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn invalid_toml_is_not_replaced() {
        let p = path();
        std::fs::write(&p, "secret = [").unwrap();
        assert!(Preferences::open(p.clone()).is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "secret = [");
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn saves_setup_and_bindings() {
        let p = path();
        let mut tx = Preferences::open(p.clone()).unwrap();
        let mut c = tx.initial.clone();
        c.setup.completed = true;
        c.keybindings.push(crate::keybindings::KeyBinding {
            key: "super+k".to_string().try_into().unwrap(),
            action: crate::keybindings::Action::Find,
        });
        tx.save(&c).unwrap();
        let restored = Preferences::open(p.clone()).unwrap();
        assert!(restored.initial.setup.completed);
        assert_eq!(restored.initial.keybindings.len(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn fresh_save_round_trips_login_arguments_and_never_writes_ai() {
        let p = path();
        let mut tx = Preferences::open(p.clone()).unwrap();
        let c = tx.initial.clone();
        tx.save(&c).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        let loaded: Config = toml::from_str(&text).unwrap();
        assert_eq!(
            toml::Value::try_from(&loaded).unwrap(),
            toml::Value::try_from(&c).unwrap()
        );
        assert!(!text.contains("[ai]"));
        std::fs::remove_file(p).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn compatibility_float_and_symlink_are_preserved() {
        let p = path();
        std::fs::write(&p, "# keep\n[appearance]\nopacity = .9\n").unwrap();
        let link = p.with_extension("link");
        std::os::unix::fs::symlink(&p, &link).unwrap();
        let mut tx = Preferences::open(link.clone()).unwrap();
        let mut c = tx.initial.clone();
        c.font.size = 16.;
        tx.save(&c).unwrap();
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(std::fs::read_to_string(&p).unwrap().contains("# keep"));
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(p).unwrap();
    }
}
