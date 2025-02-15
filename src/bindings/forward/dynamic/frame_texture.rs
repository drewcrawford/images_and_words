/*! Cross-platform frame_texture */

use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::{CPUStrategy, TextureUsage};
use crate::images::device::BoundDevice;
use crate::pixel_formats::PixelFormat;
use crate::{imp, Priority};
use crate::multibuffer::{multibuffer, Producer, ProducerReadGuard, ProducerWriteGuard, Receiver, ReceiverReadGuard};

//?
#[derive(Debug,Clone)]
pub struct FrameTextureDelivery;
#[derive(Debug)]
pub struct FrameTextureProduct<Format>(Format);


#[derive(Debug)]
pub struct FrameTexture<Format: PixelFormat>{
    _imp: imp::FrameTexture<Format>,
    producer: Producer<FrameTextureProduct<Format>>,
    receiver: Option<Receiver<FrameTextureDelivery>>,
    shared: Arc<Shared>,
    width: u16,
    height: u16,
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

#[derive(Debug)]
struct Shared {
    ///Whether a renderside is alive.
    render_side: AtomicBool,
}

/**
An opaque type associated with a [FrameTexture].  Represents the part of the texture on the "render side".

Can be passed to a [crate::bindings::BindStyle].
*/
#[derive(Debug)]
pub struct TextureRenderSide {
    receiver: Receiver<FrameTextureDelivery>,
    shared: Arc<Shared>,
}
impl Drop for TextureRenderSide {
    fn drop(&mut self) {
        self.shared.render_side.store(false, std::sync::atomic::Ordering::Relaxed);
    }
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
        GPUBorrow(self.receiver.receive())
    }
}



impl<Format: PixelFormat> FrameTexture<Format> {
    pub async fn new<I: Fn(Texel) -> Format::CPixel>(bound_device: &Arc<BoundDevice>, width: u16, height: u16, visible_to: TextureUsage, cpu_strategy: CPUStrategy, debug_name: &str, initialize_with: I, priority: Priority) -> Self  {
        let (frame_texture,products) = crate::imp::FrameTexture::new(bound_device, width, height, visible_to, cpu_strategy, debug_name, initialize_with, priority).await;
        let (producer,receiver) = multibuffer(products);
        Self {
            _imp: frame_texture,
            producer,
            receiver: Some(receiver),
            shared: Arc::new(Shared {
                render_side: AtomicBool::new(false),
            }),
            width, height,
        }
    }
    /**
    Dequeues a texture.  Resumes when a texture is available.
     */
    pub fn dequeue<'s>(&'s mut self) -> impl Future<Output=CPUAccess<Format>> + 's {
        async {
            let tex = self.producer.borrow_write().await;
            CPUAccess(tex)
        }
    }
    /**
    Returns the last texture submitted to the GPU.

    If no texture has ever been submitted to the GPU, returns any initialized texture.
     */
    pub fn last(&self) -> CPUBorrow<Format> {
        CPUBorrow(self.producer.borrow_last_read())
    }
    /**
    Returns the CPUAccess and marks the contents as ready for GPU submission.

    The renderloop will generally re-use each frame until the next frame is submitted.
    In this way failing to keep up will not drop the framerate (although it may block your subsystem).

    There is currently no support for atomically submitting two different textures together, mt2-471.
     */
    pub fn submit<'s>(&'s mut self, cpu_access: CPUAccess<Format>) -> impl Future<Output=()> + 's {
        self.producer.submit(cpu_access.0)
    }

    /**
    Gets an associated [TextureRenderSide] for this texture.
*/
    pub fn render_side(&mut self) -> TextureRenderSide {
        self.shared.render_side.store(true, std::sync::atomic::Ordering::Relaxed);
        TextureRenderSide {
            receiver: self.receiver.take().unwrap(),
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

impl<Format: PixelFormat> Drop for FrameTexture<Format> {
    fn drop(&mut self) {
        /*
        Generally we aren't necessarily performing memory management in the renderloop.  For this reason you have to keep FrameTexture
        alive until the renderloop is done.
         */
        assert!(!self.shared.render_side.load(std::sync::atomic::Ordering::Relaxed), "FrameTexture dropped while renderside still active");
    }
}
