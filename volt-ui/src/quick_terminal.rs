//! macOS global Command+grave shortcut. App/window policy belongs to the caller.
//!
//! Register and retain the guard on the main thread after starting the event loop.
//! No event tap or Accessibility permission is required. The physical ANSI grave
//! key (50) is used, irrespective of the active keyboard layout.

#![cfg(target_os = "macos")]

use std::ffi::{c_int, c_ulong, c_void};
use std::marker::PhantomData;
use std::ptr;
use std::rc::Rc;
use winit::event_loop::EventLoopProxy;

use crate::app::VoltEvent;

type OSStatus = i32;
type EventRef = *mut c_void;
type EventTargetRef = *mut c_void;
type EventHandlerRef = *mut c_void;
type EventHotKeyRef = *mut c_void;
type EventHandlerCallRef = *mut c_void;
type EventHandlerProc =
    unsafe extern "C" fn(EventHandlerCallRef, EventRef, *mut c_void) -> OSStatus;

#[repr(C)]
#[derive(Clone, Copy)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct EventHotKeyID {
    signature: u32,
    id: u32,
}

// Verified in installed SDK: Events.h, CarbonEvents.h, CarbonEventsCore.h.
const GRAVE_KEYCODE: u32 = 0x32; // kVK_ANSI_Grave
const COMMAND_MODIFIER: u32 = 1 << 8; // cmdKey (not NSEvent modifier flags)
const KEYBOARD_CLASS: u32 = u32::from_be_bytes(*b"keyb");
const HOT_KEY_PRESSED: u32 = 5;
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const HOT_KEY_ID_TYPE: u32 = u32::from_be_bytes(*b"hkid");
const NOT_HANDLED: OSStatus = -9874;
const HOT_KEY_EXISTS: OSStatus = -9878;
const EXCLUSIVE: u32 = 1 << 0; // kEventHotKeyExclusive: fail on existing ownership
const HOT_KEY_ID: EventHotKeyID = EventHotKeyID {
    signature: u32::from_be_bytes(*b"Volt"),
    id: 1,
};

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> EventTargetRef;
    fn InstallEventHandler(
        target: EventTargetRef,
        handler: EventHandlerProc,
        // SDK ItemCount and ByteCount are unsigned long, NOT UInt32.
        count: c_ulong,
        types: *const EventTypeSpec,
        user_data: *mut c_void,
        out_handler: *mut EventHandlerRef,
    ) -> OSStatus;
    fn RemoveEventHandler(handler: EventHandlerRef) -> OSStatus;
    fn RegisterEventHotKey(
        keycode: u32,
        modifiers: u32,
        id: EventHotKeyID,
        target: EventTargetRef,
        options: u32,
        out_hot_key: *mut EventHotKeyRef,
    ) -> OSStatus;
    fn UnregisterEventHotKey(hot_key: EventHotKeyRef) -> OSStatus;
    fn GetEventClass(event: EventRef) -> u32;
    fn GetEventKind(event: EventRef) -> u32;
    fn GetEventParameter(
        event: EventRef,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buffer_size: c_ulong,
        actual_size: *mut c_ulong,
        data: *mut c_void,
    ) -> OSStatus;
}

unsafe extern "C" {
    fn pthread_main_np() -> c_int;
}

fn is_main_thread() -> bool {
    // SAFETY: pthread_main_np takes no arguments and has no side effects.
    unsafe { pthread_main_np() != 0 }
}

fn matches_hot_key(class: u32, kind: u32, id: EventHotKeyID) -> bool {
    class == KEYBOARD_CLASS && kind == HOT_KEY_PRESSED && id == HOT_KEY_ID
}

/// Owns the registration, installed handler, and its stable boxed proxy userdata.
/// This guard is deliberately neither Send nor Sync; drop it on the main thread
/// before destroying the event loop. Registration errors must be surfaced by App.
pub struct GlobalHotKey {
    hot_key: EventHotKeyRef,
    handler: EventHandlerRef,
    proxy: Option<Box<EventLoopProxy<VoltEvent>>>,
    _main_thread: PhantomData<Rc<()>>,
}

