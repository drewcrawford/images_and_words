// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::imp::wgpu::cell::WgpuCell;

#[derive(Debug, Clone)]
pub struct EntryPoint {
    pub(super) instance: WgpuCell<wgpu::Instance>,
    #[allow(dead_code)]
    is_webgpu: bool,
}
impl EntryPoint {
    pub async fn new() -> Result<Self, crate::imp::wgpu::Error> {
        // logwise::info_sync!("wgpu::EntryPoint::new() started");
        // logwise::info_sync!("About to create WgpuCell on thread...");
        let status = wgpu::util::is_browser_webgpu_supported().await;
        let cell = WgpuCell::new_on_thread(move || async move {
            // logwise::info_sync!("Hello from wgpu entry point!");
            // logwise::info_sync!("Creating wgpu::InstanceDescriptor...");
            let mut descriptor = wgpu::InstanceDescriptor::from_env_or_default();
            logwise::debuginternal_sync!(
                "Default descriptor {descriptor}",
                descriptor = logwise::privacy::LogIt(&descriptor)
            );
            logwise::debuginternal_sync!("WebGPU status, {status}", status = status);
            if !status {
                descriptor.backends.remove(wgpu::Backends::BROWSER_WEBGPU);
                descriptor.backends.insert(wgpu::Backends::GL);
            }
            logwise::debuginternal_sync!(
                "Using descriptor {descriptor}",
                descriptor = logwise::privacy::LogIt(&descriptor)
            );

            let instance = wgpu::Instance::new(&descriptor);
            logwise::debuginternal_sync!(
                "Created instance {instance}",
                instance = logwise::privacy::LogIt(&instance)
            );
            instance
        })
        .await;
        // logwise::info_sync!("WgpuCell created successfully");
        Ok(EntryPoint {
            instance: cell,
            is_webgpu: status,
        })
    }
    #[allow(dead_code)]
    pub fn is_webgpu(&self) -> bool {
        self.is_webgpu
    }
}
