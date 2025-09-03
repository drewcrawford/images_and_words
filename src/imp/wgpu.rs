// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
mod bound_device;
mod buffer;
mod cell;
mod context;
mod engine;
mod entry_point;
mod error;
mod pixel_format;
mod port;
mod texture;
mod unbound_device;
mod view;

pub(crate) use bound_device::BoundDevice;
pub(crate) use buffer::{CopyInfo, GPUableBuffer, GPUableBufferStatic, MappableBuffer2};
pub(crate) use engine::Engine;
pub(crate) use entry_point::EntryPoint;
pub(crate) use error::Error;
pub(crate) use pixel_format::PixelFormat;
pub(crate) use port::Port;
pub(crate) use texture::RenderSide as TextureRenderSide;
pub(crate) use texture::{GPUableTexture2, GPUableTexture2Static, MappableTexture2};
pub(crate) use unbound_device::UnboundDevice;
pub(crate) use view::View;

/**
A trait for backend-specific synchronization requirements.
*/
#[cfg(target_arch = "wasm32")]
pub trait BackendSend {}
/**
A trait for backend-specific synchronization requirements.
*/
#[cfg(not(target_arch = "wasm32"))]
pub trait BackendSend: Send {}

#[cfg(target_arch = "wasm32")]
impl<T> BackendSend for T {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> BackendSend for T {}
