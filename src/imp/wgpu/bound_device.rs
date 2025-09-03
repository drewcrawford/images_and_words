// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::imp::Error;
use crate::imp::wgpu::cell::WgpuCell;
use crate::imp::wgpu::context::smuggle_async;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{self, Receiver, Sender};
#[cfg(not(target_arch = "wasm32"))]
use std::thread::{self, JoinHandle};
use wgpu::{Limits, PollType, Trace};

/// Internal resource management for BoundDevice
/// This type owns the actual GPU resources and handles cleanup
#[derive(Debug)]
struct BoundDeviceResources {
    pub(super) device: WgpuCell<wgpu::Device>,
    pub(super) queue: WgpuCell<wgpu::Queue>,
    pub(super) adapter: WgpuCell<wgpu::Adapter>,
    #[cfg(not(target_arch = "wasm32"))]
    poll_thread: Option<JoinHandle<()>>,
    #[cfg(not(target_arch = "wasm32"))]
    poll_shutdown: Arc<AtomicBool>,
    #[cfg(not(target_arch = "wasm32"))]
    poll_trigger: Sender<()>,
}

/// Cross-platform bound device that can be safely cloned
/// Multiple instances share the same underlying GPU resources
#[derive(Debug, Clone)]
pub struct BoundDevice {
    resources: Arc<BoundDeviceResources>,
}

impl BoundDevice {
    pub(crate) async fn bind(
        unbound_device: crate::images::device::UnboundDevice,
        _entry_point: Arc<crate::entry_point::EntryPoint>,
    ) -> Result<Self, Error> {
        let move_adapter = unbound_device.0.adapter.clone();
        let (device, queue) = smuggle_async("create device".to_string(), || async move {
            let label = wgpu::Label::from("Bound Device");
            let descriptor = wgpu::DeviceDescriptor {
                label,
                required_features: Default::default(),
                //todo: choose better limits?
                required_limits: Limits::downlevel_webgl2_defaults(),
                memory_hints: Default::default(),
                trace: Trace::Off,
            };
            let (device, queue) = move_adapter
                .assume_async(|a: &wgpu::Adapter| {
                    let a_clone = a.clone();
                    async move {
                        a_clone
                            .request_device(&descriptor)
                            .await
                            .expect("failed to create device")
                    }
                })
                .await;
            (WgpuCell::new(device), WgpuCell::new(queue))
        })
        .await;
        #[cfg(not(target_arch = "wasm32"))]
        {
            //on non-wasm platforms we should be able to clone out of the cell directly
            let jailbreak_device = device.with(|wgpu| wgpu.clone()).await;
            let poll_shutdown = Arc::new(AtomicBool::new(false));
            let shutdown_clone = poll_shutdown.clone();

            let (poll_sender, poll_receiver): (Sender<()>, Receiver<()>) = mpsc::channel();

            let poll_thread = thread::Builder::new()
                .name("wgpu_poll".to_string())
                .spawn(move || {
                    while !shutdown_clone.load(Ordering::Relaxed) {
                        // Wait for a signal that polling is needed
                        match poll_receiver.recv() {
                            Ok(_) => {
                                // Poll until the queue is empty
                                let _ = jailbreak_device.poll(PollType::Wait);
                            }
                            Err(_) => break, // Channel closed, exit thread
                        }
                    }
                })
                .expect("Failed to spawn wgpu polling thread");
            let resources = BoundDeviceResources {
                device,
                queue,
                adapter: unbound_device.0.adapter,
                poll_thread: Some(poll_thread),
                poll_shutdown,
                poll_trigger: poll_sender,
            };
            Ok(BoundDevice {
                resources: Arc::new(resources),
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            // On wasm32, we don't need a separate polling thread
            let resources = BoundDeviceResources {
                device,
                queue,
                adapter: unbound_device.0.adapter,
            };
            Ok(BoundDevice {
                resources: Arc::new(resources),
            })
        }
    }

    /// Signal the polling thread that GPU work may be ready
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_needs_poll(&self) {
        // Send a signal to the polling thread (ignore if channel is full/closed)
        let _ = self.resources.poll_trigger.send(());
    }

    /// No-op on wasm32 where polling is not needed
    #[cfg(target_arch = "wasm32")]
    pub fn set_needs_poll(&self) {
        // On wasm32, polling is handled automatically
    }

    /// Access to the wgpu device
    pub(super) fn device(&self) -> &WgpuCell<wgpu::Device> {
        &self.resources.device
    }

    /// Access to the wgpu queue
    pub(super) fn queue(&self) -> &WgpuCell<wgpu::Queue> {
        &self.resources.queue
    }

    /// Access to the wgpu adapter
    pub(super) fn adapter(&self) -> &WgpuCell<wgpu::Adapter> {
        &self.resources.adapter
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for BoundDeviceResources {
    fn drop(&mut self) {
        // Signal the polling thread to shut down
        self.poll_shutdown.store(true, Ordering::Relaxed);

        // Wait for the polling thread to finish
        if let Some(handle) = self.poll_thread.take() {
            let _ = handle.join();
        }
    }
}
