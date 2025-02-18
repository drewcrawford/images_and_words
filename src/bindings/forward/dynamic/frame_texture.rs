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
use crate::bindings::resource_tracking::{CPUReadGuard, CPUWriteGuard, ResourceTracker};
use crate::multibuffer::{multibuffer, Producer, ProducerReadGuard, ProducerWriteGuard, Receiver, ReceiverReadGuard};

/**
A single non-multibuffered texture.
*/
#[derive(Debug)]
pub struct IndividualTexture<Format> {
    imp: imp::Texture<Format>,
    width: u16,
    height: u16,
}

/**
An opaque type that references the multibuffered texture for GPU binding.
*/
#[derive(Debug)]
pub struct TextureRenderSide {

}


#[derive(Debug)]
pub struct FrameTexture<Format: PixelFormat>{
    _imp: ResourceTracker<IndividualTexture<Format>>,
    width: u16,
    height: u16,
}

impl<Format> IndividualTexture<Format> {
    pub fn width(&self) -> u16 {
        self.width
    }
    pub fn height(&self) -> u16 {
        self.height
    }
}

impl<Format: PixelFormat> Index<Texel> for IndividualTexture<Format> {
    type Output = Format::CPixel;

    fn index(&self, index: Texel) -> &Self::Output {
        todo!()
    }
}

impl<Format: PixelFormat> IndexMut<Texel> for IndividualTexture<Format> {
    fn index_mut(&mut self, index: Texel) -> &mut Self::Output {
        todo!()
    }
}

impl<Format: PixelFormat> IndividualTexture<Format> {
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
    pub fn clear(&mut self, color: Format::CPixel) {
        todo!()
    }
}







impl<Format: PixelFormat> FrameTexture<Format> {
    pub async fn new<I: Fn(Texel) -> Format::CPixel>(bound_device: &Arc<BoundDevice>, width: u16, height: u16, visible_to: TextureUsage, cpu_strategy: CPUStrategy, debug_name: &str, initialize_with: I, priority: Priority) -> Self  {

        let imp_texture = imp::Texture::new(bound_device, width, height, visible_to, debug_name, priority, initialize_with).await.unwrap();
        let individual_texture = IndividualTexture {
            imp: imp_texture,
            width, height,
        };
        let guarded = ResourceTracker::new(individual_texture);
        Self {
            _imp: guarded,
            width, height,
        }
    }
    /**
    Dequeues a texture.  Resumes when a texture is available.
     */
    pub async fn dequeue(&mut self) -> CPUWriteGuard<IndividualTexture<Format>>{
        self._imp.cpu_write().expect("Can't write now")
    }
    /**
    Returns the last texture submitted to the GPU.

    If no texture has ever been submitted to the GPU, returns any initialized texture.
     */
    pub fn last(&self) -> CPUReadGuard<IndividualTexture<Format>> {
        todo!()
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

