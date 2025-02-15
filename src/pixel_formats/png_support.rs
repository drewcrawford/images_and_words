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

    #[test] fn load_png() {
        let mut path = current_dir().unwrap();
        path.pop();
        path.push("petrucci/assets/road.png");
        let fut = Texture::<RGBA8UnormSRGB>::new_from_path(&path, async_file::Priority::unit_test());
        let soft_texture = test_executors::spin_on(fut);

        //dump to output?
        let mut file = File::create("test_load_png.tga").unwrap();
        file.write(&soft_texture.dump_tga().into_data()).unwrap();
    }
}