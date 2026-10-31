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
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL, NO, YES};
use objc::{class, msg_send, sel, sel_impl, Encode, Encoding};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::event_loop::EventLoopProxy;
use winit::window::{Window, WindowId};

use crate::app::VoltEvent;

// Nested module keeps the pure snapshot available to parent integration without
// requiring a lib.rs change in this independently owned slice.
#[path = "accessibility.rs"]
pub(crate) mod accessibility;
use accessibility::{AccessibilitySnapshot, TextRange};

const CLASS_NAME: &str = "VoltNativeTextInputView";
const ACCESSIBILITY_REQUEST_TTL: Duration = Duration::from_secs(2);

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

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSAccessibilityValueChangedNotification: *mut Object;
    fn NSAccessibilityPostNotification(element: *mut Object, notification: *mut Object);
}

struct Context {
    window_id: WindowId,
    proxy: EventLoopProxy<VoltEvent>,
    key_depth: Cell<usize>,
    custom_tab_bar: bool,
    accessibility: RefCell<AccessibilitySnapshot>,
    accessibility_last_request: Cell<Option<Instant>>,
    accessibility_window: Weak<Window>,
    accessibility_notifying: Cell<bool>,
    accessibility_needs_refresh: Cell<bool>,
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

// All AX callbacks consult only the immutable main-thread cache. RefCell
// contention or a missing context fails closed, never reaches the Performer.
fn requested_recently(last_request: Option<Instant>, now: Instant) -> bool {
    last_request
        .and_then(|last| now.checked_duration_since(last))
        .is_some_and(|elapsed| elapsed < ACCESSIBILITY_REQUEST_TTL)
}

// Expiry is silent: posting a notification here could itself create renewed
// demand. The freshness bit also closes reentrant/contended borrow paths.
fn expire_accessibility_cache(cache: &RefCell<AccessibilitySnapshot>, needs_refresh: &Cell<bool>) {
    needs_refresh.set(true);
    if let Ok(mut snapshot) = cache.try_borrow_mut() {
        *snapshot = AccessibilitySnapshot::default();
    }
}

fn record_accessibility_request(view: &Object) -> Option<Rc<Context>> {
    let context = context(view)?;
    let now = Instant::now();
    let was_requested = requested_recently(context.accessibility_last_request.get(), now);
    context.accessibility_last_request.set(Some(now));
    if !was_requested {
        expire_accessibility_cache(&context.accessibility, &context.accessibility_needs_refresh);
        // Lazy first/renewed query gets a render opportunity even while idle.
        // No cache borrow is held; no text building happens in this callback.
        if let Some(window) = context.accessibility_window.upgrade() {
            window.request_redraw();
        }
    }
    Some(context)
}

fn with_cached_snapshot<T>(
    cache: &RefCell<AccessibilitySnapshot>,
    last_request: &Cell<Option<Instant>>,
    read: impl FnOnce(&AccessibilitySnapshot) -> T,
) -> T {
    last_request.set(Some(Instant::now()));
    if let Ok(snapshot) = cache.try_borrow() {
        return read(&snapshot);
    }
    read(&AccessibilitySnapshot::default())
}

fn with_snapshot<T>(view: &Object, read: impl FnOnce(&AccessibilitySnapshot) -> T) -> T {
    if let Some(context) = record_accessibility_request(view) {
        if context.accessibility_needs_refresh.get() {
            return read(&AccessibilitySnapshot::default());
        }
        return with_cached_snapshot(
            &context.accessibility,
            &context.accessibility_last_request,
            read,
        );
    }
    read(&AccessibilitySnapshot::default())
}

fn native_range(range: Option<TextRange>) -> NSRange {
    range.map_or(
        NSRange {
            location: isize::MAX as usize,
            length: 0,
        },
        |range| NSRange {
            location: range.location,
            length: range.length,
        },
    )
}

fn ax_string(text: &str) -> *mut Object {
    unsafe {
        let string: *mut Object = msg_send![class!(NSString), alloc];
        let string: *mut Object =
            msg_send![string, initWithBytes: text.as_ptr() length: text.len() encoding: 4usize];
        msg_send![string, autorelease]
    }
}

extern "C" fn ax_element(view: &Object, _selector: Sel) -> BOOL {
    record_accessibility_request(view);
    YES
}
extern "C" fn ax_role(view: &Object, _selector: Sel) -> *mut Object {
    record_accessibility_request(view);
    ax_string("AXStaticText")
}
extern "C" fn ax_label(view: &Object, _selector: Sel) -> *mut Object {
    with_snapshot(view, |snapshot| ax_string(snapshot.label()))
}
extern "C" fn ax_value(view: &Object, _selector: Sel) -> *mut Object {
    with_snapshot(view, |snapshot| {
        // Explicit scope gate: redacted output never exposes terminal text.
        // Modal summaries remain static text, not editable/secure text fields.
        let text = match snapshot.scope() {
            accessibility::AccessibilityScope::Redacted => "",
            accessibility::AccessibilityScope::TerminalOutput
            | accessibility::AccessibilityScope::ModalSummary => snapshot.text(),
        };
        ax_string(text)
    })
}
extern "C" fn ax_number_of_characters(view: &Object, _selector: Sel) -> isize {
    with_snapshot(view, |snapshot| snapshot.number_of_characters() as isize)
}
extern "C" fn ax_visible_range(view: &Object, _selector: Sel) -> NSRange {
    with_snapshot(view, |snapshot| {
        native_range(Some(snapshot.visible_range()))
    })
}
extern "C" fn ax_selected_range(view: &Object, _selector: Sel) -> NSRange {
    with_snapshot(view, |snapshot| native_range(snapshot.selected_range()))
}
extern "C" fn ax_selected_text(view: &Object, _selector: Sel) -> *mut Object {
    with_snapshot(view, |snapshot| {
        ax_string(snapshot.selected_text().unwrap_or(""))
    })
}
extern "C" fn ax_string_for_range(view: &Object, _selector: Sel, range: NSRange) -> *mut Object {
    with_snapshot(view, |snapshot| {
        snapshot
            .string_for_range(TextRange {
                location: range.location,
                length: range.length,
            })
            .map_or(std::ptr::null_mut(), ax_string)
    })
}
extern "C" fn ax_line_for_index(view: &Object, _selector: Sel, index: isize) -> isize {
    with_snapshot(view, |snapshot| {
        usize::try_from(index)
            .ok()
            .and_then(|index| snapshot.line_for_index(index))
            .map_or(isize::MAX, |line| line as isize)
    })
}
extern "C" fn ax_range_for_line(view: &Object, _selector: Sel, line: isize) -> NSRange {
    with_snapshot(view, |snapshot| {
        native_range(
            usize::try_from(line)
                .ok()
                .and_then(|line| snapshot.range_for_line(line)),
        )
    })
}
extern "C" fn ax_attribute_settable(
    _view: &Object,
    _selector: Sel,
    _attribute: *mut Object,
) -> BOOL {
    NO
}
// Explicitly deny inherited editing/actions as well as unknown AX selectors.
// This does not affect winit's NSTextInputClient/IME selector dispatch.
extern "C" fn ax_selector_allowed(_view: &Object, _selector: Sel, selector: Sel) -> BOOL {
    let allowed = [
        sel!(isAccessibilityElement),
        sel!(accessibilityRole),
        sel!(accessibilityLabel),
        sel!(accessibilityValue),
        sel!(accessibilityNumberOfCharacters),
        sel!(accessibilityVisibleCharacterRange),
        sel!(accessibilitySelectedTextRange),
        sel!(accessibilitySelectedText),
        sel!(accessibilityStringForRange:),
        sel!(accessibilityLineForIndex:),
        sel!(accessibilityRangeForLine:),
        sel!(accessibilityFrame),
        sel!(accessibilityParent),
        sel!(accessibilityWindow),
        sel!(accessibilityTopLevelUIElement),
        sel!(accessibilityActionNames),
    ];
    if allowed.contains(&selector) {
        YES
    } else {
        NO
    }
}
extern "C" fn ax_action_names(view: &Object, _selector: Sel) -> *mut Object {
    record_accessibility_request(view);
    unsafe { msg_send![class!(NSArray), array] }
}

unsafe fn register_accessibility(declaration: &mut ClassDecl) {
    declaration.add_method(
        sel!(isAccessibilityElement),
        ax_element as extern "C" fn(&Object, Sel) -> BOOL,
    );
    for (selector, getter) in [
        (
            sel!(accessibilityRole),
            ax_role as extern "C" fn(&Object, Sel) -> *mut Object,
        ),
        (sel!(accessibilityLabel), ax_label),
        (sel!(accessibilityValue), ax_value),
        (sel!(accessibilitySelectedText), ax_selected_text),
        (sel!(accessibilityActionNames), ax_action_names),
    ] {
        declaration.add_method(selector, getter);
    }
    declaration.add_method(
        sel!(accessibilityNumberOfCharacters),
        ax_number_of_characters as extern "C" fn(&Object, Sel) -> isize,
    );
    for (selector, getter) in [
        (
            sel!(accessibilityVisibleCharacterRange),
            ax_visible_range as extern "C" fn(&Object, Sel) -> NSRange,
        ),
        (sel!(accessibilitySelectedTextRange), ax_selected_range),
    ] {
        declaration.add_method(selector, getter);
    }
    declaration.add_method(
        sel!(accessibilityStringForRange:),
        ax_string_for_range as extern "C" fn(&Object, Sel, NSRange) -> *mut Object,
    );
    declaration.add_method(
        sel!(accessibilityLineForIndex:),
        ax_line_for_index as extern "C" fn(&Object, Sel, isize) -> isize,
    );
    declaration.add_method(
        sel!(accessibilityRangeForLine:),
        ax_range_for_line as extern "C" fn(&Object, Sel, isize) -> NSRange,
    );
    declaration.add_method(
        sel!(accessibilityIsAttributeSettable:),
        ax_attribute_settable as extern "C" fn(&Object, Sel, *mut Object) -> BOOL,
    );
    declaration.add_method(
        sel!(isAccessibilitySelectorAllowed:),
        ax_selector_allowed as extern "C" fn(&Object, Sel, Sel) -> BOOL,
    );
}

// One notification per changed publication; synchronous reentrant publications
// fold into the in-flight notification and leave the newest cache readable.
// In particular, no RefCell guard survives into notify()/AppKit.
fn publish_cached_accessibility(
    cache: &RefCell<AccessibilitySnapshot>,
    notifying: &Cell<bool>,
    snapshot: AccessibilitySnapshot,
    notify: impl FnOnce(),
) {
    {
        let mut cached = cache.borrow_mut();
        if *cached == snapshot {
            return;
        }
        *cached = snapshot;
    }
    if notifying.replace(true) {
        return;
    }
    struct NotificationGuard<'a>(&'a Cell<bool>);
    impl Drop for NotificationGuard<'_> {
        fn drop(&mut self) {
            self.0.set(false);
        }
    }
    let _guard = NotificationGuard(notifying);
    notify();
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
    /// Publish on the AppKit main thread after the parent has resolved scope,
    /// active displayed output, privacy, and snapshot-relative selection.
    pub(crate) fn publish_accessibility(&self, snapshot: AccessibilitySnapshot) {
        self._context.accessibility_needs_refresh.set(false);
        publish_cached_accessibility(
            &self._context.accessibility,
            &self._context.accessibility_notifying,
            snapshot,
            || unsafe {
                NSAccessibilityPostNotification(self.view, NSAccessibilityValueChangedNotification);
            },
        );
    }

    /// Allocation-free demand hint, NOT evidence that a client is connected or
    /// absent. Parent may skip snapshot construction when false; getters return
    /// the existing cache and refresh this hint for the next loop/frame. Publish
    /// and invalidate do not refresh it (a real getter during notification does).
    pub(crate) fn accessibility_requested(&self) -> bool {
        let requested = requested_recently(
            self._context.accessibility_last_request.get(),
            Instant::now(),
        );
        if !requested {
            expire_accessibility_cache(
                &self._context.accessibility,
                &self._context.accessibility_needs_refresh,
            );
        }
        requested
    }

    /// Clear stale output before tab/pane/modal/sensitivity transitions. Until
    /// a fresh safe snapshot is published, every getter remains redacted.
    pub(crate) fn invalidate_accessibility(&self) {
        let changed = self._context.accessibility.borrow().scope()
            != accessibility::AccessibilityScope::Redacted;
        self.publish_accessibility(AccessibilitySnapshot::default());
        self._context.accessibility_needs_refresh.set(true);
        // A scope event need not otherwise schedule a frame (e.g. native menu
        // actions). Avoid leaving an interested client stuck on the cleared
        // cache; never schedule repeatedly for an already-redacted snapshot.
        if changed
            && requested_recently(
                self._context.accessibility_last_request.get(),
                Instant::now(),
            )
        {
            if let Some(window) = self._context.accessibility_window.upgrade() {
                window.request_redraw();
            }
        }
    }

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
                register_accessibility(&mut declaration);
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
            accessibility: RefCell::new(AccessibilitySnapshot::default()),
            accessibility_last_request: Cell::new(None),
            accessibility_window: Arc::downgrade(&window),
            accessibility_notifying: Cell::new(false),
            accessibility_needs_refresh: Cell::new(true),
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
    fn accessibility_demand_expires_without_sleep_or_allocation() {
        let queried = Instant::now();
        assert!(!requested_recently(None, queried));
        assert!(requested_recently(Some(queried), queried));
        assert!(requested_recently(
            Some(queried),
            queried + Duration::from_millis(1999)
        ));
        assert!(!requested_recently(
            Some(queried),
            queried + ACCESSIBILITY_REQUEST_TTL
        ));
        assert!(!requested_recently(
            Some(queried),
            queried + Duration::from_secs(3)
        ));
        assert!(!requested_recently(
            Some(queried + Duration::from_secs(1)),
            queried
        ));
    }

    #[test]
    fn publication_coalesces_and_releases_borrow_before_notification() {
        use accessibility::Sensitivity;
        let cache = RefCell::new(AccessibilitySnapshot::default());
        let notifying = Cell::new(false);
        let calls = Cell::new(0);
        let demand = Cell::new(None::<Instant>);
        let snapshot = AccessibilitySnapshot::terminal_output("safe", Sensitivity::KnownSafe, None);
        publish_cached_accessibility(&cache, &notifying, snapshot.clone(), || {
            assert_eq!(cache.try_borrow().unwrap().text(), "safe");
            assert!(cache.try_borrow_mut().is_ok());
            calls.set(calls.get() + 1);
        });
        publish_cached_accessibility(&cache, &notifying, snapshot, || calls.set(calls.get() + 1));
        assert_eq!(calls.get(), 1);
        assert_eq!(demand.get(), None); // publication did not record a getter
        with_cached_snapshot(&cache, &demand, |snapshot| {
            assert_eq!(snapshot.text(), "safe")
        });
        let queried = demand.get();
        assert!(queried.is_some());
        // A contended cache fails closed but still records real getter demand.
        {
            let _borrow = cache.borrow_mut();
            with_cached_snapshot(&cache, &demand, |snapshot| assert_eq!(snapshot.text(), ""));
        }
        let queried = demand.get().or(queried);
        assert!(!notifying.get());
        // Even without demand, invalidation changes the cache and notifies.
        publish_cached_accessibility(&cache, &notifying, AccessibilitySnapshot::default(), || {
            assert_eq!(*cache.borrow(), AccessibilitySnapshot::default());
            calls.set(calls.get() + 1);
        });
        assert_eq!(calls.get(), 2);
        assert_eq!(demand.get(), queried); // invalidation is not a client query
    }

    #[test]
    fn expired_cache_is_redacted_even_when_clear_is_contended() {
        use accessibility::Sensitivity;
        let cache = RefCell::new(AccessibilitySnapshot::terminal_output(
            "synthetic safe",
            Sensitivity::KnownSafe,
            None,
        ));
        let needs_refresh = Cell::new(false);
        {
            let _borrow = cache.borrow();
            expire_accessibility_cache(&cache, &needs_refresh);
            assert!(needs_refresh.get()); // Native getters must not read this cache.
        }
        expire_accessibility_cache(&cache, &needs_refresh);
        assert_eq!(*cache.borrow(), AccessibilitySnapshot::default());
        assert!(needs_refresh.get());
    }

    #[test]
    fn reentrant_notification_preserves_latest_cache_without_recursing() {
        use accessibility::Sensitivity;
        let cache = RefCell::new(AccessibilitySnapshot::default());
        let notifying = Cell::new(false);
        let first = AccessibilitySnapshot::terminal_output("one", Sensitivity::KnownSafe, None);
        publish_cached_accessibility(&cache, &notifying, first, || {
            let second =
                AccessibilitySnapshot::terminal_output("two", Sensitivity::KnownSafe, None);
            publish_cached_accessibility(&cache, &notifying, second, || {
                panic!("recursive notification")
            });
            assert_eq!(cache.borrow().text(), "two");
        });
        assert!(!notifying.get());
        assert_eq!(cache.borrow().text(), "two");
    }

    #[test]
    fn accessibility_selectors_register_with_native_abis() {
        unsafe {
            let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
            let mut declaration =
                ClassDecl::new("VoltAXReadOnlyTestView", class!(NSObject)).unwrap();
            register_accessibility(&mut declaration);
            let class = declaration.register();
            let object: *mut Object = msg_send![class, new];
            let role: *mut Object = msg_send![object, accessibilityRole];
            assert_eq!(inserted_string(role).as_deref(), Some("AXStaticText"));
            let element: BOOL = msg_send![object, isAccessibilityElement];
            assert_eq!(element, YES);
            let visible: NSRange = msg_send![object, accessibilityVisibleCharacterRange];
            assert_eq!((visible.location, visible.length), (0, 0));
            let characters: isize = msg_send![object, accessibilityNumberOfCharacters];
            assert_eq!(characters, 0);
            let range: NSRange = msg_send![object, accessibilityRangeForLine: -1isize];
            assert_eq!(range.location, isize::MAX as usize);
            let substring: *mut Object =
                msg_send![object, accessibilityStringForRange: NSRange { location: 1, length: 0 }];
            assert!(substring.is_null());
            let editable: BOOL =
                msg_send![object, isAccessibilitySelectorAllowed: sel!(setAccessibilityValue:)];
            assert_eq!(editable, NO);
            let _: () = msg_send![object, release];
            let _: () = msg_send![pool, drain];
        }
    }

    #[test]
    fn accessibility_missing_context_is_redacted_and_read_only() {
        unsafe {
            let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
            let object: *mut Object = msg_send![class!(NSObject), new];
            let view = &*object;
            assert_eq!(
                ax_number_of_characters(view, sel!(accessibilityNumberOfCharacters)),
                0
            );
            let visible = ax_visible_range(view, sel!(accessibilityVisibleCharacterRange));
            assert_eq!((visible.location, visible.length), (0, 0));
            let selected = ax_selected_range(view, sel!(accessibilitySelectedTextRange));
            assert_eq!(selected.location, isize::MAX as usize);
            let value = ax_value(view, sel!(accessibilityValue));
            let length: usize = msg_send![value, length];
            assert_eq!(length, 0);
            assert!(ax_string_for_range(
                view,
                sel!(accessibilityStringForRange:),
                NSRange {
                    location: usize::MAX,
                    length: 1
                }
            )
            .is_null());
            assert_eq!(
                ax_line_for_index(view, sel!(accessibilityLineForIndex:), -1),
                isize::MAX
            );
            assert_eq!(
                ax_attribute_settable(view, sel!(accessibilityIsAttributeSettable:), value),
                NO
            );
            for selector in [
                sel!(setAccessibilityValue:),
                sel!(setAccessibilitySelectedText:),
                sel!(setAccessibilitySelectedTextRange:),
                sel!(accessibilityPerformPress),
            ] {
                assert_eq!(
                    ax_selector_allowed(view, sel!(isAccessibilitySelectorAllowed:), selector),
                    NO
                );
            }
            assert_eq!(
                ax_selector_allowed(
                    view,
                    sel!(isAccessibilitySelectorAllowed:),
                    sel!(accessibilityValue)
                ),
                YES
            );
            let _: () = msg_send![object, release];
            let _: () = msg_send![pool, drain];
        }
    }

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
