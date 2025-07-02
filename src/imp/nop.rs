#![allow(dead_code)]

// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::Priority;
use crate::bindings::buffer_access::MapType;
use crate::bindings::forward::dynamic::buffer::WriteFrequency;
use crate::bindings::sampler::SamplerType;
use crate::bindings::visible_to::{CPUStrategy, GPUBufferUsage, TextureConfig, TextureUsage};
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::PassDescriptor;
use crate::imp::{GPUableTextureWrapper, MappableTextureWrapper};
use crate::pixel_formats::sealed::PixelFormat as CratePixelFormat;
use crate::send_phantom::SendPhantom;
use raw_window_handle::RawDisplayHandle;
use std::fmt::Display;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct EntryPoint;
impl EntryPoint {
    pub async fn new() -> Result<Self, Error> {
        todo!()
    }
}
pub struct UnboundDevice;

impl UnboundDevice {
    pub(crate) fn surface_strategy(&self) -> &SurfaceStrategy {
        todo!()
    }
}

pub trait BackendSend: Send {}
impl<T: Send> BackendSend for T {}
pub trait BackendSync: Sync {}
impl<T: Sync> BackendSync for T {}

impl UnboundDevice {
    pub async fn pick(
        _surface: &crate::images::view::View,
        _entry_point: &crate::entry_point::EntryPoint,
    ) -> Result<UnboundDevice, Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct View {}
impl View {
    pub async unsafe fn from_surface(
        _entrypoint: &crate::entry_point::EntryPoint,
        _raw_window_handle: raw_window_handle::RawWindowHandle,
        _raw_display_handle: RawDisplayHandle,
    ) -> Result<Self, Error> {
        todo!()
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn for_testing() -> Self {
        View {}
    }
}
#[derive(Debug)]
pub struct Port {}

impl Port {
    pub(crate) async fn new(
        _engine: &Arc<crate::images::Engine>,
        _view: crate::images::view::View,
        _camera: Camera,
        _port_reporter_send: PortReporterSend,
    ) -> Result<Self, Error> {
        Ok(Port {})
    }

    pub async fn add_fixed_pass(&mut self, _descriptor: PassDescriptor) {
        todo!()
    }
    pub async fn start(&mut self) -> Result<(), Error> {
        todo!()
    }
    pub async fn render_frame(&mut self) {
        todo!()
    }
}

#[derive(Debug)]
pub struct Sampler;

impl Sampler {
    pub fn new(
        _bound_device: &crate::images::BoundDevice,
        _coordinate_type: SamplerType,
    ) -> Result<Self, Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct FrameTexture<Format>(Format);
impl<Format> FrameTexture<Format> {
    pub async fn new<I>(
        _bound_device: &crate::images::BoundDevice,
        _config: TextureConfig<'_>,
        _initialize_with: I,
    ) -> (Self, Vec<FrameTextureProduct<Format>>) {
        todo!()
    }
}
#[derive(Debug)]
pub struct BoundDevice;

impl BoundDevice {
    pub(crate) async fn bind(
        _unbound_device: crate::images::device::UnboundDevice,
        _entry_point: Arc<crate::entry_point::EntryPoint>,
    ) -> Result<Self, Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct Engine;
impl Engine {
    pub async fn rendering_to_view(_bound_device: &Arc<crate::images::BoundDevice>) -> Self {
        todo!()
    }
}
#[derive(Debug)]
pub struct Error;
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error")
    }
}
impl std::error::Error for Error {}

#[derive(Clone)]
pub struct SurfaceStrategy;
#[derive(Debug)]
pub struct FrameTextureProduct<Format>(PhantomData<Format>);
#[derive(Debug, Clone)]
pub struct FrameTextureDelivery;
pub struct Product<Element>(Element);

impl<Element> Product<Element> {
    pub fn new<I: Fn(usize) -> Element>(
        _bound_device: &crate::images::BoundDevice,
        _size: usize,
        _write_frequency: WriteFrequency,
        _cpu_strategy: CPUStrategy,
        _debug_name: &str,
        _initialize_with: I,
    ) -> Vec<Self> {
        todo!()
    }
    pub fn write(&mut self, _index: usize, _element: Element) {
        todo!()
    }
}
#[derive(Debug, Clone)]
pub struct Delivery;
#[derive(Debug)]
pub struct Texture<Format>(Format);
impl<Format: CratePixelFormat> Texture<Format> {
    pub async fn new(
        _bound_device: &crate::images::BoundDevice,
        _width: u16,
        _height: u16,
        _visible_to: TextureUsage,
        _data: &[Format::CPixel],
        _debug_name: &str,
        _priority: Priority,
    ) -> Result<Self, Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct GPUableTexture<Format> {
    _format: PhantomData<Format>,
    width: u32,
    height: u32,
    debug_name: String,
}

impl<Format> Clone for GPUableTexture<Format> {
    fn clone(&self) -> Self {
        GPUableTexture {
            _format: PhantomData,
            width: self.width,
            height: self.height,
            debug_name: self.debug_name.clone(),
        }
    }
}

//we don't actually send the format!
unsafe impl<Format> Send for GPUableTexture<Format> {}
unsafe impl<Format> Sync for GPUableTexture<Format> {}

impl<Format> GPUableTexture<Format> {
    pub async fn new(
        _bound_device: &crate::images::BoundDevice,
        config: TextureConfig<'_>,
    ) -> Result<Self, Error> {
        Ok(GPUableTexture {
            _format: PhantomData,
            width: config.width as u32,
            height: config.height as u32,
            debug_name: config.debug_name.to_string(),
        })
    }
    pub async fn new_initialize<I>(
        _device: &crate::images::BoundDevice,
        config: TextureConfig<'_>,
        _initialize_to: I,
    ) -> Result<Self, Error> {
        Ok(GPUableTexture {
            _format: PhantomData,
            width: config.width as u32,
            height: config.height as u32,
            debug_name: config.debug_name.to_string(),
        })
    }
    pub fn render_side(&self) -> RenderSide {
        todo!()
    }

