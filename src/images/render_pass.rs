//! Render pass configuration and primitives for GPU drawing operations.
//!
//! This module provides the types needed to configure a render pass - a single draw
//! operation that processes vertices through shaders to produce rendered output.
//! 
//! # Key Components
//! 
//! - [`PassDescriptor`]: Configures a complete render pass including shaders, resource bindings,
//!   and drawing commands
//! - [`DrawCommand`]: Specifies how vertices are assembled into primitives (triangles)
//!
//! # Example
//!
//! ```
//! use images_and_words::images::render_pass::{PassDescriptor, DrawCommand};
//! use images_and_words::images::shader::{VertexShader, FragmentShader};
//! use images_and_words::bindings::BindStyle;
//!
//! // Configure a render pass for drawing a triangle
//! let vertex_shader = VertexShader::new("triangle_vs", 
//!     "@vertex fn main() -> @builtin(position) vec4<f32> { return vec4(0.0); }".to_string());
//! let fragment_shader = FragmentShader::new("triangle_fs",
//!     "@fragment fn main() -> @location(0) vec4<f32> { return vec4(1.0); }".to_string());
//! 
//! let pass = PassDescriptor::new(
//!     "triangle_pass".to_string(),
//!     vertex_shader,
//!     fragment_shader,
//!     BindStyle::new(),
//!     DrawCommand::TriangleList(3), // Draw 1 triangle (3 vertices)
//!     false, // no depth testing
//!     false  // no alpha blending
//! );
//! ```

use std::fmt::Debug;
use crate::bindings::BindStyle;
use crate::images::shader::{FragmentShader, VertexShader};

/// Configuration for a complete render pass.
/// 
/// A render pass represents a single draw operation that processes vertices through
/// vertex and fragment shaders to produce rendered output. This struct bundles together
/// all the configuration needed for the GPU to execute the draw.
/// 
/// # Components
/// 
/// - **Shaders**: Vertex and fragment shaders that process the geometry
/// - **Bindings**: Resources (buffers, textures, etc.) made available to shaders
/// - **Draw Command**: How vertices are assembled into primitives
/// - **Render State**: Depth testing and alpha blending configuration
#[derive(Debug,Clone)]
pub struct PassDescriptor {
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) name: String,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) vertex_shader: VertexShader,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fragment_shader: FragmentShader,
    pub(crate) bind_style: BindStyle,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) draw_command: DrawCommand,
    #[allow(dead_code)] //todo: mt2-496
    pub(crate) depth: bool,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) alpha: bool,
}
impl PassDescriptor {
    /// Creates a new render pass descriptor.
    /// 
    /// # Parameters
    /// 
    /// * `name` - A descriptive name for debugging and profiling
    /// * `vertex_shader` - The vertex shader that transforms vertices
    /// * `fragment_shader` - The fragment shader that produces pixel colors  
    /// * `bind_style` - Resource bindings configuration (buffers, textures, etc.)
    /// * `draw_command` - How to assemble vertices into primitives
    /// * `depth` - Whether to enable depth testing (requires a depth buffer)
    /// * `alpha` - Whether to enable alpha blending
    /// 
    /// # Design Note
    /// 
    /// We use `String` rather than `&str` for the name because backend implementations
    /// often need to manipulate these strings before passing them to graphics APIs.
    pub fn new(name: String, vertex_shader: VertexShader, fragment_shader: FragmentShader, bind_style: BindStyle,draw_command: DrawCommand,depth: bool, alpha: bool) -> Self {
        Self {
            name,
            bind_style,
            vertex_shader,
            fragment_shader,
            draw_command,
            depth,
            alpha
        }
    }
    /// Returns the name of this render pass.
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns the draw command for this render pass.
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) const fn draw_command(&self) -> &DrawCommand {
        &self.draw_command
    }
    /// Returns the resource bindings configuration for this render pass.
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) const fn bind_style(&self) -> &BindStyle { &self.bind_style }
}
/// Specifies how vertices are assembled into triangles for rendering.
/// 
/// This enum controls the primitive topology - how the GPU interprets the stream
/// of vertices to form triangles. The choice affects both how you organize your
/// vertex data and how many vertices are needed.
/// 
/// # Examples
/// 
/// ## Triangle Strip
/// ```
/// use images_and_words::images::render_pass::DrawCommand;
/// // Draw a quad using 4 vertices as a triangle strip
/// // Vertices: [A, B, C, D] form triangles: [A,B,C] and [B,C,D]
/// let draw_quad = DrawCommand::TriangleStrip(4);
/// ```
/// 
/// ## Triangle List  
/// ```
/// use images_and_words::images::render_pass::DrawCommand;
/// // Draw a quad using 6 vertices as a triangle list
/// // Vertices: [A, B, C, D, E, F] form triangles: [A,B,C] and [D,E,F]
/// let draw_quad = DrawCommand::TriangleList(6);
/// ```
#[derive(Debug,Clone)]
pub enum DrawCommand {
    /// Draws connected triangles where each vertex after the first two forms a 
    /// triangle with the previous two vertices.
    /// 
    /// For `n` vertices, this produces `n-2` triangles. This is memory-efficient
    /// for drawing connected surfaces like terrain meshes or quad strips.
    /// 
    /// The payload is the number of vertices (not triangles).
    TriangleStrip(u32),
    
    /// Draws independent triangles where each group of three vertices forms a triangle.
    /// 
    /// For `n` vertices, this produces `n/3` triangles. This is more flexible than
    /// strips but requires more vertices for connected surfaces.
    /// 
    /// The payload is the number of vertices (not triangles), which must be a multiple of 3.
    TriangleList(u32),
}
