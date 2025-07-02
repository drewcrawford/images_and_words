// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
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

```
# if cfg!(not(feature="backend_wgpu")) { return; }
# #[cfg(feature = "testing")]
# {
# use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
# use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
# use images_and_words::bindings::software::texture::Texel;
# use images_and_words::bindings::visible_to::{TextureUsage, CPUStrategy, TextureConfig};
# use images_and_words::images::projection::WorldCoord;
# use images_and_words::images::view::View;
# use images_and_words::Priority;
test_executors::sleep_on(async {
# let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
let device = engine.bound_device();

// Create a 256x256 RGBA texture
let config = TextureConfig {
    width: 256,
    height: 256,
    visible_to: TextureUsage::FragmentShaderSample,
    debug_name: "my_dynamic_texture",
    priority: Priority::UserInitiated,
    cpu_strategy: CPUStrategy::WontRead,
    mipmaps: false,
};
let mut texture = FrameTexture::<RGBA8UNorm>::new(
    &device,
    config,
    |texel| Unorm4 { r: 0, g: 0, b: 0, a: 255 }, // Initialize to black
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
# });
# }
```

# Architecture

The module uses several internal types to manage the multibuffering system:

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

# See Also

- [`forward::static::Texture`](crate::bindings::forward::static::texture::Texture) - For textures that don't need updates after creation
- [`forward::dynamic::Buffer`](crate::bindings::forward::dynamic::buffer::Buffer) - For dynamic structured data
- [`bindings`](crate::bindings) module documentation - For understanding the full type organization
*/

use crate::bindings::dirty_tracking::DirtyReceiver;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::software::texture::Texel;
use crate::bindings::visible_to::TextureConfig;
use crate::images::device::BoundDevice;
use crate::imp;
use crate::imp::BackendSend;
use crate::multibuffer::Multibuffer;
use crate::pixel_formats::sealed::PixelFormat;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;

trait DynRenderSide: Send + Debug + Sync {
    ///
    /// # Safety
    /// Must hold the guard for the lifetime of the GPU texture access.
    #[allow(dead_code)] //nop implementation does not use
    unsafe fn acquire_gpu_texture(&self) -> GPUAccess;
    fn gpu_dirty_receiver(&self) -> DirtyReceiver;
}

#[derive(Debug, Clone)]
pub(crate) struct ErasedTextureRenderSide {
    imp: Arc<dyn DynRenderSide>,
}

impl PartialEq for ErasedTextureRenderSide {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.imp, &other.imp)
    }
}

