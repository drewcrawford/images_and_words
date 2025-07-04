// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Dynamic buffer implementation for GPU data that changes during runtime.
//!
//! This module provides the [`Buffer`] type for managing GPU buffers that need to be
//! updated dynamically during rendering. Unlike static buffers which are uploaded once,
//! dynamic buffers support efficient CPU-to-GPU updates using a multibuffering strategy.
//!
//! # Overview
//!
//! Dynamic buffers are designed for data that changes during runtime, such as:
//! - Transform matrices that update per frame
//! - Particle system data
//! - UI element positions
//! - Animation data
//!
//! The implementation uses multibuffering to allow the CPU to write new data while
//! the GPU is still reading previous frames, avoiding pipeline stalls.
//!
//! # Architecture
//!
//! The module uses several key components:
//!
//! - [`Buffer`] - The main public interface for creating and accessing dynamic buffers
//! - [`CPUWriteAccess`] - CPU-side buffer that can be mapped for writing
//! - `RenderSide` - GPU-side handle used during rendering
//! - `Shared` - Shared state managing the multibuffer synchronization
//!
//! # Type Safety
//!
//! Buffers are generic over their element type `T`, which must implement the [`CRepr`]
//! trait to ensure C-compatible memory layout. This guarantees that data written from
//! the CPU will be correctly interpreted by GPU shaders.
//!
//! # Example
//!
//! ```
//! # if cfg!(not(feature="backend_wgpu")) { return; }
//! # #[cfg(feature = "testing")]
//! # {
//! use std::sync::Arc;
//! use images_and_words::bindings::forward::dynamic::buffer::{Buffer, CRepr};
//! use images_and_words::bindings::visible_to::GPUBufferUsage;
//! use images_and_words::images::projection::WorldCoord;
//! use images_and_words::images::view::View;
//! test_executors::sleep_on(async {
//! # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
//! let device = engine.bound_device();
//! // Define a C-compatible struct
//! #[repr(C)]
//! struct Vertex {
//!     x: f32,
//!     y: f32,
//!     z: f32,
//! }
//!
//! unsafe impl CRepr for Vertex {}
//!
//! // Create a dynamic buffer for 100 vertices
//! let buffer = Buffer::new(
//!     device.clone(),
//!     100,
//!     GPUBufferUsage::VertexBuffer,
//!     "vertex_buffer",
//!     |index| Vertex {
//!         x: index as f32,
//!         y: 0.0,
//!         z: 0.0,
//!     }
//! ).expect("Failed to create buffer");
//!
//! // Update buffer data
//! let mut write_guard = buffer.access_write().await;
//! write_guard.write(&[Vertex {
//!     x: 1.0,
//!     y: 2.0,
//!     z: 3.0,
//! }], 0);
//! # });
//! # }
//! ```
//!
//! # See Also
//!
//! - [`forward::static::Buffer`](crate::bindings::forward::static::buffer::Buffer) - For buffers that don't need updates after creation
//! - [`forward::dynamic::FrameTexture`](crate::bindings::forward::dynamic::frame_texture::FrameTexture) - For dynamic image data
//! - [`bindings`](crate::bindings) module documentation - For understanding the full type organization

use crate::bindings::dirty_tracking::DirtyReceiver;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::visible_to::GPUBufferUsage;
use crate::images::BoundDevice;
use crate::imp;
use crate::imp::SendPhantom;
use crate::multibuffer::CPUWriteGuard;
use crate::multibuffer::Multibuffer;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index};
use std::sync::Arc;

/// Indicates how frequently a dynamic buffer will be updated.
///
/// This enum helps the graphics backend optimize buffer placement and update strategies
/// based on expected usage patterns. While not currently used in the implementation,
/// it provides a foundation for future optimizations.
///
/// # Future Optimizations
///
/// Different write frequencies could lead to:
/// - Different memory placement strategies (system RAM vs GPU memory)
/// - Different synchronization mechanisms
/// - Different multibuffering depths
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WriteFrequency {
    /// Buffer updates significantly less than once per frame.
    ///
    /// Use this for data that changes occasionally, such as:
    /// - Level-of-detail settings
    /// - Configuration data
    /// - Infrequently updated game state
    Infrequent,

    /// Buffer updates roughly once per frame.
    ///
    /// Use this for data that changes every frame or nearly every frame, such as:
    /// - Transform matrices
    /// - Animation data
    /// - Per-frame uniform data
    Frequent,
}

