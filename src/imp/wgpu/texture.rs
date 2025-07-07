// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::Priority;
use crate::bindings::buffer_access::MapType;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::{TextureConfig, TextureUsage};
use crate::images::BoundDevice;
use crate::imp::wgpu::cell::WgpuCell;
use crate::imp::{Error, MappableBuffer2};
use crate::imp::{GPUableTextureWrapper, MappableTextureWrapper};
use crate::pixel_formats::pixel_as_bytes;
use crate::pixel_formats::sealed::PixelFormat;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use wgpu::util::{DeviceExt, TextureDataOrder};

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

pub struct MappableTexture2<Format> {
    imp: MappableBuffer2,
    format: PhantomData<Format>,
    width: u16,
    height: u16,
}

impl<Format> Mappable for MappableTexture2<Format> {
    async fn map_read(&mut self) {
        self.imp.map_read().await;
    }

    async fn map_write(&mut self) {
        self.imp.map_write().await;
    }

    fn byte_len(&self) -> usize {
        self.imp.byte_len()
    }

    async fn unmap(&mut self) {
        self.imp.unmap().await;
    }
}

unsafe impl<Format> Send for MappableTexture2<Format> {}
unsafe impl<Format> Sync for MappableTexture2<Format> {}

impl<Format: PixelFormat> MappableTexture2<Format> {
    pub async fn new<Initializer: Fn(Texel) -> Format::CPixel>(
        bound_device: &Arc<crate::images::BoundDevice>,
        width: u16,
        height: u16,
        debug_name: &str,
        _priority: Priority,
        initializer: Initializer,
    ) -> Self {
        let bytes_per_pixel = std::mem::size_of::<Format::CPixel>();
        let aligned_bytes_per_row = Self::aligned_bytes_per_row(width);
        let buffer_size = aligned_bytes_per_row * height as usize;

        let buffer = MappableBuffer2::new(
            bound_device.clone(),
            buffer_size,
            MapType::Write,
            debug_name,
            |byte_array| {
                for y in 0..height {
                    for x in 0..width {
                        let pixel_offset =
                            y as usize * aligned_bytes_per_row + x as usize * bytes_per_pixel;
                        let texel = Texel { x, y };
                        let pixel = initializer(texel);

                        unsafe {
                            let pixel_ptr =
                                byte_array.as_mut_ptr().add(pixel_offset) as *mut Format::CPixel;
                            pixel_ptr.write(pixel);
                        }
                    }
                }

                unsafe {
                    std::slice::from_raw_parts_mut(
                        byte_array.as_mut_ptr() as *mut u8,
                        byte_array.len(),
                    )
                }
            },
        )
        .await
        .expect("Mappable buffer creation");
        Self {
            imp: buffer,
            format: PhantomData,
            width,
            height,
        }
    }

    fn aligned_bytes_per_row(width: u16) -> usize {
        let bytes_per_pixel = std::mem::size_of::<Format::CPixel>();
        let unaligned_bytes_per_row = width as usize * bytes_per_pixel;

        unaligned_bytes_per_row
            .checked_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize - 1)
            .unwrap()
            .div_euclid(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize)
            .checked_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize)
            .unwrap()
    }

    pub fn replace(&mut self, src_width: u16, dst_texel: Texel, data: &[Format::CPixel]) {
        assert!(src_width > 0, "Source width must be greater than 0");

        assert!(
            data.len() % src_width as usize == 0,
            "Data length ({}) must be divisible by source width ({})",
            data.len(),
            src_width
        );
        let src_height = data.len() / src_width as usize;

        assert!(
            dst_texel.x as usize + src_width as usize <= self.width as usize,
            "Destination region exceeds texture width: dst_x({}) + src_width({}) > texture_width({})",
            dst_texel.x,
            src_width,
            self.width
        );
        assert!(
            dst_texel.y as usize + src_height <= self.height as usize,
            "Destination region exceeds texture height: dst_y({}) + src_height({}) > texture_height({})",
            dst_texel.y,
            src_height,
            self.height
        );

        use crate::pixel_formats::pixel_as_bytes;
        let data_bytes = pixel_as_bytes(data);
        let bytes_per_pixel = std::mem::size_of::<Format::CPixel>();
        let src_bytes_per_row = src_width as usize * bytes_per_pixel;
        let aligned_bytes_per_row = Self::aligned_bytes_per_row(self.width);

        for row in 0..src_height {
            let src_offset = row * src_bytes_per_row;
            let src_slice = &data_bytes[src_offset..src_offset + src_bytes_per_row];

            let dst_y = dst_texel.y as usize + row;
            let dst_x_bytes = dst_texel.x as usize * bytes_per_pixel;
            let dst_offset = dst_y * aligned_bytes_per_row + dst_x_bytes;

            self.imp.write(src_slice, dst_offset);
        }
    }
}

