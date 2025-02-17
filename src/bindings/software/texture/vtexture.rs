/*!
A VTexture is a trait that allows random reading of [Texel]s.

This is most commonly, some texture.  But it can also be a "virtual" texture,
such as a function that generates pixel values.
 */

use crate::bindings::software::texture::{Texel};
use crate::pixel_formats::sealed::PixelFormat;

pub trait VTexture<Format: PixelFormat> {
    fn width(&self) -> u16;
    fn height(&self) -> u16;

    fn read(&self,texel: Texel) -> Format::CPixel;
}

impl<Format: PixelFormat> VTexture<Format> for crate::bindings::software::texture::Texture<Format> where Format::CPixel: Clone {
    fn width(&self) -> u16 {
        Self::width(self)
    }
    fn height(&self) -> u16 {
        Self::height(self)
    }
    fn read(&self, texel: Texel) -> Format::CPixel {
        self[texel].clone()
    }
}