// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*!
A VTexture is a trait that allows random reading of [Texel]s.

This is most commonly, some texture.  But it can also be a "virtual" texture,
such as a function that generates pixel values.

# Examples

## Basic usage with a texture

```
use images_and_words::bindings::software::texture::{Texture, Texel};
use images_and_words::bindings::software::texture::vtexture::VTexture;
use images_and_words::pixel_formats::R8UNorm;

let mut texture = Texture::<R8UNorm>::new(4, 4, 0u8);
// Fill with a gradient
for y in 0..4 {
    for x in 0..4 {
        texture[Texel { x, y }] = ((x + y) * 32) as u8;
    }
}

// Read using VTexture trait
let value = texture.read(Texel { x: 2, y: 1 });
assert_eq!(value, 96);
```

## Implementing VTexture for a virtual texture

```
# use images_and_words::bindings::software::texture::{Texel};
# use images_and_words::bindings::software::texture::vtexture::VTexture;
# use images_and_words::pixel_formats::R32Float;

/// A virtual texture that generates a checkerboard pattern
struct CheckerboardTexture {
    size: u16,
    scale: u16,
}

impl VTexture<R32Float> for CheckerboardTexture {
    fn width(&self) -> u16 {
        self.size
    }

    fn height(&self) -> u16 {
        self.size
    }

    fn read(&self, texel: Texel) -> f32 {
        let x_cell = texel.x / self.scale;
        let y_cell = texel.y / self.scale;
        if (x_cell + y_cell) % 2 == 0 {
            1.0
        } else {
            0.0
        }
    }
}

let checker = CheckerboardTexture { size: 256, scale: 32 };
assert_eq!(checker.read(Texel { x: 0, y: 0 }), 1.0);
assert_eq!(checker.read(Texel { x: 32, y: 0 }), 0.0);
assert_eq!(checker.read(Texel { x: 64, y: 64 }), 1.0);
```
 */

use crate::bindings::software::texture::Texel;
use crate::pixel_formats::sealed::PixelFormat;

/// A trait for types that can be read as textures.
///
/// VTexture provides a uniform interface for reading pixel values from various sources.
/// This includes both regular [`Texture`](crate::bindings::software::texture::Texture)s
/// and "virtual" textures that generate pixel values procedurally.
///
/// # Type Parameters
///
/// * `Format` - The pixel format of the texture, determining the type of pixel values returned
pub trait VTexture<Format: PixelFormat> {
    /// Returns the width of the texture in texels.
    ///
    /// # Examples
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::Texture;
    /// use images_and_words::pixel_formats::R8UNorm;
    ///
    /// let texture = Texture::<R8UNorm>::new(100, 50, 0u8);
    /// // Texture implements VTexture, so width() is available
    /// assert_eq!(texture.width(), 100);
    /// ```
    fn width(&self) -> u16;

    /// Returns the height of the texture in texels.
    ///
    /// # Examples
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::Texture;
    /// use images_and_words::pixel_formats::R8UNorm;
    ///
    /// let texture = Texture::<R8UNorm>::new(100, 50, 0u8);
    /// // Texture implements VTexture, so height() is available
    /// assert_eq!(texture.height(), 50);
    /// ```
    fn height(&self) -> u16;

    /// Reads a pixel value at the specified texel coordinates.
    ///
    /// The behavior for out-of-bounds coordinates is implementation-defined.
    /// Regular [`Texture`](crate::bindings::software::texture::Texture)s will panic,
    /// while virtual textures might wrap, clamp, or generate values for any coordinate.
    ///
    /// # Arguments
    ///
    /// * `texel` - The integer texture coordinates to read from
    ///
    /// # Returns
    ///
    /// The pixel value at the specified coordinates
    ///
    /// # Examples
    ///
    /// ```
    /// # use images_and_words::bindings::software::texture::{Texture, Texel};
    /// # use images_and_words::bindings::software::texture::vtexture::VTexture;
    /// # use images_and_words::pixel_formats::RGBA32Float;
    /// # use images_and_words::pixel_formats::Float4;
    /// #
    /// let mut texture = Texture::<RGBA32Float>::new(2, 2, Float4 { r: 0.0, g: 0.0, b: 0.0, a: 1.0 });
    /// texture[Texel { x: 1, y: 0 }] = Float4 { r: 1.0, g: 0.5, b: 0.25, a: 1.0 };
    ///
    /// let color = texture.read(Texel { x: 1, y: 0 });
    /// assert_eq!(color.r, 1.0);
    /// assert_eq!(color.g, 0.5);
    /// ```
    fn read(&self, texel: Texel) -> Format::CPixel;
}

impl<Format: PixelFormat> VTexture<Format> for crate::bindings::software::texture::Texture<Format>
where
    Format::CPixel: Clone,
{
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
