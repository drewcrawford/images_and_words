// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
mod bound_device;
mod buffer;
mod engine;
mod entry_point;
mod error;
mod pixel_format;
mod port;
mod texture;
mod unbound_device;
mod view;

pub use bound_device::BoundDevice;
pub use buffer::{CopyInfo, GPUableBuffer, MappableBuffer};
pub use engine::Engine;
pub use entry_point::EntryPoint;
pub(crate) use error::Error;
pub use pixel_format::PixelFormat;
pub use port::Port;
pub use texture::RenderSide as TextureRenderSide;
pub use texture::{GPUableTexture, MappableTexture};
pub use unbound_device::UnboundDevice;
pub use view::View;

/// Helper function to copy from a mappable buffer to a GPU buffer
pub fn copy_mappable_to_gpuable_buffer(
    source: &MappableBuffer,
    dest: &GPUableBuffer,
    source_offset: usize,
    dest_offset: usize,
    copy_len: usize,
    copy_info: &mut CopyInfo,
) {
    copy_info.command_encoder.copy_buffer_to_buffer(
        &source.buffer,
        source_offset as u64,
        &dest.buffer,
        dest_offset as u64,
        copy_len as u64,
    );
}

/// Helper function to copy from a mappable texture to a GPU texture
pub fn copy_mappable_to_gpuable_texture<Format: crate::pixel_formats::sealed::PixelFormat>(
    source: &MappableTexture<Format>,
    dest: &GPUableTexture<Format>,
    copy_info: &mut CopyInfo,
) {
    // For textures, we need to implement the copy inside the texture module
    // where we have access to private fields
    texture::copy_texture_internal(source, dest, copy_info);
}
