//! Resource visibility and usage declarations for GPU rendering.
//!
//! This module provides enums that describe how various GPU resources (textures and buffers)
//! will be accessed by different parts of the rendering pipeline. These declarations help
//! the graphics backend optimize resource allocation, memory placement, and synchronization.
//!
//! # Overview
//!
//! When creating GPU resources, it's important to declare how they will be used so the graphics
//! driver can make optimal decisions about memory allocation and access patterns. This module
//! provides three key enums:
//!
//! - [`TextureUsage`] - Declares how textures will be accessed by shaders
//! - [`GPUBufferUsage`] - Declares how buffers will be used on the GPU
//! - [`CPUStrategy`] - Declares CPU access patterns for resources
//!
//! # Examples
//!
//! ```
//! use images_and_words::bindings::visible_to::{TextureUsage, GPUBufferUsage, CPUStrategy};
//!
//! // Declare a texture that will be sampled in fragment shaders
//! let texture_usage = TextureUsage::FragmentShaderSample;
//!
//! // Declare a buffer that will be used as vertex data
//! let buffer_usage = GPUBufferUsage::VertexBuffer;
//!
//! // Indicate that the CPU won't frequently read back from this resource
//! let cpu_strategy = CPUStrategy::WontRead;
//! ```

/// Describes how a texture resource will be used by shaders in the rendering pipeline.
///
/// This enum helps the graphics backend understand which shader stages will access
/// a texture and whether it will be read directly or sampled. The distinction between
/// reading and sampling is important:
///
/// - **Reading** means direct texel fetch operations (e.g., `texelFetch` in GLSL)
/// - **Sampling** means using texture sampling with filtering and wrapping modes
///
/// # Examples
///
/// ```
/// use images_and_words::bindings::visible_to::TextureUsage;
///
/// // A texture that will be sampled in the fragment shader (common for diffuse textures)
/// let diffuse_usage = TextureUsage::FragmentShaderSample;
///
/// // A texture containing vertex data that will be read in the vertex shader
/// let vertex_data_usage = TextureUsage::VertexShaderRead;
///
/// // A normal map that needs to be sampled in both vertex and fragment shaders
/// let normal_map_usage = TextureUsage::VertexAndFragmentShaderSample;
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureUsage {
    /// The texture will be read directly (without sampling) in the fragment shader.
    ///
    /// Use this for textures that store data rather than images, where you need
    /// exact texel values without filtering.
    FragmentShaderRead,

    /// The texture will be read directly (without sampling) in the vertex shader.
    ///
    /// Common for textures storing per-vertex or per-instance data.
    VertexShaderRead,

    /// The texture will be read directly in both vertex and fragment shaders.
    ///
    /// Use when the same texture data is needed in multiple shader stages.
    VertexAndFragmentShaderRead,

    /// The texture will be sampled (with filtering) in fragment shaders.
    ///
    /// This is the most common usage for image textures like diffuse maps,
    /// normal maps, etc.
    FragmentShaderSample,

    /// The texture will be sampled (with filtering) in vertex shaders.
    ///
    /// Less common, but useful for displacement mapping or vertex animation textures.
    VertexShaderSample,

    /// The texture will be sampled in both vertex and fragment shaders.
    ///
    /// Use when the same texture needs to be sampled in multiple shader stages,
    /// such as for height maps used for both displacement and parallax mapping.
    VertexAndFragmentShaderSample,
}

/// Describes how a buffer resource will be used on the GPU.
///
/// This enum covers the various ways buffers can be accessed in the rendering pipeline,
/// from shader data access to specialized uses like vertex and index buffers.
///
/// # Examples
///
/// ```
/// use images_and_words::bindings::visible_to::GPUBufferUsage;
///
/// // A buffer containing vertex positions and colors
/// let vertex_buffer_usage = GPUBufferUsage::VertexBuffer;
///
/// // A buffer containing triangle indices
/// let index_buffer_usage = GPUBufferUsage::Index;
///
/// // A uniform buffer accessed by the fragment shader
/// let uniform_usage = GPUBufferUsage::FragmentShaderRead;
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GPUBufferUsage {
    /// The buffer will be read as data in the vertex shader.
    ///
    /// Use for uniform buffers, storage buffers, or other data accessed
    /// by vertex shaders.
    VertexShaderRead,

    /// The buffer will be read as data in the fragment shader.
    ///
    /// Use for uniform buffers, storage buffers, or other data accessed
    /// by fragment shaders.
    FragmentShaderRead,

    /// The buffer contains vertex attribute data.
    ///
    /// This buffer will be bound as a vertex buffer and its contents will be
    /// interpreted according to the vertex layout/format specified in the pipeline.
    VertexBuffer,

    /// The buffer contains index data for indexed drawing.
    ///
    /// This buffer will be bound as an index buffer and used to specify the
    /// order in which vertices are assembled into primitives.
    Index,
}

