use std::fmt::Debug;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Arc;
use wgpu::{Extent3d, TexelCopyBufferInfoBase, TexelCopyTextureInfoBase, TextureDescriptor, TextureDimension};
use wgpu::util::{DeviceExt, TextureDataOrder};
use crate::bindings::buffer_access::MapType;
use crate::bindings::resource_tracking::GPUGuard;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::TextureUsage;
use crate::imp::{CopyInfo, Error, MappableBuffer};
use crate::multibuffer::sealed::GPUMultibuffer;
use crate::pixel_formats::pixel_as_bytes;
use crate::pixel_formats::sealed::PixelFormat;
use crate::Priority;

impl TextureUsage {
    pub const fn wgpu_usage(&self) -> wgpu::TextureUsages {
        match self {
            TextureUsage::FragmentShaderRead => {
                wgpu::TextureUsages::TEXTURE_BINDING
            }
            TextureUsage::VertexShaderRead => {
                wgpu::TextureUsages::TEXTURE_BINDING
            }
            TextureUsage::VertexAndFragmentShaderRead => {
                wgpu::TextureUsages::TEXTURE_BINDING
            }
            TextureUsage::FragmentShaderSample => {
                wgpu::TextureUsages::TEXTURE_BINDING
            }
            TextureUsage::VertexShaderSample => {
                wgpu::TextureUsages::TEXTURE_BINDING
            }
            TextureUsage::VertexAndFragmentShaderSample => {
                wgpu::TextureUsages::TEXTURE_BINDING
            }
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
    pub fn new<Initializer: Fn(Texel) -> Format::CPixel>(bound_device: &Arc<crate::images::BoundDevice>, width: u16, height: u16, debug_name: &str, _priority: Priority, initializer: Initializer) -> Self {
        let buffer = MappableBuffer::new(bound_device.clone(), width as usize * height as usize * std::mem::size_of::<Format::CPixel>(), MapType::Write, debug_name, |byte_array| {
            let elements = width as usize * height as usize;
            assert_eq!(byte_array.len(), elements * std::mem::size_of::<Format::CPixel>());
            //safety: we know the byte array is the right size
            let as_elements: &mut [MaybeUninit<Format::CPixel>] = unsafe{std::slice::from_raw_parts_mut(byte_array.as_mut_ptr() as *mut MaybeUninit<Format::CPixel>, elements)};
            for y in 0..height {
                for x in 0..width {
                    let index = y as usize * width as usize + x as usize;
                    let texel = Texel { x, y };
                    let pixel = initializer(texel);
                    as_elements[index] = MaybeUninit::new(pixel);
                }
            }
            //transmute to byte array
            //safety: we initialized all the elements
            unsafe {
                std::slice::from_raw_parts_mut(byte_array.as_mut_ptr() as *mut u8, byte_array.len())
            }
        }).expect("Mappable buffer creation");
        Self {
            imp: buffer,
            format: PhantomData,
            width,
            height,
        }
    }
    
    pub fn replace(&mut self, src_width: u16, dst_texel: Texel, data: &[Format::CPixel]) {
        assert!(src_width == self.width); //we could support this but it would involve multiple copies
        use crate::pixel_formats::pixel_as_bytes;
        let data_bytes = pixel_as_bytes(data);
        
        // Calculate destination offset based on texel position
        // Assuming the buffer represents a 2D texture laid out row-major
        let bytes_per_pixel = std::mem::size_of::<Format::CPixel>();
        let dst_offset = (dst_texel.y as usize * self.width as usize + dst_texel.x as usize) * bytes_per_pixel;
        
        self.imp.write(data_bytes, dst_offset);
    }
}

impl<Format> Debug for MappableTexture<Format> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappableTexture")
            .field("imp", &self.imp)
            .finish()
    }
}





/**
A texture mappable (only) to the GPU.

Design note, we want to track the format in types here.  For a format-less version, grab the render side.
*/
pub struct GPUableTexture<Format> {
    format: PhantomData<Format>,
    imp: wgpu::Texture,
}
impl<Format> Debug for GPUableTexture<Format> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GPUableTexture")
            .field("imp", &self.imp)
            .finish()
    }
}
//needed for CopyGuard to work
impl<Format> Clone for GPUableTexture<Format> {
    fn clone(&self) -> Self {
        GPUableTexture {
            format: PhantomData,
            imp: self.imp.clone(),
        }
    }
}
//we don't actually send the format!
unsafe impl<Format> Send for GPUableTexture<Format> {}
unsafe impl<Format> Sync for GPUableTexture<Format> {}

impl<Format: crate::pixel_formats::sealed::PixelFormat> GPUableTexture<Format> {

