// /*!
// A VTexture that consists concretely of a simplex noise Sum type.
// */
// use crate::bindings::software::texture::Texel;
// use crate::bindings::software::texture::vtexture::VTexture;
// use crate::pixel_formats::R32Float;

// #[derive(Debug,Clone)]
// pub struct VNoise<const LENGTH: usize> {
//     sum: MultiNoise<LENGTH>,
//     width: u16,
//     height: u16
// }
// impl<const LENGTH: usize> VNoise<LENGTH> {
//     pub const fn new(sum: MultiNoise<LENGTH>, width: u16, height: u16) -> Self {
//         Self {
//             sum, width,height
//         }
//     }
// }
//
// impl<const LENGTH: usize> VTexture<R32Float> for VNoise<LENGTH> {
//     fn width(&self) -> u16 {
//         self.width
//     }
//
//     fn height(&self) -> u16 {
//         self.height
//     }
//
//     fn read(&self, texel: Texel) -> f32 {
//         self.sum.generate(texel.x as f32, texel.y as f32)
//     }
// }