/// Shared state between CPU and GPU sides of a dynamic buffer.
///
/// This struct contains the multibuffer that coordinates access between
/// CPU writes and GPU reads, ensuring proper synchronization.
struct Shared {
    multibuffer: Multibuffer<imp::MappableBuffer, imp::GPUableBuffer>,
}
impl Debug for Shared {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("multibuffer", &self.multibuffer)
            .finish()
    }
}
/// A dynamically updatable GPU buffer.
///
/// `Buffer<T>` represents a GPU buffer whose contents can be efficiently updated
/// from the CPU during runtime. It uses a multibuffering strategy to allow
/// simultaneous CPU writes and GPU reads without stalling the pipeline.
///
/// # Type Parameter
///
/// The type parameter `Element` must implement [`CRepr`] to ensure C-compatible
/// memory layout for GPU interoperability.
///
/// # Thread Safety
///
/// This type is `Clone` and can be safely shared between threads. The underlying
/// synchronization ensures that CPU writes and GPU reads are properly coordinated.
///
/// # Example
///
/// ```
/// # if cfg!(not(feature="backend_wgpu")) { return; }
/// # #[cfg(feature = "testing")]
/// # {
/// use std::sync::Arc;
/// use images_and_words::bindings::forward::dynamic::buffer::{Buffer, CRepr};
/// use images_and_words::bindings::visible_to::GPUBufferUsage;
/// use images_and_words::images::projection::WorldCoord;
/// use images_and_words::images::view::View;
/// test_executors::sleep_on(async {
/// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
/// let device = engine.bound_device();
/// // Create a buffer for float values
/// let float_buffer = Buffer::<f32>::new(
///     device.clone(),
///     100,  // 100 floats
///     GPUBufferUsage::VertexShaderRead,
///     "float_data",
///     |i| i as f32 * 0.1  // Initialize with scaled index
/// ).expect("Failed to create buffer");
/// # });
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Buffer<Element> {
    shared: Arc<Shared>,
    count: usize,
    debug_name: String,
    _phantom: PhantomData<Element>,
}

/// A CPU-accessible buffer instance within the multibuffer system.
///
/// `IndividualBuffer` represents a single buffer that can be mapped for CPU access.
/// It's part of the internal multibuffering implementation and is accessed through
/// [`Buffer::access_write`].
///
/// This type provides direct access to buffer memory for reading and writing,
/// with proper synchronization handled by the multibuffer system.
///
/// # Safety
///
/// The buffer memory is directly accessible through indexing and the `write` method.
/// Users must ensure they don't write out of bounds.
pub struct CPUWriteAccess<'a, Element> {
    guard: CPUWriteGuard<'a, imp::MappableBuffer, imp::GPUableBuffer>,
    _marker: SendPhantom<Element>,
    count: usize,
}

impl<Element> Debug for CPUWriteAccess<'_, Element> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndividualBuffer")
            .field("guard", &self.guard)
            .field("count", &self.count)
            .finish()
    }
}

impl<Element> Index<usize> for CPUWriteAccess<'_, Element> {
    type Output = Element;
    fn index(&self, index: usize) -> &Self::Output {
        let offset = index * std::mem::size_of::<Element>();
        let bytes: &[u8] =
            &self.guard.deref().as_slice()[offset..offset + std::mem::size_of::<Element>()];
        unsafe { &*(bytes.as_ptr() as *const Element) }
    }
}