    pub fn as_imp(&self) {}
}

impl<Format> GPUableTextureWrapper for GPUableTexture<Format> {}

impl<Format> crate::imp::GPUableTextureWrapped for GPUableTexture<Format> {
    fn format_matches(&self, _other: &dyn crate::imp::MappableTextureWrapped) -> bool {
        todo!("format_matches not implemented for nop backend")
    }

    fn copy_from_mappable(
        &self,
        _source: &mut dyn crate::imp::MappableTextureWrapped,
        _copy_info: &mut crate::imp::CopyInfo,
    ) -> Result<(), String> {
        // Nop backend doesn't actually perform copies
        Ok(())
    }
}

pub struct MappableTexture<Format>(SendPhantom<Format>);

impl<Format> std::fmt::Debug for MappableTexture<Format> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("MappableTexture")
            .field(&"SendPhantom")
            .finish()
    }
}
impl<Format> MappableTexture<Format> {
    pub async fn new<I>(
        _bound_device: &crate::images::BoundDevice,
        _width: u16,
        _height: u16,
        _debug_name: &str,
        _priority: Priority,
        _initialize_with: I,
    ) -> Self {
        todo!()
    }

    pub fn replace(
        &mut self,
        _src_width: u16,
        _dst_texel: crate::bindings::software::texture::Texel,
        _data: &[Format::CPixel],
    ) where
        Format: CratePixelFormat,
    {
        todo!()
    }

    pub fn as_imp(&self) {}
}

// Implement Mappable trait for MappableTexture
impl<Format> crate::bindings::resource_tracking::sealed::Mappable for MappableTexture<Format> {
    async fn map_read(&mut self) {
        todo!()
    }

    async fn map_write(&mut self) {
        todo!()
    }

    fn byte_len(&self) -> usize {
        todo!()
    }

    async fn unmap(&mut self) {
        todo!()
    }
}

impl<Format: Send + Sync> MappableTextureWrapper for MappableTexture<Format> {}

impl<Format: Send + Sync + 'static> crate::imp::MappableTextureWrapped for MappableTexture<Format> {
    fn width(&self) -> u16 {
        todo!("width not implemented for nop backend")
    }

