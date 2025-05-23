/*! Defines binding types */


pub mod bind_style;
pub mod forward;

pub use bind_style::BindStyle;
pub mod visible_to;
pub mod coordinates;
pub mod software;
pub mod buffer_types;
pub(crate) mod buffer_access;
pub mod resource_tracking;
pub(crate) mod dirty_tracking;
pub mod sampler;
