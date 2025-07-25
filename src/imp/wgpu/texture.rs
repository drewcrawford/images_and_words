// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::Priority;
use crate::bindings::buffer_access::MapType;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::{TextureConfig, TextureUsage};
use crate::imp::{Error, MappableBuffer};
use crate::imp::{GPUableTextureWrapper, MappableTextureWrapper};
use crate::pixel_formats::pixel_as_bytes;
use crate::pixel_formats::sealed::PixelFormat;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use wgpu::util::{DeviceExt, TextureDataOrder};
use wgpu::{Extent3d, TextureDescriptor, TextureDimension};

impl TextureUsage {
    pub const fn wgpu_usage(&self) -> wgpu::TextureUsages {
        match self {
            TextureUsage::FragmentShaderRead => wgpu::TextureUsages::TEXTURE_BINDING,
            TextureUsage::VertexShaderRead => wgpu::TextureUsages::TEXTURE_BINDING,
            TextureUsage::VertexAndFragmentShaderRead => wgpu::TextureUsages::TEXTURE_BINDING,
            TextureUsage::FragmentShaderSample => wgpu::TextureUsages::TEXTURE_BINDING,
            TextureUsage::VertexShaderSample => wgpu::TextureUsages::TEXTURE_BINDING,
            TextureUsage::VertexAndFragmentShaderSample => wgpu::TextureUsages::TEXTURE_BINDING,
        }
    }
}

pub struct MappableTexture<Format> {
    //on wgpu, textures cannot be mapped, only buffers.
    imp: MappableBuffer,
    format: PhantomData<Format>,
    width: u16,
    height: u16,
}

impl<Format> Mappable for MappableTexture<Format> {
    async fn map_read(&mut self) {
        self.imp.map_read().await;
    }

    async fn map_write(&mut self) {
        self.imp.map_write().await;
    }

    fn byte_len(&self) -> usize {
        self.imp.byte_len()
    }

    fn unmap(&mut self) {
        self.imp.unmap();
    }
}

//we don't actually send the format!
unsafe impl<Format> Send for MappableTexture<Format> {}
unsafe impl<Format> Sync for MappableTexture<Format> {}

impl<Format: PixelFormat> MappableTexture<Format> {
    pub fn new<Initializer: Fn(Texel) -> Format::CPixel>(
        bound_device: &Arc<crate::images::BoundDevice>,
        width: u16,
        height: u16,
        debug_name: &str,
        _priority: Priority,
        initializer: Initializer,
    ) -> Self {
        let bytes_per_pixel = std::mem::size_of::<Format::CPixel>();
        let aligned_bytes_per_row = Self::aligned_bytes_per_row(width);
        // Buffer size must account for the aligned row size
        let buffer_size = aligned_bytes_per_row * height as usize;

        let buffer = MappableBuffer::new(
            bound_device.clone(),
            buffer_size,
            MapType::Write,
            debug_name,
            |byte_array| {
                // Initialize the buffer with padding
                for y in 0..height {
                    for x in 0..width {
                        let pixel_offset =
                            y as usize * aligned_bytes_per_row + x as usize * bytes_per_pixel;
                        let texel = Texel { x, y };
                        let pixel = initializer(texel);

                        // Write pixel data at the correct offset
                        unsafe {
                            let pixel_ptr =
                                byte_array.as_mut_ptr().add(pixel_offset) as *mut Format::CPixel;
                            pixel_ptr.write(pixel);
                        }
                    }
                    // The padding bytes between rows are left uninitialized (but that's OK for GPU buffers)
                }

                // Return the byte array
                unsafe {
                    std::slice::from_raw_parts_mut(
                        byte_array.as_mut_ptr() as *mut u8,
                        byte_array.len(),
                    )
                }
            },
        )
        .expect("Mappable buffer creation");
        Self {
            imp: buffer,
            format: PhantomData,
            width,
            height,
        }
    }

