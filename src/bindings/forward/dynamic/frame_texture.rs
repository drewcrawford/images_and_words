/*! Cross-platform frame_texture

 This represents a dynamic bitmap-like image.  It is multibuffered.
 */

use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::{CPUStrategy, TextureUsage};
use crate::images::device::BoundDevice;
use crate::pixel_formats::sealed::PixelFormat;
use crate::{imp, Priority};
use crate::bindings::resource_tracking::{CPUReadGuard, CPUWriteGuard, ResourceTracker};
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::imp::CopyInfo;
use crate::multibuffer::Multibuffer;

/**
A single non-multibuffered texture.
*/

pub struct IndividualTexture<Format> {
    cpu: imp::MappableTexture<Format>,
    width: u16,
    height: u16,
}

impl<Format> Debug for IndividualTexture<Format> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndividualTexture")
            .field("cpu", &self.cpu)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}


trait DynRenderSide: Send + Debug {

}



#[derive(Debug)]
pub(crate) struct ErasedTextureRenderSide {
    imp: Box<dyn DynRenderSide>,
}

impl ErasedTextureRenderSide {
    pub fn acquire_gpu_texture(&self, copy_info: &mut CopyInfo) -> GPUGuard {
        GPUGuard {

        }
    }
}

/**
An opaque type that references the multibuffered texture for GPU binding.
*/

pub struct TextureRenderSide<Format> {
    shared: Arc<Shared<Format>>
}

impl<Format> TextureRenderSide<Format> {
    pub(crate) fn erased(self) -> ErasedTextureRenderSide where Format: 'static {
        ErasedTextureRenderSide {
            imp: Box::new(self)
        }
    }
}


impl<Format> Debug for TextureRenderSide<Format> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextureRenderSide")
            .field("shared", &self.shared)
            .finish()
    }
}
impl<Format> DynRenderSide for TextureRenderSide<Format> {

}

pub struct GPUGuard {

}
impl Deref for GPUGuard {
    type Target = imp::TextureRenderSide;
    fn deref(&self) -> &Self::Target {
        todo!()
    }
}


///Shared between FrameTexture and TextureRenderSide
struct Shared<Format> {
    multibuffer: Multibuffer<IndividualTexture<Format>,imp::GPUableTexture<Format>>,
}

impl<Format> Debug for Shared<Format> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("multibuffer", &self.multibuffer)
            .finish()
    }
}

#[derive(Debug)]
pub struct FrameTexture<Format: PixelFormat>{
    shared: Arc<Shared<Format>>,
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

    const fn index_for_texel(texel: Texel, width: u16) -> usize {
        (texel.y as usize * width as usize) + texel.x as usize
    }
}

impl<Format> Mappable for IndividualTexture<Format> {
    async fn map_read(&mut self) {
        todo!()
    }
    async fn map_write(&mut self) {
        todo!()
    }
    fn unmap(&mut self) {
        todo!()
    }
    fn byte_len(&self) -> usize {
        todo!()
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

        let gpu = imp::GPUableTexture::new(bound_device, width, height, visible_to, debug_name, priority).await.unwrap();
        let cpu = imp::MappableTexture::new(bound_device, width, height, debug_name, priority, initialize_with);
        let individual_texture = IndividualTexture {
            cpu,
            width, height,
        };
        let multibuffer = Multibuffer::new(individual_texture, gpu);
        let shared = Arc::new(Shared {
            multibuffer
        });
        Self {
            shared,
            width, height,
        }
    }
    /**
    Dequeues a texture.  Resumes when a texture is available.
     */
    pub async fn dequeue(&mut self) -> CPUWriteGuard<IndividualTexture<Format>>{
        todo!()
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
    pub fn render_side(&mut self) -> TextureRenderSide<Format> {
        TextureRenderSide {
           shared: self.shared.clone(),
        }
    }
    pub fn width(&self) -> u16 {
        self.width
    }
    pub fn height(&self) -> u16 {
        self.height
    }
}

