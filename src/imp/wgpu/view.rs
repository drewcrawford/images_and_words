// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
#[derive(Debug)]
pub struct View {
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
        struct Move {
            raw_window_handle: wgpu::rwh::RawWindowHandle,
            raw_display_handle: wgpu::rwh::RawDisplayHandle,
            entrypoint: *const crate::entry_point::EntryPoint,
        }

        let move_me = send_cells::unsafe_send_cell::UnsafeSendCell::new(Move {
            raw_window_handle,
            raw_display_handle,
            entrypoint: entrypoint as *const _,
        });
        app_window::application::on_main_thread(move || {
            let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_window_handle: unsafe { move_me.get() }.raw_window_handle,
                raw_display_handle: unsafe { move_me.get() }.raw_display_handle,
            };
            let entrypoint_ptr = unsafe { move_me.get() }.entrypoint;
            let entrypoint = unsafe { &*entrypoint_ptr };
            let surface = unsafe { entrypoint.0.0.create_surface_unsafe(target)? };
            Ok(View {
                surface: Some(surface),
            })
        })
        .await
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn for_testing() -> Self {
        View { surface: None }
    }
}
