// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//at the moment we only support wgpu

pub use crate::send_phantom::SendPhantom;

pub trait GPUableTextureWrapper: Send + Sync {
}

#[cfg(not(feature = "backend_wgpu"))]
mod nop;
#[cfg(not(feature = "backend_wgpu"))]
pub use nop::*;

#[cfg(feature = "backend_wgpu")]
mod wgpu;

#[cfg(feature = "backend_wgpu")]
pub use wgpu::*;