    fn aligned_bytes_per_row(width: u16) -> usize {
        // Calculate destination offset based on texel position with proper alignment
        let bytes_per_pixel = std::mem::size_of::<Format::CPixel>();
        let unaligned_bytes_per_row = width as usize * bytes_per_pixel;

        // Use the same alignment calculation as in new()
        let aligned_bytes_per_row = unaligned_bytes_per_row
            .checked_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize - 1)
            .unwrap()
            .div_euclid(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize)
            .checked_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize)
            .unwrap();
        aligned_bytes_per_row
    }

    pub fn replace(&mut self, src_width: u16, dst_texel: Texel, data: &[Format::CPixel]) {
        assert!(src_width == self.width); //we could support this but it would involve multiple copies
        assert!(dst_texel == Texel::ZERO); //not supported at present
        use crate::pixel_formats::pixel_as_bytes;
        let data_bytes = pixel_as_bytes(data);
        let bytes_per_pixel = std::mem::size_of::<Format::CPixel>();
        let unaligned_bytes_per_row = src_width as usize * bytes_per_pixel; //this is the unaligned row size
        let aligned_bytes_per_row = Self::aligned_bytes_per_row(self.width);

        for y in 0..self.height {
            //src is tightly packed
            let src_offset = y as usize * unaligned_bytes_per_row;
            let src_slice = &data_bytes[src_offset..src_offset + unaligned_bytes_per_row];
            //dst is aligned
            let dst_offset = y as usize * aligned_bytes_per_row;
            self.imp.write(&src_slice, dst_offset);
        }
    }
}

impl<Format> Debug for MappableTexture<Format> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappableTexture")
            .field("imp", &self.imp)
            .finish()
    }
}

impl<Format: Send + Sync> MappableTextureWrapper for MappableTexture<Format> {}

impl<Format: Send + Sync + 'static> crate::imp::MappableTextureWrapped for MappableTexture<Format> {
    fn width(&self) -> u16 {
        self.width
    }

    fn height(&self) -> u16 {
        self.height
    }
}

/**
A texture mappable (only) to the GPU.

Design note, we want to track the format in types here.  For a format-less version, grab the render side.
*/
pub struct GPUableTexture<Format> {
    format: PhantomData<Format>,
    imp: wgpu::Texture,
    width: u32,
    height: u32,
    debug_name: String,
}
impl<Format> Debug for GPUableTexture<Format> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GPUableTexture")
            .field("imp", &self.imp)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("debug_name", &self.debug_name)
            .finish()
    }
}
//needed for CopyGuard to work
impl<Format> Clone for GPUableTexture<Format> {
    fn clone(&self) -> Self {
        GPUableTexture {
            format: PhantomData,
            imp: self.imp.clone(),
            width: self.width,
            height: self.height,
            debug_name: self.debug_name.clone(),
        }
    }
}
//we don't actually send the format!
unsafe impl<Format> Send for GPUableTexture<Format> {}
unsafe impl<Format> Sync for GPUableTexture<Format> {}

