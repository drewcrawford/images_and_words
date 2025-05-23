use png::{BitDepth, ColorType};
use crate::pixel_formats::{RGBA8UnormSRGB};

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

#[cfg(test)] mod tests {
    use std::env::current_dir;
    use std::fs::File;
    use std::io::Write;
    use crate::bindings::software::texture::Texture;
    use crate::pixel_formats::{RGBA8UnormSRGB};


}