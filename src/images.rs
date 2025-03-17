/*! The rendering component of images_and_words */

pub use engine::Engine;


pub mod render_pass;

pub(crate) mod device;
pub(crate) mod engine;

pub mod port;

pub use port::{PassClient, StaticTextureTicket};

pub(crate) mod camera;
pub mod shader;
pub mod view;
pub mod projection;
mod frame;
pub mod index_algorithms;
pub mod vertex_layout;

pub use device::BoundDevice;



