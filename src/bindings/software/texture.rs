/*! Software textures. */

use std::ops::{Index, IndexMut};
use std::path::Path;
use some_executor::hint::Hint;
use vec_parallel::Strategy;
use crate::bindings::software::texture::scaled_32::Scaled32;
use crate::bindings::software::texture::vtexture::VTexture;
use crate::pixel_formats::{Float4};
use crate::pixel_formats::png_support::PngPixelFormat;
use crate::pixel_formats::sealed::PixelFormat;

pub mod scaled_iterator;
pub mod scaled_row_cell;
pub mod scaled_32;
pub mod vtexture;
pub mod vnoise;

/**
This will correctly-encode a floating-point color value to SRGB.

This function handles extended values; they will be correctly extended into SRGB space.
*/
#[inline] pub fn linear_to_srgb(linear: f32) -> f32 {
  if linear <= 0.0031308 {
      12.92 * linear
  }
    else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

/**
A software (that is, emulated) texture.  Provides a texture-like experience on the CPU and in CPU memory.

This is similar to a 2D array, but it provides nice, type-safe implementations of index in various coordinate systems
and also supports things like sampling.

The software texture also uses an internal format appropriate for cheap conversion to hardware textures, which
may not be the case for arbitrary 2D arrays.  Although this conversion is cheap, it is not free; in particular
we don't try to allocate GPU or shared memory.

However, in cases where you are building up to a static, forward, GPU texture, which is the common case.
In reality the GPU will want some private format that you don't know (such as a compressed format).  So for
these cases, working in CPU memory "in the right order" is essentially the correct course of action.
*/
#[derive(Debug)]
pub struct Texture<Format: PixelFormat> {
    data: Vec<Format::CPixel>,
    width: u16,
    height: u16,
}

#[derive(Copy,Clone,PartialEq,Debug)]
pub struct Texel {
    pub x: u16,
    pub y: u16
}
impl Texel {
    pub const ZERO: Texel = Texel{x: 0, y: 0};
    const fn vec_offset(&self, width: u16) -> usize {
        width as usize * self.y as usize + self.x as usize
    }

    const fn from_vec_offset(width: u16, offset: usize) -> Texel {
        let y = offset / width as usize;
        let x = offset % width as usize;
        Texel{x: x as u16, y: y as u16 }
    }
    /**
    Creates a new texel, clamping to the specified width and height (and 0,0).
     * self: Input texel
     * dx: Shift in x direction
     * dy: Shift in y direction
     * width: max width of texture, will clamp to width-1
     * height: max height of texture, will clamp to height-1

     inlined because in practice the callsite probably uses constants
*/
    #[inline] pub const fn new_clamping(self,dx: i8,dy: i8,width:u16,height:u16) -> Self {
        let new_x = if dx >= 0 {
            let r = self.x.saturating_add(dx as u16);
            if r >= width {
                width - 1
            }
            else {
                r
            }
        }
        else {
            self.x.saturating_sub(-dx as u16)
        };
        let new_y = if dy >= 0 {
            let r = self.y.saturating_add(dy as u16);
            if r >= height {
                height - 1
            }
            else {
                r
            }
        }
        else {
            self.y.saturating_sub(-dy as u16)
        };
        Self { x: new_x, y: new_y}
    }
}

///Normalized coordinates from 0-1.
///
/// Keep in mind that we wll sample an equal amount around this region, so you potentially want to offset your normalized
/// coordinate by half (some size)
#[derive(Copy,Clone,Debug)]
pub struct Normalized {
    pub x: f32,
    pub y: f32
}
impl Normalized {
    pub fn new(x: f32, y: f32) -> Self {
        assert!(x >= 0.0 && x <= 1.0 && y >= 0.0 && y <= 1.0);
        Self {
            x, y
        }
    }
    pub const fn x(&self) -> f32 {
        self.x
    }
    pub const fn y(&self) -> f32 {
        self.y
    }
    
    #[inline] pub fn new_clamping(x: f32, y: f32) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0)
        }
    }

    #[inline] pub fn clamped_offset(self, dx: f32, dy: f32) -> Normalized {
        Normalized {
            x: (self.x + dx).clamp(0.0,1.0),
            y: (self.y + dy).clamp(0.0, 1.0)
        }
    }
}

///Pixel type that we can sample.
pub trait Sampleable: Sized + Clone {
    ///The output of the sample.  Usually, a floating point type.
    type Sampled;
    ///Calulate a weighted average.
    fn avg(elements: &[(f32,Self)]) -> Self::Sampled;
}
impl Sampleable for f32 {
    type Sampled = f32;

