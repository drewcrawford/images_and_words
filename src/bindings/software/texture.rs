/*! Software texture implementation for CPU-based image processing.

This module provides a software implementation of textures that runs entirely on the CPU.
It's designed to provide a texture-like experience with type-safe coordinate systems,
sampling operations, and efficient conversion to GPU textures.

# Overview

The software texture system is useful for:
- Building textures on the CPU before uploading to GPU
- Image processing operations that don't require GPU acceleration
- Debugging and testing texture-related code
- Working with texture data in formats optimized for GPU upload

# Key Types

- [`Texture`] - The main software texture type, a 2D array with texture-like operations
- [`Texel`] - Integer texture coordinates (x, y)
- [`Normalized`] - Normalized coordinates in the range [0, 1]
- [`Sampleable`] - Trait for types that can be sampled with filtering

# Coordinate Systems

This module uses the following coordinate conventions:
- Origin (0, 0) is at the top-left
- X increases to the right
- Y increases downward
- Normalized coordinates map [0, 1] to the full texture dimensions

# Example

```
use images_and_words::bindings::software::texture::{Texture, Texel};
use images_and_words::pixel_formats::R8UNorm;

// Create a 4x4 grayscale texture
let mut texture = Texture::<R8UNorm>::new(4, 4, 128u8);

// Write a value at specific coordinates
texture[Texel { x: 1, y: 2 }] = 255u8;

// Read the value back
let value = texture[Texel { x: 1, y: 2 }];
assert_eq!(value, 255u8);
```
*/

use std::ops::{Index, IndexMut};
use std::path::Path;
use some_executor::hint::Hint;
use crate::Strategy;
use crate::bindings::software::texture::scaled_32::Scaled32;
use crate::bindings::software::texture::vtexture::VTexture;
use crate::pixel_formats::{Float4};
use crate::pixel_formats::png_support::PngPixelFormat;
use crate::pixel_formats::sealed::PixelFormat;

pub mod scaled_iterator;
pub mod scaled_row_cell;
pub mod scaled_32;
pub mod vtexture;

/// Converts a linear color value to sRGB color space.
///
/// This function implements the standard sRGB transfer function, which applies
/// gamma correction to convert from linear light values to perceptually uniform
/// sRGB values. It correctly handles extended range values (values outside [0, 1]).
///
/// # Arguments
///
/// * `linear` - The linear color value to convert
///
/// # Returns
///
/// The sRGB-encoded color value
///
/// # Algorithm
///
/// The sRGB transfer function is:
/// - For values ≤ 0.0031308: `12.92 * linear`
/// - For values > 0.0031308: `1.055 * linear^(1/2.4) - 0.055`
///
/// # Examples
///
/// ```
/// use images_and_words::bindings::software::texture::linear_to_srgb;
///
/// // Dark values use linear scaling
/// let dark = linear_to_srgb(0.002);
/// assert!((dark - 0.02584).abs() < 0.0001);
///
/// // Bright values use gamma correction
/// let bright = linear_to_srgb(0.5);
/// assert!((bright - 0.7353569).abs() < 0.0001);
///
/// // Pure white remains white
/// let white = linear_to_srgb(1.0);
/// assert!((white - 1.0).abs() < 0.0001);
/// ```
#[inline] pub fn linear_to_srgb(linear: f32) -> f32 {
  if linear <= 0.0031308 {
      12.92 * linear
  }
    else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

/// A software texture that provides GPU-like texture operations on the CPU.
///
/// `Texture` is a 2D array of pixels with additional functionality for texture-like
/// operations including:
/// - Type-safe indexing with various coordinate systems
/// - Sampling with filtering
/// - Efficient memory layout for GPU upload
/// - Parallel initialization support
///
/// # Type Parameters
///
/// * `Format` - The pixel format, which must implement `PixelFormat` (available through some `imp::` path)
///
/// # Memory Layout
///
/// The texture data is stored in row-major order (Y-major, X-minor), which is
/// optimal for GPU texture upload. The origin (0, 0) is at the top-left corner.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```
/// use images_and_words::bindings::software::texture::{Texture, Texel};
/// use images_and_words::pixel_formats::R8UNorm;
///
/// // Create a texture filled with a single value
/// let texture = Texture::<R8UNorm>::new(256, 256, 128u8);
///
/// // Access dimensions
/// assert_eq!(texture.width(), 256);
/// assert_eq!(texture.height(), 256);
/// ```
///
/// ## Creating with a Function
///
/// ```
/// use images_and_words::bindings::software::texture::{Texture, Texel};
/// use images_and_words::pixel_formats::R8UNorm;
///
/// // Create a gradient texture
/// let texture = Texture::<R8UNorm>::new_with(256, 256, |texel| {
///     // Create a horizontal gradient
///     (texel.x as f32 / 255.0 * 255.0) as u8
/// });
/// ```
#[derive(Debug)]
pub struct Texture<Format: PixelFormat> {
    data: Vec<Format::CPixel>,
    width: u16,
    height: u16,
}

