/*! Dynamic textures with CPU-to-GPU multibuffering.

This module provides [`FrameTexture`], a dynamic texture type that supports efficient CPU-to-GPU
data transfer with automatic multibuffering. This is ideal for textures that change frequently,
such as video frames, procedurally generated content, or any texture data that needs to be
updated from the CPU side during runtime.

# Overview

`FrameTexture` implements a multibuffered texture system where:
- CPU writes are decoupled from GPU reads through automatic buffering
- Multiple CPU-side buffers allow writing new frames while the GPU reads previous ones
- Synchronization is handled automatically to prevent data races
- Memory layout is optimized based on the target architecture (unified vs discrete GPU)

# Key Features

- **Multibuffering**: Write to one buffer while the GPU reads from another
- **Automatic synchronization**: No manual fencing or synchronization required
- **Type-safe pixel formats**: Compile-time checking of pixel data types
- **Dequeue/enqueue pattern**: Familiar producer-consumer interface for frame data

# Usage Pattern

```no_run
# use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
# use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
# use images_and_words::bindings::software::texture::Texel;
# use images_and_words::bindings::visible_to::{TextureUsage, CPUStrategy};
# use images_and_words::Priority;
# use std::sync::Arc;
# async fn example(device: Arc<images_and_words::images::BoundDevice>) {
// Create a 256x256 RGBA texture
let mut texture = FrameTexture::<RGBA8UNorm>::new(
    &device,
    256, 256,
    TextureUsage::FragmentShaderSample,
    CPUStrategy::WontRead,
    "my_dynamic_texture",
    |texel| Unorm4 { r: 0, g: 0, b: 0, a: 255 }, // Initialize to black
    Priority::UserInitiated
).await;

// Dequeue a buffer for writing
let mut write_guard = texture.dequeue().await;

// Update texture data
write_guard.replace(
    256, // source width
    Texel { x: 0, y: 0 }, // destination position
    &[Unorm4 { r: 255, g: 0, b: 0, a: 255 }] // red pixel
);

// Buffer is automatically enqueued when guard is dropped
drop(write_guard);
# }
```

# Architecture

The module uses several internal types to manage the multibuffering system:

- [`IndividualTexture`]: A single CPU-accessible texture buffer
- `TextureRenderSide`: GPU-side handle for binding textures in render passes
- [`CPUWriteGuard`]: RAII guard that provides write access to texture data
- [`CPUReadGuard`]: RAII guard for reading the last submitted texture

# Performance Considerations

- On discrete GPUs: Data is transferred over PCIe bus, so minimize update frequency
- On integrated GPUs: Shared memory reduces transfer cost but may have suboptimal layout
- The `CPUStrategy` parameter hints at expected CPU access patterns for optimization

# Type Parameters

Most types in this module are generic over a `Format: PixelFormat` parameter, which determines
the pixel data type and GPU texture format. Common formats include:
- [`RGBA8UNorm`](crate::pixel_formats::RGBA8UNorm): 8-bit RGBA
- [`BGRA8UNormSRGB`](crate::pixel_formats::BGRA8UNormSRGB): 8-bit BGRA with sRGB encoding
- [`RGBA32Float`](crate::pixel_formats::RGBA32Float): 32-bit floating point RGBA
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

/// A single CPU-accessible texture buffer within the multibuffer system.
///
/// This represents one texture in the multibuffer pool. It wraps a platform-specific
/// mappable texture and provides methods for updating pixel data. While this type
/// is public for use in guard types, you typically interact with it through
/// [`CPUWriteGuard`] rather than directly.
///
/// # Type Parameters
///
/// * `Format` - The pixel format type implementing `PixelFormat`
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

/// GPU-side handle for binding dynamic textures in render passes.
///
/// This type provides access to the GPU side of a [`FrameTexture`] for use in rendering.
/// It's created by calling [`FrameTexture::render_side()`] and can be bound to shaders
/// through the [`BindStyle`](crate::bindings::BindStyle) API.
///
/// The render side automatically selects the most recent texture that has been fully
/// uploaded to the GPU, handling synchronization to ensure the GPU never reads partially
/// updated data.
///
/// # Example
///
/// ```no_run
/// # use images_and_words::bindings::BindStyle;
/// # use images_and_words::bindings::bind_style::{BindSlot, Stage};
/// # let frame_texture: images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture<images_and_words::pixel_formats::RGBA8UNorm> = todo!();
/// let mut bind_style = BindStyle::new();
/// 
/// // Bind the texture to slot 0 for the fragment shader
/// bind_style.bind_dynamic_texture(BindSlot::new(0), Stage::Fragment, &frame_texture);
/// ```
pub(crate) struct TextureRenderSide<Format: PixelFormat> {
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

/// RAII guard providing write access to texture data.
///
/// This guard is obtained by calling [`FrameTexture::dequeue()`] and provides mutable access
/// to an [`IndividualTexture`] buffer. The texture data can be modified through the guard's
/// methods, and changes are automatically synchronized when the guard is dropped.
///
/// # Automatic Enqueue
///
/// When this guard is dropped, the texture is automatically enqueued for GPU upload. This
/// ensures that:
/// - All writes are complete before GPU access
/// - The multibuffer system can provide this buffer to the GPU when needed
/// - Other CPU threads waiting to write can proceed
///
/// # Example
///
/// ```no_run
/// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
/// # use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
/// # use images_and_words::bindings::software::texture::Texel;
/// # async fn example(texture: &mut FrameTexture<RGBA8UNorm>) {
/// // Dequeue a buffer for writing
/// let mut guard = texture.dequeue().await;
///
/// // Write pixel data
/// let width = guard.width();
/// guard.replace(
///     width, // source width
///     Texel { x: 10, y: 20 }, // destination
///     &[Unorm4 { r: 255, g: 128, b: 0, a: 255 }] // orange pixel
/// );
///
/// // Texture is automatically enqueued when guard goes out of scope
/// # }
/// ```
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
/// RAII guard providing read access to the last submitted texture.
///
/// This guard is obtained by calling [`FrameTexture::last()`] and provides read-only access
/// to the most recent texture data that was submitted to the GPU. This is useful for:
/// - Debugging texture contents
/// - Implementing feedback effects
/// - Verifying texture updates
///
/// Note: This returns the last texture that was enqueued for GPU upload, which may not
/// yet be visible to the GPU if the upload is still in progress.
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

/// A multibuffered dynamic texture for efficient CPU-to-GPU data transfer.
///
/// `FrameTexture` provides a high-level interface for textures that need frequent updates
/// from the CPU side. It automatically manages multiple buffers to allow the CPU to write
/// new texture data while the GPU reads from previously submitted data.
///
/// # Type Parameters
///
/// * `Format` - The pixel format type (e.g., [`RGBA8UNorm`](crate::pixel_formats::RGBA8UNorm))
///
/// # Multibuffering
///
/// The texture maintains multiple internal buffers to prevent synchronization stalls:
/// - CPU can write to one buffer while GPU reads from another
/// - Automatic synchronization ensures data consistency
/// - No manual fence or semaphore management required
///
/// # Example
///
/// ```no_run
/// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
/// # use images_and_words::pixel_formats::{BGRA8UNormSRGB, BGRA8UnormPixelSRGB};
/// # use images_and_words::bindings::software::texture::Texel;
/// # use images_and_words::bindings::visible_to::{TextureUsage, CPUStrategy};
/// # use images_and_words::Priority;
/// # use std::sync::Arc;
/// # async fn example(device: Arc<images_and_words::images::BoundDevice>) {
/// // Create a texture for video playback
/// let mut video_texture = FrameTexture::<BGRA8UNormSRGB>::new(
///     &device,
///     1920, 1080,
///     TextureUsage::FragmentShaderSample,
///     CPUStrategy::WontRead,
///     "video_frame",
///     |_| BGRA8UnormPixelSRGB::ZERO, // Start with black
///     Priority::UserInitiated
/// ).await;
///
/// // Game loop
/// loop {
///     // Get next video frame data
///     let frame_data: Vec<BGRA8UnormPixelSRGB> = todo!("decode video frame");
///     
///     // Write frame to texture
///     let mut guard = video_texture.dequeue().await;
///     guard.replace(1920, Texel::ZERO, &frame_data);
///     drop(guard); // Enqueue for GPU
///     
///     // Render using the texture...
/// }
/// # }
/// ```
#[derive(Debug,Clone)]
pub struct FrameTexture<Format: PixelFormat>{
    shared: Arc<Shared<Format>>,
    width: u16,
    height: u16,
}

impl<Format> IndividualTexture<Format> {
    /// Returns the width of the texture in pixels.
    pub fn width(&self) -> u16 {
        self.width
    }
    
    /// Returns the height of the texture in pixels.
    pub fn height(&self) -> u16 {
        self.height
    }

    #[allow(dead_code)] //nop implementation does not use
    const fn index_for_texel(texel: Texel, width: u16) -> usize {
        (texel.y as usize * width as usize) + texel.x as usize
    }

    /// Replaces a rectangular region of the texture with new pixel data.
    ///
    /// This method copies pixel data from a source buffer into the texture at the specified
    /// destination position. The source data is assumed to be tightly packed with the given width.
    ///
    /// # Arguments
    ///
    /// * `src_width` - The width of the source data in pixels (for calculating row stride)
    /// * `dst_texel` - The top-left corner where the data should be written in the texture
    /// * `data` - The pixel data to write, in row-major order
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use images_and_words::bindings::forward::dynamic::frame_texture::IndividualTexture;
    /// # use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
    /// # use images_and_words::bindings::software::texture::Texel;
    /// # let mut texture: IndividualTexture<RGBA8UNorm> = todo!();
    /// // Write a 2x2 red square at position (10, 10)
    /// let red = Unorm4 { r: 255, g: 0, b: 0, a: 255 };
    /// texture.replace(
    ///     2, // source width
    ///     Texel { x: 10, y: 10 },
    ///     &[red, red, red, red] // 2x2 pixels
    /// );
    /// ```
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
    /// Creates a new multibuffered dynamic texture.
    ///
    /// This constructor initializes a texture with the specified dimensions and pixel format,
    /// setting up the multibuffer system for efficient CPU-to-GPU transfers.
    ///
    /// # Arguments
    ///
    /// * `bound_device` - The GPU device to create the texture on
    /// * `width` - Width of the texture in pixels
    /// * `height` - Height of the texture in pixels
    /// * `visible_to` - How the texture will be used in shaders (sampling, reading, etc.)
    /// * `_cpu_strategy` - Hint about CPU access patterns (currently unused but reserved)
    /// * `debug_name` - A name for debugging and profiling tools
    /// * `initialize_with` - Function to initialize each pixel's value
    /// * `priority` - Task priority for async operations
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
    /// # use images_and_words::pixel_formats::{R32Float};
    /// # use images_and_words::bindings::software::texture::Texel;
    /// # use images_and_words::bindings::visible_to::{TextureUsage, CPUStrategy};
    /// # use images_and_words::Priority;
    /// # use std::sync::Arc;
    /// # async fn example(device: Arc<images_and_words::images::BoundDevice>) {
    /// // Create a texture for height map data
    /// let height_map = FrameTexture::<R32Float>::new(
    ///     &device,
    ///     512, 512,
    ///     TextureUsage::VertexShaderRead,
    ///     CPUStrategy::WontRead,
    ///     "terrain_height",
    ///     |texel| {
    ///         // Initialize with a simple gradient
    ///         (texel.x as f32 + texel.y as f32) / 1024.0
    ///     },
    ///     Priority::UserInitiated
    /// ).await;
    /// # }
    /// ```
    pub async fn new<I: Fn(Texel) -> Format::CPixel>(
        bound_device: &Arc<BoundDevice>, 
        width: u16, 
        height: u16, 
        visible_to: TextureUsage, 
        _cpu_strategy: CPUStrategy, 
        debug_name: &str, 
        initialize_with: I, 
        priority: Priority
    ) -> Self  {
        let gpu = imp::GPUableTexture::new(bound_device, width, height, visible_to, debug_name, priority).await.unwrap();
        let cpu = imp::MappableTexture::new(bound_device, width, height, debug_name, priority, initialize_with);
        let individual_texture = IndividualTexture {
            cpu,
            width, height,
        };
        
        let multibuffer = Multibuffer::new(individual_texture, gpu, true);
        let shared = Arc::new(Shared {
            multibuffer
        });
        Self {
            shared,
            width, height,
        }
    }
    
    /// Dequeues a texture buffer for writing.
    ///
    /// This method waits until a buffer is available for CPU writing, then returns a guard
    /// that provides mutable access to the texture data. The guard automatically enqueues
    /// the buffer for GPU upload when dropped.
    ///
    /// # Async Behavior
    ///
    /// This method will suspend if all buffers are currently in use by the GPU. It resumes
    /// as soon as a buffer becomes available.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
    /// # use images_and_words::pixel_formats::RGBA8UNorm;
    /// # async fn example(texture: &mut FrameTexture<RGBA8UNorm>) {
    /// // Wait for an available buffer
    /// let mut guard = texture.dequeue().await;
    /// 
    /// // Modify the texture through the guard...
    /// // Buffer is automatically enqueued when guard is dropped
    /// # }
    /// ```
    pub async fn dequeue(&mut self) -> CPUWriteGuard<Format>{
        let write_guard = self.shared.multibuffer.access_write().await;
        CPUWriteGuard {
            underlying: write_guard,
        }
    }
    
    /// Returns a read-only view of the last texture submitted to the GPU.
    ///
    /// This method provides access to the most recent texture data that was enqueued
    /// for GPU upload. If no texture has been submitted yet, it returns the initial
    /// texture data.
    ///
    /// # Note
    ///
    /// This method is currently unimplemented and will panic if called.
    pub fn last(&self) -> CPUReadGuard<Format> {
        todo!()
    }

    /// Gets a render-side handle for binding this texture in render passes.
    ///
    /// The returned [`TextureRenderSide`] can be used with [`BindStyle`](crate::bindings::BindStyle)
    /// to bind this texture to shader slots. The render side automatically handles
    /// synchronization and always provides the most recent fully-uploaded texture to the GPU.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use images_and_words::bindings::BindStyle;
    /// # use images_and_words::bindings::bind_style::{BindSlot, Stage};
    /// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
    /// # use images_and_words::pixel_formats::RGBA8UNorm;
    /// # let texture: FrameTexture<RGBA8UNorm> = todo!();
    /// // This is an internal method - users should pass FrameTexture directly to bind_dynamic_texture
    /// let render_side = texture.render_side();
    /// ```
    pub(crate) fn render_side(&self) -> TextureRenderSide<Format> {
        TextureRenderSide {
           shared: self.shared.clone(),
        }
    }
    
    /// Returns the width of the texture in pixels.
    pub fn width(&self) -> u16 {
        self.width
    }
    
    /// Returns the height of the texture in pixels.
    pub fn height(&self) -> u16 {
        self.height
    }
    
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn gpu_dirty_receiver(&self) -> DirtyReceiver {
        self.shared.multibuffer.gpu_dirty_receiver()
    }
}

