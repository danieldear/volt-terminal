//! Safe public macOS file panels, opt-in through `native-desktop`.
//! Dialogs are synchronous application-modal UI; do file I/O on a worker afterward.
use std::{fmt, path::PathBuf};
#[derive(Clone, Debug)]
pub struct FileDialogOptions {
    title: String,
    suggested_name: Option<String>,
}
impl FileDialogOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            suggested_name: None,
        }
    }
    pub fn suggested_name(mut self, name: impl Into<String>) -> Self {
        self.suggested_name = Some(name.into());
        self
    }
    /// Pure configuration validation; never initializes AppKit or opens a panel.
    pub fn validate(&self) -> Result<(), FileDialogError> {
        if self.title.trim().is_empty()
            || self.title.len() > 256
            || self.title.chars().any(char::is_control)
            || self.suggested_name.as_ref().is_some_and(|name| {
                name.is_empty()
                    || name.len() > 255
                    || name == "."
                    || name == ".."
                    || name
                        .chars()
                        .any(|c| c.is_control() || c == '/' || c == '\\')
            })
        {
            return Err(FileDialogError::InvalidOptions);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileDialogError {
    InvalidOptions,
    Unsupported,
    WrongThread,
    NativeFailure,
}
impl fmt::Display for FileDialogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidOptions => "invalid file dialog title or suggested name",
            Self::Unsupported => "file dialogs require native-desktop on macOS",
            Self::WrongThread => "AppKit file dialogs require the main thread",
            Self::NativeFailure => "native file panel failed or returned an invalid file URL",
        })
    }
}
impl std::error::Error for FileDialogError {}
/// Compiled adapter availability, not an authorization/runtime guarantee.
pub fn available() -> bool {
    cfg!(all(feature = "native-desktop", target_os = "macos"))
}
pub fn open_file(options: &FileDialogOptions) -> Result<Option<PathBuf>, FileDialogError> {
    options.validate()?;
    show(options, false)
}
pub fn save_file(options: &FileDialogOptions) -> Result<Option<PathBuf>, FileDialogError> {
    options.validate()?;
    show(options, true)
}
#[cfg(not(all(feature = "native-desktop", target_os = "macos")))]
fn show(_: &FileDialogOptions, _: bool) -> Result<Option<PathBuf>, FileDialogError> {
    Err(FileDialogError::Unsupported)
}
#[cfg(all(feature = "native-desktop", target_os = "macos"))]
fn show(options: &FileDialogOptions, save: bool) -> Result<Option<PathBuf>, FileDialogError> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponseCancel, NSModalResponseOK, NSOpenPanel, NSSavePanel};
    use objc2_foundation::NSString;
    let mtm = MainThreadMarker::new().ok_or(FileDialogError::WrongThread)?;
    let open;
    let panel;
    let p: &NSSavePanel = if save {
        panel = NSSavePanel::savePanel(mtm);
        &panel
    } else {
        open = NSOpenPanel::openPanel(mtm);
        open.setCanChooseDirectories(false);
        open.setCanChooseFiles(true);
        open.setAllowsMultipleSelection(false);
        &open
    };
    p.setTitle(Some(&NSString::from_str(&options.title)));
    if let Some(name) = &options.suggested_name {
        p.setNameFieldStringValue(&NSString::from_str(name));
    }
    let response = p.runModal();
    if response == NSModalResponseCancel {
        return Ok(None);
    }
    if response != NSModalResponseOK {
        return Err(FileDialogError::NativeFailure);
    }
    let url = p.URL().ok_or(FileDialogError::NativeFailure)?;
    if !url.isFileURL() {
        return Err(FileDialogError::NativeFailure);
    }
    let path = PathBuf::from(
        url.path()
            .ok_or(FileDialogError::NativeFailure)?
            .to_string(),
    );
    if !path.is_absolute() {
        return Err(FileDialogError::NativeFailure);
    }
    Ok(Some(path))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_thread_never_opens_a_native_panel() {
        let result = std::thread::spawn(|| open_file(&FileDialogOptions::new("Open")))
            .join()
            .unwrap();
        assert_eq!(
            result,
            Err(if available() {
                FileDialogError::WrongThread
            } else {
                FileDialogError::Unsupported
            })
        );
    }
}
