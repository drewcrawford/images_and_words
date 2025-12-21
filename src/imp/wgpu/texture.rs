// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::Priority;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::{TextureConfig, TextureUsage};
use crate::images::BoundDevice;
use crate::imp::Error;
use crate::imp::wgpu::cell::WgpuCell;
use crate::imp::{GPUableTextureWrapper, MappableTextureWrapper};
use crate::pixel_formats::pixel_as_bytes;
use crate::pixel_formats::sealed::PixelFormat;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use wgpu::util::{DeviceExt, TextureDataOrder};
use wgpu::{Extent3d, TexelCopyBufferLayout, TexelCopyTextureInfo};

impl TextureUsage {
    /// Converts this texture usage to the corresponding wgpu texture usage flags.
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

use crate::imp::DirtyRect;

/// CPU-side texture storage for write_texture operations.
/// Unlike MappableBuffer2 (which uses write_buffer_with), textures need
/// a CPU-side buffer because write_texture takes a data slice directly.
pub struct MappableTexture2<Format> {
    /// Direct CPU storage - textures use write_texture which needs a data slice
    buffer: Box<[u8]>,
    format: PhantomData<Format>,
    width: u16,
    height: u16,
    dirty_rect: Option<DirtyRect>,
}

impl<Format> Mappable for MappableTexture2<Format> {
    async fn map_write(&mut self) {
        // No-op: we use direct CPU storage, no mapping needed
    }

    fn unmap(&mut self) {
        // No-op: we use direct CPU storage
    }
}

unsafe impl<Format> Send for MappableTexture2<Format> {}
unsafe impl<Format> Sync for MappableTexture2<Format> {}

impl<Format: PixelFormat> MappableTexture2<Format> {
    pub async fn new<Initializer: Fn(Texel) -> Format::CPixel>(
        _bound_device: &Arc<crate::images::BoundDevice>,
        width: u16,
        height: u16,
        _debug_name: &str,
        _priority: Priority,
        initializer: Initializer,
    ) -> Self {
        Self::new_with_region(
            _bound_device,
            width,
            height,
            _debug_name,
            _priority,
            DirtyRect::full(width, height),
            initializer,
        )
        .await
    }

    pub async fn new_with_region<Initializer: Fn(Texel) -> Format::CPixel>(
        _bound_device: &Arc<crate::images::BoundDevice>,
        width: u16,
        height: u16,
        _debug_name: &str,
        _priority: Priority,
        region: DirtyRect,
        initializer: Initializer,
    ) -> Self {
        let bytes_per_pixel = std::mem::size_of::<Format::CPixel>();
        let aligned_bytes_per_row = Self::aligned_bytes_per_row(width);
        let buffer_size = aligned_bytes_per_row * height as usize;

        // Allocate zeroed buffer
        let mut buffer = vec![0u8; buffer_size];

        // Only initialize the specified region
        for y in region.y..(region.y + region.height) {
            for x in region.x..(region.x + region.width) {
                let pixel_offset =
                    y as usize * aligned_bytes_per_row + x as usize * bytes_per_pixel;
                let texel = Texel { x, y };
                let pixel = initializer(texel);

                unsafe {
                    let pixel_ptr = buffer.as_mut_ptr().add(pixel_offset) as *mut Format::CPixel;
                    pixel_ptr.write(pixel);
                }
            }
        }

        Self {
            buffer: buffer.into_boxed_slice(),
            format: PhantomData,
            width,
            height,
            dirty_rect: Some(region),
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
        let src_height = data.len() / src_width.max(1) as usize;
        logwise::mandatory_sync!(
            "replace: dst=({x},{y}) size={w}x{h}",
            x = dst_texel.x,
            y = dst_texel.y,
            w = src_width,
            h = src_height
        );
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

            // Write directly to our buffer
            self.buffer[dst_offset..dst_offset + src_bytes_per_row].copy_from_slice(src_slice);
        }

        // Track dirty rect
        let new_dirty = DirtyRect {
            x: dst_texel.x,
            y: dst_texel.y,
            width: src_width,
            height: src_height as u16,
        };
        self.dirty_rect = Some(match self.dirty_rect {
            Some(existing) => existing.union(new_dirty),
            None => new_dirty,
        });
    }
}

impl<Format> Debug for MappableTexture2<Format> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappableTexture2")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("buffer_len", &self.buffer.len())
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
    fn as_slice(&self) -> &[u8] {
        &self.buffer
    }