impl<Format> Debug for MappableTexture2<Format> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappableTexture2")
            .field("imp", &self.imp)
            .finish()
    }
}

impl<Format: Send + Sync> MappableTextureWrapper for MappableTexture2<Format> {}

impl<Format: Send + Sync + 'static> crate::imp::MappableTextureWrapped
    for MappableTexture2<Format>
{
    fn width(&self) -> u16 {
        self.width
    }

    fn height(&self) -> u16 {
        self.height
    }
}

/**
A texture that holds both a staging buffer and a GPU texture for explicit staging operations.
Contains a staging buffer with MAP_WRITE | COPY_SRC and a GPU texture with COPY_DST | usage-specific flags.
Follows the same pattern as GPUableBuffer2.
*/
pub struct GPUableTexture2<Format> {
    format: PhantomData<Format>,
    staging_buffer: WgpuCell<wgpu::Buffer>,
    gpu_texture: WgpuCell<wgpu::Texture>,
    bound_device: Arc<BoundDevice>,
    width: u32,
    height: u32,
    debug_name: String,
}

impl<Format> Debug for GPUableTexture2<Format> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GPUableTexture2")
            .field("staging_buffer", &self.staging_buffer)
            .field("gpu_texture", &self.gpu_texture)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("debug_name", &self.debug_name)
            .finish()
    }
}

impl<Format> Clone for GPUableTexture2<Format> {
    fn clone(&self) -> Self {
        GPUableTexture2 {
            format: PhantomData,
            staging_buffer: self.staging_buffer.clone(),
            gpu_texture: self.gpu_texture.clone(),
            bound_device: self.bound_device.clone(),
            width: self.width,
            height: self.height,
            debug_name: self.debug_name.clone(),
        }
    }
}

unsafe impl<Format> Send for GPUableTexture2<Format> {}
unsafe impl<Format> Sync for GPUableTexture2<Format> {}

