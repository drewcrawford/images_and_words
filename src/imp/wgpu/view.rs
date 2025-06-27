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
    */
    #[cfg(feature = "app_window")]
    pub async unsafe fn from_surface(
        entrypoint: &crate::entry_point::EntryPoint,
        raw_window_handle: wgpu::rwh::RawWindowHandle,
        raw_display_handle: wgpu::rwh::RawDisplayHandle,
    ) -> Result<Self, super::Error> {
        let move_handles = send_cells::unsafe_send_cell::UnsafeSendCell::new((
            raw_window_handle,
            raw_display_handle,
        ));
        let entrypoint = entrypoint.clone();
        app_window::wgpu::wgpu_call_context_relaxed(async move {
            let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                //safety: see function documentation
                raw_window_handle: unsafe { move_handles.get().0 },
                raw_display_handle: unsafe { move_handles.get().1 },
            };
            //safety: see function documentation
            let surface = unsafe { entrypoint.0.0.create_surface_unsafe(target)? };
            Ok(View {
                surface: Some(surface).into(),
            })
        })
        .await
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn for_testing() -> Self {
        View { surface: None }
    }
}
