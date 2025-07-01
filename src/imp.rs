// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//at the moment we only support wgpu

pub use crate::send_phantom::SendPhantom;

pub trait GPUableTextureWrapper: Send + Sync {}

pub trait MappableTextureWrapper: Send + Sync {}

pub(crate) trait GPUableTextureWrapped: GPUableTextureWrapper {
    #[allow(dead_code)] //nop implementation does not use
    fn format_matches(&self, other: &dyn MappableTextureWrapped) -> bool;
    /// Perform a copy from a mappable texture to this GPU texture
    #[allow(dead_code)] //nop implementation does not use
    fn copy_from_mappable(
        &self,
        source: &mut dyn MappableTextureWrapped,
        copy_info: &mut crate::imp::CopyInfo,
    ) -> Result<(), String>;
}

pub(crate) trait MappableTextureWrapped: MappableTextureWrapper + std::any::Any {
    #[allow(dead_code)] //nop implementation does not use
    fn width(&self) -> u16;
    #[allow(dead_code)] //nop implementation does not use
    fn height(&self) -> u16;
}

#[cfg(not(feature = "backend_wgpu"))]
mod nop;
#[cfg(not(feature = "backend_wgpu"))]
pub use nop::*;

#[cfg(feature = "backend_wgpu")]
mod wgpu;

#[cfg(feature = "backend_wgpu")]
pub use wgpu::*;
