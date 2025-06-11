// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::imp;
///Platform-independent entrypoint implementation
///
use std::fmt::{Debug, Formatter};

#[derive(Debug)]
pub struct EntryPoint(pub(crate) crate::imp::EntryPoint);
///platform-independent error type
#[derive(Debug)]
pub struct EntryPointError(imp::Error);
impl std::fmt::Display for EntryPointError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}
impl std::error::Error for EntryPointError {}

impl EntryPoint {
    ///Must use this constructor to get an [super::images]-compatible entrypoint.
    pub async fn new() -> Result<Self, EntryPointError> {
        crate::imp::EntryPoint::new()
            .await
            .map(EntryPoint)
            .map_err(EntryPointError)
    }

}
