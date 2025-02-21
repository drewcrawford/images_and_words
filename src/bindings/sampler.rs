/*! Cross-platform sampler type. */
use std::fmt::{Debug, Display};
use std::sync::Arc;
use crate::images::device::BoundDevice;
use crate::imp;



#[derive(Debug)]
pub enum SamplerType {
    ///The sampler shall use pixel coordinates and do linear interpolation.
    ///
    /// On vulkan, this means we must specify an explicit Lod.
    PixelLinear,
    ///The sampler shall use normalized coordinates, and will do interpolation for mipmapping.
    Mipmapped,
}


