//! macOS native text bridge smoke test. No PTY, shell, or visible window.
//! Run: cargo run --locked -p volt-ui --example native_input_check
//! Tests actual Objective-C dispatch, text ownership, window targeting, and
//! subclass restoration. This does NOT simulate a click in the system picker.

#[cfg(target_os = "macos")]
mod app {
    #[derive(Debug)]
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
fn main() {
    use app::VoltEvent;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
    use winit::window::{Window, WindowId};

    struct Check {
        proxy: EventLoopProxy<VoltEvent>,
        expected: VecDeque<(WindowId, String)>,
        windows: Vec<Arc<Window>>,
        deadline: Instant,
        received: usize,
        keyboard_received: usize,
    }
    impl ApplicationHandler<VoltEvent> for Check {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if !self.windows.is_empty() {
                return;
            }
            // Repeated installation exercises reuse of the registered subclass.
            for custom_tab_bar in [false, true, true] {
                use winit::platform::macos::WindowAttributesExtMacOS;
                let mut attributes = Window::default_attributes().with_visible(false);
                if custom_tab_bar {
                    attributes = attributes
                        .with_titlebar_transparent(true)
                        .with_title_hidden(true)
                        .with_fullsize_content_view(true);
                }
                let window = Arc::new(event_loop.create_window(attributes).unwrap());
                let RawWindowHandle::AppKit(handle) = window.window_handle().unwrap().as_raw()
                else {
                    unreachable!()
                };
                let view = handle.ns_view.as_ptr().cast::<Object>();
                let original = unsafe { (&*view).class() };
                let original_move: objc::runtime::BOOL =
                    unsafe { msg_send![view, mouseDownCanMoveWindow] };
                let parent: *mut Object = unsafe { msg_send![view, superview] };
                let original_parent_class = unsafe { (&*parent).class() };
                let original_siblings: Vec<*mut Object> = unsafe {
                    let subviews: *mut Object = msg_send![parent, subviews];
                    let count: usize = msg_send![subviews, count];
                    (0..count)
                        .map(|i| msg_send![subviews, objectAtIndex: i])
                        .collect()
                };
                let original_top: *mut Object = unsafe {
                    let subviews: *mut Object = msg_send![parent, subviews];
                    msg_send![subviews, lastObject]
                };
                let bridge = native_text::NativeTextInput::install(
                    window.clone(),
                    self.proxy.clone(),
                    custom_tab_bar,
                )
                .unwrap();
                assert_ne!(unsafe { (&*view).class() }, original);
                unsafe {
                    #[repr(C)]
                    #[derive(Clone, Copy)]
                    struct Point(f64, f64);
                    unsafe impl objc::Encode for Point {
                        fn encode() -> objc::Encoding {
                            unsafe { objc::Encoding::from_str("{CGPoint=dd}") }
                        }
                    }
                    let movable: objc::runtime::BOOL = msg_send![view, mouseDownCanMoveWindow];
                    let subviews: *mut Object = msg_send![parent, subviews];
                    let top: *mut Object = msg_send![subviews, lastObject];
                    if custom_tab_bar {
                        assert_eq!(movable, objc::runtime::NO);
                        assert_ne!(
                            top, view,
                            "GPU content must stay behind native traffic lights"
                        );
                        assert_eq!(
                            (&*parent).class(),
                            original_parent_class,
                            "AppKit-owned frame must not be subclassed"
                        );
                        let opaque: objc::runtime::BOOL = msg_send![top, isOpaque];
                        assert_eq!(opaque, objc::runtime::NO);
                        for (i, expected) in original_siblings.iter().enumerate() {
                            let sibling: *mut Object = msg_send![subviews, objectAtIndex: i];
                            assert_eq!(sibling, *expected, "original native drawing order changed");
                        }
                        let p: Point =
                            msg_send![view, convertPoint: Point(110.0, 18.0) toView: parent];
                        let hit: *mut Object = msg_send![parent, hitTest: p];
                        assert_eq!(hit, view, "tab-center hit still goes to native titlebar");
                        let p: Point =
                            msg_send![view, convertPoint: Point(20.0, 18.0) toView: parent];
                        let hit: *mut Object = msg_send![view, hitTest: p];
                        assert!(
                            hit.is_null(),
                            "traffic-light area intercepted by content view"
                        );
                        let ns_window: *mut Object = msg_send![view, window];
                        for button_kind in 0usize..3 {
                            let button: *mut Object =
                                msg_send![ns_window, standardWindowButton: button_kind];
                            assert!(!button.is_null());
                            let local: Point =
                                msg_send![view, convertPoint: Point(7.0, 7.0) fromView: button];
                            let p: Point = msg_send![view, convertPoint: local toView: parent];
                            let hit: *mut Object = msg_send![parent, hitTest: p];
                            assert_eq!(
                                hit, button,
                                "native traffic-light button no longer receives hits"
                            );
                            let hidden: objc::runtime::BOOL = msg_send![button, isHidden];
                            let alpha: f64 = msg_send![button, alphaValue];
                            assert_eq!(hidden, objc::runtime::NO);
                            assert_eq!(alpha, 1.0, "native button was dimmed by custom chrome");
                        }
                    } else {
                        assert_eq!(movable, original_move, "native view drag policy changed");
                        assert_eq!(top, original_top, "native window view order changed");
                    }
                }
                for text in ["😀", "👍🏽", "👩‍💻", "🇮🇳", "❤️"] {
                    unsafe {
                        let string: *mut Object = msg_send![class!(NSString), alloc];
                        let string: *mut Object = msg_send![string, initWithBytes: text.as_ptr() length: text.len() encoding: 4usize];
                        // NSRange is two NSUInteger values; use an encoded local
                        // type to exercise the actual two-argument AppKit selector.
                        #[repr(C)]
                        #[derive(Clone, Copy)]
                        struct Range(usize, usize);
                        unsafe impl objc::Encode for Range {
                            fn encode() -> objc::Encoding {
                                unsafe { objc::Encoding::from_str("{_NSRange=QQ}") }
                            }
                        }
                        self.expected.push_back((window.id(), text.to_owned()));
                        let _: () = msg_send![view, insertText: string replacementRange: Range(isize::MAX as usize, 0)];
                        let attributed: *mut Object = msg_send![class!(NSAttributedString), alloc];
                        let attributed: *mut Object = msg_send![attributed, initWithString: string];
                        self.expected.push_back((window.id(), text.to_owned()));
                        let _: () = msg_send![view, insertText: attributed];
                        let _: () = msg_send![attributed, release];
                        let _: () = msg_send![string, release];
                    }
                }
                // Ordinary keyboard input must remain a KeyboardInput, never an
                // extra NativeText event, including when winit interprets IME.
                for ime in [false, true] {
                    window.set_ime_allowed(ime);
                    unsafe {
                        #[repr(C)]
                        #[derive(Clone, Copy)]
                        struct Point(f64, f64);
                        unsafe impl objc::Encode for Point {
                            fn encode() -> objc::Encoding {
                                unsafe { objc::Encoding::from_str("{CGPoint=dd}") }
                            }
                        }
                        let ns_window: *mut Object = msg_send![view, window];
                        let number: isize = msg_send![ns_window, windowNumber];
                        let chars: *mut Object = msg_send![class!(NSString), alloc];
                        let chars: *mut Object = msg_send![chars, initWithBytes: b"a".as_ptr() length: 1usize encoding: 4usize];
                        let event: *mut Object = msg_send![class!(NSEvent), keyEventWithType: 10usize location: Point(0.0, 0.0) modifierFlags: 0usize timestamp: 0.0f64 windowNumber: number context: std::ptr::null_mut::<Object>() characters: chars charactersIgnoringModifiers: chars isARepeat: objc::runtime::NO keyCode: 0u16];
                        assert!(!event.is_null());
                        let _: () = msg_send![view, keyDown: event];
                        let _: () = msg_send![chars, release];
                    }
                }
                window.set_title("Updated directory — 😀");
                drop(bridge);
                assert_eq!(unsafe { (&*view).class() }, original);
                assert_eq!(unsafe { (&*parent).class() }, original_parent_class);
                unsafe {
                    let subviews: *mut Object = msg_send![parent, subviews];
                    let top: *mut Object = msg_send![subviews, lastObject];
                    assert_eq!(top, original_top, "input overlay was not removed on drop");
                }
                self.windows.push(window);
            }
        }
        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            if let WindowEvent::KeyboardInput { event, .. } = event {
                assert_eq!(event.text.as_deref(), Some("a"));
                self.keyboard_received += 1;
                if self.received == 30 && self.keyboard_received == 6 {
                    event_loop.exit();
                }
            }
        }
        fn user_event(&mut self, event_loop: &ActiveEventLoop, event: VoltEvent) {
            let VoltEvent::NativeText { window_id, text } = event;
            assert_eq!(self.expected.pop_front(), Some((window_id, text)));
            self.received += 1;
            if self.received == 30 && self.keyboard_received == 6 {
                event_loop.exit();
            }
        }
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            assert!(
                Instant::now() < self.deadline,
                "timed out waiting for native text"
            );
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
        }
    }
    let event_loop = EventLoop::<VoltEvent>::with_user_event().build().unwrap();
    let mut check = Check {
        proxy: event_loop.create_proxy(),
        expected: VecDeque::new(),
        windows: Vec::new(),
        deadline: Instant::now() + Duration::from_secs(10),
        received: 0,
        keyboard_received: 0,
    };
    event_loop.run_app(&mut check).unwrap();
    assert_eq!(check.received, 30);
    assert_eq!(check.keyboard_received, 6);
    assert!(check.expected.is_empty());
    println!("PASS: custom/native tab hit routing, unchanged native drawing order and all 3 traffic lights, 30 native insertions, 3 window identities, NSString + NSAttributedString, both selectors, 6 nonduplicated keyboard inputs, subclass restoration");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("SKIP: AppKit text input is macOS-only");
}
