// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
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

#[derive(Debug)]
pub struct Port {
    internal: Option<PortInternal>,
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
            internal: Some(internal),
        })
    }

    pub async fn add_fixed_pass(&mut self, descriptor: PassDescriptor) {
        self.internal
            .as_mut()
            .expect("Port internal missing")
            .add_fixed_pass(descriptor)
            .await;
    }

    pub async fn render_frame(&mut self) {
        //logwise::info_sync!("Rendering frame...");
        let internal = self.internal.take().expect("Port internal missing");
        let internal = smuggle_async("render_frame".to_string(), || async move {
            internal.render_frame().await
        })
        .await;
        self.internal = Some(internal);
    }
}
