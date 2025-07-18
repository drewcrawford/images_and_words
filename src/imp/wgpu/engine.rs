// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use std::sync::Arc;

#[derive(Debug)]
pub struct Engine;
impl Engine {
    pub async fn rendering_to_view(_bound_device: &Arc<crate::images::BoundDevice>) -> Self {
        logwise::info_sync!("wgpu::Engine::rendering_to_view() started");
        //do we actually need to do anything?
        logwise::info_sync!("wgpu::Engine::rendering_to_view() completed");
        Engine
    }
}
