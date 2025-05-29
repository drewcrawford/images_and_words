#[derive(Debug,Clone,Copy)]
pub enum SamplerType {
    ///The sampler shall use normalized coordinates, and will do interpolation for mipmapping.
    Mipmapped,
}