impl<Format: crate::pixel_formats::sealed::PixelFormat> GPUableTexture<Format> {
    pub async fn new_initialize<I: Fn(Texel) -> Format::CPixel>(
        bound_device: &crate::images::BoundDevice,
        config: TextureConfig<'_>,
        initializer: I,
    ) -> Result<Self, Error> {
        let descriptor = Self::get_descriptor(
            config.debug_name,
            config.width,
            config.height,
            config.visible_to,
            config.mipmaps,
        );
        //todo: could optimize probably?
        let pixels = config.width as usize * config.height as usize;
        let mut src_buf = Vec::with_capacity(pixels);
        for y in 0..config.height {
            for x in 0..config.width {
                src_buf.push(initializer(Texel { x, y }));
            }
        }
        if config.mipmaps {
            let mut current_mip_level = 1;
            //these properties are per base mip
            let mut base_mip_start = 0;
            let mut base_mip_width = config.width;
            let mut base_mip_height = config.height;
            let mut _mip_size = descriptor.mip_level_size(current_mip_level);

            while let Some(mip_size) = _mip_size {
                //generate a new mipmap
                let mip_width = mip_size.width as u16;
                let mip_height = mip_size.height as u16;
                let physical_size = mip_size.physical_size(descriptor.format);
                let pad_x = physical_size.width as u16 - mip_width;
                let pad_y = physical_size.height as u16 - mip_height;
                let current_mip_start = src_buf.len();

                //get the base mip level
                for mip_y in 0..mip_height {
                    for mip_x in 0..mip_width {
                        //get upper left texel in base mip
                        let base_x = mip_x * 2;
                        let base_y = mip_y * 2;
                        //calculate each index
                        let base_index =
                            base_y as usize * base_mip_width as usize + base_x as usize;
                        let base_index_right =
                            base_y as usize * base_mip_width as usize + (base_x + 1) as usize;
                        let base_index_down =
                            (base_y + 1) as usize * base_mip_width as usize + base_x as usize;
                        let base_index_down_right =
                            (base_y + 1) as usize * base_mip_width as usize + (base_x + 1) as usize;
                        //get the pixels
                        let base_pixel = src_buf[base_mip_start + base_index].clone();
                        let base_pixel_right = if base_mip_width <= 1 {
                            //no right pixel
                            base_pixel.clone()
                        } else {
                            src_buf[base_mip_start + base_index_right].clone()
                        };
                        //get the pixel below
                        let base_pixel_down = if base_mip_height <= 1 {
                            //no down pixel
                            base_pixel.clone()
                        } else {
                            src_buf[base_mip_start + base_index_down].clone()
                        };
                        //get the pixel down right
                        let base_pixel_down_right = if base_mip_width <= 1 && base_mip_height <= 1 {
                            //only one pixel; use base
                            base_pixel.clone()
                        } else if base_mip_width <= 1 {
                            //no right pixel
                            base_pixel_down.clone() //use down pixel?
                        } else if base_mip_height <= 1 {
                            //no down pixel
                            base_pixel_right.clone() //use right pixel?
                        } else {
                            src_buf[base_mip_start + base_index_down_right].clone()
                        };
                        //average them
                        use crate::pixel_formats::sealed::CPixelTrait;
                        let average_pixel = Format::CPixel::avg(&[
                            base_pixel,
                            base_pixel_right,
                            base_pixel_down,
                            base_pixel_down_right,
                        ]);
                        //set the pixel
                        src_buf.push(average_pixel);
                    }
                    let last_px = src_buf.last().unwrap().clone();
                    for _pad_x in 0..pad_x {
                        //pad by extending last pixel
                        src_buf.push(last_px.clone());
                    }
                }
                let last_px = src_buf.last().unwrap().clone();
                for _pad_y in 0..pad_y {
                    for _x in 0..physical_size.width as u16 {
                        //pad by extending last pixel
                        src_buf.push(last_px.clone());
                    }
                }
                //finish the mip level
                base_mip_start = current_mip_start;
                base_mip_width = mip_width;
                base_mip_height = mip_height;
                current_mip_level += 1;
                _mip_size = descriptor.mip_level_size(current_mip_level);
            }
        }

        let texture = bound_device.0.device.create_texture_with_data(
            &bound_device.0.queue,
            &descriptor,
            TextureDataOrder::default(),
            pixel_as_bytes(&src_buf),
        );
        Ok(Self {
            format: PhantomData,
            imp: texture,
            width: config.width as u32,
            height: config.height as u32,
            debug_name: config.debug_name.to_string(),
        })
    }

    fn get_descriptor(
        debug_name: &str,
        width: u16,
        height: u16,
        visible_to: TextureUsage,
        mipmaps: bool,
    ) -> TextureDescriptor {
        let mip_level_count = if mipmaps {
            // 3
            width.max(height).ilog2() + 1
        } else {
            1
        };
        TextureDescriptor {
            label: Some(debug_name),
            size: Extent3d {
                width: width.into(),
                height: height.into(),
                depth_or_array_layers: 1,
            },
            mip_level_count,
            sample_count: 1, //?
            dimension: TextureDimension::D2,
            format: Format::WGPU_FORMAT,
            usage: visible_to.wgpu_usage() | wgpu::TextureUsages::COPY_DST,
            view_formats: &[], //?
        }
    }

    pub async fn new(
        bound_device: &crate::images::BoundDevice,
        config: TextureConfig<'_>,
    ) -> Result<Self, Error> {
        let descriptor = Self::get_descriptor(
            config.debug_name,
            config.width,
            config.height,
            config.visible_to,
            config.mipmaps,
        );
        let texture = bound_device.0.device.create_texture(&descriptor);
        Ok(Self {
            format: PhantomData,
            imp: texture,
            width: config.width as u32,
            height: config.height as u32,
            debug_name: config.debug_name.to_string(),
        })
    }

