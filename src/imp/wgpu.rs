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

pub use bound_device::BoundDevice;
pub use buffer::{
    CopyInfo, GPUableBuffer, GPUableBuffer2, GPUableBuffer2Static, MappableBuffer, MappableBuffer2,
};
pub use engine::Engine;
pub use entry_point::EntryPoint;
pub(crate) use error::Error;
pub use pixel_format::PixelFormat;
pub use port::Port;
pub use texture::RenderSide as TextureRenderSide;
pub use texture::{
    GPUableTexture, GPUableTexture2, GPUableTexture2Static, MappableTexture, MappableTexture2,
};
pub use unbound_device::UnboundDevice;
pub use view::View;

/**
A trait for backend-specific synchronization requirements.
*/
#[cfg(target_arch = "wasm32")]
pub trait BackendSync {}
/**
A trait for backend-specific synchronization requirements.
*/
#[cfg(not(target_arch = "wasm32"))]
pub trait BackendSync: Sync {}

#[cfg(target_arch = "wasm32")]
impl<T> BackendSync for T {}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Sync> BackendSync for T {}

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

/// Copy from a mappable buffer to a GPU buffer within an existing command encoder.
///
/// This function is designed for batched operations in render pipelines where multiple
/// buffer copies need to be grouped together for efficiency. It records the copy command
/// in the provided command encoder but does not wait for completion.
///
/// # Use Cases
/// - Dynamic buffer updates during render passes
/// - Batching multiple buffer copies for performance
/// - Operations where the caller manages command submission and synchronization
///
/// # Contrast with `copy_from_buffer`
/// - This function uses an existing command encoder (via `CopyInfo`)
/// - Takes a mutable reference to source (caller manages lifetime)
/// - Does not wait for GPU completion (fire-and-forget)
/// - Designed for render pipeline integration
///
/// For standalone operations that need guaranteed completion, use
/// `GPUableBuffer::copy_from_buffer` instead.
///
/// # Arguments
/// * `source` - The mappable buffer to copy from (must remain alive until command submission)
/// * `dest` - The GPU buffer to copy to
/// * `source_offset` - Byte offset in the source buffer
/// * `dest_offset` - Byte offset in the destination buffer  
/// * `copy_len` - Number of bytes to copy
/// * `copy_info` - Contains the command encoder to record the copy operation
pub fn copy_mappable_to_gpuable_buffer(
    source: &mut MappableBuffer,
    dest: &GPUableBuffer,
    source_offset: usize,
    dest_offset: usize,
    copy_len: usize,
    copy_info: &mut CopyInfo<'_>,
) {
    source.wgpu_buffer().assume(move |source_buffer_guard| {
        dest.buffer().assume(move |dest_buffer_guard| {
            copy_info.command_encoder.copy_buffer_to_buffer(
                source_buffer_guard,
                source_offset as u64,
                dest_buffer_guard,
                dest_offset as u64,
                copy_len as u64,
            );
        });
    });
}
