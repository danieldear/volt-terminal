#![cfg(feature = "native")]
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use volt_ui_kit::native::file_dialog::FileDialogOptions;
use volt_ui_kit::native::files::{read_text_file, save_text_file, SavePolicy, MAX_TEXT_FILE_BYTES};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "volt-file-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> PathBuf {
        self.0.join("notes.txt")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn bounded_utf8_roundtrip_conflict_and_create_new_never_clobber() {
    let f = Fixture::new();
    let path = f.path();
    save_text_file(&path, "hello 🌿", SavePolicy::CreateNew).unwrap();
    assert_eq!(read_text_file(&path).unwrap(), "hello 🌿");
    assert!(save_text_file(&path, "clobber", SavePolicy::CreateNew).is_err());
    assert!(save_text_file(&path, "clobber", SavePolicy::MatchContents("stale")).is_err());
    assert_eq!(read_text_file(&path).unwrap(), "hello 🌿");
    save_text_file(&path, "updated", SavePolicy::MatchContents("hello 🌿")).unwrap();
    assert_eq!(read_text_file(&path).unwrap(), "updated");
    fs::write(&path, [0xff, 0xfe]).unwrap();
    assert!(read_text_file(&path).is_err());
    assert!(save_text_file(
        &path,
        &"x".repeat(MAX_TEXT_FILE_BYTES + 1),
        SavePolicy::ApprovedReplacement
    )
    .is_err());
    fs::write(&path, vec![b'x'; MAX_TEXT_FILE_BYTES + 1]).unwrap();
    assert!(read_text_file(&path).is_err());
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
}
#[test]
fn invalid_dialog_configuration_is_rejected_without_os_side_effects() {
    assert!(FileDialogOptions::new("Open notes").validate().is_ok());
    assert!(FileDialogOptions::new("\n").validate().is_err());
    assert!(FileDialogOptions::new("Save")
        .suggested_name("../escape")
        .validate()
        .is_err());
}
#[cfg(unix)]
#[test]
fn links_and_special_files_are_rejected_and_mode_is_preserved() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let f = Fixture::new();
    let path = f.path();
    let link = f.0.join("link.txt");
    save_text_file(&path, "original", SavePolicy::CreateNew).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&path, &link).unwrap();
    assert!(read_text_file(&link).is_err());
    assert!(save_text_file(&link, "bad", SavePolicy::ApprovedReplacement).is_err());
    assert!(read_text_file(&f.0).is_err());
    save_text_file(&path, "updated", SavePolicy::MatchContents("original")).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(read_text_file(&path).unwrap(), "updated");
}

#[test]
fn missing_destination_failure_retains_existing_file_and_exact_limit_is_supported() {
    let f = Fixture::new();
    let path = f.path();
    let exact = "x".repeat(MAX_TEXT_FILE_BYTES);
    save_text_file(&path, &exact, SavePolicy::CreateNew).unwrap();
    assert_eq!(read_text_file(&path).unwrap().len(), MAX_TEXT_FILE_BYTES);
    assert!(save_text_file(
        &f.0.join("missing/notes.txt"),
        "text",
        SavePolicy::CreateNew
    )
    .is_err());
    assert!(save_text_file(&path, "other", SavePolicy::MatchContents("stale")).is_err());
    assert_eq!(read_text_file(&path).unwrap(), exact);
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
}
#[cfg(unix)]
#[test]
fn explicit_replacement_does_not_bypass_readonly_policy() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let path = f.path();
    save_text_file(&path, "protected", SavePolicy::CreateNew).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(save_text_file(&path, "replacement", SavePolicy::ApprovedReplacement).is_err());
    assert_eq!(read_text_file(&path).unwrap(), "protected");
}
