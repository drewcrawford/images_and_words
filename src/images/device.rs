// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//!Cross-platform IMAGE device wrappers
//!
//! On vulkan, image device and compute device are potentially distinct
use std::fmt::Formatter;
use std::sync::Arc;

use crate::entry_point::EntryPoint;
use crate::images::view::View;
use crate::imp;

///Cross-platform unbound device, images edition
pub(crate) struct UnboundDevice(pub(crate) crate::imp::UnboundDevice);
impl UnboundDevice {
    ///Pick a device for the associated surface
    pub async fn pick(view: &View, entry_point: &EntryPoint) -> Result<UnboundDevice, PickError> {
        crate::imp::UnboundDevice::pick(view, entry_point)
            .await
            .map(UnboundDevice)
            .map_err(PickError)
    }
}

#[derive(Debug)]
pub struct PickError(imp::Error);
impl std::fmt::Display for PickError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}
impl std::error::Error for PickError {}

///Cross-platform bound device, images edition
///
/// We want to expose this because it does not depend on Surface, so it does not need to be generic.  This
/// is a limitation of our [super::Engine] type.
#[derive(Debug)]
pub struct BoundDevice(pub(crate) imp::BoundDevice);

impl AsRef<imp::BoundDevice> for BoundDevice {
    fn as_ref(&self) -> &imp::BoundDevice {
        &self.0
    }
}

#[derive(Debug)]
pub struct BindError(imp::Error);
impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}
impl std::error::Error for BindError {}

impl BoundDevice {
    /*
    Vulkan prefers to create this impl as Arc because it points to itself internally.
     */
    pub(crate) async fn bind(
        unbound_device: UnboundDevice,
        entry_point: Arc<EntryPoint>,
    ) -> Result<Self, BindError> {
        let bind = crate::imp::BoundDevice::bind(unbound_device, entry_point)
            .await
            .map_err(BindError)?;
        Ok(Self(bind))
    }
}

// Boilerplate implementations

impl Clone for BoundDevice {
    fn clone(&self) -> Self {
        // Safe to clone - resources are shared via Arc in the backend implementation.
        // Multiple BoundDevice instances can safely share the same GPU resources.
        Self(self.0.clone())
    }
}

impl PartialEq for BoundDevice {
    fn eq(&self, other: &Self) -> bool {
        // Two BoundDevices are equal if they reference the same underlying resources.
        // This is equivalent to Arc pointer equality in the backend.
        std::ptr::eq(&self.0 as *const _, &other.0 as *const _)
    }
}

impl Eq for BoundDevice {
    // BoundDevice can implement Eq since it represents a resource handle,
    // and equality is well-defined (same underlying GPU resources).
}

impl std::hash::Hash for BoundDevice {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash based on the pointer to the underlying implementation.
        // This ensures that cloned instances have the same hash.
        std::ptr::hash(&self.0 as *const _, state);
    }
}
