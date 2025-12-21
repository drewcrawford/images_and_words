// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
#[cfg(feature = "exfiltrate")]
mod debug_capture;
pub mod guards;
pub mod internal;
pub mod prepared_pass;
pub mod types;

use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::PassDescriptor;
use crate::imp::Error;
use crate::imp::wgpu::context::smuggle_async;
use internal::PortInternal;
use std::sync::Arc;
use wasm_safe_mutex::Mutex;

#[derive(Debug)]
pub struct Port {
    /// Internal port state wrapped in a Mutex for interior mutability.
    /// This allows Port methods to take `&self` while still being able to
    /// move ownership of PortInternal into async blocks.
    internal: Mutex<Option<PortInternal>>,
}

impl Port {
    pub(crate) async fn new(
        engine: &Arc<crate::images::Engine>,
        view: crate::images::view::View,
        camera: Camera,
        port_reporter_send: PortReporterSend,
    ) -> Result<Self, Error> {
        let internal = PortInternal::new(engine, view, camera, port_reporter_send).await?;
        Ok(Port {
            internal: Mutex::new(Some(internal)),
        })
    }

    pub async fn add_fixed_pass(&self, descriptor: PassDescriptor) {
        let mut guard = self.internal.lock_async().await;
        let internal = (*guard).as_mut().expect("Port internal missing");
        internal.add_fixed_pass(descriptor).await;
    }

    pub async fn render_frame(&self) {
        //logwise::info_sync!("Rendering frame...");
        let internal = self
            .internal
            .lock_async()
            .await
            .take()
            .expect("Port internal missing");
        let internal = smuggle_async("render_frame".to_string(), || async move {
            internal.render_frame().await
        })
        .await;
        *self.internal.lock_async().await = Some(internal);
    }
}