impl ErasedTextureRenderSide {
    #[allow(dead_code)] //nop implementation does not use
    pub unsafe fn acquire_gpu_texture(&self) -> GPUAccess {
        unsafe { self.imp.acquire_gpu_texture() }
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
/// ```
/// # if cfg!(not(feature="backend_wgpu")) { return; }
/// # #[cfg(feature = "testing")]
/// # {
/// # use images_and_words::bindings::BindStyle;
/// # use images_and_words::bindings::bind_style::{BindSlot, Stage};
/// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
/// # use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
/// # use images_and_words::bindings::visible_to::{TextureUsage, CPUStrategy, TextureConfig};
/// # use images_and_words::images::projection::WorldCoord;
/// # use images_and_words::images::view::View;
/// # use images_and_words::Priority;
/// test_executors::sleep_on(async {
/// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
/// let device = engine.bound_device();
/// # let config = TextureConfig { width: 256, height: 256, visible_to: TextureUsage::FragmentShaderSample, debug_name: "test", priority: Priority::UserInitiated, cpu_strategy: CPUStrategy::WontRead, mipmaps: false };
/// # let frame_texture = FrameTexture::<RGBA8UNorm>::new(&device, config, |_| Unorm4 { r: 0, g: 0, b: 0, a: 255 }).await;
/// let mut bind_style = BindStyle::new();
///
/// // Bind the texture to slot 0 for the fragment shader
/// bind_style.bind_dynamic_texture(BindSlot::new(0), Stage::Fragment, &frame_texture);
/// # });
/// # }
/// ```
pub(crate) struct TextureRenderSide<Format: PixelFormat> {
    shared: Arc<Shared<Format>>,
}

impl<Format: PixelFormat> TextureRenderSide<Format> {
    pub(crate) fn erased(self) -> ErasedTextureRenderSide
    where
        Format: 'static,
    {
        ErasedTextureRenderSide {
            imp: Arc::new(self),
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
    unsafe fn acquire_gpu_texture(&self) -> GPUAccess {
        let mut guard = unsafe { self.shared.multibuffer.access_gpu() };

        // Take the dirty guard if present
        let dirty_guard = guard.take_dirty_guard();

        // Get the render side and GPU texture
        let render_side = guard.as_imp().render_side();
        let gpu_texture: Box<dyn imp::GPUableTextureWrapped> = Box::new(guard.as_imp().clone());

        // Create GPUGuard with the dirty guard stored separately
        let our_guard = GPUGuard {
            underlying: guard,
            dirty_guard,
            render_side: render_side.clone(),
        };

        // Create the dirty guard wrapper if we have dirty data
        let dirty_guard_box = if our_guard.dirty_guard.is_some() {
            Some(Box::new(our_guard) as Box<dyn DynGuard>)
        } else {
            None
        };

        GPUAccess {
            dirty_guard: dirty_guard_box,
            gpu_texture,
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
/// to texture data. The texture data can be modified through the guard's
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
/// ```
/// # if cfg!(not(feature="backend_wgpu")) { return; }
/// # #[cfg(feature = "testing")]
/// # {
/// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
/// # use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
/// # use images_and_words::bindings::software::texture::Texel;
/// # use images_and_words::bindings::visible_to::{TextureUsage, CPUStrategy, TextureConfig};
/// # use images_and_words::images::projection::WorldCoord;
/// # use images_and_words::images::view::View;
/// # use images_and_words::Priority;
/// test_executors::sleep_on(async {
/// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
/// let device = engine.bound_device();
/// # let config = TextureConfig { width: 256, height: 256, visible_to: TextureUsage::FragmentShaderSample, debug_name: "test", priority: Priority::UserInitiated, cpu_strategy: CPUStrategy::WontRead, mipmaps: false };
/// # let mut texture = FrameTexture::<RGBA8UNorm>::new(&device, config, |_| Unorm4 { r: 0, g: 0, b: 0, a: 255 }).await;
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
/// # });
/// # }
/// ```
#[derive(Debug)]
pub struct CPUWriteGuard<'a, Format: PixelFormat> {
    underlying: crate::multibuffer::CPUWriteGuard<
        'a,
        imp::MappableTexture<Format>,
        imp::GPUableTexture<Format>,
    >,
    width: u16,
    height: u16,
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
    underlying:
        crate::multibuffer::GPUGuard<imp::MappableTexture<Format>, imp::GPUableTexture<Format>>,
    // Store the dirty guard separately so we can access the source texture
    dirty_guard: Option<crate::bindings::resource_tracking::GPUGuard<imp::MappableTexture<Format>>>,
    render_side: imp::TextureRenderSide,
}

impl<Format: PixelFormat> Debug for GPUGuard<Format> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GPUGuard")
            .field("underlying", &self.underlying)
            .field("render_side", &self.render_side)
            .finish()
    }
}

impl<Format: PixelFormat> DynGuard for GPUGuard<Format> {
    fn perform_copy(
        &mut self,
        destination: &mut dyn imp::GPUableTextureWrapped,
        copy_info: &mut imp::CopyInfo,
    ) -> Result<(), String> {
        if let Some(dirty_guard) = &mut self.dirty_guard {
            // Dereference the dirty guard to get the MappableTexture
            let source: &mut imp::MappableTexture<Format> = dirty_guard;
            // Use the type-erased copy method
            destination.copy_from_mappable(source, copy_info)
        } else {
            Err("perform_copy called on GPUGuard without dirty data".to_string())
        }
    }
}

/// Guards access to the underlying GPU texture during rendering.
///
/// `GPUAccess` provides access to the GPU texture with optional dirty data
/// that needs to be copied. This type is created internally by the rendering
/// system and maintains the texture's availability for GPU operations.
pub(crate) struct GPUAccess {
    // Option<Box<DynGuard>> if we need to perform a copy, otherwise None (how we erase Format in this field)
    dirty_guard: Option<Box<dyn DynGuard>>,
    // Underlying GPU type, typecast to Box<dyn GPUableTextureWrapped> (how we erase Format in this field)
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) gpu_texture: Box<dyn imp::GPUableTextureWrapped>,
    // The render side for creating views (always available)
    pub(crate) render_side: imp::TextureRenderSide,
}

impl Debug for GPUAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GPUAccess")
            .field("dirty_guard", &self.dirty_guard.is_some())
            .field("render_side", &self.render_side)
            .finish()
    }
}

impl GPUAccess {
    #[allow(dead_code)] //nop implementation does not use
    pub fn take_dirty_guard(&mut self) -> Option<Box<dyn DynGuard>> {
        self.dirty_guard.take()
    }
}

/// Trait for type-erased guard that provides access to source texture for copying
pub(crate) trait DynGuard: Debug + BackendSend {
    /// Perform the copy from the stored source to the given destination
    #[allow(dead_code)] //nop implementation does not use
    fn perform_copy(
        &mut self,
        destination: &mut dyn imp::GPUableTextureWrapped,
        copy_info: &mut imp::CopyInfo,
    ) -> Result<(), String>;
}

///Shared between FrameTexture and TextureRenderSide
struct Shared<Format: PixelFormat> {
    multibuffer: Multibuffer<imp::MappableTexture<Format>, imp::GPUableTexture<Format>>,
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
/// ```
/// # if cfg!(not(feature="backend_wgpu")) { return; }
/// # #[cfg(feature = "testing")]
/// # {
/// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
/// # use images_and_words::pixel_formats::{BGRA8UNormSRGB, BGRA8UnormPixelSRGB};
/// # use images_and_words::bindings::software::texture::Texel;
/// # use images_and_words::bindings::visible_to::{TextureUsage, CPUStrategy, TextureConfig};
/// # use images_and_words::images::projection::WorldCoord;
/// # use images_and_words::images::view::View;
/// # use images_and_words::Priority;
/// test_executors::sleep_on(async {
/// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
/// let device = engine.bound_device();
///
/// // Create a texture for video playback
/// let config = TextureConfig {
///     width: 1920,
///     height: 1080,
///     visible_to: TextureUsage::FragmentShaderSample,
///     debug_name: "video_frame",
///     priority: Priority::UserInitiated,
///     cpu_strategy: CPUStrategy::WontRead,
///     mipmaps: false,
/// };
/// let mut video_texture = FrameTexture::<BGRA8UNormSRGB>::new(
///     &device,
///     config,
///     |_| BGRA8UnormPixelSRGB::ZERO, // Start with black
/// ).await;
///
/// // Simplified example - just write one frame instead of a loop
/// let frame_data: Vec<BGRA8UnormPixelSRGB> = vec![BGRA8UnormPixelSRGB::ZERO; 1920 * 1080];
///
/// // Write frame to texture
/// let mut guard = video_texture.dequeue().await;
/// guard.replace(1920, Texel::ZERO, &frame_data);
/// drop(guard); // Enqueue for GPU
/// # });
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct FrameTexture<Format: PixelFormat> {
    shared: Arc<Shared<Format>>,
    width: u16,
    height: u16,
}

impl<'a, Format: PixelFormat> CPUWriteGuard<'a, Format> {
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
    /// ```
    /// # if cfg!(not(feature="backend_wgpu")) { return; }
    /// # #[cfg(feature = "testing")]
    /// # {
    /// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
    /// # use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
    /// # use images_and_words::bindings::software::texture::Texel;
    /// # use images_and_words::bindings::visible_to::{TextureUsage, CPUStrategy, TextureConfig};
    /// # use images_and_words::images::projection::WorldCoord;
    /// # use images_and_words::images::view::View;
    /// # use images_and_words::Priority;
    /// test_executors::sleep_on(async {
    /// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
    /// let device = engine.bound_device();
    /// # let config = TextureConfig { width: 256, height: 256, visible_to: TextureUsage::FragmentShaderSample, debug_name: "test", priority: Priority::UserInitiated, cpu_strategy: CPUStrategy::WontRead, mipmaps: false };
    /// # let mut frame_texture = FrameTexture::<RGBA8UNorm>::new(&device, config, |_| Unorm4 { r: 0, g: 0, b: 0, a: 255 }).await;
    /// # let mut guard = frame_texture.dequeue().await;
    /// // Write a full row of red pixels at row 10
    /// let red = Unorm4 { r: 255, g: 0, b: 0, a: 255 };
    /// let width = guard.width();
    /// let pixels = vec![red; width as usize];
    /// guard.replace(
    ///     width, // source width must match texture width
    ///     Texel { x: 0, y: 10 },
    ///     &pixels // full row of pixels
    /// );
    /// # });
    /// # }
    /// ```
    pub fn replace(&mut self, src_width: u16, dst_texel: Texel, data: &[Format::CPixel])
    where
        Format: PixelFormat,
    {
        self.underlying.replace(src_width, dst_texel, data);
    }