    fn avg(elements: &[(f32, Self)]) -> Self::Sampled {
        let mut avg = 0.0;
        for element in elements {
            avg += element.0 * element.1
        }
        avg
    }
}

pub trait Round<Output> {
    fn round(&self) -> Output;
}
impl Round<i32> for f32 {
    fn round(&self) -> i32 {
        f32::round(*self) as i32
    }
}

impl Sampleable for i32 {
    type Sampled = f32;

    fn avg(elements: &[(f32, Self)]) -> Self::Sampled {
        let mut avg = 0.0;
        for element in elements {
            avg += element.0 * element.1 as f32
        }
        avg
    }
}

impl Sampleable for Float4 {
    type Sampled = Float4;

    fn avg(elements: &[(f32, Self)]) -> Self::Sampled {
        let mut avg = Float4::default();
        for element in elements {
            avg.r += element.0 * element.1.r;
            avg.g += element.0 * element.1.g;
            avg.b += element.0 * element.1.b;
            avg.a += element.0 * element.1.a;

        }
        avg
    }
}

impl<Format: PixelFormat> Texture<Format> {
    /**Initializes the texture, setting all elements to the value specified.*/
    pub fn new(width: u16, height: u16, initialize_element: Format::CPixel)  -> Self where Format::CPixel : Clone {
        let mut vec = Vec::with_capacity(width as usize * height as usize);
        for _ in 0..(width as u32*height as u32) {
            vec.push(initialize_element.clone())
        }
        Self {
            width,
            height,
            data: vec
        }
    }
    /**
    Initializes a new texture, setting all elements to the value specified. */
    pub fn new_with<F: Fn(Texel) -> Format::CPixel>(width: u16, height: u16, initialize_with: F)  -> Self  {
        let mut vec = Vec::with_capacity(width as usize * height as usize);
        for y in 0..height {
            for x in 0..width {
                vec.push(initialize_with(Texel{x,y}))
            }
        }
        Self {
            width,
            height,
            data: vec
        }
    }
    /**
    Parallel version of [Self::new_with]
    */
    pub async fn new_with_parallel<F: Fn(Texel) -> Format::CPixel + Sync + Clone + Send + 'static>(width: u16, height: u16, priority: some_executor::Priority, strategy: Strategy, initialize_with: F) -> Self {
        let len = width as usize * height as usize;
        let build_vec = vec_parallel::build_vec(len, strategy, move |index | {
            let t = Texel::from_vec_offset(width, index);
            initialize_with(t)
        });
        let mut clone_box = some_executor::current_executor::current_executor();

        let f = build_vec.spawn_on(&mut clone_box, priority, Hint::CPU);

        let vec = f.await;
        Self {
            width,
            height,
            data: vec,
        }
    }
    /**
    Creates a new soft-texture from an asset at the specified path.
*/
    pub async fn new_from_path(path: &Path, priority: async_file::Priority) -> Self where Format: PngPixelFormat, Format::CPixel: std::fmt::Debug {
        let file = async_file::File::open(path, priority).await.unwrap();
        let data = file.read_all(priority).await.unwrap();

        println!("read {} bytes",data.len());
        let decoder = png::Decoder::new(&*data);
        let mut reader = decoder.read_info().unwrap();
        println!("will decode {}x{}",reader.info().width, reader.info().height);
        //allocate an output buffer that is correctly-aligned
        let vec_capacity = reader.info().width as usize * reader.info().height as usize;
        let mut buf = Vec::<Format::CPixel>::with_capacity(vec_capacity);
        let num_bytes = buf.capacity() * std::mem::size_of::<Format::CPixel>();
        assert!(num_bytes >= reader.output_buffer_size());

        assert!(reader.info().color_type == Format::png_color_type());
        assert!(reader.info().bit_depth == Format::png_bit_depth());
        //get a slice to the raw bytes
        let byte_slice = unsafe{std::slice::from_raw_parts_mut(buf.as_mut_slice().as_mut_ptr() as *mut u8, num_bytes)};
        let info = reader.next_frame(byte_slice).unwrap();
        let actual_elements = info.width as usize * info.height as usize;
        unsafe{buf.set_len(actual_elements)};

        // let pixel = &buf[85];
        // println!("buf {pixel:?}");
        Self {
            data: buf,
            width: info.width.try_into().unwrap(),
            height: info.height.try_into().unwrap(),
        }
    }

    /**Creates a new texture by copying the provided texture. */
    pub fn new_cloning(cloning: &impl VTexture<Format>) -> Self {
        let width = cloning.width();
        let height = cloning.height();
        let mut vec = Vec::with_capacity(width as usize * height as usize);
        for y in 0..height {
            for x in 0..width {
                vec.push(cloning.read(Texel{x,y}))
            }
        }
        Self {
            width,
            height,
            data: vec
        }
    }
    #[inline] pub fn width(&self) -> u16 {
        self.width
    }
    #[inline] pub fn height(&self) -> u16 {
        self.height
    }

    ///The texture data, in a layout suitable for creating a GPU texture.
    ///
    /// On Metal, this is stored in Y-major, X-minor form, where y:0 is up, y:inf is down,
    /// x:0 is left, x:inf is right.
    #[inline] pub(crate) fn texture_data(&self) -> &[Format::CPixel] {
        &self.data
    }

    pub fn map<F: Fn(&Format::CPixel) -> T::CPixel, T: PixelFormat>(&self, mapfn: F) -> Texture<T> where T::CPixel: Default + Clone + std::fmt::Debug {
        let new = Texture::new_with(self.width,self.height, |texel| {
            let ours = &self[texel];
            let theirs = mapfn(ours);
            theirs
        });
        new
    }


    /**
    Dumps to a file on disk, in our internal format.

    This is primarily useful for debugging...
*/
    pub fn dump_c_to(&self, path: &std::path::Path) where Format: PixelFormat{
        let u8_slice: &[u8] = unsafe {
            std::slice::from_raw_parts(self.texture_data() as *const _ as *const u8, self.texture_data().len() * std::mem::size_of::<Format::CPixel>())
        };
        std::fs::write(path, u8_slice).unwrap()
    }
}

