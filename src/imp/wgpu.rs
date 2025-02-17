use std::fmt::Display;
use std::marker::PhantomData;
use std::sync::Arc;
use wgpu::{Extent3d, TextureDescriptor, TextureDimension};
use crate::bindings::forward::dynamic::buffer::WriteFrequency;
use crate::bindings::sampler::SamplerType;
use crate::bindings::visible_to::{CPUStrategy, TextureUsage};
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::PassTrait;
use crate::images::view::View as CrateView;
use crate::pixel_formats::sealed::PixelFormat as CratePixelFormat;
use crate::{Priority};

mod entry_point;
mod unbound_device;
mod view;
mod error;
mod bound_device;
mod engine;
mod port;
mod pixel_format;
mod texture;

pub use entry_point::EntryPoint;
pub use unbound_device::UnboundDevice;
pub use view::View;
pub(crate) use error::Error;
pub use bound_device::BoundDevice;
pub use engine::Engine;
pub use port::Port;
pub use pixel_format::PixelFormat;
pub use texture::Texture;




#[derive(Debug)]
pub struct Sampler;

impl Sampler {
    pub fn new(_bound_device: &crate::images::BoundDevice, _coordinate_type: SamplerType) -> Result<Self,Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct FrameTexture<Format>(Format);
impl<Format> FrameTexture<Format> {
    pub async fn new<I>(_bound_device: &crate::images::BoundDevice, _width: u16, _height: u16, _visible_to: TextureUsage, _cpu_strategy: CPUStrategy, _debug_name: &str, _initialize_with: I, _priority: Priority) -> (Self, Vec<crate::bindings::forward::dynamic::frame_texture::FrameTextureProduct<Format>>) {
        todo!()
    }
}




#[derive(Clone)]
pub struct SurfaceStrategy;
#[derive(Debug)]
pub struct FrameTextureProduct<Format>(PhantomData<Format>);
#[derive(Debug,Clone)]
pub struct FrameTextureDelivery;
pub struct Product<Element>(Element);

impl<Element> Product<Element> {
    pub fn new<I: Fn(usize) -> Element>(_bound_device: &crate::images::BoundDevice, _size: usize, _write_frequency: WriteFrequency, _cpu_strategy: CPUStrategy, _debug_name: &str, _initialize_with: I) -> Vec<Self> {
        todo!()
    }
    pub fn write(&mut self, _index: usize, _element: Element) {
        todo!()
    }
}
#[derive(Debug,Clone)]
pub struct Delivery;
