//! Bridge AppKit's out-of-band text insertion (Emoji & Symbols) into Volt.
//!
//! winit 0.30 only commits `insertText:replacementRange:` with marked text.
//! The character picker has no marked text and no keyboard event, so it is
//! otherwise lost. Subclass only our view; never swizzle all NSViews. Keyboard
//! and composed input remain owned by winit to avoid inserting text twice.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL, YES};
use objc::{class, msg_send, sel, sel_impl, Encode, Encoding};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::event_loop::EventLoopProxy;
use winit::window::{Window, WindowId};

use crate::app::VoltEvent;

const CLASS_NAME: &str = "VoltNativeTextInputView";

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
}

thread_local! {
    // Main-thread only. Rc cloning does not clone EventLoopProxy / wake CFRunLoop.
    static CONTEXTS: RefCell<HashMap<usize, Rc<Context>>> = RefCell::default();
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
    _window: Arc<Window>,
    _context: Rc<Context>,
}

impl NativeTextInput {
    pub(crate) fn install(
        window: Arc<Window>,
        proxy: EventLoopProxy<VoltEvent>,
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
        });
        CONTEXTS.with(|contexts| contexts.borrow_mut().insert(view as usize, context.clone()));
        // No ivars are added: the subclass has exactly the original layout.
        unsafe {
            object_setClass(view, subclass);
        }
        Ok(Self {
            view,
            original_class,
            _window: window,
            _context: context,
        })
    }
}

impl Drop for NativeTextInput {
    fn drop(&mut self) {
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
