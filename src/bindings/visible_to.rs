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
//! ```no_run
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
/// ```no_run
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
/// ```no_run
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
/// ```no_run
/// use images_and_words::bindings::visible_to::CPUStrategy;
///
/// // A render target that we'll read back for screenshots
/// let screenshot_strategy = CPUStrategy::ReadsFrequently;
///
/// // A texture that stays on the GPU
/// let texture_strategy = CPUStrategy::WontRead;
/// ```
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