/// Integer texture coordinates representing a specific pixel location.
///
/// `Texel` uses 16-bit unsigned integers for coordinates, supporting textures
/// up to 65535x65535 pixels. The origin (0, 0) is at the top-left corner.
///
/// # Examples
///
/// ```
/// use images_and_words::bindings::software::texture::Texel;
///
/// // Create a texel at position (10, 20)
/// let texel = Texel { x: 10, y: 20 };
///
/// // Use the ZERO constant for the origin
/// let origin = Texel::ZERO;
/// assert_eq!(origin.x, 0);
/// assert_eq!(origin.y, 0);
/// ```
#[derive(Copy,Clone,PartialEq,Debug)]
pub struct Texel {
    /// X coordinate (horizontal position)
    pub x: u16,
    /// Y coordinate (vertical position)
    pub y: u16
}
impl Texel {
    /// The origin texel at coordinates (0, 0).
    pub const ZERO: Texel = Texel{x: 0, y: 0};
    /// Converts texel coordinates to a linear array index.
    ///
    /// Used internally for indexing into the texture's data array.
    const fn vec_offset(&self, width: u16) -> usize {
        width as usize * self.y as usize + self.x as usize
    }

    /// Converts a linear array index back to texel coordinates.
    ///
    /// Used internally for parallel initialization.
    const fn from_vec_offset(width: u16, offset: usize) -> Texel {
        let y = offset / width as usize;
        let x = offset % width as usize;
        Texel{x: x as u16, y: y as u16 }
    }
    /// Creates a new texel by offsetting this texel and clamping to texture bounds.
    ///
    /// This method is useful for neighbor access patterns where you want to ensure
    /// the resulting coordinates stay within the texture boundaries.
    ///
    /// # Arguments
    ///
    /// * `dx` - Offset in the X direction (can be negative)
    /// * `dy` - Offset in the Y direction (can be negative)
    /// * `width` - Texture width (clamps to width-1)
    /// * `height` - Texture height (clamps to height-1)
    ///
    /// # Returns
    ///
    /// A new `Texel` with coordinates clamped to [0, width-1] × [0, height-1]
    ///
    /// # Examples
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::Texel;
    ///
    /// let texel = Texel { x: 5, y: 5 };
    ///
    /// // Move right by 2, down by 1
    /// let neighbor = texel.new_clamping(2, 1, 10, 10);
    /// assert_eq!(neighbor.x, 7);
    /// assert_eq!(neighbor.y, 6);
    ///
    /// // Clamping at boundaries
    /// let edge = Texel { x: 9, y: 9 };
    /// let clamped = edge.new_clamping(5, 5, 10, 10);
    /// assert_eq!(clamped.x, 9); // Clamped to width-1
    /// assert_eq!(clamped.y, 9); // Clamped to height-1
    /// ```
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

/// Normalized texture coordinates in the range [0, 1].
///
/// Normalized coordinates map the entire texture to a unit square, where:
/// - (0, 0) corresponds to the top-left corner
/// - (1, 1) corresponds to the bottom-right corner
///
/// When sampling, remember that the sampling kernel extends around the coordinate,
/// so you may want to offset by half a texel for pixel-perfect sampling.
///
/// # Examples
///
/// ```
/// use images_and_words::bindings::software::texture::Normalized;
///
/// // Create normalized coordinates at the center
/// let center = Normalized::new(0.5, 0.5);
///
/// // Create with clamping (values outside [0,1] are clamped)
/// let clamped = Normalized::new_clamping(1.5, -0.2);
/// assert_eq!(clamped.x(), 1.0);
/// assert_eq!(clamped.y(), 0.0);
/// ```
#[derive(Copy,Clone,Debug)]
pub struct Normalized {
    /// X coordinate in range [0, 1]
    pub x: f32,
    /// Y coordinate in range [0, 1]
    pub y: f32
}
impl Normalized {
    /// Creates new normalized coordinates.
    ///
    /// # Panics
    ///
    /// Panics if x or y are outside the range [0, 1].
    pub fn new(x: f32, y: f32) -> Self {
        assert!((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y));
        Self {
            x, y
        }
    }
    /// Returns the X coordinate.
    pub const fn x(&self) -> f32 {
        self.x
    }
    /// Returns the Y coordinate.
    pub const fn y(&self) -> f32 {
        self.y
    }
    