    pub fn render_side(&self) -> RenderSide {
        RenderSide {
            texture: self.imp.clone(),
        }
    }
}

impl<Format> GPUableTextureWrapper for GPUableTexture<Format> {}

impl<Format: crate::pixel_formats::sealed::PixelFormat> crate::imp::GPUableTextureWrapped
    for GPUableTexture<Format>
{
    fn format_matches(&self, other: &dyn crate::imp::MappableTextureWrapped) -> bool {
        // Check if dimensions match
        if self.width != other.width() as u32 || self.height != other.height() as u32 {
            return false;
        }

        // Try to downcast to get the wgpu format
        // We need to use Any trait for this
        let other_any = other as &dyn std::any::Any;

        // Try to downcast to our specific MappableTextureWrappedWgpu trait object
        // This is a bit tricky - we need to check if it implements our trait
        // For now, we'll check if it's exactly our type with matching format
        if let Some(_other_texture) = other_any.downcast_ref::<MappableTexture<Format>>() {
            // If we can downcast to the exact same type, formats match
            return true;
        }

        // If we can't downcast to the same type, formats don't match
        false
    }

    fn copy_from_mappable(
        &self,
        source: &dyn crate::imp::MappableTextureWrapped,
        copy_info: &mut crate::imp::CopyInfo,
    ) -> Result<(), String> {
        // First check format compatibility
        if !self.format_matches(source) {
            return Err(format!(
                "Format mismatch: GPU texture is {}x{}, but source is {}x{} or has incompatible format",
                self.width,
                self.height,
                source.width(),
                source.height()
            ));
        }

        // Try to downcast to MappableTexture<Format>
        let source_any = source as &dyn std::any::Any;

        if let Some(source_concrete) = source_any.downcast_ref::<MappableTexture<Format>>() {
            // Perform the copy using the existing copy_texture_internal function
            copy_texture_internal(source_concrete, self, copy_info);
            Ok(())
        } else {
            Err("Failed to downcast source texture to concrete type".to_string())
        }
    }
}

#[derive(Debug)]
pub struct CopyGuard<Format, SourceGuard> {
    #[allow(dead_code)] // guard keeps source alive during copy operation
    guard: SourceGuard,
    gpu: GPUableTexture<Format>,
}
impl<Format, SourceGuard> AsRef<GPUableTexture<Format>> for CopyGuard<Format, SourceGuard> {
    fn as_ref(&self) -> &GPUableTexture<Format> {
        &self.gpu
    }
}

impl<Format> AsRef<MappableTexture<Format>> for MappableTexture<Format> {
    fn as_ref(&self) -> &MappableTexture<Format> {
        self
    }
}

/// Internal helper function to copy from a mappable texture to a GPU texture
pub(super) fn copy_texture_internal<Format: crate::pixel_formats::sealed::PixelFormat>(
    source: &MappableTexture<Format>,
    dest: &GPUableTexture<Format>,
    copy_info: &mut super::CopyInfo,
) {
    use wgpu::{Extent3d, TexelCopyBufferInfoBase, TexelCopyTextureInfoBase};

    // Calculate bytes per row with proper alignment
    let unaligned_bytes_per_row =
        source.width as u32 * std::mem::size_of::<Format::CPixel>() as u32;
    let aligned_bytes_per_row = unaligned_bytes_per_row
        .checked_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
        .unwrap()
        .div_euclid(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        .checked_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        .unwrap();

    let source_base = TexelCopyBufferInfoBase {
        buffer: &source.imp.buffer,
        layout: wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(aligned_bytes_per_row),
            rows_per_image: Some(source.height as u32),
        },
    };
    let dest_base = TexelCopyTextureInfoBase {
        texture: &dest.imp,
        mip_level: 0,
        origin: Default::default(),
        aspect: Default::default(),
    };
    copy_info.command_encoder.copy_buffer_to_texture(
        source_base,
        dest_base,
        Extent3d {
            width: source.width as u32,
            height: source.height as u32,
            depth_or_array_layers: 1,
        },
    );
}

#[derive(Debug, Clone)]
pub struct RenderSide {
    pub(super) texture: wgpu::Texture,
}

impl PartialEq for RenderSide {
    fn eq(&self, other: &Self) -> bool {
        self.texture == other.texture
    }
}
