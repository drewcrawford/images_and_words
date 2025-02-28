use std::fmt::Debug;
use std::marker::PhantomData;
use wgpu::{Extent3d, TextureDescriptor, TextureDimension, TextureViewDescriptor};
use wgpu::util::{DeviceExt, TextureDataOrder};
use crate::bindings::resource_tracking::GPUGuard;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::TextureUsage;
use crate::imp::{CopyInfo, Error};
use crate::multibuffer::sealed::GPUMultibuffer;
use crate::pixel_formats::pixel_as_bytes;
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
    imp: wgpu::Buffer,
    format: PhantomData<Format>,
}

impl<Format> Debug for MappableTexture<Format> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappableTexture")
            .field("imp", &self.imp)
            .finish()
    }
}



/**
A texxture mappable (only) to the GPU.
*/

#[derive(Debug)]
pub struct GPUableTexture<Format> {
    format: PhantomData<Format>,
    imp: wgpu::Texture,
}
impl<Format: crate::pixel_formats::sealed::PixelFormat> GPUableTexture<Format> {

    pub async fn new_initialize<I: Fn(Texel) -> Format::CPixel>(bound_device: &crate::images::BoundDevice, width: u16, height: u16, visible_to: TextureUsage, debug_name: &str, priority: Priority, initializer: I) -> Result<Self, Error> {
        let descriptor = Self::get_descriptor(debug_name, width, height, visible_to);
        let data_order = TextureDataOrder::default(); //?
        //todo: could optimize probably?
        let pixels = (width as usize * height as usize);
        let mut src_buf = Vec::with_capacity(pixels);
        for x in 0..width {
            for y in 0..height {
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

pub struct CopyGuard {

}
impl<Format> AsRef<GPUableTexture<Format>> for CopyGuard {
    fn as_ref(&self) -> &GPUableTexture<Format> {
        todo!()
    }
}

impl<Format> GPUMultibuffer for GPUableTexture<Format> {
    type CorrespondingMappedType = MappableTexture<Format>;
    type OutGuard<InGuard> = CopyGuard;

    unsafe fn copy_from_buffer<'a, Guarded>(&self, source_offset: usize, dest_offset: usize, copy_len: usize, info: &mut CopyInfo<'a>, guard: GPUGuard<Guarded>) -> Self::OutGuard<GPUGuard<Guarded>>
    where
        Guarded: AsRef<Self::CorrespondingMappedType>,
        Guarded: Mappable
    {
        todo!()
    }
}

pub struct RenderSide {
    pub(super) texture: wgpu::Texture,
}


