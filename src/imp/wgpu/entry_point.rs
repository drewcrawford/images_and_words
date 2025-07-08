// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::imp::wgpu::cell::WgpuCell;

#[derive(Debug, Clone)]
pub struct EntryPoint(pub(super) WgpuCell<wgpu::Instance>);
impl EntryPoint {
    pub async fn new() -> Result<Self, crate::imp::wgpu::Error> {
        let cell = WgpuCell::new_on_thread(move || async move {
            let descriptor = wgpu::InstanceDescriptor::from_env_or_default();
            wgpu::Instance::new(&descriptor)
        })
        .await;
        Ok(EntryPoint(cell))
    }
}
