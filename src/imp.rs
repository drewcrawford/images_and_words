// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//at the moment we only support wgpu

pub(crate) use crate::send_phantom::SendPhantom;
use std::pin::Pin;

pub trait GPUableTextureWrapper: Send + Sync {}

pub trait MappableTextureWrapper: Send + Sync {}

pub(crate) trait GPUableTextureWrapped: GPUableTextureWrapper {
    #[allow(dead_code)] //nop implementation does not use
    fn format_matches(&self, other: &dyn MappableTextureWrapped) -> bool;
    /// Perform a copy from a mappable texture to this GPU texture
    ///
    /// # Safety
    /// Keep guard alive
    #[allow(dead_code)] //nop implementation does not use
    unsafe fn copy_from_mappable<'f>(
        &'f self,
        source: &'f mut dyn MappableTextureWrapped,
        copy_info: &'f mut crate::imp::CopyInfo,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + 'f>>;
}

pub(crate) trait MappableTextureWrapped: MappableTextureWrapper + std::any::Any {
    #[allow(dead_code)] //nop implementation does not use
    fn width(&self) -> u16;
    #[allow(dead_code)] //nop implementation does not use
    fn height(&self) -> u16;

    fn as_slice(&self) -> &[u8];
}

#[cfg(not(feature = "backend_wgpu"))]
mod nop;
#[cfg(not(feature = "backend_wgpu"))]
pub(crate) use nop::*;

#[cfg(feature = "backend_wgpu")]
mod wgpu;

#[cfg(feature = "backend_wgpu")]
pub(crate) use wgpu::*;

#[cfg(feature = "exfiltrate")]
use wasm_safe_mutex::Mutex;

#[cfg(feature = "exfiltrate")]
pub(crate) static DUMP_NEXT_FRAME: Mutex<
    Option<std::sync::mpsc::Sender<exfiltrate::command::ImageInfo>>,
> = Mutex::new(None);
