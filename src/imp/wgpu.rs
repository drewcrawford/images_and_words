use std::marker::PhantomData;

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
pub use texture::{GPUableTexture, MappableTexture};
pub use texture::RenderSide as TextureRenderSide;
pub use buffer::{MappableBuffer, GPUableBuffer, CopyInfo};









#[derive(Clone)]
pub struct SurfaceStrategy;
#[derive(Debug)]
pub struct FrameTextureProduct<Format>(PhantomData<Format>);
#[derive(Debug,Clone)]
pub struct FrameTextureDelivery;

#[derive(Debug,Clone)]
pub struct Delivery;