impl<Format: crate::pixel_formats::sealed::PixelFormat> GPUableTexture2<Format> {
    pub async fn new(
        bound_device: &Arc<crate::images::BoundDevice>,
        config: TextureConfig<'_>,
    ) -> Result<Self, Error> {
        let staging_usage = wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC;
        let texture_usage = config.visible_to.wgpu_usage() | wgpu::TextureUsages::COPY_DST;

        let staging_debug_name = format!("{}_staging", config.debug_name);
        let _texture_debug_name = format!("{}_texture", config.debug_name);
        let move_device = bound_device.clone();
        let move_device2 = bound_device.clone();

        // Calculate staging buffer size
        let bytes_per_pixel = std::mem::size_of::<Format::CPixel>() as u32;
        let unaligned_bytes_per_row = config.width as u32 * bytes_per_pixel;
        let aligned_bytes_per_row = unaligned_bytes_per_row
            .checked_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            .unwrap()
            .div_euclid(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .checked_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .unwrap();
        let staging_buffer_size = aligned_bytes_per_row * config.height as u32;

        // Create staging buffer
        let staging_buffer = WgpuCell::new_on_thread(move || async move {
            move_device
                .0
                .device
                .with(move |device| {
                    let descriptor = wgpu::BufferDescriptor {
                        label: Some(&staging_debug_name),
                        size: staging_buffer_size as u64,
                        usage: staging_usage,
                        mapped_at_creation: false,
                    };
                    device.create_buffer(&descriptor)
                })
                .await
        })
        .await;

        // Create GPU texture
        let config_debug_name = config.debug_name.to_string();
        let config_width = config.width;
        let config_height = config.height;
        let config_visible_to = config.visible_to;
        let config_mipmaps = config.mipmaps;

        let gpu_texture = WgpuCell::new_on_thread(move || async move {
            move_device2
                .0
                .device
                .with(move |device| {
                    let descriptor = Self::get_texture_descriptor(
                        &config_debug_name,
                        config_width,
                        config_height,
                        config_visible_to,
                        config_mipmaps,
                        texture_usage,
                    );
                    device.create_texture(&descriptor)
                })
                .await
        })
        .await;

        Ok(Self {
            format: PhantomData,
            staging_buffer,
            gpu_texture,
            bound_device: bound_device.clone(),
            width: config.width as u32,
            height: config.height as u32,
            debug_name: config.debug_name.to_string(),
        })
    }

    fn get_texture_descriptor(
        debug_name: &str,
        width: u16,
        height: u16,
        _visible_to: TextureUsage,
        mipmaps: bool,
        usage: wgpu::TextureUsages,
    ) -> wgpu::TextureDescriptor {
        let mip_level_count = if mipmaps {
            width.max(height).ilog2() + 1
        } else {
            1
        };
        wgpu::TextureDescriptor {
            label: Some(debug_name),
            size: wgpu::Extent3d {
                width: width.into(),
                height: height.into(),
                depth_or_array_layers: 1,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Format::WGPU_FORMAT,
            usage,
            view_formats: &[],
        }
    }

    pub fn render_side(&self) -> RenderSide {
        RenderSide {
            texture: self.gpu_texture.clone(),
        }
    }
}
impl<Format> GPUableTextureWrapper for GPUableTexture2<Format> {}

impl<Format: crate::pixel_formats::sealed::PixelFormat> crate::imp::GPUableTextureWrapped
    for GPUableTexture2<Format>
{
    fn format_matches(&self, other: &dyn crate::imp::MappableTextureWrapped) -> bool {
        // Check if dimensions match
        if self.width != other.width() as u32 || self.height != other.height() as u32 {
            return false;
        }

        // Try to downcast to get the wgpu format
        let other_any = other as &dyn std::any::Any;

        // Check if it's exactly our type with matching format (MappableTexture2)
        if let Some(_other_texture) = other_any.downcast_ref::<MappableTexture2<Format>>() {
            // If we can downcast to the exact same type, formats match
            return true;
        }

        // If we can't downcast to the same type, formats don't match
        false
    }

    fn copy_from_mappable(
        &self,
        _source: &mut dyn crate::imp::MappableTextureWrapped,
        _copy_info: &mut crate::imp::CopyInfo,
    ) -> Result<(), String> {
        // The sync copy interface is deprecated for GPUableTexture2.
        // Copies should happen during acquire guards using the async copy_from_mappable_texture2 method.
        panic!(
            "Sync copy interface removed for GPUableTexture2 - use async copy during acquire guards"
        );
    }
}

/**
A static texture that holds only a single GPU wgpu::Texture.
Like GPUableTexture2 but without the staging buffer - for static texture data that doesn't change.
*/
#[derive(Debug, Clone)]
pub struct GPUableTexture2Static<Format> {
    format: PhantomData<Format>,
    gpu_texture: WgpuCell<wgpu::Texture>,
    #[allow(dead_code)]
    bound_device: Arc<BoundDevice>,
    #[allow(dead_code)]
    debug_name: String,
}

impl<Format: crate::pixel_formats::sealed::PixelFormat> GPUableTexture2Static<Format> {
    #[allow(dead_code)]
    pub async fn new(
        bound_device: &Arc<crate::images::BoundDevice>,
        config: TextureConfig<'_>,
    ) -> Result<Self, Error> {
        let texture_usage = config.visible_to.wgpu_usage() | wgpu::TextureUsages::COPY_DST;

        let texture_debug_name = format!("{}_static", config.debug_name);
        let move_device = bound_device.clone();

        // Create GPU texture
        let _config_debug_name = config.debug_name.to_string();
        let config_width = config.width;
        let config_height = config.height;
        let config_visible_to = config.visible_to;
        let config_mipmaps = config.mipmaps;

        let gpu_texture = WgpuCell::new_on_thread(move || async move {
            move_device
                .0
                .device
                .with(move |device| {
                    let descriptor = Self::get_texture_descriptor(
                        &texture_debug_name,
                        config_width,
                        config_height,
                        config_visible_to,
                        config_mipmaps,
                        texture_usage,
                    );
                    device.create_texture(&descriptor)
                })
                .await
        })
        .await;

        Ok(Self {
            format: PhantomData,
            gpu_texture,
            bound_device: bound_device.clone(),
            debug_name: config.debug_name.to_string(),
        })
    }

    fn get_texture_descriptor(
        debug_name: &str,
        width: u16,
        height: u16,
        _visible_to: TextureUsage,
        mipmaps: bool,
        usage: wgpu::TextureUsages,
    ) -> wgpu::TextureDescriptor {
        let mip_level_count = if mipmaps {
            width.max(height).ilog2() + 1
        } else {
            1
        };
        wgpu::TextureDescriptor {
            label: Some(debug_name),
            size: wgpu::Extent3d {
                width: width.into(),
                height: height.into(),
                depth_or_array_layers: 1,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Format::WGPU_FORMAT,
            usage,
            view_formats: &[],
        }
    }

    pub fn render_side(&self) -> RenderSide {
        RenderSide {
            texture: self.gpu_texture.clone(),
        }
    }
}

impl<Format: crate::pixel_formats::sealed::PixelFormat> GPUableTexture2Static<Format> {
    /// Creates a new static texture with initial data provided during creation.
    ///
    /// This method creates a GPU texture using `create_texture_with_data` and initializes
    /// it with data using the provided initializer function. This is the most efficient
    /// way to create static textures since it avoids the need for a separate staging buffer
    /// and copy operation.
    ///
    /// # Arguments
    /// * `bound_device` - The GPU device to create the texture on
    /// * `config` - Texture configuration (size, format, usage, etc.)
    /// * `initializer` - Function to initialize the texture data
    ///
    /// # Returns
    /// Returns a `GPUableTexture2Static` with the initialized data.
    pub async fn new_with_data<I: Fn(Texel) -> Format::CPixel>(
        bound_device: &Arc<crate::images::BoundDevice>,
        config: TextureConfig<'_>,
        initializer: I,
    ) -> Result<Self, Error> {
        let texture_usage = config.visible_to.wgpu_usage() | wgpu::TextureUsages::COPY_DST;

        let texture_debug_name = format!("{}_static", config.debug_name);
        let move_device = bound_device.clone();
        let move_queue = bound_device.0.queue.clone();

        // Generate texture data
        let pixels = config.width as usize * config.height as usize;
        let mut src_buf = Vec::with_capacity(pixels);
        for y in 0..config.height {
            for x in 0..config.width {
                src_buf.push(initializer(Texel { x, y }));
            }
        }

        // Handle mipmaps if requested
        if config.mipmaps {
            let mut current_mip_level = 1;
            let mut base_mip_start = 0;
            let mut base_mip_width = config.width;
            let mut base_mip_height = config.height;
            let descriptor = Self::get_texture_descriptor(
                &texture_debug_name,
                config.width,
                config.height,
                config.visible_to,
                config.mipmaps,
                texture_usage,
            );
            let mut _mip_size = descriptor.mip_level_size(current_mip_level);

            while let Some(mip_size) = _mip_size {
                let mip_width = mip_size.width as u16;
                let mip_height = mip_size.height as u16;
                let physical_size = mip_size.physical_size(descriptor.format);
                let pad_x = physical_size.width as u16 - mip_width;
                let pad_y = physical_size.height as u16 - mip_height;
                let current_mip_start = src_buf.len();

                for mip_y in 0..mip_height {
                    for mip_x in 0..mip_width {
                        let base_x = mip_x * 2;
                        let base_y = mip_y * 2;
                        let base_index =
                            base_y as usize * base_mip_width as usize + base_x as usize;
                        let base_index_right =
                            base_y as usize * base_mip_width as usize + (base_x + 1) as usize;
                        let base_index_down =
                            (base_y + 1) as usize * base_mip_width as usize + base_x as usize;
                        let base_index_down_right =
                            (base_y + 1) as usize * base_mip_width as usize + (base_x + 1) as usize;

                        let base_pixel = src_buf[base_mip_start + base_index].clone();
                        let base_pixel_right = if base_mip_width <= 1 {
                            base_pixel.clone()
                        } else {
                            src_buf[base_mip_start + base_index_right].clone()
                        };
                        let base_pixel_down = if base_mip_height <= 1 {
                            base_pixel.clone()
                        } else {
                            src_buf[base_mip_start + base_index_down].clone()
                        };
                        let base_pixel_down_right = if base_mip_width <= 1 && base_mip_height <= 1 {
                            base_pixel.clone()
                        } else if base_mip_width <= 1 {
                            base_pixel_down.clone()
                        } else if base_mip_height <= 1 {
                            base_pixel_right.clone()
                        } else {
                            src_buf[base_mip_start + base_index_down_right].clone()
                        };

                        use crate::pixel_formats::sealed::CPixelTrait;
                        let average_pixel = Format::CPixel::avg(&[
                            base_pixel,
                            base_pixel_right,
                            base_pixel_down,
                            base_pixel_down_right,
                        ]);
                        src_buf.push(average_pixel);
                    }
                    let last_px = src_buf.last().unwrap().clone();
                    for _pad_x in 0..pad_x {
                        src_buf.push(last_px.clone());
                    }
                }
                let last_px = src_buf.last().unwrap().clone();
                for _pad_y in 0..pad_y {
                    for _x in 0..physical_size.width as u16 {
                        src_buf.push(last_px.clone());
                    }
                }
                base_mip_start = current_mip_start;
                base_mip_width = mip_width;
                base_mip_height = mip_height;
                current_mip_level += 1;
                _mip_size = descriptor.mip_level_size(current_mip_level);
            }
        }

        let _config_debug_name = config.debug_name.to_string();
        let config_width = config.width;
        let config_height = config.height;
        let config_visible_to = config.visible_to;
        let config_mipmaps = config.mipmaps;

        let gpu_texture = move_device
            .0
            .device
            .with(move |device| {
                move_queue.assume(move |q| {
                    let descriptor = Self::get_texture_descriptor(
                        &texture_debug_name,
                        config_width,
                        config_height,
                        config_visible_to,
                        config_mipmaps,
                        texture_usage,
                    );
                    let texture = device.create_texture_with_data(
                        q,
                        &descriptor,
                        TextureDataOrder::default(),
                        pixel_as_bytes(&src_buf),
                    );
                    WgpuCell::new(texture)
                })
            })
            .await;

        Ok(Self {
            format: PhantomData,
            gpu_texture,
            bound_device: bound_device.clone(),
            debug_name: config.debug_name.to_string(),
        })
    }
}

unsafe impl<Format> Send for GPUableTexture2Static<Format> {}
unsafe impl<Format> Sync for GPUableTexture2Static<Format> {}

impl<Format> GPUableTextureWrapper for GPUableTexture2Static<Format> {}

#[derive(Debug, Clone)]
pub struct RenderSide {
    pub(super) texture: WgpuCell<wgpu::Texture>,
}

impl PartialEq for RenderSide {
    fn eq(&self, other: &Self) -> bool {
        self.texture == other.texture
    }
}
