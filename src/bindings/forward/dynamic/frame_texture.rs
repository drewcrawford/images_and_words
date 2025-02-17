/*! Cross-platform frame_texture

 This represents a dynamic bitmap-like image.
 */

use std::future::Future;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::{CPUStrategy, TextureUsage};
use crate::images::device::BoundDevice;
use crate::pixel_formats::sealed::PixelFormat;
use crate::{imp, Priority};
use crate::multibuffer::{multibuffer, Producer, ProducerReadGuard, ProducerWriteGuard, Receiver, ReceiverReadGuard};

//?
#[derive(Debug,Clone)]
pub struct FrameTextureDelivery;
#[derive(Debug)]
pub struct FrameTextureProduct<Format>(Format);

impl<Format> FrameTextureProduct<Format> {
    pub fn width(&self) -> u16 {
        todo!()
    }
    pub fn height(&self) -> u16 {
        todo!()
    }
    /**A fast path for setting the entire texture to a single value.*/
    pub fn clear(&mut self,color: Format::CPixel) where Format: PixelFormat {
        todo!()
    }
}


#[derive(Debug)]
pub struct FrameTexture<Format: PixelFormat>{
    _imp: imp::Texture<Format>,
    width: u16,
    height: u16,
}

impl<Format: PixelFormat> Index<Texel> for FrameTextureProduct<Format> {
    type Output = Format::CPixel;

    fn index(&self, index: Texel) -> &Self::Output {
        todo!()
    }
}

impl<Format: PixelFormat> IndexMut<Texel> for FrameTextureProduct<Format> {
    fn index_mut(&mut self, index: Texel) -> &mut Self::Output {
        todo!()
    }
}

impl<Format: PixelFormat> FrameTextureProduct<Format> {
    /**
    A fast path for iterating over pixel addresses.  You can read or write each pixel as desired.

    This function is substantially faster than Index for bulk operations because we can eliminate bounds checking on a per-pixel basis.

    * start: The starting texel
    * past_end: The texel after the last one to iterate over.  This will be bounds-checked against the texture size.
    * f: A function that will be called for each pixel.  It will be passed the pixel address and the texel.  You can read or write
    the pixel as desired.
    */
    pub fn blend<Blend: Fn(Texel, &mut Format::CPixel)>(&mut self, start: Texel, past_end: Texel,blend: Blend) {
        todo!()
    }
}
#[derive(Debug)]
pub struct CPUAccess<Format: PixelFormat>(ProducerWriteGuard<FrameTextureProduct<Format>>);
impl<Format: PixelFormat> Deref for CPUAccess<Format> {
    type Target = FrameTextureProduct<Format>;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
impl<Format: PixelFormat> DerefMut for CPUAccess<Format> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}
#[derive(Debug)]
pub struct CPUBorrow<Format: PixelFormat>(ProducerReadGuard<FrameTextureProduct<Format>>);
impl<'a, Format: PixelFormat> Deref for CPUBorrow<Format> {
    type Target = FrameTextureProduct<Format>;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}


/**
An opaque type associated with a [FrameTexture].  Represents the part of the texture on the "render side".

Can be passed to a [crate::bindings::BindStyle].
*/
#[derive(Debug)]
pub struct TextureRenderSide {

}
#[derive(Debug,Clone)]
pub(crate) struct GPUBorrow(pub(crate) ReceiverReadGuard<FrameTextureDelivery>);
impl Deref for GPUBorrow {
    type Target = FrameTextureDelivery;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
impl TextureRenderSide {
    pub(crate) fn dequeue_render(&mut self) -> GPUBorrow {
        todo!()
    }
}



impl<Format: PixelFormat> FrameTexture<Format> {
    pub async fn new<I: Fn(Texel) -> Format::CPixel>(bound_device: &Arc<BoundDevice>, width: u16, height: u16, visible_to: TextureUsage, cpu_strategy: CPUStrategy, debug_name: &str, initialize_with: I, priority: Priority) -> Self  {

        let underlying_texture = imp::Texture::new(bound_device, width, height, visible_to, debug_name, priority, initialize_with).await.unwrap();
        Self {
            _imp: underlying_texture,
            width, height,
        }
    }
    /**
    Dequeues a texture.  Resumes when a texture is available.
     */
    pub fn dequeue<'s>(&'s mut self) -> impl Future<Output=CPUAccess<Format>> + 's {
        async {
            todo!()
        }
    }
    /**
    Returns the last texture submitted to the GPU.

    If no texture has ever been submitted to the GPU, returns any initialized texture.
     */
    pub fn last(&self) -> CPUBorrow<Format> {
        todo!()
    }
    /**
    Returns the CPUAccess and marks the contents as ready for GPU submission.

    The renderloop will generally re-use each frame until the next frame is submitted.
    In this way failing to keep up will not drop the framerate (although it may block your subsystem).

    There is currently no support for atomically submitting two different textures together, mt2-471.
     */
    pub fn submit<'s>(&'s mut self, cpu_access: CPUAccess<Format>) -> impl Future<Output=()> + 's {
        async { todo!() }
    }

    /**
    Gets an associated [TextureRenderSide] for this texture.
*/
    pub fn render_side(&mut self) -> TextureRenderSide {
        TextureRenderSide {

        }
    }
    pub fn width(&self) -> u16 {
        self.width
    }
    pub fn height(&self) -> u16 {
        self.height
    }
}

