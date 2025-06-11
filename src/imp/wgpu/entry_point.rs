// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
#[derive(Debug)]
pub struct EntryPoint(pub(super) wgpu::Instance);
impl EntryPoint {
    pub async fn new() -> Result<Self, crate::imp::wgpu::Error> {
        let descriptor = wgpu::InstanceDescriptor::from_env_or_default();
        let wgpu_instance = wgpu::Instance::new(&descriptor);
        Ok(EntryPoint(wgpu_instance))
    }
}
