use std::fmt::Display;
use std::marker::PhantomData;
use std::sync::Arc;
use crate::bindings::forward::dynamic::buffer::WriteFrequency;
use crate::bindings::sampler::SamplerType;
use crate::bindings::visible_to::{CPUStrategy, TextureUsage};
use crate::images::camera::Camera;
use crate::images::port::PortReporterSend;
use crate::images::render_pass::PassTrait;
use crate::images::view::View;
use crate::pixel_formats::PixelFormat;
use crate::{Priority};

mod entry_point;

pub use entry_point::EntryPoint;


pub struct UnboundDevice;

impl UnboundDevice {
    pub(crate) fn surface_strategy(&self) -> &SurfaceStrategy {
        todo!()
    }
}

impl UnboundDevice {
    pub fn pick(_surface: &crate::images::surface::Surface, _entry_point: &crate::entry_point::EntryPoint) -> Result<UnboundDevice,Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct Surface;

impl Surface {
    pub(crate) fn new(_p0: View, _p1: &Arc<crate::entry_point::EntryPoint>) -> Result<Self,Error>{
        todo!()
    }
}

#[derive(Debug)]
pub struct Port;

impl Port {
    pub(crate) fn new(_engine: &Arc<crate::images::Engine>, _surface: crate::images::surface::Surface, _initial_surface_strategy: crate::images::surface::SurfaceStrategy, _camera: Camera, _port_reporter_send:PortReporterSend) -> Result<Self,Error> {
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
pub struct BoundDevice;

impl BoundDevice {
    pub(crate) fn bind(_unbound_device: crate::images::device::UnboundDevice, _entry_point: Arc<crate::entry_point::EntryPoint>)-> Result<Arc<Self>,Error>{
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