use std::marker::PhantomData;
use wgpu::{Extent3d, TextureDescriptor, TextureDimension};
use wgpu::util::{DeviceExt, TextureDataOrder};
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::TextureUsage;
use crate::imp::Error;
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

#[derive(Debug)]
pub struct Texture<Format> {
    format: PhantomData<Format>,
    imp: wgpu::Texture,
}
impl<Format: crate::pixel_formats::sealed::PixelFormat> Texture<Format> {
    pub async fn new<Initializer: Fn(Texel) -> Format::CPixel>(bound_device: &crate::images::BoundDevice, width: u16, height: u16, visible_to: TextureUsage, debug_name: &str, priority: Priority, initializer: Initializer) -> Result<Self, Error> {
        let texture_descriptor = TextureDescriptor {
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
        };
        let data_order = TextureDataOrder::default(); //?
        //todo: could optimize probably?
        let pixels = (width as usize * height as usize);
        let mut src_buf = Vec::with_capacity(pixels);
        for x in 0..width {
            for y in 0..height {
                src_buf.push(initializer(Texel{x,y}));
            }
        }

        let texture = bound_device.0.device.create_texture_with_data(&bound_device.0.queue, &texture_descriptor, data_order, pixel_as_bytes(&src_buf));

        Ok(Self {
            format: PhantomData,
            imp: texture,
        })
    }

}


