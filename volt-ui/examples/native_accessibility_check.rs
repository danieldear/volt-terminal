//! Synthetic-only macOS AX ABI smoke. No PTY, shell, or user content.
//! Run: cargo run --locked -p volt-ui --example native_accessibility_check
//! Creates hidden winit views on the main thread and dispatches real Objective-C
//! AX selectors. This is not VoiceOver/full actionable-controls acceptance.

#[cfg(target_os = "macos")]
mod app {
    pub enum VoltEvent {
        NativeText {
            window_id: winit::window::WindowId,
            text: String,
        },
    }
}

#[cfg(target_os = "macos")]
#[path = "../src/native_text.rs"]
mod native_text;

#[cfg(target_os = "macos")]
mod smoke {
    use super::{app::VoltEvent, native_text};
    use native_text::accessibility::{AccessibilitySnapshot, Sensitivity, TextRange};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, BOOL, NO, YES};
    use objc::{class, msg_send, sel, sel_impl, Encode, Encoding};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::sync::Arc;
    use std::time::Duration;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
    use winit::window::{Window, WindowId};

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Range {
        location: usize,
        length: usize,
    }
    unsafe impl Encode for Range {
        fn encode() -> Encoding {
            unsafe { Encoding::from_str("{_NSRange=QQ}") }
        }
    }
    const NOT_FOUND: usize = isize::MAX as usize;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn object_setClass(object: *mut Object, class: *const Class) -> *const Class;
    }

    unsafe fn string(value: *mut Object) -> String {
        assert!(!value.is_null(), "expected NSString, got nil");
        let valid: BOOL = msg_send![value, isKindOfClass: class!(NSString)];
        assert_eq!(valid, YES);
        let length: usize = msg_send![value, lengthOfBytesUsingEncoding: 4usize];
        if length == 0 {
            return String::new();
        }
        let bytes: *const u8 = msg_send![value, UTF8String];
        assert!(!bytes.is_null());
        std::str::from_utf8(std::slice::from_raw_parts(bytes, length))
            .unwrap()
            .to_owned()
    }

    unsafe fn assert_redacted(view: *mut Object) {
        let value: *mut Object = msg_send![view, accessibilityValue];
        assert_eq!(string(value), "");
        let label: *mut Object = msg_send![view, accessibilityLabel];
        assert_eq!(string(label), "");
        let count: isize = msg_send![view, accessibilityNumberOfCharacters];
        assert_eq!(count, 0);
        let visible: Range = msg_send![view, accessibilityVisibleCharacterRange];
        assert_eq!(
            visible,
            Range {
                location: 0,
                length: 0
            }
        );
        let selected: Range = msg_send![view, accessibilitySelectedTextRange];
        assert_eq!(
            selected,
            Range {
                location: NOT_FOUND,
                length: 0
            }
        );
        let text: *mut Object = msg_send![view, accessibilitySelectedText];
        assert_eq!(string(text), "");
        let invalid: *mut Object =
            msg_send![view, accessibilityStringForRange: Range { location: 1, length: 0 }];
        assert!(invalid.is_null());
    }

    struct Check {
        proxy: EventLoopProxy<VoltEvent>,
        completed: bool,
    }
    impl ApplicationHandler<VoltEvent> for Check {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.completed {
                return;
            }
            unsafe {
                let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
                for chrome in [false, true] {
                    use winit::platform::macos::WindowAttributesExtMacOS;
                    let attributes = Window::default_attributes()
                        .with_visible(false)
                        .with_titlebar_transparent(chrome)
                        .with_title_hidden(chrome)
                        .with_fullsize_content_view(chrome);
                    let window = Arc::new(event_loop.create_window(attributes).unwrap());
                    let RawWindowHandle::AppKit(handle) = window.window_handle().unwrap().as_raw()
                    else {
                        panic!("expected AppKit view")
                    };
                    let view = handle.ns_view.as_ptr().cast::<Object>();
                    let original = (&*view).class();
                    let bridge = native_text::NativeTextInput::install(
                        window.clone(),
                        self.proxy.clone(),
                        chrome,
                    )
                    .unwrap();
                    assert!(!bridge.accessibility_requested());
                    // First real AX request starts TTL, but returns no old text.
                    assert_redacted(view);
                    assert!(bridge.accessibility_requested());
                    let role: *mut Object = msg_send![view, accessibilityRole];
                    assert_eq!(string(role), "AXStaticText");
                    let element: BOOL = msg_send![view, isAccessibilityElement];
                    assert_eq!(element, YES);

                    let synthetic = "A😀\nCafe\u{301}\0";
                    bridge.publish_accessibility(AccessibilitySnapshot::terminal_output(
                        synthetic,
                        Sensitivity::KnownSafe,
                        Some(TextRange {
                            location: 1,
                            length: 2,
                        }),
                    ));
                    let value: *mut Object = msg_send![view, accessibilityValue];
                    assert_eq!(string(value), synthetic);
                    let label: *mut Object = msg_send![view, accessibilityLabel];
                    assert_eq!(string(label), "Terminal output");
                    let units: isize = msg_send![view, accessibilityNumberOfCharacters];
                    assert_eq!(units, 10);
                    let visible: Range = msg_send![view, accessibilityVisibleCharacterRange];
                    assert_eq!(
                        visible,
                        Range {
                            location: 0,
                            length: 10
                        }
                    );
                    let selected: Range = msg_send![view, accessibilitySelectedTextRange];
                    assert_eq!(
                        selected,
                        Range {
                            location: 1,
                            length: 2
                        }
                    );
                    let selected_text: *mut Object = msg_send![view, accessibilitySelectedText];
                    assert_eq!(string(selected_text), "😀");
                    let substring: *mut Object =
                        msg_send![view, accessibilityStringForRange: selected];
                    assert_eq!(string(substring), "😀");
                    let split: *mut Object = msg_send![view, accessibilityStringForRange: Range { location: 2, length: 1 }];
                    assert!(split.is_null(), "surrogate-split range must fail closed");
                    let overflow: *mut Object = msg_send![view, accessibilityStringForRange: Range { location: usize::MAX, length: 1 }];
                    assert!(overflow.is_null());
                    let line: isize = msg_send![view, accessibilityLineForIndex: 5isize];
                    assert_eq!(line, 1);
                    let line_range: Range = msg_send![view, accessibilityRangeForLine: 1isize];
                    assert_eq!(
                        line_range,
                        Range {
                            location: 4,
                            length: 6
                        }
                    );
                    let editable: BOOL = msg_send![view, isAccessibilitySelectorAllowed: sel!(setAccessibilityValue:)];
                    assert_eq!(editable, NO);

                    bridge.invalidate_accessibility();
                    assert_redacted(view);
                    for privacy in [
                        Sensitivity::Unknown,
                        Sensitivity::NoEcho,
                        Sensitivity::ManualSecure,
                    ] {
                        bridge.publish_accessibility(AccessibilitySnapshot::terminal_output(
                            "SYNTHETIC WITHHELD",
                            privacy,
                            None,
                        ));
                        assert_redacted(view);
                    }
                    bridge.publish_accessibility(AccessibilitySnapshot::modal_summary(
                        "Synthetic dialog",
                        "Synthetic safe summary",
                    ));
                    let label: *mut Object = msg_send![view, accessibilityLabel];
                    assert_eq!(string(label), "Synthetic dialog");
                    let value: *mut Object = msg_send![view, accessibilityValue];
                    assert_eq!(string(value), "Synthetic safe summary");
                    bridge.invalidate_accessibility();
                    assert_redacted(view);

                    if !chrome {
                        bridge.publish_accessibility(AccessibilitySnapshot::terminal_output(
                            "SYNTHETIC OLD CACHE",
                            Sensitivity::KnownSafe,
                            None,
                        ));
                        std::thread::sleep(Duration::from_millis(2100));
                        // Native getter itself must clear expiry before reading:
                        // no parent frame and no PTY access are involved here.
                        assert_redacted(view);
                        assert!(bridge.accessibility_requested());
                        bridge.publish_accessibility(AccessibilitySnapshot::terminal_output(
                            "SYNTHETIC REFRESHED",
                            Sensitivity::KnownSafe,
                            None,
                        ));
                        let value: *mut Object = msg_send![view, accessibilityValue];
                        assert_eq!(string(value), "SYNTHETIC REFRESHED");
                    }
                    drop(bridge);
                    assert_eq!(
                        (&*view).class(),
                        original,
                        "Drop did not restore original class"
                    );

                    // Also exercise the existing later-subclass restoration guard.
                    let bridge = native_text::NativeTextInput::install(
                        window.clone(),
                        self.proxy.clone(),
                        chrome,
                    )
                    .unwrap();
                    let later = if let Some(class) = Class::get("VoltAXSmokeLaterView") {
                        class
                    } else {
                        ClassDecl::new("VoltAXSmokeLaterView", (&*view).class())
                            .unwrap()
                            .register()
                    };
                    object_setClass(view, later);
                    drop(bridge);
                    assert_eq!((&*view).class(), later, "Drop overwrote later subclass");
                    object_setClass(view, original); // leave the hidden winit view intact
                }
                let _: () = msg_send![pool, drain];
            }
            self.completed = true;
            println!(
                "PASS: synthetic AX getters/UTF-16/privacy/TTL/modal/class restoration (no PTY)"
            );
            println!("Remaining: VoiceOver, notifications observed by an AX client, range geometry, and full actionable controls/modal focus.");
            event_loop.exit();
        }
        fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: VoltEvent) {
            let VoltEvent::NativeText { window_id, text } = event;
            panic!(
                "unexpected input event in synthetic AX smoke: {window_id:?}, {} units",
                text.encode_utf16().count()
            );
        }
        fn window_event(
            &mut self,
            _event_loop: &ActiveEventLoop,
            _id: WindowId,
            _event: WindowEvent,
        ) {
        }
    }

    pub fn run() {
        let event_loop = EventLoop::<VoltEvent>::with_user_event().build().unwrap();
        let mut check = Check {
            proxy: event_loop.create_proxy(),
            completed: false,
        };
        event_loop.run_app(&mut check).unwrap();
        assert!(check.completed);
    }
}

#[cfg(target_os = "macos")]
fn main() {
    smoke::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("native_accessibility_check requires macOS");
}
