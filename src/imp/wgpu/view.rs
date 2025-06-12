use r#continue::continuation;
use std::pin::Pin;

// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
#[derive(Debug)]
pub struct View {
    pub(super) surface: Option<wgpu::Surface<'static>>,
}

//this solution may be more portable than app_window's?

//main thread platforms
#[cfg(target_os="macos")]
async fn wgpu_exec<F, R>(f: F) -> R
where
    R: 'static,
    F: Future<Output = R>,
{
    let (sender, fut) = continuation();
    // Box the future, erase the lifetime
    let f = Box::pin(f) as Pin<Box<dyn Future<Output = F::Output> + '_>>;
    // Cast the lifetime to 'static
    // SAFETY: this is safe because we refuse to return until the future is complete,
    let f_static = unsafe {
        std::mem::transmute::<
            Pin<Box<dyn Future<Output = F::Output> + '_>>,
            Pin<Box<dyn Future<Output = F::Output> + 'static>>,
        >(f)
    };
    app_window::wgpu::wgpu_spawn(async {
        let r = f_static.await;
        sender.send(r);
    });
    fut.await
}

//non-main-thread platforms
#[cfg(not(target_os = "macos"))]
async fn wgpu_exec<F, R>(f: F) -> R
where
    R: 'static,
    F: Future<Output = R>,
{
    f.await
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
        wgpu_exec(async {
            let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_window_handle,
                raw_display_handle,
            };
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
