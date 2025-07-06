// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Static GPU buffer implementation for immutable data.
//!
//! This module provides the [`Buffer`] type for managing GPU buffers that contain
//! data that doesn't change after creation. Static buffers are ideal for resources
//! like mesh geometry, lookup tables, or any other data that remains constant
//! throughout the application's lifetime.
//!
//! # Overview
//!
//! Static buffers differ from dynamic buffers in that they:
//! - Are uploaded to the GPU once during creation
//! - Cannot be modified after creation
//! - Are optimized for GPU-only memory placement
//! - Have lower overhead than dynamic buffers
//!
//! # Use Cases
//!
//! Static buffers are perfect for:
//! - Mesh vertex and index data
//! - Precomputed lookup tables
//! - Constant coefficient arrays
//! - Any data that won't change during runtime
//!
//! # Example
//!
//! ```
//! # if cfg!(not(feature="backend_wgpu")) { return; }
//! # #[cfg(feature = "testing")]
//! # {
//! # use images_and_words::bindings::forward::r#static::buffer::Buffer;
//! # use images_and_words::bindings::forward::dynamic::buffer::CRepr;
//! # use images_and_words::bindings::visible_to::GPUBufferUsage;
//! # use images_and_words::images::projection::WorldCoord;
//! # use images_and_words::images::view::View;
//! # test_executors::spawn_local(async {
//! # let view = images_and_words::images::View::for_testing();
//! # let engine = images_and_words::images::Engine::rendering_to(view, images_and_words::images::projection::WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
//! # let device = engine.bound_device();
//! // Define a vertex type
//! #[repr(C)]
//! struct Vertex {
//!     position: [f32; 3],
//!     color: [f32; 4],
//! }
//!
//! unsafe impl CRepr for Vertex {}
//!
//! // Create a static buffer with triangle vertices
//! let vertex_buffer = Buffer::new(
//!     device.clone(),
//!     3,  // 3 vertices for a triangle
//!     GPUBufferUsage::VertexBuffer,
//!     "triangle_vertices",
//!     |index| match index {
//!         0 => Vertex { position: [-0.5, -0.5, 0.0], color: [1.0, 0.0, 0.0, 1.0] },
//!         1 => Vertex { position: [ 0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0, 1.0] },
//!         2 => Vertex { position: [ 0.0,  0.5, 0.0], color: [0.0, 0.0, 1.0, 1.0] },
//!         _ => unreachable!()
//!     }
//! ).await.expect("Failed to create buffer");
//! # }, "static_buffer_creation_doctest");
//! # }
//! ```
//!
//! # See Also
//!
//! - [`forward::dynamic::Buffer`](crate::bindings::forward::dynamic::buffer::Buffer) - For buffers that need frequent updates
//! - [`forward::static::Texture`](crate::bindings::forward::static::texture::Texture) - For immutable image data
//! - [`bindings`](crate::bindings) module documentation - For understanding the full type organization

use crate::bindings::buffer_access::MapType;
use crate::bindings::forward::dynamic::buffer::CRepr;
use crate::images::BoundDevice;
use crate::imp;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Arc;

/// A static GPU buffer containing immutable data.
///
/// `Buffer<T>` represents a GPU buffer whose contents are set once during creation
/// and cannot be modified afterwards. This makes static buffers ideal for data that
/// doesn't change, such as mesh geometry or lookup tables.
///
/// # Type Parameter
///
/// The type parameter `Element` must implement [`CRepr`] to ensure C-compatible
/// memory layout for GPU interoperability.
///
/// # Performance
///
/// Static buffers offer several performance advantages:
/// - Single upload operation during creation
/// - Can be placed in GPU-only memory for optimal access speed
/// - No synchronization overhead during rendering
/// - No CPU-side memory allocation after creation
///
/// # Example
///
/// ```
/// # if cfg!(not(feature="backend_wgpu")) { return; }
/// # #[cfg(feature = "testing")]
/// # {
/// # use images_and_words::bindings::forward::r#static::buffer::Buffer;
/// # use images_and_words::bindings::forward::dynamic::buffer::CRepr;
/// # use images_and_words::bindings::visible_to::GPUBufferUsage;
/// # use images_and_words::images::projection::WorldCoord;
/// # use images_and_words::images::view::View;
/// # test_executors::spawn_local(async {
/// # let view = images_and_words::images::View::for_testing();
/// # let engine = images_and_words::images::Engine::rendering_to(view, images_and_words::images::projection::WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
/// # let device = engine.bound_device();
/// // Create a buffer of precomputed sine values
/// let sine_lut = Buffer::new(
///     device.clone(),
///     256,
///     GPUBufferUsage::FragmentShaderRead,
///     "sine_lookup_table",
///     |i| (i as f32 * std::f32::consts::TAU / 256.0).sin()
/// ).await.expect("Failed to create buffer");
/// # }, "static_buffer_sine_lut_doctest");
/// # }
/// ```
#[derive(Debug)]
pub struct Buffer<Element> {
    pub(crate) imp: imp::GPUableBuffer,
    #[allow(dead_code)] //nop implementation does not use
    count: usize,
    element: PhantomData<Element>,
}