impl GlobalHotKey {
    /// Register Command+grave. Conflicts are errors, never silently replaced.
    pub fn register(proxy: EventLoopProxy<VoltEvent>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            is_main_thread(),
            "global shortcut registration requires the macOS main thread"
        );
        let mut guard = Self {
            hot_key: ptr::null_mut(),
            handler: ptr::null_mut(),
            proxy: Some(Box::new(proxy)),
            _main_thread: PhantomData,
        };
        let event_type = EventTypeSpec {
            event_class: KEYBOARD_CLASS,
            event_kind: HOT_KEY_PRESSED,
        };
        let user_data = (&mut **guard.proxy.as_mut().expect("proxy initialized"))
            as *mut EventLoopProxy<VoltEvent> as *mut c_void;
        // SAFETY: main thread; the Box allocation remains stable until handler
        // removal, including rollback on partial registration failure.
        let target = unsafe { GetApplicationEventTarget() };
        anyhow::ensure!(
            !target.is_null(),
            "Carbon application event target is unavailable"
        );
        let status = unsafe {
            InstallEventHandler(
                target,
                hot_key_handler,
                1,
                &event_type,
                user_data,
                &mut guard.handler,
            )
        };
        anyhow::ensure!(
            status == 0,
            "InstallEventHandler failed (OSStatus {status})"
        );
        anyhow::ensure!(
            !guard.handler.is_null(),
            "InstallEventHandler returned a null handler"
        );
        // SAFETY: SDK ABI; target and output storage are valid. Exclusive
        // registration refuses existing ownership rather than silently sharing it.
        let status = unsafe {
            RegisterEventHotKey(
                GRAVE_KEYCODE,
                COMMAND_MODIFIER,
                HOT_KEY_ID,
                target,
                EXCLUSIVE,
                &mut guard.hot_key,
            )
        };
        if status == HOT_KEY_EXISTS {
            anyhow::bail!(
                "Command+grave global shortcut is already registered (OSStatus {status})"
            );
        }
        anyhow::ensure!(
            status == 0,
            "RegisterEventHotKey for Command+grave failed (OSStatus {status})"
        );
        anyhow::ensure!(
            !guard.hot_key.is_null(),
            "RegisterEventHotKey returned a null registration"
        );
        Ok(guard)
    }
}

impl Drop for GlobalHotKey {
    fn drop(&mut self) {
        assert!(
            is_main_thread(),
            "GlobalHotKey must be dropped on the macOS main thread"
        );
        // SAFETY: all refs came from Carbon on this thread. Stop delivery first,
        // then detach the callback, and only then allow the Box to be freed.
        if !self.hot_key.is_null() {
            let status = unsafe { UnregisterEventHotKey(self.hot_key) };
            if status != 0 {
                eprintln!("volt-ui: UnregisterEventHotKey failed (OSStatus {status})");
            }
        }
        if !self.handler.is_null() {
            let status = unsafe { RemoveEventHandler(self.handler) };
            if status != 0 {
                eprintln!("volt-ui: RemoveEventHandler failed (OSStatus {status}); retaining callback userdata for safety");
                // Carbon may still call the handler. A bounded leak on this
                // exceptional failure is preferable to a dangling callback.
                if let Some(proxy) = self.proxy.take() {
                    let _ = Box::leak(proxy);
                }
            }
        }
    }
}

