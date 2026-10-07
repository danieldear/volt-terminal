//! Route native content-view input into Volt, including Emoji & Symbols.
//!
//! winit 0.30 only commits `insertText:replacementRange:` with marked text.
//! The character picker has no marked text and no keyboard event, so it is
//! otherwise lost. Subclass only our view; never swizzle all NSViews. Keyboard
//! and composed input remain owned by winit to avoid inserting text twice.
//! Custom tabs route titlebar hit-tests to the content view without changing
//! AppKit's drawing order. Native traffic lights remain above GPU content.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL, NO, YES};
use objc::{class, msg_send, sel, sel_impl, Encode, Encoding};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::event_loop::EventLoopProxy;
use winit::window::{Window, WindowId};

use crate::app::VoltEvent;

const CLASS_NAME: &str = "VoltNativeTextInputView";

#[repr(C)]
#[derive(Clone, Copy)]
struct NSPoint {
    x: f64,
    y: f64,
}

unsafe impl Encode for NSPoint {
    fn encode() -> Encoding {
        unsafe { Encoding::from_str("{CGPoint=dd}") }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NSRange {
    location: usize,
    length: usize,
}

// macOS is 64-bit on all supported targets; match Foundation's NSUInteger pair.
unsafe impl Encode for NSRange {
    fn encode() -> Encoding {
        unsafe { Encoding::from_str("{_NSRange=QQ}") }
    }
}

#[link(name = "objc")]
unsafe extern "C" {
    fn object_setClass(object: *mut Object, class: *const Class) -> *const Class;
}

struct Context {
    window_id: WindowId,
    proxy: EventLoopProxy<VoltEvent>,
    key_depth: Cell<usize>,
    custom_tab_bar: bool,
}

thread_local! {
    // Main-thread only. Rc cloning does not clone EventLoopProxy / wake CFRunLoop.
    static CONTEXTS: RefCell<HashMap<usize, Rc<Context>>> = RefCell::default();
    static CHROME_CONTEXTS: RefCell<HashMap<usize, ChromeContext>> = RefCell::default();
}

fn context(view: &Object) -> Option<Rc<Context>> {
    CONTEXTS.with(|contexts| {
        contexts
            .borrow()
            .get(&(view as *const Object as usize))
            .cloned()
    })
}

fn forwards_direct_text(key_depth: usize, marked: bool) -> bool {
    key_depth == 0 && !marked
}

fn native_chrome_contains(x: f64, y: f64) -> bool {
    // Logical points, matching the renderer's 78-point traffic-light reserve.
    // WinitView is flipped, so its local y=0 is the top of the window.
    (0.0..78.0).contains(&x) && (0.0..38.0).contains(&y)
}

extern "C" fn hit_test(view: &Object, _selector: Sel, point: NSPoint) -> *mut Object {
    if context(view).is_some_and(|c| c.custom_tab_bar) {
        let parent: *mut Object = unsafe { msg_send![view, superview] };
        let local: NSPoint = unsafe { msg_send![view, convertPoint: point fromView: parent] };
        if native_chrome_contains(local.x, local.y) {
            // Leave the real traffic lights (and macOS sharing controls) in
            // AppKit's titlebar. Everything else reaches the content view.
            return std::ptr::null_mut();
        }
    }
    Class::get(CLASS_NAME)
        .and_then(Class::superclass)
        .map_or(std::ptr::null_mut(), |superclass| unsafe {
            msg_send![super(view, superclass), hitTest: point]
        })
}

// An empty, nonopaque NSView routes titlebar input without moving the Metal
// view or subclassing AppKit-owned frame/titlebar classes. It never draws.
#[derive(Clone, Copy)]
struct ChromeContext {
    content: *mut Object,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NSSize {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct NSRect {
    origin: NSPoint,
    size: NSSize,
}
unsafe impl Encode for NSRect {
    fn encode() -> Encoding {
        unsafe { Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}") }
    }
}

const CHROME_CLASS_NAME: &str = "VoltChromeInputView";
extern "C" fn chrome_hit_test(overlay: &Object, _selector: Sel, point: NSPoint) -> *mut Object {
    let route = CHROME_CONTEXTS.with(|contexts| {
        contexts
            .borrow()
            .get(&(overlay as *const Object as usize))
            .copied()
    });
    let Some(route) = route else {
        return std::ptr::null_mut();
    };
    unsafe {
        let parent: *mut Object = msg_send![overlay, superview];
        let local: NSPoint = msg_send![route.content, convertPoint: point fromView: parent];
        if local.x < 78.0 || !(0.0..38.0).contains(&local.y) {
            return std::ptr::null_mut();
        }
        // Preserve native controls beyond the left reserve too (e.g. sharing).
        // Ask siblings only, never the parent, to avoid recursing into ourselves.
        let siblings: *mut Object = msg_send![parent, subviews];
        let count: usize = msg_send![siblings, count];
        for i in (0..count).rev() {
            let sibling: *mut Object = msg_send![siblings, objectAtIndex: i];
            if std::ptr::eq(sibling, overlay) || sibling == route.content {
                continue;
            }
            let hit: *mut Object = msg_send![sibling, hitTest: point];
            let mut ancestor = hit;
            while !ancestor.is_null() && ancestor != parent {
                let control: BOOL = msg_send![ancestor, isKindOfClass: class!(NSControl)];
                if control == YES {
                    return std::ptr::null_mut();
                }
                ancestor = msg_send![ancestor, superview];
            }
        }
        msg_send![route.content, hitTest: point]
    }
}

struct ChromeInput {
    overlay: *mut Object,
}
impl ChromeInput {
    unsafe fn install(content: *mut Object) -> anyhow::Result<Self> {
        let parent: *mut Object = msg_send![content, superview];
        anyhow::ensure!(
            !parent.is_null(),
            "content view has no parent for chrome routing"
        );
        let subclass = if let Some(class) = Class::get(CHROME_CLASS_NAME) {
            class
        } else {
            let mut decl = ClassDecl::new(CHROME_CLASS_NAME, class!(NSView))
                .ok_or_else(|| anyhow::anyhow!("cannot register chrome input view"))?;
            decl.add_method(
                sel!(hitTest:),
                chrome_hit_test as extern "C" fn(&Object, Sel, NSPoint) -> *mut Object,
            );
            decl.register()
        };
        let bounds: NSRect = msg_send![parent, bounds];
        let overlay: *mut Object = msg_send![subclass, alloc];
        let overlay: *mut Object = msg_send![overlay, initWithFrame: bounds];
        anyhow::ensure!(!overlay.is_null(), "cannot create chrome input view");
        let _: () = msg_send![overlay, setAutoresizingMask: 18usize]; // width + height
        CHROME_CONTEXTS.with(|contexts| {
            contexts
                .borrow_mut()
                .insert(overlay as usize, ChromeContext { content })
        });
        let _: () = msg_send![parent, addSubview: overlay positioned: 1isize relativeTo: std::ptr::null_mut::<Object>()];
        Ok(Self { overlay })
    }
}
impl Drop for ChromeInput {
    fn drop(&mut self) {
        unsafe {
            CHROME_CONTEXTS.with(|contexts| contexts.borrow_mut().remove(&(self.overlay as usize)));
            let _: () = msg_send![self.overlay, removeFromSuperview];
            let _: () = msg_send![self.overlay, release];
        }
    }
}

extern "C" fn mouse_down_can_move_window(view: &Object, _selector: Sel) -> BOOL {
    if context(view).is_some_and(|c| c.custom_tab_bar) {
        NO
    } else {
        Class::get(CLASS_NAME)
            .and_then(Class::superclass)
            .map_or(NO, |superclass| unsafe {
                msg_send![super(view, superclass), mouseDownCanMoveWindow]
            })
    }
}

extern "C" fn key_down(view: &Object, _selector: Sel, event: *mut Object) {
    let context = context(view);
    if let Some(context) = &context {
        context.key_depth.set(context.key_depth.get() + 1);
    }
    // Use our registered class, not the instance's class, so a later subclass
    // cannot cause this super call to recurse into our override.
    if let Some(superclass) = Class::get(CLASS_NAME).and_then(Class::superclass) {
        unsafe {
            let _: () = msg_send![super(view, superclass), keyDown: event];
        }
    }
    if let Some(context) = &context {
        context.key_depth.set(context.key_depth.get() - 1);
    }
}

unsafe fn inserted_string(mut value: *mut Object) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let attributed: BOOL = msg_send![value, isKindOfClass: class!(NSAttributedString)];
    if attributed == YES {
        value = msg_send![value, string];
    }
    let is_string: BOOL = msg_send![value, isKindOfClass: class!(NSString)];
    if is_string != YES {
        return None;
    }
    // Copy while the AppKit object is alive. Byte length, not CStr, preserves
    // embedded NULs and complete multi-scalar emoji (ZWJ / modifiers / flags).
    let length: usize = msg_send![value, lengthOfBytesUsingEncoding: 4usize]; // NSUTF8StringEncoding
    let bytes: *const u8 = msg_send![value, UTF8String];
    if bytes.is_null() || length == 0 {
        return None;
    }
    std::str::from_utf8(std::slice::from_raw_parts(bytes, length))
        .ok()
        .map(str::to_owned)
}

// Older AppKit senders use NSResponder's single-argument spelling.
extern "C" fn insert_text_legacy(view: &Object, _selector: Sel, value: *mut Object) {
    insert_text(
        view,
        sel!(insertText:replacementRange:),
        value,
        NSRange {
            location: isize::MAX as usize,
            length: 0,
        },
    );
}

extern "C" fn insert_text(view: &Object, _selector: Sel, value: *mut Object, range: NSRange) {
    let marked: BOOL = unsafe { msg_send![view, hasMarkedText] };
    if let Some(context) = context(view) {
        if forwards_direct_text(context.key_depth.get(), marked == YES) {
            if let Some(text) = unsafe { inserted_string(value) } {
                let _ = context.proxy.send_event(VoltEvent::NativeText {
                    window_id: context.window_id,
                    text,
                });
                return;
            }
        }
    }
    if let Some(superclass) = Class::get(CLASS_NAME).and_then(Class::superclass) {
        unsafe {
            let _: () =
                msg_send![super(view, superclass), insertText: value replacementRange: range];
        }
    }
}

/// Owns the subclass installation and keeps the native view alive until restored.
/// Rc makes this guard !Send / !Sync: creation and destruction stay on AppKit's
/// main thread, just like winit window event handling.
pub(crate) struct NativeTextInput {
    view: *mut Object,
    original_class: &'static Class,
    chrome_input: Option<ChromeInput>,
    _window: Arc<Window>,
    _context: Rc<Context>,
}

impl NativeTextInput {
    pub(crate) fn install(
        window: Arc<Window>,
        proxy: EventLoopProxy<VoltEvent>,
        custom_tab_bar: bool,
    ) -> anyhow::Result<Self> {
        let is_main: BOOL = unsafe { msg_send![class!(NSThread), isMainThread] };
        anyhow::ensure!(
            is_main == YES,
            "native text input must be installed on the main thread"
        );
        let handle = window.window_handle()?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            anyhow::bail!("native text input requires an AppKit view");
        };
        let view = handle.ns_view.as_ptr().cast::<Object>();
        let original_class = unsafe { (&*view).class() };
        let subclass = if let Some(class) = Class::get(CLASS_NAME) {
            anyhow::ensure!(
                class.superclass() == Some(original_class),
                "unexpected native view class"
            );
            class
        } else {
            let mut declaration = ClassDecl::new(CLASS_NAME, original_class)
                .ok_or_else(|| anyhow::anyhow!("cannot register native text input view"))?;
            unsafe {
                declaration.add_method(
                    sel!(hitTest:),
                    hit_test as extern "C" fn(&Object, Sel, NSPoint) -> *mut Object,
                );
                declaration.add_method(
                    sel!(mouseDownCanMoveWindow),
                    mouse_down_can_move_window as extern "C" fn(&Object, Sel) -> BOOL,
                );
                declaration.add_method(
                    sel!(insertText:),
                    insert_text_legacy as extern "C" fn(&Object, Sel, *mut Object),
                );
                declaration.add_method(
                    sel!(keyDown:),
                    key_down as extern "C" fn(&Object, Sel, *mut Object),
                );
                declaration.add_method(
                    sel!(insertText:replacementRange:),
                    insert_text as extern "C" fn(&Object, Sel, *mut Object, NSRange),
                );
            }
            declaration.register()
        };
        let context = Rc::new(Context {
            window_id: window.id(),
            proxy,
            key_depth: Cell::new(0),
            custom_tab_bar,
        });
        // Install chrome routing before replacing the content class so a failed
        // installation leaves both the original view and context map untouched.
        let chrome_input = if custom_tab_bar {
            Some(unsafe { ChromeInput::install(view)? })
        } else {
            None
        };
        CONTEXTS.with(|contexts| contexts.borrow_mut().insert(view as usize, context.clone()));
        unsafe {
            object_setClass(view, subclass);
        }
        Ok(Self {
            view,
            original_class,
            chrome_input,
            _window: window,
            _context: context,
        })
    }
}

impl Drop for NativeTextInput {
    fn drop(&mut self) {
        // Remove frame routing while the content view and window are still alive.
        self.chrome_input.take();
        CONTEXTS.with(|contexts| contexts.borrow_mut().remove(&(self.view as usize)));
        // The retained Window keeps view valid during restoration. Don't undo
        // another component's later subclass if one was installed on top of us.
        unsafe {
            if Some((&*self.view).class()) == Class::get(CLASS_NAME) {
                object_setClass(self.view, self.original_class);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traffic_lights_keep_native_input_but_tabs_do_not() {
        assert!(native_chrome_contains(20.0, 18.0));
        assert!(native_chrome_contains(77.99, 37.99));
        assert!(!native_chrome_contains(78.0, 18.0));
        assert!(!native_chrome_contains(20.0, 38.0));
        assert!(!native_chrome_contains(-1.0, 18.0));
    }

    #[test]
    fn only_out_of_band_unmarked_text_is_forwarded() {
        assert!(forwards_direct_text(0, false));
        assert!(!forwards_direct_text(0, true));
        assert!(!forwards_direct_text(1, false));
        assert!(!forwards_direct_text(2, false));
        assert!(!forwards_direct_text(1, true));
    }

    #[test]
    fn copies_plain_and_attributed_native_text_without_losing_scalars() {
        unsafe {
            let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
            for text in ["😀", "👍🏽", "👩‍💻", "🇮🇳", "❤️", "a\0b", "plain"] {
                let string: *mut Object = msg_send![class!(NSString), alloc];
                let string: *mut Object = msg_send![string, initWithBytes: text.as_ptr() length: text.len() encoding: 4usize];
                assert_eq!(inserted_string(string).as_deref(), Some(text));
                let attributed: *mut Object = msg_send![class!(NSAttributedString), alloc];
                let attributed: *mut Object = msg_send![attributed, initWithString: string];
                assert_eq!(inserted_string(attributed).as_deref(), Some(text));
                let _: () = msg_send![attributed, release];
                let _: () = msg_send![string, release];
            }
            assert_eq!(inserted_string(std::ptr::null_mut()), None);
            let _: () = msg_send![pool, drain];
        }
    }
}