impl<Element> CPUWriteAccess<'_, Element> {
    /// Writes data to the buffer at the given offset.
    ///
    /// # Parameters
    ///
    /// * `data` - Slice of elements to write to the buffer
    /// * `dst_offset` - Starting element index in the buffer where data will be written
    ///
    /// # Panics
    ///
    /// Panics if `dst_offset + data.len()` exceeds the buffer size.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(not(feature="backend_wgpu")) { return; }
    /// # #[cfg(feature = "testing")]
    /// # {
    /// use images_and_words::bindings::forward::dynamic::buffer::{Buffer, CRepr};
    /// use images_and_words::bindings::visible_to::GPUBufferUsage;
    /// use images_and_words::images::projection::WorldCoord;
    /// use images_and_words::images::view::View;
    /// test_executors::sleep_on(async {
    /// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
    /// let device = engine.bound_device();
    /// let buffer = Buffer::<f32>::new(device.clone(), 100, GPUBufferUsage::VertexShaderRead, "test", |i| i as f32).expect("Failed to create buffer");
    /// let mut write_guard = buffer.access_write().await;
    ///
    /// // Write 3 floats starting at index 5
    /// write_guard.write(&[1.0, 2.0, 3.0], 5);
    /// # });
    /// # }
    /// ```
    pub fn write(&mut self, data: &[Element], dst_offset: usize)
    where
        Element: CRepr,
    {
        let offset = dst_offset * std::mem::size_of::<Element>();
        let bytes = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data))
        };
        self.guard.deref_mut().write(bytes, offset);
    }
}
impl<Element> Mappable for CPUWriteAccess<'_, Element> {
    async fn map_read(&mut self) {
        self.guard.deref_mut().map_read().await;
    }
    async fn map_write(&mut self) {
        self.guard.deref_mut().map_write().await;
    }
    fn unmap(&mut self) {
        self.guard.deref_mut().unmap();
    }
    fn byte_len(&self) -> usize {
        self.count * std::mem::size_of::<Element>()
    }
}

/// GPU-side handle for a dynamic buffer.
///
/// `RenderSide` provides access to the buffer during GPU operations. It manages
/// synchronization with the CPU side and ensures that the GPU always reads the
/// most recent data that has been written by the CPU.
///
/// This type is used internally by the render pass system and is not directly
/// accessible to users.
pub(crate) struct RenderSide<Element> {
    shared: Arc<Shared>,
    count: usize,
    #[allow(dead_code)] //nop implementation does not use
    debug_name: String,
    _phantom: PhantomData<Element>,
}

impl<Element> Debug for RenderSide<Element> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderSide")
            .field("shared", &self.shared)
            .field("count", &self.count)
            .finish()
    }
}

/// Guards access to the underlying GPU buffer during rendering.
///
/// `GPUAccess` ensures exclusive access to the GPU buffer while it's being used
/// in a render pass. This type is created internally by the rendering system
/// and maintains the buffer's availability for GPU operations.
///
/// The guard implements RAII - the buffer is automatically released when
/// the guard is dropped, allowing the multibuffer system to recycle it.
#[derive(Debug)]
pub(crate) struct GPUAccess {
    #[allow(dead_code)] //nop implementation does not use
    dirty_guard: Option<crate::bindings::resource_tracking::GPUGuard<imp::MappableBuffer>>,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) gpu_buffer: imp::GPUableBuffer,
}
impl GPUAccess {
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn as_ref(&self) -> &imp::GPUableBuffer {
        &self.gpu_buffer
    }
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn take_dirty_guard(
        &mut self,
    ) -> Option<crate::bindings::resource_tracking::GPUGuard<imp::MappableBuffer>> {
        self.dirty_guard.take()
    }
}

impl<Element> RenderSide<Element> {
    pub(crate) fn erased_render_side(self) -> ErasedRenderSide
    where
        Element: Send + Sync + 'static,
    {
        ErasedRenderSide {
            element_size: std::mem::size_of::<Element>(),
            byte_size: self.count * std::mem::size_of::<Element>(),
            imp: Arc::new(self),
        }
    }
}

/// Type-erased interface for render-side buffer operations.
///
/// This trait provides a uniform interface for accessing GPU buffers during
/// rendering, abstracting over the specific element type. It's used internally
/// by the render pass system to manage buffers of different types.
pub(crate) trait SomeRenderSide: Send + Sync + Debug {
    /// Acquires exclusive access to the GPU buffer for rendering.
    ///
    /// # Safety
    ///
    /// The caller must keep the returned guard alive for the entire duration
    /// of GPU operations using this buffer.
    #[allow(dead_code)] //nop implementation does not use
    unsafe fn acquire_gpu_buffer(&self) -> GPUAccess;

    /// Returns a receiver for dirty state notifications.
    ///
    /// This is used by the render system to track when buffers need to be
    /// copied from CPU to GPU memory.
    fn dirty_receiver(&self) -> DirtyReceiver;

    /// Returns a raw pointer to the underlying GPU buffer.
    ///
    /// # Safety
    ///
    /// This method bypasses all synchronization. The caller must ensure
    /// no data races occur.
    #[allow(dead_code)] //nop implementation does not use
    unsafe fn unsafe_imp(&self) -> &imp::GPUableBuffer;
}

