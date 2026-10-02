//! Process-wide macOS Secure Event Input ownership.
//!
//! This prevents other ordinary applications from observing keyboard events;
//! it does not hide terminal output, protect the clipboard, or authenticate a
//! password prompt. Only show the lock after the OS call succeeds.

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn EnableSecureEventInput() -> i32;
    fn DisableSecureEventInput() -> i32;
}

#[derive(Default)]
pub(crate) struct SecureInput {
    enabled: bool,
}

impl SecureInput {
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn set(&mut self, requested: bool) {
        if requested == self.enabled {
            return;
        }
        let status = unsafe {
            if requested {
                EnableSecureEventInput()
            } else {
                DisableSecureEventInput()
            }
        };
        if status == 0 {
            self.enabled = requested;
        } else {
            eprintln!("volt-ui: Secure Event Input failed with OSStatus {status}");
        }
    }
}

impl Drop for SecureInput {
    fn drop(&mut self) {
        self.set(false);
    }
}
