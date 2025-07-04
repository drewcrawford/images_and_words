use crate::entry_point::EntryPoint;
use crate::images::view::ViewForImp;
use crate::imp::wgpu::context::smuggle;
use r#continue::continuation;
use std::sync::Arc;

// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
#[derive(Debug)]
pub struct View {
    //need surface to be dropped first here
    pub(super) surface: Option<wgpu::Surface<'static>>,
    pub(super) parent: Arc<ViewForImp>,
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
        let wgpu_surface = smuggle("create_surface".to_string(), move || {
            entrypoint.0.0.create_surface(view_clone)
        })
        .await?;

        Ok(View {
            surface: Some(wgpu_surface),
            parent: view_clone2,
        })
    }

    pub async fn provide_entry_point(
        &mut self,
        entry_point: &EntryPoint,
    ) -> Result<(), crate::imp::Error> {
        todo!()
    }
}
