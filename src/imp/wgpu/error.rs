// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use std::fmt::Display;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    NoSuchAdapter,
    RequestDevice(#[from] wgpu::RequestDeviceError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CreateSurface(e) => write!(f, "{e}"),
            Error::NoSuchAdapter => write!(f, "No such adapter"),
            Error::RequestDevice(e) => write!(f, "{e}"),
        }
    }
}
