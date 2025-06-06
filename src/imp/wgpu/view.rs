#[derive(Debug)]
pub struct View {
    pub(super) surface: Option<wgpu::Surface<'static>>,
}

impl View {
    #[cfg(feature = "app_window")]
    pub async fn from_surface(
        entrypoint: &crate::entry_point::EntryPoint,
        raw_window_handle: wgpu::rwh::RawWindowHandle,
        raw_display_handle: wgpu::rwh::RawDisplayHandle,
    ) -> Result<Self, super::Error> {
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_window_handle,
            raw_display_handle,
        };
        //not really ok but ignore me!
        let surface = unsafe { entrypoint.0.0.create_surface_unsafe(target)? };
        Ok(View {
            surface: Some(surface),
        })
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn for_testing() -> Self {
        View { surface: None }
    }
}
