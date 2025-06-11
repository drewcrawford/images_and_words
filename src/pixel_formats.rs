// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Type-safe pixel format definitions for GPU textures.
//!
//! This module provides compile-time type-safe pixel format definitions used throughout
//! images_and_words for GPU texture operations. Each pixel format encodes:
//!
//! - Number of channels (R, RG, RGB, RGBA)
//! - Data type per channel (8-bit unorm, 16-bit unorm, 16/32-bit float, 32-bit sint)
//! - Color space (linear or sRGB)
//! - Memory layout guarantees
//!
//! # Design Philosophy
//!
//! Pixel formats are implemented as zero-sized types rather than enums to enable:
//! - Compile-time type checking of pixel format compatibility
//! - Zero-cost abstractions with optimized code generation
//! - Type-safe texture operations without runtime dispatch
//!
//! # Available Formats
//!
//! ## Single Channel
//! - [`R8UNorm`] - 8-bit normalized unsigned integer (0-255 mapped to 0.0-1.0)
//! - [`R16Float`] - 16-bit half-precision float
//! - [`R32Float`] - 32-bit single-precision float
//! - [`R32SInt`] - 32-bit signed integer
//!
//! ## Multi-Channel
//! - [`RGFloat`] - 2-channel 32-bit float (8 bytes total)
//! - [`RGBA8UNorm`] - 4-channel 8-bit normalized (4 bytes total)
//! - [`RGBA8UnormSRGB`] - 4-channel 8-bit normalized with sRGB encoding
//! - [`BGRA8UNormSRGB`] - 4-channel 8-bit normalized with sRGB encoding (BGRA order)
//! - [`RGBA16Unorm`] - 4-channel 16-bit normalized (8 bytes total)
//! - [`RGBA32Float`] - 4-channel 32-bit float (16 bytes total)
//!
//! # Examples
//!
//! ```
//! # if cfg!(not(feature="backend_wgpu")) { return; }
//! use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
//!
//! // Create a red pixel
//! let red_pixel = Unorm4 { r: 255, g: 0, b: 0, a: 255 };
//! ```
//!
//! ```
//! # if cfg!(not(feature="backend_wgpu")) { return; }
//! use images_and_words::pixel_formats::{BGRA8UNormSRGB, BGRA8UnormPixelSRGB, Float4};
//!
//! // Convert from linear float to sRGB
//! let linear_color = Float4 { r: 0.5, g: 0.0, b: 0.0, a: 1.0 };
//! let srgb_pixel: BGRA8UnormPixelSRGB = linear_color.into();
//! ```

/*
Quick note on type design.  We could implement pixel formats with an enum

```rust
enum PixelFormat {
    U8,U16
}
impl PixelFormat {
    fn bytes_per_row(&self) -> u8 { match self { todo!() }}
}

struct ManagedTexture {
    pixel_format: PixelFormat,
    /* ... */
}