    /// Asynchronously drops the guard, properly releasing the resource.
    ///
    /// This method must be called before the guard is dropped. Failure to call
    /// this method will result in a panic when the guard's Drop implementation runs.
    pub async fn async_drop(self) {
        self.underlying.async_drop().await;
    }
}

impl<Format: PixelFormat> Mappable for CPUWriteGuard<'_, Format> {
    async fn map_read(&mut self) {
        self.underlying.map_read().await;
    }
    async fn map_write(&mut self) {
        self.underlying.map_write().await;
    }
    async fn unmap(&mut self) {
        self.underlying.unmap().await;
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
    /// * `config` - Texture configuration parameters (dimensions, usage, priority, etc.)
    /// * `initialize_with` - Function to initialize each pixel's value
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(not(feature="backend_wgpu")) { return; }
    /// # #[cfg(feature = "testing")]
    /// # {
    /// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
    /// # use images_and_words::pixel_formats::{R32Float};
    /// # use images_and_words::bindings::software::texture::Texel;
    /// # use images_and_words::bindings::visible_to::{TextureUsage, CPUStrategy, TextureConfig};
    /// # use images_and_words::images::projection::WorldCoord;
    /// # use images_and_words::images::view::View;
    /// # use images_and_words::Priority;
    /// test_executors::sleep_on(async {
    /// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
    /// let device = engine.bound_device();
    ///
    /// // Create a texture for height map data
    /// let config = TextureConfig {
    ///     width: 512,
    ///     height: 512,
    ///     visible_to: TextureUsage::VertexShaderRead,
    ///     debug_name: "terrain_height",
    ///     priority: Priority::UserInitiated,
    ///     cpu_strategy: CPUStrategy::WontRead,
    ///     mipmaps: false,  // Dynamic textures typically don't use mipmaps
    /// };
    ///
    /// let height_map = FrameTexture::<R32Float>::new(
    ///     &device,
    ///     config,
    ///     |texel| {
    ///         // Initialize with a simple gradient
    ///         (texel.x as f32 + texel.y as f32) / 1024.0
    ///     },
    /// ).await;
    /// # });
    /// # }
    /// ```
    pub async fn new<I: Fn(Texel) -> Format::CPixel>(
        bound_device: &Arc<BoundDevice>,
        config: TextureConfig<'_>,
        initialize_with: I,
    ) -> Self {
        let gpu = imp::GPUableTexture::new(bound_device, config)
            .await
            .unwrap();
        let cpu = imp::MappableTexture::new(
            bound_device,
            config.width,
            config.height,
            config.debug_name,
            config.priority,
            initialize_with,
        )
        .await;

        let multibuffer = Multibuffer::new(cpu, gpu, true);
        let shared = Arc::new(Shared { multibuffer });
        Self {
            shared,
            width: config.width,
            height: config.height,
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
    /// ```
    /// # if cfg!(not(feature="backend_wgpu")) { return; }
    /// # #[cfg(feature = "testing")]
    /// # {
    /// # use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
    /// # use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
    /// # use images_and_words::bindings::visible_to::{TextureUsage, CPUStrategy, TextureConfig};
    /// # use images_and_words::images::projection::WorldCoord;
    /// # use images_and_words::images::view::View;
    /// # use images_and_words::Priority;
    /// test_executors::sleep_on(async {
    /// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
    /// let device = engine.bound_device();
    /// # let config = TextureConfig { width: 256, height: 256, visible_to: TextureUsage::FragmentShaderSample, debug_name: "test", priority: Priority::UserInitiated, cpu_strategy: CPUStrategy::WontRead, mipmaps: false };
    /// # let mut texture = FrameTexture::<RGBA8UNorm>::new(&device, config, |_| Unorm4 { r: 0, g: 0, b: 0, a: 255 }).await;
    /// // Wait for an available buffer
    /// let mut guard = texture.dequeue().await;
    ///
    /// // Modify the texture through the guard...
    /// // Buffer is automatically enqueued when guard is dropped
    /// # });
    /// # }
    /// ```
    pub async fn dequeue(&mut self) -> CPUWriteGuard<Format> {
        let write_guard = self.shared.multibuffer.access_write().await;
        CPUWriteGuard {
            underlying: write_guard,
            width: self.width,
            height: self.height,
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