unsafe extern "C" fn hot_key_handler(
    _call: EventHandlerCallRef,
    event: EventRef,
    user_data: *mut c_void,
) -> OSStatus {
    if event.is_null() || user_data.is_null() || !is_main_thread() {
        return NOT_HANDLED;
    }
    // SAFETY: Carbon supplies a live event for the duration of this callback.
    let class = unsafe { GetEventClass(event) };
    let kind = unsafe { GetEventKind(event) };
    if class != KEYBOARD_CLASS || kind != HOT_KEY_PRESSED {
        return NOT_HANDLED;
    }
    let mut id = EventHotKeyID::default();
    let mut size: c_ulong = 0;
    let mut actual_type = 0;
    let status = unsafe {
        GetEventParameter(
            event,
            DIRECT_OBJECT,
            HOT_KEY_ID_TYPE,
            &mut actual_type,
            std::mem::size_of::<EventHotKeyID>() as c_ulong,
            &mut size,
            (&mut id as *mut EventHotKeyID).cast(),
        )
    };
    if status != 0
        || actual_type != HOT_KEY_ID_TYPE
        || size != std::mem::size_of::<EventHotKeyID>() as c_ulong
        || !matches_hot_key(class, kind, id)
    {
        return NOT_HANDLED;
    }
    // SAFETY: guard owns this stable allocation until handler removal. Enqueue
    // only; never create windows or re-enter App/Carbon from the native callback.
    // Contain Rust panics at the C ABI boundary (closed event loop is harmless).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let proxy = unsafe { &*user_data.cast::<EventLoopProxy<VoltEvent>>() };
        let _ = proxy.send_event(VoltEvent::ToggleQuickTerminal);
    }));
    if result.is_ok() {
        0
    } else {
        NOT_HANDLED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_uses_carbon_command_and_physical_grave() {
        assert_eq!(GRAVE_KEYCODE, 50);
        assert_eq!(COMMAND_MODIFIER, 256);
        assert_eq!(EXCLUSIVE, 1);
        assert_eq!(KEYBOARD_CLASS, 0x6b657962);
        assert_eq!(HOT_KEY_ID_TYPE, 0x686b6964);
    }

    #[test]
    fn only_our_pressed_hot_key_matches() {
        assert!(matches_hot_key(KEYBOARD_CLASS, HOT_KEY_PRESSED, HOT_KEY_ID));
        assert!(!matches_hot_key(KEYBOARD_CLASS, 6, HOT_KEY_ID));
        assert!(!matches_hot_key(0, HOT_KEY_PRESSED, HOT_KEY_ID));
        assert!(!matches_hot_key(
            KEYBOARD_CLASS,
            HOT_KEY_PRESSED,
            EventHotKeyID {
                signature: 0,
                ..HOT_KEY_ID
            }
        ));
        assert!(!matches_hot_key(
            KEYBOARD_CLASS,
            HOT_KEY_PRESSED,
            EventHotKeyID {
                id: 2,
                ..HOT_KEY_ID
            }
        ));
    }

    #[test]
    fn carbon_struct_layout_is_two_u32_fields() {
        assert_eq!(std::mem::size_of::<EventHotKeyID>(), 8);
        assert_eq!(std::mem::align_of::<EventHotKeyID>(), 4);
        assert_eq!(std::mem::size_of::<EventTypeSpec>(), 8);
        assert_eq!(std::mem::align_of::<EventTypeSpec>(), 4);
        assert_eq!(std::mem::offset_of!(EventHotKeyID, id), 4);
        assert_eq!(std::mem::offset_of!(EventTypeSpec, event_kind), 4);
    }
}

/// Keep only the transient window out of native tab groups and AppKit restoration.
pub(crate) fn configure_window(window: &winit::window::Window) -> anyhow::Result<()> {
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    anyhow::ensure!(
        is_main_thread(),
        "quick window setup requires the main thread"
    );
    let handle = window.window_handle()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        anyhow::bail!("quick terminal requires an AppKit window");
    };
    // SAFETY: winit owns this live NSView/NSWindow; all messages run on main.
    unsafe {
        let view = handle.ns_view.as_ptr().cast::<objc::runtime::Object>();
        let native: *mut objc::runtime::Object = msg_send![view, window];
        anyhow::ensure!(!native.is_null(), "quick terminal NSWindow is unavailable");
        let _: () = msg_send![native, setTabbingMode: 2isize]; // NSWindowTabbingModeDisallowed
        let _: () = msg_send![native, setRestorable: objc::runtime::NO];
    }
    Ok(())
}

/// Bring Volt forward for a shortcut received while another application is active.
pub(crate) fn activate_app() {
    use objc::{class, msg_send, sel, sel_impl};
    assert!(
        is_main_thread(),
        "quick terminal activation requires the main thread"
    );
    // SAFETY: NSApplication is process-global and activation is main-thread only.
    unsafe {
        let app: *mut objc::runtime::Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, activateIgnoringOtherApps: objc::runtime::YES];
    }
}

/// Turn a detached quick window into an ordinary, visible standalone window.
/// Keep custom tabs and the existing NSWindow/PTYs; do not create a replacement.
pub(crate) fn promote_window(window: &winit::window::Window) -> anyhow::Result<()> {
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    anyhow::ensure!(
        is_main_thread(),
        "quick window promotion requires the main thread"
    );
    // Restore ordinary stacking even if acquiring the native handle fails.
    window.set_window_level(winit::window::WindowLevel::Normal);
    let handle = window.window_handle()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        anyhow::bail!("quick terminal requires an AppKit window");
    };
    // SAFETY: same live, main-thread NSWindow as configure_window; do not alter
    // process-global automatic tabbing or touch any terminal/pane ownership.
    unsafe {
        let view = handle.ns_view.as_ptr().cast::<objc::runtime::Object>();
        let native: *mut objc::runtime::Object = msg_send![view, window];
        anyhow::ensure!(!native.is_null(), "quick terminal NSWindow is unavailable");
        let _: () = msg_send![native, setRestorable: objc::runtime::YES];
    }
    Ok(())
}
