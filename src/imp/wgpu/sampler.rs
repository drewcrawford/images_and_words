use wgpu::{AddressMode, SamplerDescriptor};
use crate::bindings::sampler::SamplerType;
use crate::imp::Error;

#[derive(Debug)]
pub struct Sampler {
    pub(crate) sampler: wgpu::Sampler,
}

impl Sampler {
    pub fn new(bound_device: &crate::images::BoundDevice, coordinate_type: SamplerType) -> Result<Self,Error> {
        let min_filter = match coordinate_type {
            SamplerType::Mipmapped => wgpu::FilterMode::Linear,
            SamplerType::PixelLinear => wgpu::FilterMode::Linear,
        };

        let mag_filter = match coordinate_type {
            SamplerType::Mipmapped => wgpu::FilterMode::Linear,
            SamplerType::PixelLinear => wgpu::FilterMode::Nearest,
        };

        let mipmap_filter = match coordinate_type {
            SamplerType::Mipmapped => wgpu::FilterMode::Linear,
            SamplerType::PixelLinear => wgpu::FilterMode::Nearest,
        };

        let s = SamplerDescriptor {
            label: None,
            address_mode_u: AddressMode::ClampToEdge, //?
            address_mode_v: AddressMode::ClampToEdge, //?
            address_mode_w: AddressMode::ClampToEdge, //?
            mag_filter,
            min_filter,
            mipmap_filter,
            lod_min_clamp: 0.0,
            lod_max_clamp: 14.0, //?
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        };
        let s = bound_device.0.device.create_sampler(&s);
        Ok(Self {
            sampler: s,
        })
    }
}