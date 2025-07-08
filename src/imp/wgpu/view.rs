use crate::images::view::ViewForImp;
use crate::imp::wgpu::cell::WgpuCell;
use raw_window_handle::HasWindowHandle;
use std::sync::Arc;

// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
#[derive(Debug)]
pub struct View {
    //need surface to be dropped first here
    pub(super) surface: Option<WgpuCell<wgpu::Surface<'static>>>,
    pub(super) _parent: Arc<ViewForImp>,
}

impl View {
    /**
    Creates a new view for the given entry point.
    */
    pub async fn from_surface(
        entrypoint: &crate::entry_point::EntryPoint,
        view: ViewForImp,
    ) -> Result<Self, super::Error> {
        let entrypoint = entrypoint.clone();
        let view_clone = Arc::new(view);
        let view_clone2 = view_clone.clone();
        match view_clone.window_handle() {
            Ok(_e) => {}
            Err(_not_supported_error) => {
                return Ok(View {
                    surface: None,
                    _parent: view_clone2,
                });
            }
        }
        let wgpu_surface = entrypoint.0.0.assume(|entrypoint| {
            WgpuCell::new(
                entrypoint
                    .create_surface(view_clone)
                    .expect("Failed to create surface"),
            )
        });

        Ok(View {
            surface: Some(wgpu_surface),
            _parent: view_clone2,
        })
    }
}
