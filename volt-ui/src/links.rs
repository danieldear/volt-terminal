//! Link detection lives in volt-core; opening a link is the app's.
pub use volt_core::links::*;

pub fn open(url: &str) -> std::io::Result<()> {
    if !safe_web_url(url) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "unsafe link",
        ));
    }
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(target_os = "macos"))]
    let program = "xdg-open";
    // No shell interpolation. Reap the launcher without blocking the UI.
    let mut child = std::process::Command::new(program).arg(url).spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