enum Bindable {
    ManagedTexture(ManagedTexture), /* ... */
}
```

This would let us eliminate the vtable for the managed texture, replacing it with the enum dispatch pattern.
The downside is it erases some optimizations, like loop unrolling of the bytes_per_row.

The issue though is that we want to typecheck if you're using the pixel format correctly.
e.g., we have a function like `write(x:u32, y: u32, value: u8 or u16)`.  In order to do that typecheck,
we need the pixel format as part of the type.

There's a similar thing going on for 2d/3d.
 */
pub(crate) mod png_support;

use crate::pixel_formats::sealed::{CPixelTrait, PixelFormat, ReprC};
use std::fmt::Debug;
use tgar::PixelBGRA;

pub use half::f16;

/// Sealed traits for pixel format type safety.
///
/// This module uses the sealed trait pattern to ensure that only the pixel
/// formats defined in this crate can be used with texture APIs. This prevents
/// users from accidentally creating incompatible pixel format types.
pub(crate) mod sealed {
    use std::fmt::Debug;

    /// Core trait for pixel format types.
    ///
    /// This trait is sealed and cannot be implemented outside this crate.
    /// Each pixel format type implements this to specify its memory layout
    /// and associated pixel type.
    pub trait PixelFormat:
        std::fmt::Debug + Send + Sync + 'static + crate::imp::PixelFormat
    {
        /// Number of bytes per pixel for this format.
        const BYTES_PER_PIXEL: u8;

        /// The concrete pixel type with guaranteed C-compatible memory layout.
        ///
        /// This type is what you actually read/write when accessing texture data.
        type CPixel: Clone + Debug + Send + ReprC + CPixelTrait;
    }

    /// Marker trait indicating C-compatible memory layout.
    ///
    /// Types implementing this trait have predictable memory layout with:
    /// - No padding between fields
    /// - No uninitialized bytes
    /// - Stable field ordering
    ///
    /// # Safety
    ///
    /// This trait is unsafe to implement because incorrect implementation
    /// could lead to undefined behavior when casting to/from byte slices.
    pub unsafe trait ReprC {}

    /// Operations supported by pixel types.
    pub trait CPixelTrait {
        /// Compute the average of an array of pixels.
        ///
        /// Used for mipmap generation and filtering operations.
        #[allow(dead_code)] //nop implementation does not use
        fn avg<const C: usize>(arr: &[Self; C]) -> Self
        where
            Self: Sized;
    }
}

/// Convert a slice of C-compatible pixels to raw bytes.
///
/// # Safety
///
/// This function is safe because it requires `T: ReprC`, which guarantees
/// C-compatible memory layout with no padding or uninitialized bytes.
#[allow(dead_code)] //nop implementation does not use
pub(crate) fn pixel_as_bytes<T: ReprC>(t: &[T]) -> &[u8] {
    //safe because we know that T is repr(C)
    //(we offloaded the safety check to the ReprC trait)
    unsafe { std::slice::from_raw_parts(t.as_ptr() as *const u8, std::mem::size_of_val(t)) }
}

/// 8-bit normalized unsigned integer format with a single red channel.
///
/// Values are stored as 0-255 and interpreted as 0.0-1.0 when sampled.
/// This format is commonly used for:
/// - Grayscale images
/// - Alpha masks
/// - Single-channel data like height maps
///
/// # Examples
///
/// ```
/// use images_and_words::pixel_formats::R8UNorm;
///
/// // The pixel type for R8UNorm is u8
/// let pixel: u8 = 128;
/// // This pixel value represents 128/255 ≈ 0.502 when normalized
/// ```
#[derive(Debug, Clone)]
pub struct R8UNorm;
impl PixelFormat for R8UNorm {
    const BYTES_PER_PIXEL: u8 = 1;
    type CPixel = u8;
}

unsafe impl ReprC for u8 {}
impl CPixelTrait for u8 {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = 0;
        for i in arr {
            sum += *i as u32;
        }
        (sum / C as u32) as u8
    }
}

/// 16-bit normalized unsigned integer format with RGBA channels.
///
/// Each channel uses 16 bits (0-65535 mapped to 0.0-1.0), providing higher precision
/// than 8-bit formats. Total size is 8 bytes per pixel.
///
/// Useful for:
/// - High dynamic range textures
/// - Intermediate render targets
/// - Normal maps requiring extra precision
#[derive(Debug, Clone)]
pub struct RGBA16Unorm;
impl PixelFormat for RGBA16Unorm {
    const BYTES_PER_PIXEL: u8 = 2 * 4;
    type CPixel = RGBA16Pixel;
}

impl CPixelTrait for RGBA16Pixel {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = (0_u32, 0_u32, 0_u32, 0_u32);
        for i in arr {
            sum.0 += i.r as u32;
            sum.1 += i.g as u32;
            sum.2 += i.b as u32;
            sum.3 += i.a as u32;
        }
        let c = C as u32;
        RGBA16Pixel {
            r: (sum.0 / c).try_into().unwrap(),
            g: (sum.1 / c).try_into().unwrap(),
            b: (sum.2 / c).try_into().unwrap(),
            a: (sum.3 / c).try_into().unwrap(),
        }
    }
}

/// Two-channel 32-bit floating point format.
///
/// Each channel is a full 32-bit float. Total size is 8 bytes per pixel.
/// Commonly used for:
/// - UV coordinates
/// - 2D vector fields
/// - Two-channel HDR data
#[derive(Debug, Clone)]
pub struct RGFloat;
impl PixelFormat for RGFloat {
    const BYTES_PER_PIXEL: u8 = 8;
    type CPixel = RGFloatPixel;
}

