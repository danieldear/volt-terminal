//! Opt-in, bounded layout state. Never saves output, commands, environment,
//! process IDs, task status, OSC titles or credentials.
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
const MAX_BYTES: u64 = 128 * 1024;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub version: u8,
    pub windows: Vec<WindowLayout>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowLayout {
    pub width: u32,
    pub height: u32,
    pub active: usize,
    pub tabs: Vec<TabLayout>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabLayout {
    pub active_pane: usize,
    pub color: Option<String>,
    pub root: Node,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Node {
    Pane {
        cwd: Option<PathBuf>,
    },
    Split {
        vertical: bool,
        ratio: f32,
        children: Box<[Node; 2]>,
    },
}
impl Node {
    fn validate(&self, depth: usize, panes: &mut usize) -> io::Result<()> {
        if depth > 8 {
            return Err(io::Error::other("Layout is too deep"));
        }
        match self {
            Self::Pane { cwd } => {
                *panes += 1;
                if *panes > 16 || cwd.as_ref().is_some_and(|p| !valid_path(p)) {
                    return Err(io::Error::other("Invalid layout pane"));
                }
            }
            Self::Split {
                ratio, children, ..
            } => {
                if !ratio.is_finite() || !(0.1..=0.9).contains(ratio) {
                    return Err(io::Error::other("Invalid split ratio"));
                }
                for child in children.iter() {
                    child.validate(depth + 1, panes)?;
                }
            }
        }
        Ok(())
    }
}
fn valid_path(p: &Path) -> bool {
    p.is_absolute()
        && p.as_os_str().len() <= 4096
        && !p.to_string_lossy().chars().any(char::is_control)
}
impl Layout {
    fn validate(&self) -> io::Result<()> {
        if self.version != 1 || self.windows.is_empty() || self.windows.len() > 8 {
            return Err(io::Error::other("Unsupported or invalid layout"));
        }
        for window in &self.windows {
            if !(320..=7680).contains(&window.width)
                || !(200..=4320).contains(&window.height)
                || window.tabs.is_empty()
                || window.tabs.len() > 16
                || window.active >= window.tabs.len()
            {
                return Err(io::Error::other("Invalid layout window"));
            }
            for tab in &window.tabs {
                if tab
                    .color
                    .as_deref()
                    .is_some_and(|c| volt_renderer::tab_color::TabColor::from_id(c).is_none())
                {
                    return Err(io::Error::other("Invalid tab color"));
                }
                let mut panes = 0;
                tab.root.validate(0, &mut panes)?;
                if tab.active_pane >= panes {
                    return Err(io::Error::other("Invalid active pane"));
                }
            }
        }
        Ok(())
    }
}
pub fn path() -> Option<PathBuf> {
    Some(volt_config::config_dir()?.join("layout.json"))
}
pub fn load(path: &Path) -> io::Result<Layout> {
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err(io::Error::other("Layout is not a bounded regular file"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Invalid layout file"));
    }
    let mut data = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut data)?;
    if data.len() as u64 > MAX_BYTES {
        return Err(io::Error::other("Layout exceeds size limit"));
    }
    let result: Layout = serde_json::from_slice(&data).map_err(io::Error::other)?;
    result.validate()?;
    Ok(result)
}
pub fn save(path: &Path, layout: &Layout) -> io::Result<()> {
    layout.validate()?;
    let bytes = serde_json::to_vec(layout).map_err(io::Error::other)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(io::Error::other("Layout exceeds size limit"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid layout path"))?;
    std::fs::create_dir_all(parent)?;
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if !meta.is_file() {
            return Err(io::Error::other("Refusing non-regular layout destination"));
        }
    }
    let tmp = parent.join(format!(".layout-{}.tmp", std::process::id()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(&tmp)?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn layout() -> Layout {
        Layout {
            version: 1,
            windows: vec![WindowLayout {
                width: 1400,
                height: 900,
                active: 0,
                tabs: vec![TabLayout {
                    active_pane: 0,
                    color: Some("amber".into()),
                    root: Node::Pane {
                        cwd: Some(PathBuf::from("/tmp")),
                    },
                }],
            }],
        }
    }
    #[test]
    fn schema_is_not_a_command_or_output_transport() {
        let mut l = layout();
        assert!(l.validate().is_ok());
        l.version = 2;
        assert!(l.validate().is_err());
        l.version = 1;
        l.windows[0].active = 2;
        assert!(l.validate().is_err());
        assert!(serde_json::from_str::<Node>(
            r#"{"kind":"pane","cwd":"/tmp","command":"rm -rf /"}"#
        )
        .is_err());
        assert!(!valid_path(Path::new("../tmp")));
        assert!(!valid_path(Path::new("/tmp\nsecret")));
    }
    #[test]
    fn atomic_private_roundtrip_corrupt_missing_and_symlink() {
        let dir = std::env::temp_dir().join(format!("volt-session-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("layout.json");
        assert!(load(&path).is_err());
        save(&path, &layout()).unwrap();
        assert_eq!(load(&path).unwrap().windows.len(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::write(&path, "{}").unwrap();
        assert!(load(&path).is_err());
        std::fs::remove_file(&path).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.join("other"), &path).unwrap();
            assert!(load(&path).is_err());
            assert!(save(&path, &layout()).is_err());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn resource_limits_reject_bad_ratios_and_excessive_tabs() {
        let mut l = layout();
        l.windows[0].tabs[0].root = Node::Split {
            vertical: true,
            ratio: f32::NAN,
            children: Box::new([Node::Pane { cwd: None }, Node::Pane { cwd: None }]),
        };
        assert!(l.validate().is_err());
        let mut l = layout();
        for _ in 0..16 {
            l.windows[0].tabs.push(TabLayout {
                active_pane: 0,
                color: None,
                root: Node::Pane { cwd: None },
            });
        }
        assert!(l.validate().is_err());
    }
}