///Bilinear sample on row/cell coordinates
pub fn sample_bilinear<T: VTexture<Format>, Format: PixelFormat>(texture: &T, scaled: Scaled32) -> <<Format as PixelFormat>::CPixel as Sampleable>::Sampled where Format::CPixel: Sampleable {
    let base_x = scaled.cell_i();
    let base_y = scaled.cell_j();
    let w11 = (1.0 - base_x) * (1.0 - base_y); //0,0
    let w12 = (1.0 - base_x) * base_y; //0,1
    let w21 = base_x * (1.0 - base_y); //1,0
    let w22 = base_x * base_y; //1,1
    let down = scaled.row() + 1;
    let right = scaled.cell() + 1;
    let c11 = Texel{x: scaled.cell(), y: scaled.row()}; //0,0
    let c12 = Texel{x: scaled.cell(), y: down}; //0,1
    let c21 = Texel{x: right, y: scaled.row()}; //1,0
    let c22 = Texel{x: right, y: down};
    let v11 = texture.read(c11).clone();
    let v12 = texture.read(c12).clone();
    let v21 = texture.read(c21).clone();
    let v22 = texture.read(c22).clone();
    Format::CPixel::avg(&[(w11,v11),(w12,v12),(w21,v21),(w22,v22)])
}

impl<Format: PixelFormat> Texture<Format> where Format::CPixel: Into<tgar::PixelBGRA> + Clone{
    ///Dumps the texture into a TGA format.
    pub fn dump_tga(&self) -> tgar::BGRA {
        let mut vec = Vec::with_capacity(self.width as usize * self.height as usize);
        for y in 0..self.height {
            for x in 0..self.width {
                let read_px = self[Texel{x,y}].clone();
                let converted_px = read_px.into();
                vec.push(converted_px);
            }
        }
        tgar::BGRA::new(self.width as u16, self.height as u16, &vec)
    }
}

impl<Format: PixelFormat> Index<Texel> for Texture<Format> {
    type Output = Format::CPixel;

    fn index(&self, index: Texel) -> &Self::Output {
        assert!(index.x < self.width && index.y < self.height);
        /*
        I have benched this to be faster than going through index, in both debug and release builds.

        The mechanism is unknown to me, it does not appear to be assertions as that is checked separately.

        The only downside I know of is safety, but we checked above that we are in bounds, so everything 'should' be fine
        (famous last words)
         */
        let ptr = self.data.as_ptr();
        let index = index.vec_offset(self.width);
        unsafe{&* ptr.add(index)}
    }
}
impl<Format: PixelFormat> IndexMut<Texel> for Texture<Format> {
    fn index_mut(&mut self, index: Texel) -> &mut Self::Output {
        assert!(index.x < self.width && index.y < self.height);
        &mut self.data[index.vec_offset(self.width)]
    }
}