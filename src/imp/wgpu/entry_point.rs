// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::imp::wgpu::cell::WgpuCell;

#[derive(Debug, Clone)]
pub struct EntryPoint(pub(super) WgpuCell<wgpu::Instance>);
impl EntryPoint {
    pub async fn new() -> Result<Self, crate::imp::wgpu::Error> {
        let cell = WgpuCell::new_on_thread(move || async move {
            logwise::info_sync!("Hello from wgpu entry point!");
            let descriptor = wgpu::InstanceDescriptor::from_env_or_default();
            let c = wgpu::Instance::new(&descriptor);
            c
        })
        .await;
        Ok(EntryPoint(cell))
    }
}
