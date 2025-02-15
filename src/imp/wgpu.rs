use std::fmt::Display;
use std::marker::PhantomData;
use std::sync::Arc;
use crate::bindings::forward::dynamic::buffer::WriteFrequency;
use crate::bindings::sampler::SamplerType;
use crate::bindings::visible_to::{CPUStrategy, TextureUsage};
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::PassTrait;
use crate::images::view::View as CrateView;
use crate::pixel_formats::PixelFormat;
use crate::{Priority};

mod entry_point;
mod unbound_device;
mod view;
mod error;
mod bound_device;

pub use entry_point::EntryPoint;
pub use unbound_device::UnboundDevice;
pub use view::View;
pub(crate) use error::Error;
pub use bound_device::BoundDevice;


#[derive(Debug)]
pub struct Port;

impl Port {
    pub(crate) fn new(_engine: &Arc<crate::images::Engine>, _view: CrateView, _camera: Camera, _port_reporter_send:PortReporterSend) -> Result<Self,Error> {
        todo!()
    }
}

impl Port {
    pub async fn add_fixed_pass<const N: usize, P: PassTrait<N>>(&mut self, _p: P) -> P::DescriptorResult {
        todo!()
    }
    pub async fn start(&mut self) -> Result<(),Error> {
        todo!()
    }
}

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


#[derive(Debug)]
pub struct Engine;
impl Engine {
    pub async fn rendering_to_view(_bound_device: &Arc<crate::images::BoundDevice>) -> Self {
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
#[derive(Debug)]
pub struct Texture<Format>(Format);
impl<Format: PixelFormat> Texture<Format> {
    pub async fn new(_bound_device: &crate::images::BoundDevice, _width: u16, _height: u16, _visible_to: TextureUsage, _data: &[Format::CPixel], _debug_name: &str, _priority: Priority) -> Result<Self, Error> {
        todo!()
    }
}