/// Error type for static buffer operations.
///
/// This error wraps underlying implementation errors that can occur during
/// buffer creation. Common causes include:
///
/// - Out of memory conditions
/// - Invalid buffer sizes (e.g., zero-sized buffers)
/// - GPU device errors
/// - Backend-specific limitations
#[derive(Debug, thiserror::Error)]
#[error("Static buffer error: {0}")]
pub struct Error(#[from] imp::Error);

/// Initializes a byte array with typed elements using a provided initializer function.
///
/// This function is used internally to populate buffer memory with initial values
/// during buffer creation. It handles the unsafe conversion between byte arrays
/// and typed element arrays while ensuring proper initialization.
///
/// # Parameters
///
/// * `element_count` - Number of elements to initialize
/// * `byte_array` - Uninitialized byte array with exactly `element_count * size_of::<Element>()` bytes
/// * `initializer` - Function that produces an element value given its index
///
/// # Returns
///
/// A mutable slice of initialized bytes ready for GPU upload.
///
/// # Panics
///
/// Panics if the byte array length doesn't match the expected size for the given
/// element count and type.
///
/// # Safety
///
/// This function performs unsafe memory transmutation. It's safe because:
/// - The `CRepr` trait ensures `Element` has C-compatible layout
/// - We verify the byte array has the correct size
/// - All bytes are initialized before being returned
pub(crate) fn initialize_byte_array_with<Element, I: Fn(usize) -> Element>(
    element_count: usize,
    byte_array: &mut [MaybeUninit<u8>],
    initializer: I,
) -> &mut [u8]
where
    Element: CRepr,
{
    let byte_size = element_count * std::mem::size_of::<Element>();
    assert_eq!(byte_array.len(), byte_size);
    //transmute to element type
    let as_elements: &mut [MaybeUninit<Element>] = unsafe {
        std::slice::from_raw_parts_mut(
            byte_array.as_mut_ptr() as *mut MaybeUninit<Element>,
            element_count,
        )
    };
    for (i, element) in as_elements.iter_mut().enumerate() {
        *element = MaybeUninit::new(initializer(i));
    }
    //represent that we initialized the buffer!

    unsafe { std::slice::from_raw_parts_mut(byte_array.as_mut_ptr() as *mut u8, byte_size) }
}

impl<Element> Buffer<Element> {
    /// Creates a new static buffer with the specified size and initial data.
    ///
    /// This method creates a GPU buffer and uploads the initial data in a single
    /// operation. Once created, the buffer contents cannot be modified.
    ///
    /// # Parameters
    ///
    /// * `device` - The GPU device to create the buffer on
    /// * `count` - Number of elements in the buffer
    /// * `usage` - How the buffer will be used on the GPU (vertex data, uniform, etc.)
    /// * `debug_name` - Human-readable name for debugging and profiling
    /// * `initializer` - Function to generate each element by index
    ///
    /// # Returns
    ///
    /// Returns `Ok(Buffer)` on success, or an [`Error`] if buffer creation fails.
    ///
    /// # Performance
    ///
    /// The initializer function is called once for each element during buffer creation.
    /// For large buffers, ensure the initializer is efficient.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(not(feature="backend_wgpu")) { return; }
    /// # #[cfg(feature = "testing")]
    /// # {
    /// # use images_and_words::bindings::forward::r#static::buffer::Buffer;
    /// # use images_and_words::bindings::forward::dynamic::buffer::CRepr;
    /// # use images_and_words::bindings::visible_to::GPUBufferUsage;
    /// # use images_and_words::images::projection::WorldCoord;
    /// # use images_and_words::images::view::View;
    /// # test_executors::spawn_local(async {
    /// # let view = images_and_words::images::View::for_testing();
    /// # let engine = images_and_words::images::Engine::rendering_to(view, images_and_words::images::projection::WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
    /// # let device = engine.bound_device();
    /// // Create an index buffer for a quad (two triangles)
    /// let indices = Buffer::new(
    ///     device.clone(),
    ///     6,  // 6 indices for 2 triangles
    ///     GPUBufferUsage::Index,
    ///     "quad_indices",
    ///     |i| match i {
    ///         0 => 0u16, 1 => 1, 2 => 2,  // First triangle
    ///         3 => 2,    4 => 3, 5 => 0,  // Second triangle
    ///         _ => unreachable!()
    ///     }
    /// ).await.expect("Failed to create buffer");
    /// # }, "static_buffer_quad_indices_doctest");
    /// # }
    /// ```
    ///
    /// # Implementation Details
    ///
    /// 1. Creates a CPU-mappable staging buffer
    /// 2. Initializes the staging buffer using the provided initializer
    /// 3. Creates the final GPU buffer with the specified usage
    /// 4. Copies data from staging to GPU buffer
    /// 5. The staging buffer is automatically cleaned up
    pub async fn new(
        device: Arc<BoundDevice>,
        count: usize,
        usage: crate::bindings::visible_to::GPUBufferUsage,
        debug_name: &str,
        initializer: impl Fn(usize) -> Element,
    ) -> Result<Self, Error>
    where
        Element: CRepr,
    {
        let byte_size = std::mem::size_of::<Element>() * count;
        let mappable = imp::MappableBuffer::new(
            device.clone(),
            byte_size,
            MapType::Write,
            debug_name,
            |bytes| initialize_byte_array_with(count, bytes, initializer),
        )
        .await?;

        let imp = imp::GPUableBuffer::new(device, byte_size, usage, debug_name).await;

        imp.copy_from_buffer(mappable, 0, 0, byte_size).await;

        Ok(Self {
            imp,
            count,
            element: PhantomData,
        })
    }
}
