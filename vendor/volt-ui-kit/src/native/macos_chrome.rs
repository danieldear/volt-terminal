//! Narrow public AppKit chrome integration. No raw-pointer dereference, private
//! selectors, view reparenting, button resizing or replacement of winit's delegate.
use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSTitlebarSeparatorStyle, NSToolbar, NSWindow, NSWindowButton,
    NSWindowStyleMask, NSWindowToolbarStyle,
};
use objc2_foundation::NSString;
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

/// Called once per window creation, on winit's main-thread resumed lifecycle.
/// NSWindow retains its toolbar; AppKit owns layout during resize/fullscreen.
pub(super) fn install_unified_titlebar(window: &Window) -> Result<(), String> {
    if !objc2::available!(macos = 11.0) {
        eprintln!("Volt UI: unified overlay requires macOS 11; retaining compact native controls");
        return Ok(());
    }
    let mtm = MainThreadMarker::new().ok_or("AppKit chrome requires the main thread")?;
    let ns_window = app_window(window, mtm)?;
    if ns_window.toolbar().is_some() {
        return Err("unified overlay cannot replace an existing native toolbar".into());
    }
    let toolbar = NSToolbar::initWithIdentifier(
        NSToolbar::alloc(mtm),
        &NSString::from_str("dev.volt-ui.overlay"),
    );
    toolbar.setVisible(true);
    ns_window.setToolbarStyle(NSWindowToolbarStyle::Unified);
    ns_window.setToolbar(Some(&toolbar));
    ns_window.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);
    // Preserve full-size content and a transparent native chrome surface. The
    // toolkit still paints the header; only system window geometry changes.
    ns_window.setTitlebarAppearsTransparent(true);
    Ok(())
}

/// In fullscreen AppKit moves a visible toolbar into its own opaque titlebar
/// window pinned over the content, hiding the toolkit-painted header. The
/// overlay toolbar is empty, so hide it while fullscreen (the titlebar then
/// auto-hides and reveals on hover) and show it again in a normal window.
pub(super) fn sync_fullscreen_toolbar(window: &Window) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Ok(ns_window) = app_window(window, mtm) else {
        return;
    };
    let Some(toolbar) = ns_window.toolbar() else {
        return;
    };
    let fullscreen = ns_window
        .styleMask()
        .contains(NSWindowStyleMask::FullScreen);
    if toolbar.isVisible() == fullscreen {
        toolbar.setVisible(!fullscreen);
    }
}

pub(super) fn app_window(
    window: &Window,
    mtm: MainThreadMarker,
) -> Result<Retained<NSWindow>, String> {
    let handle = window.window_handle().map_err(|e| e.to_string())?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return Err("expected AppKit window handle".into());
    };
    // Obtain retained AppKit objects through its safe API, then compare identity
    // with winit's borrowed NSView handle. Never dereference or take ownership of it.
    let app = NSApplication::sharedApplication(mtm);
    app.windows()
        .iter()
        .find(|candidate| {
            candidate.contentView().is_some_and(|view| {
                std::ptr::from_ref(&*view).cast::<std::ffi::c_void>() == handle.ns_view.as_ptr()
            })
        })
        .ok_or_else(|| "winit content view was not found in AppKit windows".into())
}

/// Read-only diagnostics after a frame, only when VOLT_UI_TRACE is enabled.
pub(super) fn trace_geometry(window: &Window) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Ok(ns_window) = app_window(window, mtm) else {
        return;
    };
    // AppKit moves fullscreen controls into its own transient titlebar window.
    // Coordinates there must not be reported relative to the content window.
    if ns_window
        .styleMask()
        .contains(NSWindowStyleMask::FullScreen)
    {
        eprintln!("VOLT_UI_CHROME fullscreen=true geometry=system-managed");
        return;
    }
    for (label, kind) in [
        ("close", NSWindowButton::CloseButton),
        ("minimize", NSWindowButton::MiniaturizeButton),
        ("zoom", NSWindowButton::ZoomButton),
    ] {
        if let Some(button) = ns_window.standardWindowButton(kind) {
            let bounds = button.bounds();
            let rect = button.convertRect_toView(bounds, None);
            let top = ns_window.frame().size.height - rect.origin.y - rect.size.height;
            eprintln!("VOLT_UI_CHROME button={label} left={:.1} top={top:.1} size={:.1}x{:.1} bounds={:.1}x{:.1}",
                rect.origin.x, rect.size.width, rect.size.height, bounds.size.width, bounds.size.height);
        }
    }
}
