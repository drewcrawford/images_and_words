// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//at the moment we only support wgpu

pub(crate) use crate::send_phantom::SendPhantom;

/// Represents a dirty region of a texture that needs to be copied to the GPU.
#[derive(Debug, Clone, Copy)]
pub struct DirtyRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl DirtyRect {
    /// Creates a new dirty rect covering the entire texture.
    pub fn full(width: u16, height: u16) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// Returns the union (bounding box) of two dirty rects.
    pub fn union(self, other: Self) -> Self {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.width).max(other.x + other.width);
        let y2 = (self.y + self.height).max(other.y + other.height);
        Self {
            x: x1,
            y: y1,
            width: x2 - x1,
            height: y2 - y1,
        }
    }
}
#[cfg(feature = "exfiltrate")]
use exfiltrate::command::ImageInfo;
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

    /// Takes the dirty rect, leaving None in its place.
    /// Returns the region that needs to be copied to the GPU.
    #[allow(dead_code)] //nop implementation does not use
    fn take_dirty_rect(&mut self) -> Option<crate::imp::DirtyRect>;
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
pub(crate) enum DumpMessage {
    Image(ImageInfo),
    Expect(usize),
}

#[cfg(feature = "exfiltrate")]
pub(crate) static DUMP_NEXT_FRAME: Mutex<Option<wasm_safe_mutex::mpsc::Sender<DumpMessage>>> =
    Mutex::new(None);
