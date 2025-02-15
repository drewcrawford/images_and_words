/*! Cross-platform sampler type. */
use std::fmt::{Debug, Display};
use std::sync::Arc;
use crate::images::device::BoundDevice;
use crate::imp;

#[derive(Debug,thiserror::Error)]
pub struct Error(#[from] imp::Error);

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sampler error: {}", self.0)
    }
}


pub enum SamplerType {
    ///The sampler shall use pixel coordinates and do linear interpolation.
    ///
    /// On vulkan, this means we must specify an explicit Lod.
    PixelLinear,
    ///The sampler shall use normalized coordinates, and will do interpolation for mipmapping.
    Mipmapped,
}

#[derive(Debug)]
pub struct Sampler(pub(crate) imp::Sampler);
impl Sampler {
    pub fn new(device: &Arc<BoundDevice>, coordinate_type: SamplerType) -> Result<Self,Error> {
        Ok(Self(crate::imp::Sampler::new(device, coordinate_type)?))
    }
}
