/// macOS CoreVideo `CVDisplayLink` integration.
///
/// Drives redraws at the display's native refresh rate instead of relying
/// purely on winit's timer.  On non-macOS targets this module is empty.
#[cfg(target_os = "macos")]
mod inner {
    use std::ffi::c_void;
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use winit::event_loop::EventLoopProxy;

    use crate::app::VoltEvent;

    type CVDisplayLinkRef = *mut c_void;
    type CVOptionFlags = u64;
    type CVReturn = i32;
    type CVTimeStamp = c_void;
    type CVDisplayLinkOutputCallback = Option<
        extern "C" fn(
            CVDisplayLinkRef,
            *const CVTimeStamp,
            *const CVTimeStamp,
            CVOptionFlags,
            *mut CVOptionFlags,
            *mut c_void,
        ) -> CVReturn,
    >;

    #[link(name = "CoreVideo", kind = "framework")]
    unsafe extern "C" {
        fn CVDisplayLinkCreateWithActiveCGDisplays(
            displayLinkOut: *mut CVDisplayLinkRef,
        ) -> CVReturn;
        fn CVDisplayLinkSetOutputCallback(
            displayLink: CVDisplayLinkRef,
            callback: CVDisplayLinkOutputCallback,
            userInfo: *mut c_void,
        ) -> CVReturn;
        fn CVDisplayLinkStart(displayLink: CVDisplayLinkRef) -> CVReturn;
        fn CVDisplayLinkStop(displayLink: CVDisplayLinkRef) -> CVReturn;
        fn CVDisplayLinkRelease(displayLink: CVDisplayLinkRef);
    }

    struct DisplayLinkContext {
        pending: Arc<AtomicBool>,
        proxy: EventLoopProxy<VoltEvent>,
    }

    pub struct DisplayLinkScheduler {
        display_link: CVDisplayLinkRef,
        pending: Arc<AtomicBool>,
        context: *mut DisplayLinkContext,
    }

    extern "C" fn display_link_output_callback(
        _display_link: CVDisplayLinkRef,
        _in_now: *const CVTimeStamp,
        _in_output_time: *const CVTimeStamp,
        _flags_in: CVOptionFlags,
        _flags_out: *mut CVOptionFlags,
        user_info: *mut c_void,
    ) -> CVReturn {
        if user_info.is_null() {
            return 0;
        }
        let ctx = unsafe { &*(user_info as *mut DisplayLinkContext) };
        if ctx.pending.swap(false, Ordering::AcqRel) {
            if let Err(err) = ctx.proxy.send_event(VoltEvent::DisplayLinkTick) {
                eprintln!("volt-ui: failed to send DisplayLinkTick event: {err}");
            }
        }
        0
    }

    impl DisplayLinkScheduler {
        pub fn new(proxy: EventLoopProxy<VoltEvent>) -> anyhow::Result<Self> {
            let mut display_link: CVDisplayLinkRef = null_mut();
            let create_result =
                unsafe { CVDisplayLinkCreateWithActiveCGDisplays(&mut display_link) };
            if create_result != 0 || display_link.is_null() {
                return Err(anyhow::anyhow!(
                    "CVDisplayLinkCreateWithActiveCGDisplays failed with code {create_result}"
                ));
            }

            let pending = Arc::new(AtomicBool::new(false));
            let context = Box::new(DisplayLinkContext {
                pending: Arc::clone(&pending),
                proxy,
            });
            let context_ptr = Box::into_raw(context);

            let callback_result = unsafe {
                CVDisplayLinkSetOutputCallback(
                    display_link,
                    Some(display_link_output_callback),
                    context_ptr as *mut c_void,
                )
            };
            if callback_result != 0 {
                unsafe {
                    CVDisplayLinkRelease(display_link);
                    drop(Box::from_raw(context_ptr));
                }
                return Err(anyhow::anyhow!(
                    "CVDisplayLinkSetOutputCallback failed with code {callback_result}"
                ));
            }

            let start_result = unsafe { CVDisplayLinkStart(display_link) };
            if start_result != 0 {
                unsafe {
                    CVDisplayLinkRelease(display_link);
                    drop(Box::from_raw(context_ptr));
                }
                return Err(anyhow::anyhow!(
                    "CVDisplayLinkStart failed with code {start_result}"
                ));
            }

            Ok(Self {
                display_link,
                pending,
                context: context_ptr,
            })
        }

        pub fn request_redraw(&self) {
            self.pending.store(true, Ordering::Release);
        }
    }

    impl Drop for DisplayLinkScheduler {
        fn drop(&mut self) {
            unsafe {
                if !self.display_link.is_null() {
                    let _ = CVDisplayLinkStop(self.display_link);
                    CVDisplayLinkRelease(self.display_link);
                }
                if !self.context.is_null() {
                    drop(Box::from_raw(self.context));
                }
            }
        }
    }

    // SAFETY: The display link callback is invoked from a CoreVideo thread, but
    // DisplayLinkContext is only accessed through the raw pointer which we own.
    unsafe impl Send for DisplayLinkScheduler {}
}

#[cfg(target_os = "macos")]
pub use inner::DisplayLinkScheduler;