    /// Creates new normalized coordinates, clamping values to [0, 1].
    ///
    /// Unlike [`new`](Self::new), this method doesn't panic on out-of-range values.
    #[inline] pub fn new_clamping(x: f32, y: f32) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0)
        }
    }

    /// Offsets the coordinates by the given amounts, clamping to [0, 1].
    ///
    /// # Arguments
    ///
    /// * `dx` - Offset to add to X coordinate
    /// * `dy` - Offset to add to Y coordinate
    #[inline] pub fn clamped_offset(self, dx: f32, dy: f32) -> Normalized {
        Normalized {
            x: (self.x + dx).clamp(0.0,1.0),
            y: (self.y + dy).clamp(0.0, 1.0)
        }
    }
}

/// Trait for pixel types that can be sampled with filtering.
///
/// This trait enables bilinear and other filtering operations on pixel data.
/// Types implementing this trait can compute weighted averages for smooth
/// interpolation between pixels.
///
/// # Examples
///
/// The trait is implemented for common pixel types:
///
/// ```
/// use images_and_words::bindings::software::texture::Sampleable;
///
/// // Float values average directly
/// let samples = [(0.5, 1.0f32), (0.5, 3.0f32)];
/// let avg = f32::avg(&samples);
/// assert_eq!(avg, 2.0);
///
/// // Integer values produce float results
/// let samples = [(0.25, 100i32), (0.75, 200i32)];
/// let avg = i32::avg(&samples);
/// assert_eq!(avg, 175.0);
/// ```
pub trait Sampleable: Sized + Clone {
    /// The output type of sampling operations.
    /// Usually a floating-point type for smooth interpolation.
    type Sampled;
    
