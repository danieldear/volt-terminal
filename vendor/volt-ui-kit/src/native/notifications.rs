//! Opt-in local notifications, not remote push or custom notification delegates.
//! A successful submission means OS acceptance, NEVER proof of visible delivery.
//!
//! Enable `native-notifications` on macOS, package as an identified `.app`,
//! and start operations on the native main thread. Permission is never automatic.
//! Settings queries, custom actions and forced foreground presentation are unavailable.
//!
//! ```no_run
//! use std::time::Duration;
//! use volt_ui_kit::native::notifications::{self, LocalNotification, NotificationError};
//!
//! fn submit_after_explicit_authorization() -> Result<(), NotificationError> {
//!     let note = LocalNotification::new("build.ready", "Build ready", "Checks passed.")
//!         .after(Duration::from_secs(8));
//!     notifications::send(&note, |accepted| {
//!         // Marshal into your bounded UI mailbox; this OS thread must not edit UI.
//!         // Success means accepted, not displayed.
//!         let _ = accepted;
//!     })
//! }
//! ```
use std::{fmt, time::Duration};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotificationCapabilities {
    pub authorization: bool,
    pub local_requests: bool,
    pub removal: bool,
    pub settings_query: bool,
    pub actions: bool,
    pub foreground_presentation: bool,
}
/// Compiled adapter only; not permission, bundle readiness or presentation status.
pub fn capabilities() -> NotificationCapabilities {
    let available = cfg!(all(feature = "native-notifications", target_os = "macos"));
    NotificationCapabilities {
        authorization: available,
        local_requests: available,
        removal: available,
        settings_query: false,
        actions: false,
        foreground_presentation: false,
    }
}
#[derive(Clone, Debug)]
pub struct LocalNotification {
    id: String,
    title: String,
    body: String,
    delay: Option<Duration>,
}
impl LocalNotification {
    pub fn new(id: impl Into<String>, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: body.into(),
            delay: None,
        }
    }
    /// One-shot delay between 1 second and 24 hours; no repeating timer.
    pub fn after(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Pure validation; never initializes a notification center or prompts.
    pub fn validate(&self) -> Result<(), NotificationError> {
        validate_id(&self.id)?;
        if self.title.trim().is_empty()
            || self.title.len() > 128
            || self.title.chars().any(char::is_control)
            || self.body.trim().is_empty()
            || self.body.len() > 4096
            || self.body.chars().any(|c| c.is_control() && c != '\n')
            || self
                .delay
                .is_some_and(|d| d < Duration::from_secs(1) || d > Duration::from_secs(86400))
        {
            return Err(NotificationError::InvalidNotification);
        }
        Ok(())
    }
}
fn validate_id(id: &str) -> Result<(), NotificationError> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        Err(NotificationError::InvalidNotification)
    } else {
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationError {
    InvalidNotification,
    Unsupported,
    WrongThread,
    MissingAppBundle,
    NativeFailure,
}
impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidNotification => "invalid notification ID, title, body or one-shot delay",
            Self::Unsupported => "local notifications require native-notifications on macOS",
            Self::WrongThread => "start notification operations on the native main thread",
            Self::MissingAppBundle => "local notifications require a running .app with a bundle identifier; package the executable first",
            Self::NativeFailure => "UserNotifications rejected the operation; check authorization, bundle/signing and OS settings (native details unavailable)",
        })
    }
}
impl std::error::Error for NotificationError {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Authorization {
    Granted,
    Denied,
}
/// Explicitly request ALERT permission only. Never called by construction/send.
/// On synchronous error the callback is not invoked. Otherwise it runs at most
/// once, on an unspecified OS thread: marshal into UiSender, never edit UI state.
/// Callback must not panic or block; no guaranteed OS completion deadline.
/// This is a decision at request time, not a live notification-settings query.
pub fn request_authorization(
    callback: impl FnOnce(Result<Authorization, NotificationError>) + Send + 'static,
) -> Result<(), NotificationError> {
    backend::request_authorization(callback)
}
/// Submit a silent, one-shot local request. Equal IDs use OS replacement semantics.
/// Success from the callback is OS acceptance, not a banner/visibility receipt.
/// Does not ask permission; foreground/Focus/settings may suppress presentation.
pub fn send(
    notification: &LocalNotification,
    callback: impl FnOnce(Result<(), NotificationError>) + Send + 'static,
) -> Result<(), NotificationError> {
    notification.validate()?;
    backend::send(notification, callback)
}
/// Ask the OS to remove matching pending AND delivered notifications. Does not
/// cancel an in-flight add; serialize add/removal in your application. There is
/// no completion/visual disappearance receipt for these framework methods.
pub fn remove(id: &str) -> Result<(), NotificationError> {
    validate_id(id)?;
    backend::remove(id)
}
#[cfg(not(all(feature = "native-notifications", target_os = "macos")))]
mod backend {
    use super::*;
    pub(super) fn request_authorization(
        _: impl FnOnce(Result<Authorization, NotificationError>) + Send + 'static,
    ) -> Result<(), NotificationError> {
        Err(NotificationError::Unsupported)
    }
    pub(super) fn send(
        _: &LocalNotification,
        _: impl FnOnce(Result<(), NotificationError>) + Send + 'static,
    ) -> Result<(), NotificationError> {
        Err(NotificationError::Unsupported)
    }
    pub(super) fn remove(_: &str) -> Result<(), NotificationError> {
        Err(NotificationError::Unsupported)
    }
}
#[cfg(any(test, all(feature = "native-notifications", target_os = "macos")))]
mod completion {
    use std::sync::Mutex;
    /// Defensively one-shot even if a native block is accidentally called again.
    pub(super) fn invoke<T, F: FnOnce(T)>(slot: &Mutex<Option<F>>, value: T) {
        let callback = slot.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(callback) = callback {
            callback(value);
        }
    }
}
#[cfg(all(feature = "native-notifications", target_os = "macos"))]
mod backend {
    use super::*;
    use block2::RcBlock;
    use objc2::{rc::Retained, runtime::Bool, MainThreadMarker};
    use objc2_foundation::{NSArray, NSBundle, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNTimeIntervalNotificationTrigger, UNUserNotificationCenter,
    };
    use std::{path::PathBuf, sync::Mutex};
    fn center() -> Result<Retained<UNUserNotificationCenter>, NotificationError> {
        MainThreadMarker::new().ok_or(NotificationError::WrongThread)?;
        // Check BEFORE currentNotificationCenter: unbundled executables can assert.
        let bundle = NSBundle::mainBundle();
        let identifier = bundle
            .bundleIdentifier()
            .ok_or(NotificationError::MissingAppBundle)?;
        let path = PathBuf::from(
            bundle
                .bundleURL()
                .path()
                .ok_or(NotificationError::MissingAppBundle)?
                .to_string(),
        );
        let exe = std::env::current_exe().map_err(|_| NotificationError::MissingAppBundle)?;
        if identifier.to_string().trim().is_empty()
            || path.extension().and_then(|s| s.to_str()) != Some("app")
            || !exe.starts_with(path.join("Contents/MacOS"))
        {
            return Err(NotificationError::MissingAppBundle);
        }
        Ok(UNUserNotificationCenter::currentNotificationCenter())
    }
    pub(super) fn request_authorization(
        callback: impl FnOnce(Result<Authorization, NotificationError>) + Send + 'static,
    ) -> Result<(), NotificationError> {
        let center = center()?;
        let callback = Mutex::new(Some(callback));
        let block = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            let result = if !error.is_null() {
                Err(NotificationError::NativeFailure)
            } else if granted.as_bool() {
                Ok(Authorization::Granted)
            } else {
                Ok(Authorization::Denied)
            };
            completion::invoke(&callback, result);
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert,
            &block,
        );
        Ok(())
    }
    pub(super) fn send(
        notification: &LocalNotification,
        callback: impl FnOnce(Result<(), NotificationError>) + Send + 'static,
    ) -> Result<(), NotificationError> {
        let center = center()?;
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&notification.title));
        content.setBody(&NSString::from_str(&notification.body));
        let trigger = notification.delay.map(|d| {
            UNTimeIntervalNotificationTrigger::triggerWithTimeInterval_repeats(
                d.as_secs_f64(),
                false,
            )
        });
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(&notification.id),
            &content,
            trigger.as_deref().map(|v| &**v),
        );
        let callback = Mutex::new(Some(callback));
        let block = RcBlock::new(move |error: *mut NSError| {
            completion::invoke(
                &callback,
                if error.is_null() {
                    Ok(())
                } else {
                    Err(NotificationError::NativeFailure)
                },
            );
        });
        center.addNotificationRequest_withCompletionHandler(&request, Some(&block));
        Ok(())
    }
    pub(super) fn remove(id: &str) -> Result<(), NotificationError> {
        let center = center()?;
        let ids = NSArray::from_retained_slice(&[NSString::from_str(id)]);
        center.removePendingNotificationRequestsWithIdentifiers(&ids);
        center.removeDeliveredNotificationsWithIdentifiers(&ids);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn completion_is_once_and_drops_lock_before_invoking_user_code() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        };
        let count = Arc::new(AtomicUsize::new(0));
        let cloned = count.clone();
        type Callback = Box<dyn FnOnce(()) + Send>;
        let slot: Arc<Mutex<Option<Callback>>> = Arc::default();
        let weak = Arc::downgrade(&slot);
        *slot.lock().unwrap() = Some(Box::new(move |()| {
            // A real reentrant observation must succeed before callback returns.
            assert!(weak.upgrade().unwrap().try_lock().unwrap().is_none());
            cloned.fetch_add(1, Ordering::Relaxed);
        }));
        super::completion::invoke(&slot, ());
        super::completion::invoke(&slot, ());
        assert_eq!(count.load(Ordering::Relaxed), 1);
        assert!(slot.lock().unwrap().is_none());
    }
}
