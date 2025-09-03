// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Shader types for GPU programming.
//!
//! This module provides types for vertex and fragment shaders using WGSL (WebGPU Shading Language).
//! Shaders are the programmable stages of the GPU pipeline that transform vertices and determine
//! pixel colors.
//!
//! # Overview
//!
//! The shader types in this module are wrappers around WGSL code strings. They are used when
//! creating render passes to define the programmable GPU behavior for rendering operations.
//!
//! Currently, only WGSL is supported as the shading language, which provides good cross-platform
//! compatibility through the wgpu backend.

/// A fragment shader that runs for each pixel/fragment to determine its color.
///
/// Fragment shaders are executed after rasterization and are responsible for computing
/// the final color of each pixel. They typically sample textures, apply lighting calculations,
/// and perform other per-pixel operations.
///
/// # Examples
///
/// Creating a simple red fragment shader:
///
/// ```
/// use images_and_words::images::shader::FragmentShader;
///
/// let shader = FragmentShader::new(
///     "red_shader",
///     r#"
///     @fragment
///     fn fs_main() -> @location(0) vec4<f32> {
///         return vec4<f32>(1.0, 0.0, 0.0, 1.0);  // Red color
///     }
///     "#.to_string()
/// );
/// ```
///
/// Fragment shader that samples from a texture:
///
/// ```
/// use images_and_words::images::shader::FragmentShader;
///
/// let shader = FragmentShader::new(
///     "texture_shader",
///     r#"
///     @group(0) @binding(0)
///     var t_diffuse: texture_2d<f32>;
///     @group(0) @binding(1)
///     var s_diffuse: sampler;
///
///     @fragment
///     fn fs_main(
///         @location(0) tex_coords: vec2<f32>,
///     ) -> @location(0) vec4<f32> {
///         return textureSample(t_diffuse, s_diffuse, tex_coords);
///     }
///     "#.to_string()
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FragmentShader {
    //may need additional type design for future backends
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) wgsl_code: String,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) label: &'static str,
}

/// A vertex shader that transforms vertex positions and prepares data for rasterization.
///
/// Vertex shaders are executed once for each vertex and are responsible for transforming
/// vertex positions from model space to clip space. They can also pass per-vertex data
/// (like texture coordinates, normals, or colors) to the fragment shader.
///
/// # Examples
///
/// Creating a simple pass-through vertex shader:
///
/// ```
/// use images_and_words::images::shader::VertexShader;
///
/// let shader = VertexShader::new(
///     "passthrough_vertex",
///     r#"
///     @vertex
///     fn vs_main(
///         @location(0) position: vec3<f32>,
///     ) -> @builtin(position) vec4<f32> {
///         return vec4<f32>(position, 1.0);
///     }
///     "#.to_string()
/// );
/// ```
///
/// Vertex shader with transformation matrix:
///
/// ```
/// use images_and_words::images::shader::VertexShader;
///
/// let shader = VertexShader::new(
///     "transform_vertex",
///     r#"
///     struct Uniforms {
///         transform: mat4x4<f32>,
///     }
///     @group(0) @binding(0)
///     var<uniform> uniforms: Uniforms;
///
///     struct VertexOutput {
///         @builtin(position) clip_position: vec4<f32>,
///         @location(0) tex_coords: vec2<f32>,
///     }
///
///     @vertex
///     fn vs_main(
///         @location(0) position: vec3<f32>,
///         @location(1) tex_coords: vec2<f32>,
///     ) -> VertexOutput {
///         var out: VertexOutput;
///         out.clip_position = uniforms.transform * vec4<f32>(position, 1.0);
///         out.tex_coords = tex_coords;
///         return out;
///     }
///     "#.to_string()
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VertexShader {
    //may need additional type design for future backends
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) wgsl_code: String,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) label: &'static str,
}

impl FragmentShader {
    /// Creates a new fragment shader with the given label and WGSL code.
    ///
    /// The label is used for debugging and error messages. The WGSL code should contain
    /// a fragment shader entry point function (typically named `fs_main` or similar).
    ///
    /// # Arguments
    ///
    /// * `label` - A static string label for debugging purposes
    /// * `wgsl_code` - The WGSL shader code as a string
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::shader::FragmentShader;
    ///
    /// let shader = FragmentShader::new(
    ///     "my_fragment_shader",
    ///     "@fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }".to_string()
    /// );
    /// ```
    pub fn new(label: &'static str, wgsl_code: String) -> Self {
        Self { label, wgsl_code }
    }
}

// Boilerplate for FragmentShader
impl std::fmt::Display for FragmentShader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FragmentShader({})", self.label)
    }
}

impl AsRef<str> for FragmentShader {
    fn as_ref(&self) -> &str {
        &self.wgsl_code
    }
}

impl VertexShader {
    /// Creates a new vertex shader with the given label and WGSL code.
    ///
    /// The label is used for debugging and error messages. The WGSL code should contain
    /// a vertex shader entry point function (typically named `vs_main` or similar).
    ///
    /// # Arguments
    ///
    /// * `label` - A static string label for debugging purposes
    /// * `wgsl_code` - The WGSL shader code as a string
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::shader::VertexShader;
    ///
    /// let shader = VertexShader::new(
    ///     "my_vertex_shader",
    ///     "@vertex fn vs_main() -> @builtin(position) vec4<f32> { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }".to_string()
    /// );
    /// ```
    pub fn new(label: &'static str, wgsl_code: String) -> Self {
        Self { label, wgsl_code }
    }
}

// Boilerplate for VertexShader
impl std::fmt::Display for VertexShader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VertexShader({})", self.label)
    }
}

impl AsRef<str> for VertexShader {
    fn as_ref(&self) -> &str {
        &self.wgsl_code
    }
}