impl<Element: Send + Sync + 'static> SomeRenderSide for RenderSide<Element> {
    unsafe fn acquire_gpu_buffer(&self) -> GPUAccess {
        let mut underlying_guard = unsafe { self.shared.multibuffer.access_gpu() };

        // Take the dirty guard if present
        let dirty_guard = underlying_guard.take_dirty_guard();

        // Get the GPU buffer - clone it from the guard
        let gpu_buffer = underlying_guard.as_imp().clone();

        GPUAccess {
            dirty_guard,
            gpu_buffer,
        }
    }
    fn dirty_receiver(&self) -> DirtyReceiver {
        self.shared.multibuffer.gpu_dirty_receiver()
    }
    unsafe fn unsafe_imp(&self) -> &imp::GPUableBuffer {
        unsafe { self.shared.multibuffer.access_gpu_unsafe() }
    }
}

/// Type-erased render-side handle for dynamic buffers.
///
/// This struct allows the render system to work with buffers of different element
/// types uniformly. It stores metadata about the buffer (element size, total size)
/// along with a type-erased handle to the actual buffer implementation.
///
/// Used internally by the binding system to pass buffers to render passes without
/// requiring knowledge of the specific element type.
#[derive(Debug, Clone)]
pub(crate) struct ErasedRenderSide {
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) element_size: usize,
    pub(crate) imp: Arc<dyn SomeRenderSide>,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) byte_size: usize,
}

impl PartialEq for ErasedRenderSide {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.imp, &other.imp)
    }
}

impl ErasedRenderSide {
    pub fn dirty_receiver(&self) -> DirtyReceiver {
        self.imp.dirty_receiver()
    }
}

/// Error type for dynamic buffer operations.
///
/// This error wraps underlying implementation errors that can occur during
/// buffer creation or operations. Common causes include:
///
/// - Out of memory conditions
/// - Invalid buffer sizes (e.g., zero-sized buffers)
/// - GPU device errors
/// - Backend-specific limitations
#[derive(thiserror::Error, Debug)]
pub struct Error(#[from] imp::Error);

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl<Element> Buffer<Element> {
    /// Creates a new dynamic buffer with the specified size and usage.
    ///
    /// # Parameters
    ///
    /// * `bound_device` - The GPU device to create the buffer on
    /// * `size` - Number of elements in the buffer
    /// * `usage` - How the buffer will be used on the GPU (vertex data, uniform, etc.)
    /// * `debug_name` - Human-readable name for debugging and profiling
    /// * `initialize_with` - Function to initialize each element by index
    ///
    /// # Returns
    ///
    /// Returns `Ok(Buffer)` on success, or an [`Error`] if buffer creation fails.
    ///
    /// # Panics
    ///
    /// Panics if `size` is 0, as zero-sized buffers are not allowed.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(not(feature="backend_wgpu")) { return; }
    /// # #[cfg(feature = "testing")]
    /// # {
    /// use std::sync::Arc;
    /// use images_and_words::bindings::forward::dynamic::buffer::{Buffer, CRepr};
    /// use images_and_words::bindings::visible_to::GPUBufferUsage;
    /// use images_and_words::images::projection::WorldCoord;
    /// use images_and_words::images::view::View;
    /// test_executors::sleep_on(async {
    /// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
    /// let device = engine.bound_device();
    /// // Create a buffer of 256 floats initialized to their index
    /// let buffer = Buffer::new(
    ///     device.clone(),
    ///     256,
    ///     GPUBufferUsage::FragmentShaderRead,
    ///     "index_buffer",
    ///     |i| i as f32
    /// ).expect("Failed to create buffer");
    /// # });
    /// # }
    /// ```
    pub fn new(
        bound_device: Arc<BoundDevice>,
        size: usize,
        usage: GPUBufferUsage,
        debug_name: &str,
        initialize_with: impl Fn(usize) -> Element,
    ) -> Result<Self, Error>
    where
        Element: CRepr,
    {
        let byte_size = size * std::mem::size_of::<Element>();
        assert_ne!(byte_size, 0, "Zero-sized buffers are not allowed");

        let map_type = crate::bindings::buffer_access::MapType::Write; //todo: optimize for read vs write, etc.

        let mappable_buffer = imp::MappableBuffer::new(
            bound_device.clone(),
            byte_size,
            map_type,
            debug_name,
            move |byte_array| {
                crate::bindings::forward::r#static::buffer::initialize_byte_array_with(
                    size,
                    byte_array,
                    initialize_with,
                )
            },
        )?;

        let gpu_buffer = imp::GPUableBuffer::new(bound_device, byte_size, usage, debug_name);

        Ok(Self {
            shared: Arc::new(Shared {
                multibuffer: Multibuffer::new(mappable_buffer, gpu_buffer, true),
            }),
            count: size,
            debug_name: debug_name.to_string(),
            _phantom: PhantomData,
        })
    }
    /// Acquires write access to the buffer's CPU-side data.
    ///
    /// This method waits until a buffer is available for CPU writing and returns
    /// a guard that provides mutable access to the buffer contents. The buffer
    /// remains locked for CPU access until the guard is dropped.
    ///
    /// When the guard is dropped, the buffer is automatically marked as dirty,
    /// signaling that its contents need to be uploaded to the GPU.
    ///
    /// # Asynchronous Behavior
    ///
    /// This method may suspend if all buffers in the multibuffer are currently
    /// being used by the GPU. It will resume once a buffer becomes available.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(not(feature="backend_wgpu")) { return; }
    /// # #[cfg(feature = "testing")]
    /// # {
    /// use images_and_words::bindings::forward::dynamic::buffer::{Buffer, CRepr};
    /// use images_and_words::bindings::visible_to::GPUBufferUsage;
    /// use images_and_words::images::projection::WorldCoord;
    /// use images_and_words::images::view::View;
    /// test_executors::sleep_on(async {
    /// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
    /// let device = engine.bound_device();
    /// let buffer = Buffer::<f32>::new(device.clone(), 100, GPUBufferUsage::VertexShaderRead, "test", |i| i as f32).expect("Failed to create buffer");
    /// let mut write_guard = buffer.access_write().await;
    ///
    /// // Update buffer contents
    /// write_guard.write(&[1.0, 2.0, 3.0], 0);
    ///
    /// // Guard automatically marks buffer as dirty when dropped
    /// # });
    /// # }
    /// ```
    pub async fn access_write(&self) -> CPUWriteAccess<'_, Element> {
        let guard = self.shared.multibuffer.access_write().await;

        CPUWriteAccess {
            guard,
            _marker: SendPhantom::new(),
            count: self.count,
        }
    }

    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn gpu_dirty_receiver(&self) -> DirtyReceiver {
        self.shared.multibuffer.gpu_dirty_receiver()
    }

    /// Creates a render-side handle for use in GPU operations.
    ///
    /// This method is used internally by the binding system to create a handle
    /// that can be used during rendering. The returned `RenderSide` provides
    /// access to the GPU buffer during render passes.
    pub(crate) fn render_side(&self) -> RenderSide<Element> {
        RenderSide {
            shared: self.shared.clone(),
            count: self.count,
            debug_name: self.debug_name.clone(),
            _phantom: PhantomData,
        }
    }
}

