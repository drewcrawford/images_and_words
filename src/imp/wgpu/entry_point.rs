// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::imp::wgpu::cell::WgpuCell;

#[derive(Debug, Clone)]
pub struct EntryPoint(pub(super) WgpuCell<wgpu::Instance>);
impl EntryPoint {
    pub async fn new() -> Result<Self, crate::imp::wgpu::Error> {
        logwise::info_sync!("wgpu::EntryPoint::new() started");
        logwise::info_sync!("About to create WgpuCell on thread...");
        let cell = WgpuCell::new_on_thread(move || async move {
            logwise::info_sync!("Hello from wgpu entry point!");
            logwise::info_sync!("Creating wgpu::InstanceDescriptor...");
            let descriptor = wgpu::InstanceDescriptor::from_env_or_default();
            logwise::info_sync!("Creating wgpu::Instance...");
            let c = wgpu::Instance::new(&descriptor);
            logwise::info_sync!("wgpu::Instance created successfully");
            c
        })
        .await;
        logwise::info_sync!("WgpuCell created successfully");
        Ok(EntryPoint(cell))
    }
}
