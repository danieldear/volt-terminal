//! Public, main-thread AppKit window material. The winit content view, Metal
//! layer, native buttons and delegate keep their existing ownership and parent.
use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowButton, NSWindowOrderingMode, NSWorkspace,
};
use winit::window::Window;

pub(super) struct WindowMaterial {
    view: Retained<NSVisualEffectView>,
}

impl WindowMaterial {
    pub fn install(window: &Window) -> Result<Self, String> {
        if !objc2::available!(macos = 10.14) {
            return Err("public window material requires macOS 10.14".into());
        }
        let mtm = MainThreadMarker::new().ok_or("AppKit material requires the main thread")?;
        let ns_window = super::macos_chrome::app_window(window, mtm)?;
        let content = ns_window
            .contentView()
            .ok_or("missing winit content view")?;
        // superview() is unsafe in objc2 (a weak relationship); do not weaken
        // the toolkit's forbid(unsafe_code) contract to use it. This public
        // retained ancestor query works for ordinary decorated winit windows.
        let button = ns_window
            .standardWindowButton(NSWindowButton::CloseButton)
            .ok_or("public window material currently requires decorated window chrome")?;
        let parent = content
            .ancestorSharedWithView(&button)
            .ok_or("missing shared native window frame view")?;
        if !parent
            .subviews()
            .iter()
            .any(|view| std::ptr::eq(&*view, &*content))
        {
            return Err("native content is not a direct child of the shared frame view".into());
        }
        let view =
            NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), content.frame());
        view.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
        view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        view.setState(NSVisualEffectState::FollowsWindowActiveState);
        view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        // A sibling below the winit view, never a replacement content view or
        // a child above its CAMetalLayer. AppKit manages resize and fullscreen.
        parent.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Below, Some(&content));
        ns_window.setOpaque(false);
        let reduced = NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceTransparency();
        eprintln!("Volt UI: public AppKit window material active (reduced_transparency={reduced}); AppKit owns material appearance");
        Ok(Self { view })
    }

    /// Keep the frame-view hierarchy stable when switching blur backends.
    /// A hidden effect view contributes no material tint/blur; it remains
    /// attached so returning to Material does not tear down/rebuild the
    /// native window underlay (and its associated frame geometry).
    pub fn set_enabled(&self, enabled: bool) {
        if self.view.isHidden() != enabled {
            return;
        }
        self.view.setHidden(!enabled);
        if let Some(window) = self.view.window() {
            window.setViewsNeedDisplay(true);
            window.invalidateShadow();
        }
        if std::env::var_os("VOLT_UI_TRACE").is_some() {
            eprintln!("VOLT_UI_BACKDROP material_attached=true material_visible={enabled}");
        }
    }
}

impl Drop for WindowMaterial {
    fn drop(&mut self) {
        // The standalone host and this main-thread-only retained view are
        // destroyed on the UI thread, before dropping the window.
        let window = self.view.window();
        self.view.removeFromSuperview();
        if let Some(window) = window {
            // The window's backdrop hierarchy changed. Do not reuse cached
            // frame/shadow geometry from the material surface when a raw
            // compositor blur is enabled next. Schedule native refresh rather
            // than calling displayIfNeeded inside a winit event callback.
            window.setViewsNeedDisplay(true);
            window.invalidateShadow();
        }
    }
}

/// Read on the UI thread; raw compositor blur does not honor this automatically.
pub(super) fn reduced_transparency() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceTransparency()
}