impl CPixelTrait for RGFloatPixel {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = (0.0, 0.0);
        for i in arr {
            sum.0 += i.r;
            sum.1 += i.g;
        }
        let c = C as f32;
        RGFloatPixel {
            r: sum.0 / c,
            g: sum.1 / c,
        }
    }
}

/// Pixel type for [`RGFloat`] format.
///
/// Contains two 32-bit floating point values with C-compatible memory layout.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct RGFloatPixel {
    /// Red channel value
    pub r: f32,
    /// Green channel value
    pub g: f32,
}
unsafe impl ReprC for RGFloatPixel {}

/// Pixel type for [`RGBA16Unorm`] format.
///
/// Contains four 16-bit normalized values with C-compatible memory layout.
/// Values range from 0-65535.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct RGBA16Pixel {
    /// Red channel (0-65535)
    pub r: u16,
    /// Green channel (0-65535)
    pub g: u16,
    /// Blue channel (0-65535)
    pub b: u16,
    /// Alpha channel (0-65535)
    pub a: u16,
}
unsafe impl ReprC for RGBA16Pixel {}

/// 32-bit signed integer format with a single red channel.
///
/// Unlike normalized formats, values are not normalized to 0.0-1.0.
/// Useful for:
/// - Index buffers
/// - Integer compute data
/// - Non-visual data storage
#[derive(Debug, Clone)]
pub struct R32SInt;
impl PixelFormat for R32SInt {
    const BYTES_PER_PIXEL: u8 = 4;
    type CPixel = i32;
}
unsafe impl ReprC for i32 {}
impl CPixelTrait for i32 {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = 0;
        for i in arr {
            sum += *i;
        }
        sum / C as i32
    }
}

/// 32-bit single-precision float format with a single red channel.
///
/// Provides full floating point precision for a single channel.
/// Note: This format is sampleable on Metal, unlike some other float formats.
///
/// Common uses:
/// - Depth buffers
/// - Single-channel HDR data
/// - Distance fields
#[derive(Debug, Clone)]
pub struct R32Float;
impl PixelFormat for R32Float {
    const BYTES_PER_PIXEL: u8 = 4;
    type CPixel = f32;
}

impl CPixelTrait for f32 {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = 0.0;
        for i in arr {
            sum += *i;
        }
        sum / C as f32
    }
}
/// 16-bit half-precision float format with a single red channel.
///
/// Uses IEEE 754 half-precision format. More compact than R32Float
/// but with reduced range and precision.
///
/// Useful for:
/// - Memory-constrained applications
/// - Mobile GPUs
/// - Cases where full float precision isn't needed
#[derive(Debug, Clone)]
pub struct R16Float;
impl PixelFormat for R16Float {
    const BYTES_PER_PIXEL: u8 = 2;
    type CPixel = half::f16;
}

impl CPixelTrait for half::f16 {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = half::f16::ZERO;
        for i in arr {
            sum += *i;
        }
        sum / half::f16::from_f32(C as f32)
    }
}
unsafe impl ReprC for half::f16 {}
unsafe impl ReprC for f32 {}

