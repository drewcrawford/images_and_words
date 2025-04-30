use std::fmt::Debug;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use log::debug;
use wgpu::{Extent3d, TexelCopyBufferInfoBase, TexelCopyTextureInfoBase, TextureDescriptor, TextureDimension, TextureViewDescriptor};
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
        todo!()
    }

    async fn map_write(&mut self) {
        todo!()
    }

    fn byte_len(&self) -> usize {
        todo!()
    }

    fn unmap(&mut self) {
        todo!()
    }
}

//we don't actually send the format!
unsafe impl<Format> Send for MappableTexture<Format> {}
unsafe impl<Format> Sync for MappableTexture<Format> {}

impl<Format: PixelFormat> MappableTexture<Format> {
    pub fn new<Initializer: Fn(Texel) -> Format::CPixel>(bound_device: &crate::images::BoundDevice, width: u16, height: u16, debug_name: &str, priority: Priority, initializer: Initializer) -> Self {
        let buffer = MappableBuffer::new(&bound_device, width as usize * height as usize * std::mem::size_of::<Format::CPixel>(), MapType::Write, debug_name, |byte_array| {
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

    pub async fn new_initialize<I: Fn(Texel) -> Format::CPixel>(bound_device: &crate::images::BoundDevice, width: u16, height: u16, visible_to: TextureUsage, debug_name: &str, priority: Priority, initializer: I) -> Result<Self, Error> {
        let descriptor = Self::get_descriptor(debug_name, width, height, visible_to);
        let data_order = TextureDataOrder::default(); //?
        //todo: could optimize probably?
        let pixels = (width as usize * height as usize);
        let mut src_buf = Vec::with_capacity(pixels);
        for y in 0..height {
            for x in 0..width {
                src_buf.push(initializer(Texel{x,y}));
            }
        }
        let texture = bound_device.0.device.create_texture_with_data(&bound_device.0.queue, &descriptor, TextureDataOrder::default(), pixel_as_bytes(&src_buf));
        Ok(Self {
            format: PhantomData,
            imp: texture,
        })
    }

    fn get_descriptor(debug_name: &str, width: u16, height: u16, visible_to: TextureUsage) -> TextureDescriptor {
        TextureDescriptor {
            label: Some(debug_name),
            size: Extent3d {
                width: width.into(),
                height: height.into(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1, //?
            sample_count: 1, //?
            dimension: TextureDimension::D2,
            format: Format::WGPU_FORMAT,
            usage:visible_to.wgpu_usage(),
            view_formats: &[], //?
        }
    }

    pub async fn new(bound_device: &crate::images::BoundDevice, width: u16, height: u16, visible_to: TextureUsage, debug_name: &str, priority: Priority) -> Result<Self, Error> {
        let descriptor = Self::get_descriptor(debug_name, width, height, visible_to);
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

pub struct CopyGuard<Format,SourceGuard> {
    guard: SourceGuard,
    gpu: GPUableTexture<Format>,
}
impl<Format,SourceGuard> AsRef<GPUableTexture<Format>> for CopyGuard<Format,SourceGuard> {
    fn as_ref(&self) -> &GPUableTexture<Format> {
        &self.gpu
    }
}

impl<Format> GPUMultibuffer for GPUableTexture<Format> {
    type CorrespondingMappedType = MappableTexture<Format>;
    type OutGuard<InGuard> = CopyGuard<Format,InGuard>;

    unsafe fn copy_from_buffer<'a, Guarded>(&self, source_offset: usize, dest_offset: usize, copy_len: usize, info: &mut CopyInfo<'a>, guard: GPUGuard<Guarded>) -> Self::OutGuard<GPUGuard<Guarded>>
    where
        Guarded: AsRef<Self::CorrespondingMappedType>,
        Guarded: Mappable
    {
        let source_base = TexelCopyBufferInfoBase {
            buffer: &guard.as_ref().imp.buffer,
            layout: Default::default(),
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
            depth_or_array_layers: 0,
        });
        CopyGuard {
            guard,
            gpu: self.clone(),
        }
    }
}

pub struct RenderSide {
    pub(super) texture: wgpu::Texture,
}


