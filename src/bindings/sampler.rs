/*! Cross-platform sampler type. */
use std::fmt::Debug;



#[derive(Debug,Clone,Copy)]
pub enum SamplerType {
    ///The sampler shall use pixel coordinates and do linear interpolation.
    ///
    /// On vulkan, this means we must specify an explicit Lod.
    PixelLinear,
    ///The sampler shall use normalized coordinates, and will do interpolation for mipmapping.
    Mipmapped,
}