/// Describes the CPU's access pattern for a GPU resource.
///
/// This enum helps the graphics backend decide where to place resources in memory
/// (e.g., GPU-only memory vs. shared memory) and how to optimize synchronization.
///
/// # Performance Considerations
///
/// - `ReadsFrequently`: Resources may be placed in memory that's accessible to both
///   CPU and GPU, which might be slower for GPU access but faster for CPU readback.
/// - `WontRead`: Resources can be placed in GPU-only memory for optimal GPU performance.
///
/// # Examples
///
/// ```
/// use images_and_words::bindings::visible_to::CPUStrategy;
///
/// // A render target that we'll read back for screenshots
/// let screenshot_strategy = CPUStrategy::ReadsFrequently;
///
/// // A texture that stays on the GPU
/// let texture_strategy = CPUStrategy::WontRead;
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CPUStrategy {
    /// The CPU will frequently read from this resource.
    ///
    /// Use this when you need to read back rendered results, capture screenshots,
    /// or perform CPU-side analysis of GPU computations. The resource may be placed
    /// in memory that's accessible to both CPU and GPU.
    ReadsFrequently,

    /// The CPU will not frequently read from this resource.
    ///
    /// This is the most common case for textures and buffers that are uploaded once
    /// and used only by the GPU. Allows the backend to use GPU-only memory for
    /// optimal performance.
    WontRead,
}

/// Configuration parameters for texture creation.
///
/// This struct groups together commonly used parameters for texture creation
/// to reduce the number of function arguments and improve API ergonomics.
///
/// # Examples
///
/// ```
/// use images_and_words::bindings::visible_to::{TextureConfig, TextureUsage, CPUStrategy};
/// use images_and_words::Priority;
///
/// // For a static texture
/// let static_config = TextureConfig {
///     width: 1024,
///     height: 768,
///     visible_to: TextureUsage::FragmentShaderSample,
///     debug_name: "diffuse_texture",
///     priority: Priority::unit_test(),
///     cpu_strategy: CPUStrategy::WontRead,  // Static textures don't need CPU access
///     mipmaps: true,  // Static textures can have mipmaps
/// };
///
/// // For a dynamic texture
/// let dynamic_config = TextureConfig {
///     width: 1024,
///     height: 768,
///     visible_to: TextureUsage::FragmentShaderSample,
///     debug_name: "video_frame",
///     priority: Priority::unit_test(),
///     cpu_strategy: CPUStrategy::ReadsFrequently,  // Dynamic textures may need CPU access
///     mipmaps: false,  // Dynamic textures typically don't use mipmaps
/// };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct TextureConfig<'a> {
    /// Width of the texture in pixels.
    pub width: u16,

    /// Height of the texture in pixels.
    pub height: u16,

    /// Declares how the texture will be accessed by shaders.
    pub visible_to: TextureUsage,

    /// Debug name for the texture (used in graphics debugging tools).
    pub debug_name: &'a str,

    /// Priority for resource allocation and scheduling.
    pub priority: crate::Priority,

    /// CPU access pattern hint.
    /// - For static textures: typically `CPUStrategy::WontRead`
    /// - For dynamic textures: depends on usage pattern
    pub cpu_strategy: CPUStrategy,

    /// Whether to generate mipmaps.
    /// - For static textures: user choice based on usage
    /// - For dynamic textures: typically `false` since content changes frequently
    pub mipmaps: bool,
}
