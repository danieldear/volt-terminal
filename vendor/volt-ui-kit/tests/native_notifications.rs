#![cfg(feature = "native")]
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use volt_ui_kit::native::notifications::{self, LocalNotification, NotificationError};
#[test]
fn notification_configuration_is_bounded_and_never_silently_truncated() {
    assert!(
        LocalNotification::new("build.ready", "Build ready", "All checks passed.")
            .validate()
            .is_ok()
    );
    for id in ["", "bad/path", "private space", "a\n", "☃"] {
        assert_eq!(
            LocalNotification::new(id, "Title", "Body").validate(),
            Err(NotificationError::InvalidNotification)
        );
    }
    assert!(LocalNotification::new("a".repeat(65), "Title", "Body")
        .validate()
        .is_err());
    assert!(LocalNotification::new("build", "t".repeat(129), "Body")
        .validate()
        .is_err());
    assert!(LocalNotification::new("build", "Title", "b".repeat(4097))
        .validate()
        .is_err());
    assert!(
        LocalNotification::new("build", "Title", "Line one\nLine two")
            .validate()
            .is_ok()
    );
    for delay in [
        Duration::ZERO,
        Duration::from_millis(999),
        Duration::from_secs(86401),
    ] {
        assert!(LocalNotification::new("build", "Title", "Body")
            .after(delay)
            .validate()
            .is_err());
    }
}
#[test]
fn capability_reporting_never_promises_action_or_foreground_delegates() {
    let c = notifications::capabilities();
    let compiled = cfg!(all(feature = "native-notifications", target_os = "macos"));
    assert_eq!(c.authorization, compiled);
    assert_eq!(c.local_requests, compiled);
    assert_eq!(c.removal, compiled);
    assert!(!c.actions && !c.foreground_presentation && !c.settings_query);
}
#[test]
fn invalid_requests_fail_synchronously_without_running_callback() {
    let count = Arc::new(AtomicUsize::new(0));
    let cloned = count.clone();
    let invalid = LocalNotification::new("invalid/id", "Title", "Body");
    assert_eq!(
        notifications::send(&invalid, move |_| {
            cloned.fetch_add(1, Ordering::Relaxed);
        }),
        Err(NotificationError::InvalidNotification)
    );
    assert_eq!(count.load(Ordering::Relaxed), 0);
    assert_eq!(
        notifications::remove("invalid/id"),
        Err(NotificationError::InvalidNotification)
    );
}
#[test]
fn wrong_thread_or_unsupported_never_requests_os_permission() {
    let result = std::thread::spawn(|| {
        notifications::request_authorization(|_| panic!("must not reach OS"))
    })
    .join()
    .unwrap();
    assert_eq!(
        result,
        Err(if notifications::capabilities().authorization {
            NotificationError::WrongThread
        } else {
            NotificationError::Unsupported
        })
    );
}
