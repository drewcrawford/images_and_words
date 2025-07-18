// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::pixel_formats::{
    BGRA8UNormSRGB, R8UNorm, R16Float, R32Float, R32SInt, RGBA8UNorm, RGBA8UnormSRGB, RGBA16Unorm,
    RGBA32Float, RGFloat,
};

pub trait PixelFormat {
    const WGPU_FORMAT: wgpu::TextureFormat;
}

impl PixelFormat for R8UNorm {
    const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
}
impl PixelFormat for RGBA16Unorm {
    const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Unorm;
}
impl PixelFormat for RGFloat {
    const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Float;
}
impl PixelFormat for R32SInt {
    const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Sint;
}
impl PixelFormat for R32Float {
    const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
}
impl PixelFormat for RGBA8UNorm {
    const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
}
impl PixelFormat for BGRA8UNormSRGB {
    const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
}
impl PixelFormat for RGBA32Float {
    const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
}
impl PixelFormat for RGBA8UnormSRGB {
    const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
}
impl PixelFormat for R16Float {
    const WGPU_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
}
