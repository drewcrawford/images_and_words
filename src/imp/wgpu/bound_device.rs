// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::imp::Error;
use app_window::wgpu::WgpuCell;
use std::sync::{Arc, Mutex};
use wgpu::{Limits, Trace};

#[derive(Debug)]
pub(super) struct Wgpu {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) adapter: wgpu::Adapter,
}

#[derive(Debug)]
pub struct BoundDevice {
    pub(crate) wgpu: Mutex<WgpuCell<Wgpu>>,
}

impl BoundDevice {
    pub(crate) async fn bind(
        unbound_device: crate::images::device::UnboundDevice,
        _entry_point: Arc<crate::entry_point::EntryPoint>,
    ) -> Result<Self, Error> {
        let wgpu = WgpuCell::new_on_thread(|| {
            async move {
                let label = wgpu::Label::from("Bound Device");
                let descriptor = wgpu::DeviceDescriptor {
                    label,
                    required_features: Default::default(),
                    //todo: choose better limits?
                    required_limits: Limits::downlevel_webgl2_defaults(),
                    memory_hints: Default::default(),
                    trace: Trace::Off,
                };
                let (device, q) = unbound_device
                    .0
                    .adapter
                    .request_device(&descriptor)
                    .await
                    .expect("Failed to create device");

                Wgpu {
                    device,
                    queue: q,
                    adapter: unbound_device.0.adapter.get().clone(),
                }
            }
        })
        .await;
        Ok(BoundDevice { wgpu: wgpu.into() })
    }
}