    pub async fn new_initialize<I: Fn(Texel) -> Format::CPixel>(bound_device: &crate::images::BoundDevice, width: u16, height: u16, visible_to: TextureUsage, generate_mipmaps: bool, debug_name: &str, _priority: Priority, initializer: I) -> Result<Self, Error> {
        let descriptor = Self::get_descriptor(debug_name, width, height, visible_to, generate_mipmaps);
        //todo: could optimize probably?
        let pixels = width as usize * height as usize;
        let mut src_buf = Vec::with_capacity(pixels);
        for y in 0..height {
            for x in 0..width {
                src_buf.push(initializer(Texel{x,y}));
            }
        }
        if generate_mipmaps {
            let mut current_mip_level = 1;
            //these properties are per base mip
            let mut base_mip_start = 0;
            let mut base_mip_width = width;
            let mut base_mip_height = height;
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
                        let base_index = base_y as usize * base_mip_width as usize + base_x as usize;
                        let base_index_right = base_y as usize * base_mip_width as usize + (base_x + 1) as usize;
                        let base_index_down = (base_y + 1) as usize * base_mip_width as usize + base_x as usize;
                        let base_index_down_right = (base_y + 1) as usize * base_mip_width as usize + (base_x + 1) as usize;
                        //get the pixels
                        let base_pixel = src_buf[base_mip_start + base_index].clone();
                        let base_pixel_right = if base_mip_width <= 1 {
                            //no right pixel
                            base_pixel.clone()
                        }
                        else {
                            src_buf[base_mip_start + base_index_right].clone()
                        };
                        //get the pixel below
                        let base_pixel_down = if base_mip_height <= 1 {
                            //no down pixel
                            base_pixel.clone()
                        }
                        else {
                            src_buf[base_mip_start + base_index_down].clone()
                        };
                        //get the pixel down right
                        let base_pixel_down_right = if base_mip_width <= 1 && base_mip_height <= 1 {
                            //only one pixel; use base
                            base_pixel.clone()
                        }
                        else if base_mip_width <= 1 {
                            //no right pixel
                            base_pixel_down.clone() //use down pixel?
                        }
                        else if base_mip_height <= 1 {
                            //no down pixel
                            base_pixel_right.clone() //use right pixel?
                        }
                        else {
                            src_buf[base_mip_start + base_index_down_right].clone()
                        };
                        //average them
                        use crate::pixel_formats::sealed::CPixelTrait;
                        let average_pixel = Format::CPixel::avg(&[base_pixel, base_pixel_right, base_pixel_down, base_pixel_down_right]);
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

        let texture = bound_device.0.device.create_texture_with_data(&bound_device.0.queue, &descriptor, TextureDataOrder::default(), pixel_as_bytes(&src_buf));
        Ok(Self {
            format: PhantomData,
            imp: texture,
        })
    }

    fn get_descriptor(debug_name: &str, width: u16, height: u16, visible_to: TextureUsage, mipmaps: bool) -> TextureDescriptor {
        let mip_level_count = if mipmaps {
            // 3
            width.max(height).ilog2() as u32 + 1
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

    pub async fn new(bound_device: &crate::images::BoundDevice, width: u16, height: u16, visible_to: TextureUsage, debug_name: &str, _priority: Priority) -> Result<Self, Error> {
        let descriptor = Self::get_descriptor(debug_name, width, height, visible_to, false);
        let texture = bound_device.0.device.create_texture(&descriptor);
        Ok(Self {
            format: PhantomData,
            imp: texture,
        })
    }

    pub fn render_side(&self) -> RenderSide {
        RenderSide {
            texture: self.imp.clone(),
        }
    }

}
#[derive(Debug)]
pub struct CopyGuard<Format,SourceGuard> {
    #[allow(dead_code)] // guard keeps source alive during copy operation
    guard: SourceGuard,
    gpu: GPUableTexture<Format>,
}
impl<Format,SourceGuard> AsRef<GPUableTexture<Format>> for CopyGuard<Format,SourceGuard> {
    fn as_ref(&self) -> &GPUableTexture<Format> {
        &self.gpu
    }
}

impl<Format: crate::pixel_formats::sealed::PixelFormat> GPUMultibuffer for GPUableTexture<Format> {
    type CorrespondingMappedType = MappableTexture<Format>;
    type OutGuard<InGuard> = CopyGuard<Format,InGuard>;

    unsafe fn copy_from_buffer<'a, Guarded>(&self, _source_offset: usize, _dest_offset: usize, _copy_len: usize, info: &mut CopyInfo<'a>, guard: GPUGuard<Guarded>) -> Self::OutGuard<GPUGuard<Guarded>>
    where
        Guarded: AsRef<Self::CorrespondingMappedType>,
        Guarded: Mappable
    {
        let source_base = TexelCopyBufferInfoBase {
            buffer: &guard.as_ref().imp.buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(guard.as_ref().width as u32 * std::mem::size_of::<Format::CPixel>() as u32),
                rows_per_image: Some(guard.as_ref().height as u32),
            },
        };
        let dest_base = TexelCopyTextureInfoBase {

            texture: &self.imp,
            mip_level: 0,
            origin: Default::default(),
            aspect: Default::default(),
        };
        info.command_encoder.copy_buffer_to_texture(source_base, dest_base, Extent3d {
            width: guard.as_ref().width as u32,
            height: guard.as_ref().height as u32,
            depth_or_array_layers: 1,
        });
        CopyGuard {
            guard,
            gpu: self.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderSide {
    pub(super) texture: wgpu::Texture,
}


