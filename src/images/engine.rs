// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::entry_point::{EntryPoint, EntryPointError};
use crate::images::device::BoundDevice;
use crate::images::device::{BindError, PickError, UnboundDevice};
use crate::images::port::Port;
use crate::images::projection::WorldCoord;
use crate::images::view::View;
use crate::imp;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct Engine {
    //note that drop order is significant here.
    ///engine's main rendering port.
    /// Wrapped in a Mutex so that we can mutate this overlapped with accessing device and entry_point.
    /// Note that we can't use RefCell because we want to be able to access this from multiple threads.
    main_port: Mutex<Option<Port>>,
    //device we bound to this engine.  Arc because it gets moved into the render_thread.
    device: Arc<BoundDevice>,
    _entry_point: Arc<EntryPoint>,
    _engine: crate::imp::Engine,
}

impl Engine {
    #[cfg(feature = "testing")]
    pub async fn for_testing() -> Result<Arc<Self>, CreateError> {
        Self::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await
    }
    pub async fn rendering_to(
        mut view: View,
        initial_camera_position: WorldCoord,
    ) -> Result<Arc<Self>, CreateError> {
        let entry_point = Arc::new(EntryPoint::new().await?);
        view.provide_entry_point(&entry_point)
            .await
            .expect("Can't provide entry point");

        let (initial_width, initial_height, initial_scale) = view.size_scale().await;

        let unbound_device = UnboundDevice::pick(&view, &entry_point).await?;
        let bound_device = Arc::new(BoundDevice::bind(unbound_device, entry_point.clone()).await?);
        let initial_port = Mutex::new(None);
        let imp = crate::imp::Engine::rendering_to_view(&bound_device).await;
        let r = Arc::new(Engine {
            main_port: initial_port,
            device: bound_device,
            _entry_point: entry_point,
            _engine: imp,
        });
        let final_port = Port::new(
            &r,
            view,
            initial_camera_position,
            (initial_width, initial_height, initial_scale),
        )
        .await
        .unwrap();
        r.main_port.lock().unwrap().replace(final_port);
        Ok(r)
    }
    pub fn main_port_mut(&self) -> PortGuard<'_> {
        PortGuard {
            guard: self.main_port.lock().unwrap(),
        }
    }
    pub fn bound_device(&self) -> &Arc<BoundDevice> {
        &self.device
    }
}
/**
An opaque guard type for ports.
*/
pub struct PortGuard<'a> {
    guard: std::sync::MutexGuard<'a, Option<Port>>,
}
impl Deref for PortGuard<'_> {
    type Target = Port;

    fn deref(&self) -> &Self::Target {
        self.guard.as_ref().unwrap()
    }
}
impl DerefMut for PortGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_mut().unwrap()
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CreateError {
    #[error("Can't create engine {0}")]
    EntryPoint(#[from] EntryPointError),
    #[error("Can't find a GPU {0}")]
    Gpu(#[from] PickError),
    #[error("Can't bind GPU {0}")]
    Bind(#[from] BindError),
    #[error("Can't create port {0}")]
    Port(#[from] super::port::Error),
    #[error("Implementation error {0}")]
    Imp(#[from] imp::Error),
}
