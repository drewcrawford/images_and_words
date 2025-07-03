use app_window::wgpu::{wgpu_begin_context, wgpu_in_context, wgpu_smuggle};
use r#continue::continuation;

// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
#[derive(Debug)]
pub struct View {
    //to meet our thread requirements, we may need to send this to another thread
    //as part of its creation, or to bind it to a device
    pub(super) surface: Option<wgpu::Surface<'static>>,
}

impl View {
    /**
    Creates a new view for the given entry point.

    This view will not be usable until it is bound to a device.

    # Safety
    This function is unsafe because the underlying surface is created with a raw window handle.
    If the raw window handle is not valid or the display handle is incorrect, it may lead to undefined behavior.

    # Threading
    Typically, this function should be called from [app_window::wgpu::wgpu_in_context].
    */
    #[cfg(feature = "app_window")]
    pub async unsafe fn from_surface(
        entrypoint: &crate::entry_point::EntryPoint,
        raw_window_handle: send_cells::UnsafeSendCell<wgpu::rwh::RawWindowHandle>,
        raw_display_handle: send_cells::UnsafeSendCell<wgpu::rwh::RawDisplayHandle>,
    ) -> Result<Self, super::Error> {
        let entrypoint = entrypoint.clone();

        let surface = wgpu_smuggle(|| {
            async move {
                let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                    //safety: see function documentation
                    raw_window_handle: unsafe { *raw_window_handle.get() },
                    raw_display_handle: unsafe { *raw_display_handle.get() },
                };
                entrypoint.0.0.create_surface_unsafe(target)
            }
        })
        .await
        .expect("Can't create surface");

        Ok(View {
            surface: Some(surface).into(),
        })
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn for_testing() -> Self {
        View { surface: None }
    }
}
