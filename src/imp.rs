// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//at the moment we only support wgpu

pub use crate::send_phantom::SendPhantom;

pub trait GPUableTextureWrapper: Send + Sync {}

pub trait MappableTextureWrapper: Send + Sync {}

pub(crate) trait GPUableTextureWrapped: GPUableTextureWrapper {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn format_matches(&self, other: &dyn MappableTextureWrapped) -> bool;
}

pub(crate) trait MappableTextureWrapped: MappableTextureWrapper + std::any::Any {
    fn width(&self) -> u16;
    fn height(&self) -> u16;
    fn byte_len(&self) -> usize;
}

#[cfg(not(feature = "backend_wgpu"))]
mod nop;
#[cfg(not(feature = "backend_wgpu"))]
pub use nop::*;

#[cfg(feature = "backend_wgpu")]
mod wgpu;

#[cfg(feature = "backend_wgpu")]
pub use wgpu::*;