    fn take_dirty_rect(&mut self) -> Option<DirtyRect> {
        // Access field directly to avoid ambiguity with trait method
        self.dirty_rect.take()
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
            move_device.0.device().assume(move |device| {
                let descriptor = wgpu::BufferDescriptor {
                    label: Some(&staging_debug_name),
                    size: staging_buffer_size as u64,
                    usage: staging_usage,
                    mapped_at_creation: false,
                };
                device.create_buffer(&descriptor)
            })
        })
        .await;

        // Create GPU texture
        let config_debug_name = config.debug_name.to_string();
        let config_width = config.width;
        let config_height = config.height;
        let config_visible_to = config.visible_to;
        let config_mipmaps = config.mipmaps;

        let gpu_texture = WgpuCell::new_on_thread(move || async move {
            move_device2.0.device().assume(move |device| {
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
    ) -> wgpu::TextureDescriptor<'_> {
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

    unsafe fn copy_from_mappable<'f>(
        &'f self,
        source: &'f mut dyn crate::imp::MappableTextureWrapped,
        _copy_info: &'f mut crate::imp::CopyInfo<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + 'f>> {
        Box::pin(async move {
            // Get dirty rect - if None, nothing to copy
            let dirty_rect = match source.take_dirty_rect() {
                Some(rect) => {
                    logwise::mandatory_sync!(
                        "copy_from_mappable: got dirty rect ({x},{y}) {w}x{h}",
                        x = rect.x,
                        y = rect.y,
                        w = rect.width,
                        h = rect.height
                    );
                    rect
                }
                None => {
                    logwise::mandatory_sync!(
                        "texture_copy_data: {name} - skipping (no dirty rect)",
                        name = logwise::privacy::LogIt(&self.debug_name)
                    );
                    return Ok(());
                }
            };

            // Use queue.write_texture() to bypass staging buffer overhead
            // This avoids the expensive map_async + get_mapped_range_mut on WebGPU/wasm
            assert!(self.format_matches(source), "Texture formats do not match");

            let _copy_data_guard = logwise::profile_begin!("texture_copy_data");

            let bytes_per_pixel = std::mem::size_of::<Format::CPixel>();
            let aligned_bytes_per_row =
                MappableTexture2::<Format>::aligned_bytes_per_row(self.width as u16);

            let first_dirty_row = dirty_rect.y as usize;
            let dirty_row_count = dirty_rect.height as usize;
            let dirty_col_start = dirty_rect.x as usize;
            let dirty_col_bytes = dirty_rect.width as usize * bytes_per_pixel;

            // Actual bytes we'll copy (just the dirty rectangle)
            let copied_bytes = dirty_row_count * dirty_col_bytes;

            logwise::mandatory_sync!(
                "texture_copy_data: {name} rect ({x},{y}) {w}x{h} ({size_kb} KB of {total_kb} KB) via write_texture",
                name = logwise::privacy::LogIt(&self.debug_name),
                x = dirty_rect.x,
                y = dirty_rect.y,
                w = dirty_rect.width,
                h = dirty_rect.height,
                size_kb = copied_bytes / 1024,
                total_kb = (self.height as usize * aligned_bytes_per_row) / 1024
            );

            // Get the source data for the dirty region
            // We pass the full source rows containing the dirty rect, with proper byte offset
            let source_slice = source.as_slice();
            let buffer_offset = first_dirty_row * aligned_bytes_per_row;
            let x_offset_bytes = dirty_col_start * bytes_per_pixel;
            let data_start = buffer_offset + x_offset_bytes;
            let data_end = buffer_offset
                + (dirty_row_count - 1) * aligned_bytes_per_row
                + x_offset_bytes
                + dirty_col_bytes;
            let dirty_data = &source_slice[data_start..data_end];

            self.bound_device.0.queue().assume(|queue| {
                self.gpu_texture.assume(|gpu_texture| {
                    queue.write_texture(
                        TexelCopyTextureInfo {
                            texture: gpu_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: dirty_rect.x as u32,
                                y: first_dirty_row as u32,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        dirty_data,
                        TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(aligned_bytes_per_row.try_into().unwrap()),
                            rows_per_image: Some(dirty_row_count as u32),
                        },
                        Extent3d {
                            width: dirty_rect.width as u32,
                            height: dirty_row_count as u32,
                            depth_or_array_layers: 1,
                        },
                    );
                });
            });
            drop(_copy_data_guard);

            Ok(())
        })
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
            move_device.0.device().assume(move |device| {
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
    ) -> wgpu::TextureDescriptor<'_> {
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
        let move_queue = bound_device.0.queue().clone();

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
            .device()
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

impl<Format> PartialEq for GPUableTexture2Static<Format> {
    fn eq(&self, other: &Self) -> bool {
        self.gpu_texture == other.gpu_texture
    }
}

impl<Format> Eq for GPUableTexture2Static<Format> {}

impl<Format> std::hash::Hash for GPUableTexture2Static<Format> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.gpu_texture.hash(state);
    }
}

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