    /// Calculates a weighted average of samples.
    ///
    /// # Arguments
    ///
    /// * `elements` - Slice of (weight, value) pairs where weights should sum to 1.0
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
    /// Creates a new texture with all pixels initialized to the same value.
    ///
    /// # Arguments
    ///
    /// * `width` - Texture width in pixels
    /// * `height` - Texture height in pixels
    /// * `initialize_element` - The value to fill all pixels with
    ///
    /// # Examples
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::Texture;
    /// use images_and_words::pixel_formats::R8UNorm;
    ///
    /// // Create a 64x64 texture filled with gray (128)
    /// let texture = Texture::<R8UNorm>::new(64, 64, 128u8);
    /// assert_eq!(texture.width(), 64);
    /// assert_eq!(texture.height(), 64);
    /// ```
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
    /// Creates a new texture with pixels initialized by a function.
    ///
    /// This allows creating textures with patterns, gradients, or other
    /// procedural content.
    ///
    /// # Arguments
    ///
    /// * `width` - Texture width in pixels
    /// * `height` - Texture height in pixels
    /// * `initialize_with` - Function that computes the pixel value for each texel
    ///
    /// # Examples
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::{Texture, Texel};
    /// use images_and_words::pixel_formats::R8UNorm;
    ///
    /// // Create a checkerboard pattern
    /// let texture = Texture::<R8UNorm>::new_with(64, 64, |texel| {
    ///     if (texel.x / 8 + texel.y / 8) % 2 == 0 {
    ///         255u8  // White
    ///     } else {
    ///         0u8    // Black
    ///     }
    /// });
    /// ```
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
    /// Creates a new texture with pixels initialized by a function, computed in parallel.
    ///
    /// This is useful for expensive per-pixel computations that can benefit from
    /// parallelization.
    ///
    /// # Arguments
    ///
    /// * `width` - Texture width in pixels
    /// * `height` - Texture height in pixels
    /// * `priority` - Execution priority for the parallel tasks
    /// * `strategy` - Parallelization strategy
    /// * `initialize_with` - Function that computes the pixel value for each texel
    ///
    /// # Examples
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::{Texture, Texel};
    /// use images_and_words::pixel_formats::{RGBA32Float, Float4};
    /// use images_and_words::{Priority, Strategy};
    /// 
    /// test_executors::sleep_on(async {
    ///     // Create a complex procedural texture in parallel
    ///     let texture = Texture::<RGBA32Float>::new_with_parallel(
    ///         512, 512, 
    ///         Priority::UserInitiated,
    ///         Strategy::One,
    ///         |texel| {
    ///             // Expensive per-pixel computation
    ///             let x = texel.x as f32 / 512.0;
    ///             let y = texel.y as f32 / 512.0;
    ///             Float4 {
    ///                 r: (x * y).sin(),
    ///                 g: (x - y).cos(),
    ///                 b: (x + y).sin() * 0.5 + 0.5,
    ///                 a: 1.0,
    ///             }
    ///         }
    ///     ).await;
    ///     
    ///     assert_eq!(texture.width(), 512);
    ///     assert_eq!(texture.height(), 512);
    /// });
    /// ```
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
    /// Loads a texture from a PNG file.
    ///
    /// The pixel format must support PNG loading through the `PngPixelFormat` trait.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the PNG file
    /// * `priority` - I/O priority for file reading
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - The file cannot be opened or read
    /// - The PNG format doesn't match the expected pixel format
    /// - The PNG is corrupted or invalid
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # //this is no_run due to file IO
    /// # async fn example() {
    /// use images_and_words::bindings::software::texture::Texture;
    /// use images_and_words::pixel_formats::RGBA8UnormSRGB;
    /// use std::path::Path;
    /// # let priority: async_file::Priority = todo!();
    ///
    /// let texture = Texture::<RGBA8UnormSRGB>::new_from_path(
    ///     Path::new("assets/texture.png"),
    ///     priority
    /// ).await;
    /// # }
    /// ```
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

    /// Creates a new texture by copying data from any type implementing `VTexture`.
    ///
    /// This is useful for converting between different texture representations
    /// or creating a concrete texture from a virtual/procedural one.
    ///
    /// # Arguments
    ///
    /// * `cloning` - The source texture to copy from
    ///
    /// # Examples
    ///
    /// ```
    /// # use images_and_words::bindings::software::texture::{Texture, Texel, vtexture::VTexture};
    /// # use images_and_words::pixel_formats::R8UNorm;
    /// # struct ProceduralTexture;
    /// # impl VTexture<R8UNorm> for ProceduralTexture {
    /// #     fn width(&self) -> u16 { 32 }
    /// #     fn height(&self) -> u16 { 32 }
    /// #     fn read(&self, texel: Texel) -> u8 { 0 }
    /// # }
    /// // Clone from a procedural texture
    /// let procedural = ProceduralTexture;
    /// let concrete = Texture::<R8UNorm>::new_cloning(&procedural);
    /// ```
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
    /// Returns the width of the texture in pixels.
    #[inline] pub fn width(&self) -> u16 {
        self.width
    }
    /// Returns the height of the texture in pixels.
    #[inline] pub fn height(&self) -> u16 {
        self.height
    }

    /// Returns the raw texture data in GPU-friendly layout.
    ///
    /// The data is stored in row-major order (Y-major, X-minor):
    /// - Y=0 is the top row
    /// - X=0 is the leftmost column
    ///
    /// This layout is suitable for direct upload to GPU textures.
    #[inline] pub(crate) fn texture_data(&self) -> &[Format::CPixel] {
        &self.data
    }

    /// Creates a new texture by applying a function to each pixel.
    ///
    /// This is useful for pixel format conversions or applying effects.
    ///
    /// # Type Parameters
    ///
    /// * `F` - Function type that maps from source to destination pixel
    /// * `T` - Destination pixel format
    ///
    /// # Arguments
    ///
    /// * `mapfn` - Function to apply to each pixel
    ///
    /// # Examples
    ///
    /// ```
    /// # use images_and_words::bindings::software::texture::{Texture, Texel};
    /// # use images_and_words::pixel_formats::{R8UNorm, R32Float};
    /// // Convert from 8-bit to float
    /// let texture_u8 = Texture::<R8UNorm>::new(32, 32, 128u8);
    /// let texture_f32: Texture<R32Float> = texture_u8.map(|&pixel| {
    ///     pixel as f32 / 255.0
    /// });
    /// ```
    pub fn map<F: Fn(&Format::CPixel) -> T::CPixel, T: PixelFormat>(&self, mapfn: F) -> Texture<T> where T::CPixel: Default + Clone + std::fmt::Debug {
        
        Texture::new_with(self.width,self.height, |texel| {
            let ours = &self[texel];
            
            mapfn(ours)
        })
    }


    /// Writes the raw texture data to a file for debugging.
    ///
    /// The data is written in the internal memory layout without any
    /// file format headers. This is primarily useful for debugging
    /// texture content.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the raw data will be written
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be written.
    pub fn dump_c_to(&self, path: &std::path::Path) where Format: PixelFormat{
        let u8_slice: &[u8] = unsafe {
            std::slice::from_raw_parts(self.texture_data() as *const _ as *const u8, std::mem::size_of_val(self.texture_data()))
        };
        std::fs::write(path, u8_slice).unwrap()
    }
}

