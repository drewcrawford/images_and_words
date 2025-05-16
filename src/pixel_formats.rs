/*!Contains various info about pixel formats */

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

use std::fmt::Debug;
use tgar::PixelBGRA;
use crate::pixel_formats::sealed::{CPixelTrait, PixelFormat, ReprC};

pub use half::f16;

pub(crate) mod sealed {
    use std::fmt::Debug;

    pub trait PixelFormat: std::fmt::Debug + Send + Sync + 'static + crate::imp::PixelFormat {
        const BYTES_PER_PIXEL: u8;
        ///A type we can use.  Guaranteed to have correct memory layout.
        type CPixel: Clone + Debug + Send + ReprC + CPixelTrait;
    }
    /**
    Marker trait that indicates that the type has a C-compatible memory layout.
    */
    pub unsafe trait ReprC {

    }

    pub trait CPixelTrait {
        fn avg<const C: usize>(arr: &[Self; C]) -> Self where Self: Sized;
    }
}

pub(crate) fn pixel_as_bytes<T: ReprC> (t: &[T]) -> &[u8] {
    //safe because we know that T is repr(C)
    //(we offloaded the safety check to the ReprC trait)
    unsafe {
        std::slice::from_raw_parts(t.as_ptr() as *const u8, t.len() * std::mem::size_of::<T>())
    }
}



#[derive(Debug,Clone)]
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

#[derive(Debug,Clone)]
pub struct RGBA16Unorm;
impl PixelFormat for RGBA16Unorm {
    const BYTES_PER_PIXEL: u8 = 2 * 4;
    type CPixel = RGBA16Pixel;
}

impl CPixelTrait for RGBA16Pixel {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = (0 as u32, 0 as u32, 0 as u32, 0 as u32);
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

#[derive(Debug,Clone)]
pub struct RGFloat;
impl PixelFormat for RGFloat {
    const BYTES_PER_PIXEL: u8 = 8;
    type CPixel = RGFloatPixel;

}

impl CPixelTrait for RGFloatPixel {
    fn avg<const C: usize>(arr: &[Self; C]) -> Self {
        let mut sum = (0.0, 0.0);
        for i in arr {
            sum.0 += i.r as f32;
            sum.1 += i.g as f32;
        }
        let c = C as f32;
        RGFloatPixel {
            r: sum.0 / c,
            g: sum.1 / c,
        }
    }
}

#[repr(C)]
#[derive(Clone,Debug)]
pub struct RGFloatPixel {
    pub r: f32,
    pub g: f32,
}
unsafe impl ReprC for RGFloatPixel {}

#[repr(C)]
#[derive(Clone,Debug)]
pub struct RGBA16Pixel {
    r: u16,
    g: u16,
    b: u16,
    a: u16,
}
unsafe impl ReprC for RGBA16Pixel {}

#[derive(Debug,Clone)]
///32-bit signed integer type, R channel
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

#[derive(Debug,Clone)]
///Single-precision float format.  This is sampleable on Metal.
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

///C-compatible 4-field u8 type
#[repr(C)]
#[derive(Clone,Debug)]
pub struct Unorm4 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8
}
unsafe impl ReprC for Unorm4 {}
impl Unorm4 {
    pub fn from_floats(float4: Float4) -> Self {
        Unorm4 {
            r: (float4.r * 255.0).round() as u8,
            g: (float4.g * 255.0).round() as u8,
            b: (float4.b * 255.0).round() as u8,
            a: (float4.a * 255.0).round() as u8,

        }
    }
}
impl Into<PixelBGRA> for Unorm4 {
    fn into(self) -> PixelBGRA {
        PixelBGRA {
            r: self.r,
            b: self.b,
            g: self.g,
            a: self.a,
        }
    }
}
impl Default for Unorm4 {
    fn default() -> Self {
        Self { r: 0, g: 0, b: 0, a: 0}
    }
}
#[derive(Debug,Clone)]
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


/**
Currently the preferred form for pre-lit, narrow-color textures.

Could be optimized further on a per-platform basis, see obsidian://open?vault=mt2&file=IW%2FTexture%20formats for details.
*/
#[derive(Debug,Copy,Clone)]
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
#[derive(Debug,Clone,Copy,PartialEq)]
#[repr(C)]
pub struct BGRA8UnormPixelSRGB {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}
unsafe impl ReprC for BGRA8UnormPixelSRGB {}
impl BGRA8UnormPixelSRGB {
    pub const ZERO: BGRA8UnormPixelSRGB = Self { b: 0, g: 0, r: 0, a: 0 };
    #[inline] pub fn from_srgb_gamma_floats(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: (255.0 * r).round() as u8,
            g: (255.0 * g).round() as u8,
            b: (255.0 * b).round() as u8,
            a: (255.0 * a).round() as u8,
        }
    }
}
impl From<Float4> for BGRA8UnormPixelSRGB {
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

#[repr(C)]
#[derive(Clone,Debug)]
pub struct Float4 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

unsafe impl ReprC for Float4 {}
impl From<BGRA8UnormPixelSRGB> for Float4 {
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
        Float4{ r: 0.0, g: 0.0, b: 0.0, a: 0.0}
    }
}
impl Into<tgar::PixelBGRA> for Float4 {
    fn into(self) -> PixelBGRA {
        PixelBGRA {
            b: (self.b * 255.0) as u8,
            g: (self.g * 255.0) as u8,
            r: (self.r * 255.0) as u8,
            a: (self.a * 255.0) as u8,
        }
    }
}
#[derive(Debug,Clone)]
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
#[derive(Debug,Clone)]
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
/**
Currently only used for png support. */
#[repr(C)]
#[derive(Debug,Clone)]
pub struct RGBA8UnormSRGBPixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl From<RGBA8UnormSRGBPixel> for BGRA8UnormPixelSRGB {
    fn from(f: RGBA8UnormSRGBPixel) -> Self {
        Self{
            r: f.r,
            g: f.g,
            b: f.b,
            a: f.a,
        }
    }
}

impl Into<tgar::PixelBGRA> for BGRA8UnormPixelSRGB {
    fn into(self) -> PixelBGRA {
        /*
        It seems to be unspecified whether or not TGA files
    are stored in any particular colorspace.
         See https://github.com/microsoft/DirectXTex/issues/136 for some discussion
         */
        PixelBGRA {
            b: self.b,
            g: self.g,
            r: self.r,
            a: self.a
        }
    }
}

impl Into<tgar::PixelBGRA> for RGBA8UnormSRGBPixel {
    fn into(self) -> PixelBGRA {
        PixelBGRA {
            b: self.b,
            r: self.r,
            g: self.g,
            a: self.a,
        }
    }
}