/// C-compatible RGBA pixel with 8-bit normalized unsigned values.
///
/// This is the pixel type for [`RGBA8UNorm`]. Values range from 0-255
/// and are interpreted as 0.0-1.0 when used in shaders.
///
/// # Examples
///
/// ```
/// use images_and_words::pixel_formats::{Unorm4, Float4};
///
/// // Create from individual channels
/// let opaque_red = Unorm4 { r: 255, g: 0, b: 0, a: 255 };
///
/// // Convert from normalized floats
/// let float_color = Float4 { r: 1.0, g: 0.5, b: 0.0, a: 1.0 };
/// let unorm_color = Unorm4::from_floats(float_color);
/// ```
#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct Unorm4 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
unsafe impl ReprC for Unorm4 {}
impl Unorm4 {
    /// Convert from normalized float values (0.0-1.0) to 8-bit values (0-255).
    ///
    /// Values are clamped to the valid range and rounded to nearest integer.
    pub fn from_floats(float4: Float4) -> Self {
        Unorm4 {
            r: (float4.r * 255.0).round().clamp(0.0, 255.0) as u8,
            g: (float4.g * 255.0).round().clamp(0.0, 255.0) as u8,
            b: (float4.b * 255.0).round().clamp(0.0, 255.0) as u8,
            a: (float4.a * 255.0).round().clamp(0.0, 255.0) as u8,
        }
    }
}
impl From<Unorm4> for PixelBGRA {
    fn from(val: Unorm4) -> Self {
        PixelBGRA {
            r: val.r,
            b: val.b,
            g: val.g,
            a: val.a,
        }
    }
}
/// 8-bit normalized unsigned integer format with RGBA channels.
///
/// The most common texture format for color images. Each channel uses 8 bits
/// (0-255 mapped to 0.0-1.0). Total size is 4 bytes per pixel.
///
/// # Examples
///
/// ```
/// # if cfg!(not(feature="backend_wgpu")) { return; }
/// # #[cfg(feature = "testing")]
/// # {
/// use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
/// use images_and_words::bindings::forward::r#static::texture::Texture;
/// use images_and_words::bindings::visible_to::{TextureUsage, TextureConfig, CPUStrategy};
/// # use images_and_words::Priority;
/// # use images_and_words::images::projection::WorldCoord;
/// # use images_and_words::images::view::View;
/// # test_executors::sleep_on(async {
/// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
/// # let device = engine.bound_device();
///
/// // Create a texture with this format
/// let config = TextureConfig {
///     width: 256,
///     height: 256,
///     visible_to: TextureUsage::FragmentShaderSample,
///     debug_name: "my_texture",
///     priority: Priority::UserInitiated,
///     cpu_strategy: CPUStrategy::WontRead,
///     mipmaps: false,
/// };
/// let texture = Texture::<RGBA8UNorm>::new(
///     &device,
///     config,
///     |_| Unorm4 { r: 255, g: 128, b: 0, a: 255 }
/// ).await.expect("Failed to create texture");
/// # });
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct RGBA8UNorm;
impl PixelFormat for RGBA8UNorm {
    const BYTES_PER_PIXEL: u8 = 4;
    type CPixel = Unorm4;
}

impl CPixelTrait for Unorm4 {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = (0, 0, 0, 0);
        for i in arr {
            sum.0 += i.r as u32;
            sum.1 += i.g as u32;
            sum.2 += i.b as u32;
            sum.3 += i.a as u32;
        }
        let c = C as u32;
        Unorm4 {
            r: (sum.0 / c) as u8,
            g: (sum.1 / c) as u8,
            b: (sum.2 / c) as u8,
            a: (sum.3 / c) as u8,
        }
    }
}

/// 8-bit normalized unsigned integer format with BGRA channel order and sRGB encoding.
///
/// This is the preferred format for pre-lit, narrow-color textures. The BGRA
/// channel order is optimal for many GPUs and display systems. The sRGB encoding
/// provides perceptually uniform color representation.
///
/// Features:
/// - Automatic sRGB to linear conversion on sampling
/// - Automatic linear to sRGB conversion on writing
/// - Optimal memory layout for many platforms
///
/// Could be optimized further on a per-platform basis.
#[derive(Debug, Copy, Clone)]
pub struct BGRA8UNormSRGB;
impl PixelFormat for BGRA8UNormSRGB {
    const BYTES_PER_PIXEL: u8 = 4;
    type CPixel = BGRA8UnormPixelSRGB;
}

