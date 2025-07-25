// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::pixel_formats::RGBA8UnormSRGB;
use png::{BitDepth, ColorType};

/**
# Safety

The methods must be implemented correctly to ensure that we can safely use the pixel format with PNG encoding and decoding.
*/
pub unsafe trait PngPixelFormat {
    fn png_color_type() -> png::ColorType;
    fn png_bit_depth() -> png::BitDepth;
}

unsafe impl PngPixelFormat for RGBA8UnormSRGB {
    fn png_color_type() -> ColorType {
        ColorType::Rgba
    }

    fn png_bit_depth() -> BitDepth {
        BitDepth::Eight
    }
}
