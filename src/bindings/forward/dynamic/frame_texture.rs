/*! Cross-platform frame_texture

 This represents a dynamic bitmap-like image.  It is multibuffered.
 */

use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::{CPUStrategy, TextureUsage};
use crate::images::device::BoundDevice;
use crate::pixel_formats::sealed::PixelFormat;
use crate::{imp, Priority};
use crate::bindings::dirty_tracking::DirtyReceiver;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::imp::{CopyInfo, MappableTexture};
use crate::multibuffer::Multibuffer;

/**
A single non-multibuffered texture.
*/

pub struct IndividualTexture<Format> {
    cpu: imp::MappableTexture<Format>,
    width: u16,
    height: u16
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

impl<Format> AsRef<imp::MappableTexture<Format>> for IndividualTexture<Format> {
    fn as_ref(&self) -> &MappableTexture<Format> {
        &self.cpu
    }
}




trait DynRenderSide: Send + Debug + Sync {
    ///
    /// # Safety
    /// Must hold the guard for the lifetime of the GPU texture access.
    #[allow(dead_code)] //nop implementation does not use
    unsafe fn acquire_gpu_texture(&self, copy_info: &mut CopyInfo) -> ErasedGPUGuard;
    fn gpu_dirty_receiver(&self) -> DirtyReceiver;
}

trait DynGuard {
    #[allow(dead_code)] //nop implementation does not use
    fn as_imp(&self) -> imp::TextureRenderSide;
}
impl<Format: PixelFormat> DynGuard for GPUGuard<Format> {
    fn as_imp(&self) -> crate::imp::TextureRenderSide {
        self.underlying.as_imp().render_side()
    }
}



#[derive(Debug,Clone)]
pub(crate) struct ErasedTextureRenderSide {
    imp: Arc<dyn DynRenderSide>,
}

impl ErasedTextureRenderSide {
    #[allow(dead_code)] //nop implementation does not use
    pub unsafe fn acquire_gpu_texture(&self, copy_info: &mut CopyInfo) -> ErasedGPUGuard {
        let guard = unsafe { self.imp.acquire_gpu_texture(copy_info) };
        guard
    }
    pub fn gpu_dirty_receiver(&self) -> DirtyReceiver {
        self.imp.gpu_dirty_receiver()
    }
}

/**
An opaque type that references the multibuffered texture for GPU binding.
*/

pub struct TextureRenderSide<Format: PixelFormat> {
    shared: Arc<Shared<Format>>
}

impl<Format: PixelFormat> TextureRenderSide<Format> {
    pub(crate) fn erased(self) -> ErasedTextureRenderSide where Format: 'static {
        ErasedTextureRenderSide {
            imp: Arc::new(self)
        }
    }
}


impl<Format: PixelFormat> Debug for TextureRenderSide<Format> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextureRenderSide")
            .field("shared", &self.shared)
            .finish()
    }
}
impl<Format: PixelFormat> DynRenderSide for TextureRenderSide<Format> {
    unsafe fn acquire_gpu_texture(&self, copy_info: &mut CopyInfo) -> ErasedGPUGuard {
        let guard = unsafe { self.shared.multibuffer.access_gpu(copy_info) };
        let our_guard = GPUGuard {
            underlying: guard,
        };
        let render_side = our_guard.underlying.as_imp().render_side();
        ErasedGPUGuard {
            erasing: Box::new(our_guard),
            render_side,
        }
    }
    fn gpu_dirty_receiver(&self) -> DirtyReceiver {
        self.shared.multibuffer.gpu_dirty_receiver()
    }
}

#[derive(Debug)]
pub struct CPUWriteGuard<'a, Format: PixelFormat> {
    underlying: crate::multibuffer::CPUWriteGuard<'a, IndividualTexture<Format>, imp::GPUableTexture<Format>>
}
impl<'a, Format: PixelFormat> Deref for CPUWriteGuard<'a, Format> {
    type Target = IndividualTexture<Format>;
    fn deref(&self) -> &Self::Target {
        self.underlying.deref()
    }
}

impl<'a, Format: PixelFormat> DerefMut for CPUWriteGuard<'a, Format> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.underlying.deref_mut()
    }
}
#[derive(Debug)]
pub struct CPUReadGuard<Format: PixelFormat> {
    format: PhantomData<Format>,
}

#[allow(dead_code)] //nop implementation does not use
struct GPUGuard<Format: PixelFormat> {
    underlying: crate::multibuffer::GPUGuard<IndividualTexture<Format>, imp::GPUableTexture<Format>>,
}
pub struct ErasedGPUGuard {
    #[allow(dead_code)] //nop implementation does not use
    erasing: Box<dyn DynGuard>,
    render_side: imp::TextureRenderSide
}

impl Deref for ErasedGPUGuard {
    type Target = imp::TextureRenderSide;
    fn deref(&self) -> &Self::Target {
        &self.render_side
    }
}



///Shared between FrameTexture and TextureRenderSide
struct Shared<Format: PixelFormat> {
    multibuffer: Multibuffer<IndividualTexture<Format>,imp::GPUableTexture<Format>>,
}

impl<Format: PixelFormat> Debug for Shared<Format> {
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

    #[allow(dead_code)] //nop implementation does not use
    const fn index_for_texel(texel: Texel, width: u16) -> usize {
        (texel.y as usize * width as usize) + texel.x as usize
    }

    pub fn replace(&mut self, src_width: u16, dst_texel: Texel, data: &[Format::CPixel]) where Format: PixelFormat {
        self.cpu.replace(src_width, dst_texel, data);

    }
}

impl<Format: PixelFormat> Mappable for IndividualTexture<Format> {
    async fn map_read(&mut self) {
        self.cpu.map_read().await;
    }
    async fn map_write(&mut self) {
        self.cpu.map_write().await;
    }
    fn unmap(&mut self) {
        self.cpu.unmap();
    }
    fn byte_len(&self) -> usize {
        (self.width as usize) * (self.height as usize) * std::mem::size_of::<Format::CPixel>()
    }

}










impl<Format: PixelFormat> FrameTexture<Format> {
    pub async fn new<I: Fn(Texel) -> Format::CPixel>(bound_device: &Arc<BoundDevice>, width: u16, height: u16, visible_to: TextureUsage, _cpu_strategy: CPUStrategy, debug_name: &str, initialize_with: I, priority: Priority) -> Self  {

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
    pub async fn dequeue(&mut self) -> CPUWriteGuard<Format>{
        let write_guard = self.shared.multibuffer.access_write().await;
        CPUWriteGuard {
            underlying: write_guard,
        }
    }
    /**
    Returns the last texture submitted to the GPU.

    If no texture has ever been submitted to the GPU, returns any initialized texture.
     */
    pub fn last(&self) -> CPUReadGuard<Format> {
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
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn gpu_dirty_receiver(&self) -> DirtyReceiver {
        self.shared.multibuffer.gpu_dirty_receiver()
    }
}

