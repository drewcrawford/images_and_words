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
mod sampler;
mod buffer;

pub use entry_point::EntryPoint;
pub use unbound_device::UnboundDevice;
pub use view::View;
pub(crate) use error::Error;
pub use bound_device::BoundDevice;
pub use engine::Engine;
pub use port::Port;
pub use pixel_format::PixelFormat;
pub use texture::Texture;
pub use sampler::Sampler;
pub use buffer::MappableBuffer;









#[derive(Clone)]
pub struct SurfaceStrategy;
#[derive(Debug)]
pub struct FrameTextureProduct<Format>(PhantomData<Format>);
#[derive(Debug,Clone)]
pub struct FrameTextureDelivery;

#[derive(Debug,Clone)]
pub struct Delivery;