    fn height(&self) -> u16 {
        todo!("height not implemented for nop backend")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSide;

pub type TextureRenderSide = RenderSide;

#[derive(Debug)]
pub struct MappableBuffer;
impl MappableBuffer {
    pub async fn new<F>(
        _bound_device: Arc<crate::images::BoundDevice>,
        _byte_size: usize,
        _map_type: MapType,
        _debug_name: &str,
        _callback: F,
    ) -> Result<Self, Error>
    where
        F: FnOnce(&mut [std::mem::MaybeUninit<u8>]) -> &[u8],
    {
        todo!()
    }

    pub async fn map_read(&mut self) {
        todo!()
    }

    pub async fn map_write(&mut self) {
        todo!()
    }

    pub async fn unmap(&mut self) {
        todo!()
    }

    pub fn byte_len(&self) -> usize {
        todo!()
    }

    pub fn as_slice(&self) -> &[u8] {
        todo!()
    }

    pub fn write(&mut self, _data: &[u8], _dst_offset: usize) {
        todo!()
    }
}

// Implement Mappable trait for MappableBuffer
impl crate::bindings::resource_tracking::sealed::Mappable for MappableBuffer {
    async fn map_read(&mut self) {
        self.map_read().await
    }

    async fn map_write(&mut self) {
        self.map_write().await
    }

    fn byte_len(&self) -> usize {
        self.byte_len()
    }

    async fn unmap(&mut self) {
        self.unmap().await
    }
}

impl AsRef<MappableBuffer> for MappableBuffer {
    fn as_ref(&self) -> &MappableBuffer {
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GPUableBuffer;
impl GPUableBuffer {
    pub async fn new(
        _bound_device: Arc<crate::images::BoundDevice>,
        _byte_size: usize,
        _usage: GPUBufferUsage,
        _debug_name: &str,
    ) -> Self {
        todo!()
    }

    pub async fn copy_from_buffer(
        &self,
        _source: MappableBuffer,
        _source_offset: usize,
        _dest_offset: usize,
        _copy_len: usize,
    ) {
        todo!()
    }
}

#[derive(Debug)]
pub struct BindTargetBufferImp;

#[derive(Debug)]
pub struct CopyInfo<'a> {
    pub(crate) command_encoder: PhantomData<&'a ()>,
}

#[derive(Debug)]
pub struct CopyGuard<SourceGuard> {
    source_guard: SourceGuard,
    gpu_buffer: GPUableBuffer,
}

impl<SourceGuard> AsRef<GPUableBuffer> for CopyGuard<SourceGuard> {
    fn as_ref(&self) -> &GPUableBuffer {
        &self.gpu_buffer
    }
}

#[derive(Debug)]
pub struct TextureCopyGuard<Format, SourceGuard> {
    source_guard: SourceGuard,
    gpu_texture: GPUableTexture<Format>,
}

impl<Format, SourceGuard> AsRef<GPUableTexture<Format>> for TextureCopyGuard<Format, SourceGuard> {
    fn as_ref(&self) -> &GPUableTexture<Format> {
        &self.gpu_texture
    }
}

pub trait PixelFormat {}

// Implement PixelFormat for all pixel format types
impl PixelFormat for crate::pixel_formats::R8UNorm {}
impl PixelFormat for crate::pixel_formats::RGBA16Unorm {}
impl PixelFormat for crate::pixel_formats::RGFloat {}
impl PixelFormat for crate::pixel_formats::R32SInt {}
impl PixelFormat for crate::pixel_formats::R32Float {}
impl PixelFormat for crate::pixel_formats::RGBA8UNorm {}
impl PixelFormat for crate::pixel_formats::BGRA8UNormSRGB {}
impl PixelFormat for crate::pixel_formats::RGBA32Float {}
impl PixelFormat for crate::pixel_formats::RGBA8UnormSRGB {}
impl PixelFormat for crate::pixel_formats::R16Float {}

impl<Format> AsRef<MappableTexture<Format>> for MappableTexture<Format> {
    fn as_ref(&self) -> &MappableTexture<Format> {
        self
    }
}

/// Helper function to copy from a mappable buffer to a GPU buffer
pub fn copy_mappable_to_gpuable_buffer(
    _source: &MappableBuffer,
    _dest: &GPUableBuffer,
    _source_offset: usize,
    _dest_offset: usize,
    _copy_len: usize,
    _copy_info: &mut CopyInfo,
) {
    // No-op implementation
}

/// Helper function to copy from a mappable texture to a GPU texture
pub fn copy_mappable_to_gpuable_texture<Format: crate::pixel_formats::sealed::PixelFormat>(
    _source: &MappableTexture<Format>,
    _dest: &GPUableTexture<Format>,
    _copy_info: &mut CopyInfo,
) {
    // No-op implementation
}