impl CPixelTrait for BGRA8UnormPixelSRGB {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = (0, 0, 0, 0);
        for i in arr {
            sum.0 += i.b as u32;
            sum.1 += i.g as u32;
            sum.2 += i.r as u32;
            sum.3 += i.a as u32;
        }
        let c = C as u32;
        BGRA8UnormPixelSRGB {
            b: (sum.0 / c) as u8,
            g: (sum.1 / c) as u8,
            r: (sum.2 / c) as u8,
            a: (sum.3 / c) as u8,
        }
    }
}
/// Pixel type for [`BGRA8UNormSRGB`] format.
///
/// Stores color values in sRGB space with BGRA channel order.
/// Values are automatically converted between linear and sRGB space
/// by the GPU when sampling or writing.
///
/// # Examples
///
/// ```
/// use images_and_words::pixel_formats::{BGRA8UnormPixelSRGB, Float4};
///
/// // Create from sRGB values
/// let srgb_pixel = BGRA8UnormPixelSRGB::from_srgb_gamma_floats(1.0, 0.5, 0.0, 1.0);
///
/// // Convert from linear color
/// let linear_color = Float4 { r: 0.5, g: 0.0, b: 0.0, a: 1.0 };
/// let srgb_pixel: BGRA8UnormPixelSRGB = linear_color.into();
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct BGRA8UnormPixelSRGB {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}
unsafe impl ReprC for BGRA8UnormPixelSRGB {}
impl BGRA8UnormPixelSRGB {
    /// Transparent black constant.
    pub const ZERO: BGRA8UnormPixelSRGB = Self {
        b: 0,
        g: 0,
        r: 0,
        a: 0,
    };

    /// Create from sRGB gamma-corrected float values (0.0-1.0).
    ///
    /// Input values are already in sRGB space and are simply scaled to 0-255.
    #[inline]
    pub fn from_srgb_gamma_floats(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: (255.0 * r).round() as u8,
            g: (255.0 * g).round() as u8,
            b: (255.0 * b).round() as u8,
            a: (255.0 * a).round() as u8,
        }
    }
}
impl From<Float4> for BGRA8UnormPixelSRGB {
    /// Convert from linear to sRGB color space.
    ///
    /// Applies the sRGB transfer function to convert from linear
    /// color values to gamma-corrected sRGB values.
    fn from(color: Float4) -> Self {
        let r = if color.r < 0.0031308 {
            12.92 * color.r
        } else {
            1.055 * color.r.powf(1.0 / 2.4) - 0.055
        };
        let g = if color.g < 0.0031308 {
            12.92 * color.g
        } else {
            1.055 * color.g.powf(1.0 / 2.4) - 0.055
        };
        let b = if color.b < 0.0031308 {
            12.92 * color.b
        } else {
            1.055 * color.b.powf(1.0 / 2.4) - 0.055
        };
        let a = color.a;
        Self {
            b: (b * 255.0).round() as u8,
            g: (g * 255.0).round() as u8,
            r: (r * 255.0).round() as u8,
            a: (a * 255.0).round() as u8,
        }
    }
}

/// Four-channel floating point color.
///
/// This is the pixel type for [`RGBA32Float`]. Values are stored as linear
/// color values (not gamma corrected). Can be converted to/from sRGB formats.
///
/// # Examples
///
/// ```
/// # if cfg!(not(feature="backend_wgpu")) { return; }
/// use images_and_words::pixel_formats::{Float4, BGRA8UnormPixelSRGB};
///
/// // Create a linear color
/// let linear_red = Float4 { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
///
/// // Convert to sRGB
/// let srgb_pixel: BGRA8UnormPixelSRGB = linear_red.into();
///
/// // Convert back to linear
/// let linear_again: Float4 = srgb_pixel.into();
/// ```
#[repr(C)]
#[derive(Clone, Debug)]
pub struct Float4 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

unsafe impl ReprC for Float4 {}
impl From<BGRA8UnormPixelSRGB> for Float4 {
    /// Convert from sRGB to linear color space.
    ///
    /// Applies the inverse sRGB transfer function to convert from
    /// gamma-corrected sRGB values to linear color values.
    fn from(c: BGRA8UnormPixelSRGB) -> Self {
        let r_s = c.r as f32 / 255.0;
        let g_s = c.g as f32 / 255.0;
        let b_s = c.b as f32 / 255.0;
        let a_s = c.a as f32 / 255.0;
        let r = if r_s <= 0.04045 {
            r_s / 12.92
        } else {
            ((r_s + 0.055) / 1.055).powf(2.4)
        };
        let g = if g_s <= 0.04045 {
            g_s / 12.92
        } else {
            ((g_s + 0.055) / 1.055).powf(2.4)
        };
        let b = if b_s <= 0.04045 {
            b_s / 12.92
        } else {
            ((b_s + 0.055) / 1.055).powf(2.4)
        };
        let a = if a_s <= 0.04045 {
            a_s / 12.92
        } else {
            ((a_s + 0.055) / 1.055).powf(2.4)
        };
        Self { r, g, b, a }
    }
}