/// Marker trait for types with C-compatible memory representation.
///
/// Types implementing this trait are guaranteed to have a memory layout that
/// matches their C equivalents, allowing them to be safely passed between
/// CPU and GPU code.
///
/// # Safety
///
/// This trait is `unsafe` because incorrect implementation can lead to undefined
/// behavior when data is interpreted by the GPU. Implementors must ensure:
///
/// - The type is `#[repr(C)]` or a primitive type
/// - The type contains no padding that could have undefined values
/// - All fields recursively satisfy C representation requirements
/// - The type contains no pointers or references
///
/// # Implementing for Custom Types
///
/// ```
/// # if cfg!(not(feature="backend_wgpu")) { return; }
/// # #[cfg(feature = "testing")]
/// # {
/// #[repr(C)]
/// struct Vertex {
///     position: [f32; 3],
///     normal: [f32; 3],
///     uv: [f32; 2],
/// }
///
/// // Safety: Vertex is repr(C) and contains only CRepr types
/// unsafe impl images_and_words::bindings::forward::dynamic::buffer::CRepr for Vertex {}
/// # }
/// ```
///
/// # Pre-implemented Types
///
/// This trait is already implemented for all primitive numeric types that
/// are commonly used in GPU programming.
pub unsafe trait CRepr {}

unsafe impl CRepr for u64 {}
unsafe impl CRepr for u32 {}
unsafe impl CRepr for u16 {}
unsafe impl CRepr for u8 {}
unsafe impl CRepr for f32 {}
unsafe impl CRepr for f64 {}
unsafe impl CRepr for i32 {}
unsafe impl CRepr for i16 {}
unsafe impl CRepr for i8 {}