/// Performs bilinear sampling on a texture at the given scaled coordinates.
///
/// Bilinear sampling interpolates between the four nearest texels to produce
/// smooth results when sampling between pixel centers.
///
/// # Arguments
///
/// * `texture` - The texture to sample from
/// * `scaled` - The scaled coordinates specifying the sample position
///
/// # Returns
///
/// The interpolated pixel value at the given coordinates
///
/// # Algorithm
///
/// The function computes a weighted average of the four nearest texels:
/// - Top-left (weight: (1-fx) * (1-fy))
/// - Top-right (weight: fx * (1-fy))
/// - Bottom-left (weight: (1-fx) * fy)
/// - Bottom-right (weight: fx * fy)
///
/// Where fx and fy are the fractional parts of the coordinates.
///
/// # Examples
///
/// ```
/// # use images_and_words::bindings::software::texture::{Texture, sample_bilinear};
/// # use images_and_words::bindings::software::texture::scaled_32::Scaled32;
/// # use images_and_words::pixel_formats::R32Float;
/// // Create a simple texture
/// let texture = Texture::<R32Float>::new(64, 64, 0.5);
/// 
/// // Sample at position (10.5, 20.3)
/// let coords = Scaled32::new(10, 20, 0.5, 0.3);
/// let sampled = sample_bilinear(&texture, coords);
/// 
/// // The result is a weighted average of the 4 neighboring pixels
/// assert!((sampled - 0.5).abs() < 0.01);
/// ```
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
    /// Converts the texture to TGA format for saving.
    ///
    /// This is useful for debugging and exporting textures to a common
    /// image format that can be viewed in standard image viewers.
    ///
    /// # Returns
    ///
    /// A TGA image in BGRA format
    ///
    /// # Examples
    ///
    /// ```
    /// # use images_and_words::bindings::software::texture::{Texture, Texel};
    /// # use images_and_words::pixel_formats::{BGRA8UNormSRGB, BGRA8UnormPixelSRGB};
    /// // Create a small texture with gray pixels
    /// let pixel = BGRA8UnormPixelSRGB { b: 128, g: 128, r: 128, a: 255 };
    /// let texture = Texture::<BGRA8UNormSRGB>::new(2, 2, pixel);
    /// 
    /// let tga = texture.dump_tga();
    /// // The TGA struct contains the image data ready to be saved
    /// ```
    pub fn dump_tga(&self) -> tgar::BGRA {
        let mut vec = Vec::with_capacity(self.width as usize * self.height as usize);
        for y in 0..self.height {
            for x in 0..self.width {
                let read_px = self[Texel{x,y}].clone();
                let converted_px = read_px.into();
                vec.push(converted_px);
            }
        }
        tgar::BGRA::new(self.width, self.height, &vec)
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