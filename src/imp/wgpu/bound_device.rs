use crate::imp::Error;
use std::sync::Arc;
use wgpu::{Limits, Trace};

#[derive(Debug)]
pub struct BoundDevice {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
}

impl BoundDevice {
    pub(crate) async fn bind(
        unbound_device: crate::images::device::UnboundDevice,
        _entry_point: Arc<crate::entry_point::EntryPoint>,
    ) -> Result<Self, Error> {
        let label = wgpu::Label::from("Bound Device");
        let descriptor = wgpu::DeviceDescriptor {
            label,
            required_features: Default::default(),
            //todo: choose better limits?
            required_limits: Limits::downlevel_webgl2_defaults(),
            memory_hints: Default::default(),
            trace: Trace::Off,
        };
        let (device, q) = unbound_device.0.adapter.request_device(&descriptor).await?;

        Ok(BoundDevice { device, queue: q })
    }
}