impl Default for Float4 {
    fn default() -> Self {
        Float4 {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }
    }
}
impl From<Float4> for tgar::PixelBGRA {
    fn from(val: Float4) -> Self {
        PixelBGRA {
            b: (val.b * 255.0) as u8,
            g: (val.g * 255.0) as u8,
            r: (val.r * 255.0) as u8,
            a: (val.a * 255.0) as u8,
        }
    }
}
/// 32-bit floating point format with RGBA channels.
///
/// Each channel is a full 32-bit IEEE 754 float. Total size is 16 bytes per pixel.
/// This format provides maximum precision and dynamic range.
///
/// Common uses:
/// - HDR render targets
/// - Scientific visualization
/// - Intermediate computation buffers
/// - Post-processing effects
#[derive(Debug, Clone)]
pub struct RGBA32Float;
impl PixelFormat for RGBA32Float {
    const BYTES_PER_PIXEL: u8 = 16;
    type CPixel = Float4;
}

impl CPixelTrait for Float4 {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = (0.0, 0.0, 0.0, 0.0);
        for i in arr {
            sum.0 += i.r;
            sum.1 += i.g;
            sum.2 += i.b;
            sum.3 += i.a;
        }
        let c = C as f32;
        Float4 {
            r: sum.0 / c,
            g: sum.1 / c,
            b: sum.2 / c,
            a: sum.3 / c,
        }
    }
}
/// 8-bit normalized unsigned integer format with RGBA channel order and sRGB encoding.
///
/// Similar to [`BGRA8UNormSRGB`] but with RGBA channel order. The sRGB encoding
/// provides automatic gamma correction.
///
/// Note: BGRA order is often preferred for performance reasons on many platforms.
#[derive(Debug, Clone)]
pub struct RGBA8UnormSRGB;
impl PixelFormat for RGBA8UnormSRGB {
    const BYTES_PER_PIXEL: u8 = 4;
    type CPixel = RGBA8UnormSRGBPixel;
}

impl CPixelTrait for RGBA8UnormSRGBPixel {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = (0, 0, 0, 0);
        for i in arr {
            sum.0 += i.r as u32;
            sum.1 += i.g as u32;
            sum.2 += i.b as u32;
            sum.3 += i.a as u32;
        }
        let c = C as u32;
        RGBA8UnormSRGBPixel {
            r: (sum.0 / c) as u8,
            g: (sum.1 / c) as u8,
            b: (sum.2 / c) as u8,
            a: (sum.3 / c) as u8,
        }
    }
}

unsafe impl ReprC for RGBA8UnormSRGBPixel {}
/// Pixel type for [`RGBA8UnormSRGB`] format.
///
/// Currently primarily used for PNG support as PNG files typically
/// use RGBA channel order.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct RGBA8UnormSRGBPixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl From<RGBA8UnormSRGBPixel> for BGRA8UnormPixelSRGB {
    fn from(f: RGBA8UnormSRGBPixel) -> Self {
        Self {
            r: f.r,
            g: f.g,
            b: f.b,
            a: f.a,
        }
    }
}

impl From<BGRA8UnormPixelSRGB> for tgar::PixelBGRA {
    fn from(val: BGRA8UnormPixelSRGB) -> Self {
        /*
            It seems to be unspecified whether or not TGA files
        are stored in any particular colorspace.
             See https://github.com/microsoft/DirectXTex/issues/136 for some discussion
             */
        PixelBGRA {
            b: val.b,
            g: val.g,
            r: val.r,
            a: val.a,
        }
    }
}

impl From<RGBA8UnormSRGBPixel> for tgar::PixelBGRA {
    fn from(val: RGBA8UnormSRGBPixel) -> Self {
        PixelBGRA {
            b: val.b,
            r: val.r,
            g: val.g,
            a: val.a,
        }
    }
}
