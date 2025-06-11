// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//!Cross-platform IMAGE device wrappers
//!
//! On vulkan, image device and compute device are potentially distinct
use std::fmt::Formatter;
use std::sync::Arc;

use crate::entry_point::{EntryPoint, EntryPointError};
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

#[derive(thiserror::Error, Debug)]
pub(crate) enum EitherError {
    #[error("Error with entrypoint {0}")]
    EntryPoint(#[from] EntryPointError),
    #[error("Error binding a device {0}")]
    Bind(BindError),
    #[error("Error picking a device {0}")]
    Pick(PickError),
}
impl From<BindError> for EitherError {
    fn from(e: BindError) -> Self {
        Self::Bind(e)
    }
}

impl From<PickError> for EitherError {
    fn from(e: PickError) -> Self {
        Self::Pick(e)
    }
}

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
