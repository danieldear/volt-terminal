//! Optional bounded UTF-8 file helpers. Call on a worker, not the UI thread.
//! These are not a filesystem sandbox or a multi-process transaction manager.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
/// Current lightweight text editor limit; oversized files are rejected, never truncated.
pub const MAX_TEXT_FILE_BYTES: usize = 8192;
/// Replacement is always explicit. Matching detects an external edit before commit,
/// but cannot eliminate another process's race between the check and rename.
#[derive(Clone, Copy, Debug)]
pub enum SavePolicy<'a> {
    CreateNew,
    MatchContents(&'a str),
    ApprovedReplacement,
}
fn error(kind: io::ErrorKind, message: &str) -> io::Error {
    io::Error::new(kind, message)
}
fn regular(path: &Path) -> io::Result<fs::Metadata> {
    let m = fs::symlink_metadata(path)?;
    if !m.file_type().is_file() {
        return Err(error(
            io::ErrorKind::InvalidInput,
            "expected a regular file, not a link, directory or device",
        ));
    }
    Ok(m)
}
pub fn read_text_file(path: &Path) -> io::Result<String> {
    regular(path)?;
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(error(
            io::ErrorKind::InvalidInput,
            "expected a regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_TEXT_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_TEXT_FILE_BYTES {
        return Err(error(
            io::ErrorKind::InvalidData,
            "text file exceeds the 8 KiB editor limit",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| error(io::ErrorKind::InvalidData, "text file is not valid UTF-8"))
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
/// Write a private sibling, sync its contents, then commit without truncating the
/// original. New-file commits use a hard link so an existing path is never replaced;
/// where links are unsupported (exFAT, many network shares) an exclusive create is used.
/// Existing-file rename replacement is platform dependent and may return an error.
/// Unix permission bits are retained; ACLs/xattrs/hard-link identity are not copied.
/// Parent directories are not fsynced: this is not a power-loss durability guarantee.
pub fn save_text_file(path: &Path, text: &str, policy: SavePolicy<'_>) -> io::Result<()> {
    if text.len() > MAX_TEXT_FILE_BYTES {
        return Err(error(
            io::ErrorKind::InvalidInput,
            "text exceeds the 8 KiB editor limit",
        ));
    }
    if path.file_name().is_none() {
        return Err(error(io::ErrorKind::InvalidInput, "missing file name"));
    }
    let original = match fs::symlink_metadata(path) {
        Ok(_) => Some(regular(path)?),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    if original
        .as_ref()
        .is_some_and(|m| m.permissions().readonly())
    {
        return Err(error(
            io::ErrorKind::PermissionDenied,
            "target is read-only",
        ));
    }
    match policy {
        SavePolicy::CreateNew if original.is_some() => {
            return Err(error(io::ErrorKind::AlreadyExists, "target already exists"))
        }
        SavePolicy::MatchContents(expected) if read_text_file(path)? != expected => {
            return Err(error(
                io::ErrorKind::AlreadyExists,
                "file changed on disk; reopen it or use Save As",
            ))
        }
        _ => {}
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut created = None;
    for _ in 0..16 {
        let temporary = parent.join(format!(
            ".volt-save-{}-{stamp}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary) {
            Ok(file) => {
                created = Some((Temporary(temporary), file));
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    let (temporary, mut file) = created.ok_or_else(|| {
        error(
            io::ErrorKind::AlreadyExists,
            "cannot allocate a private save file",
        )
    })?;
    file.write_all(text.as_bytes())?;
    if let Some(m) = &original {
        file.set_permissions(m.permissions())?;
    }
    file.sync_all()?;
    drop(file);
    // Recheck after writing. This protects ordinary external edits, not malicious races.
    if let SavePolicy::MatchContents(expected) = policy {
        if read_text_file(path)? != expected {
            return Err(error(
                io::ErrorKind::AlreadyExists,
                "file changed while saving; original retained",
            ));
        }
    }
    if original.is_some() {
        regular(path)?;
        fs::rename(&temporary.0, path)?;
        Ok(())
    } else {
        commit_new(&temporary.0, path, text, |from, to| fs::hard_link(from, to))
    }
}
/// Commit a new file without ever replacing an existing path. A hard link is
/// atomic; filesystems without hard links (exFAT/FAT, many SMB/NFS shares)
/// fall back to an exclusive create-and-write, which still refuses an
/// existing path but can leave a partial file if the process dies mid-write.
fn commit_new(
    temporary: &Path,
    path: &Path,
    text: &str,
    link: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    match link(temporary, path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
        Err(_) => {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                // Match the private staged file even on a linkless volume.
                options.mode(0o600);
            }
            let mut file = options.open(path)?;
            let written = file
                .write_all(text.as_bytes())
                .and_then(|()| file.sync_all());
            if written.is_err() {
                // Never leave a partial file this call created.
                drop(file);
                let _ = fs::remove_file(path);
            }
            written
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_files_fall_back_when_hard_links_are_unsupported_and_never_clobber() {
        let dir = std::env::temp_dir().join(format!(
            "volt-commit-new-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let (temporary, path) = (dir.join("staged"), dir.join("note.txt"));
        fs::write(&temporary, "draft").unwrap();
        let unsupported = |_: &Path, _: &Path| -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "no hard links"))
        };
        commit_new(&temporary, &path, "draft", unsupported).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "draft");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        // An existing path is refused by the fallback too.
        let err = commit_new(&temporary, &path, "other", unsupported).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&path).unwrap(), "draft");
        fs::remove_dir_all(&dir).unwrap();
    }
}
