// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Defines the way resources are bound for a render pass.
//!
//! This module provides a high-level abstraction for binding GPU resources such as buffers,
//! textures, and samplers to specific shader stages. The `BindStyle` struct acts as a
//! configuration object that describes which resources should be bound to which slots
//! during rendering.
//!
//! # Key Concepts
//!
//! - **Bind Slots**: Resources are bound to numbered slots that correspond to binding
//!   locations in shaders
//! - **Shader Stages**: Resources can be bound to vertex or fragment shader stages
//! - **Resource Types**: Supports static/dynamic buffers, textures, samplers, and
//!   special bindings like camera matrices and frame counters
//!
//! # Example
//!
//! ```
//! # if cfg!(not(feature="backend_wgpu")) { return; }
//! # #[cfg(feature = "testing")]
//! # {
//! use images_and_words::bindings::forward::r#static::buffer::Buffer;
//! use images_and_words::pixel_formats::{BGRA8UNormSRGB};
//! # test_executors::sleep_on(async {
//! # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
//! # let bound_device = engine.bound_device().clone();
//! let my_buffer: Buffer<u8> = Buffer::new(bound_device, 1024, images_and_words::bindings::visible_to::GPUBufferUsage::VertexBuffer, "my_buffer", |index| 2).await.expect("can't create buffer");
//! # use images_and_words::bindings::bind_style::{BindSlot, Stage};
//! # use images_and_words::bindings::BindStyle;
//! # use images_and_words::images::projection::WorldCoord;
//! use images_and_words::images::view::View;
//! let mut bind_style = BindStyle::new();
//!
//! // Bind a camera matrix to slot 0 for the vertex shader
//! bind_style.bind_camera_matrix(BindSlot::new(0), Stage::Vertex);
//!
//! // Bind a static buffer to slot 1 for the fragment shader
//! bind_style.bind_static_buffer(BindSlot::new(1), Stage::Fragment, &my_buffer);
//! # });
//! # }
//! ```

use crate::bindings::forward::dynamic::buffer::ErasedRenderSide;
use crate::bindings::forward::dynamic::frame_texture::ErasedTextureRenderSide;
use crate::bindings::sampler::SamplerType;
use std::collections::HashMap;
use std::fmt::Debug;
/// Describes how resources are bound for a render pass.
///
/// This struct collects all resource bindings that will be used during rendering.
/// It maintains a mapping of bind slots to resources and tracks special bindings
/// like index buffers separately.
///
/// Resources are not immediately bound when methods are called; instead, this struct
/// builds up a description that is later used by the rendering backend to perform
/// the actual GPU bindings.
#[derive(Debug, Clone, PartialEq)]
pub struct BindStyle {
    pub(crate) binds: HashMap<u32, BindInfo>,
    pub(crate) index_buffer: Option<crate::imp::GPUableBuffer>,
}

/// Internal enumeration of all possible binding targets.
///
/// This enum represents the different types of resources that can be bound to shaders.
/// Each variant corresponds to a specific type of GPU resource or special binding.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BindTarget {
    /// A static buffer that doesn't change during rendering
    StaticBuffer(crate::imp::GPUableBuffer),
    /// A dynamic buffer that can be updated between frames
    DynamicBuffer(ErasedRenderSide),
    /// The camera transformation matrix (resolved at render time)
    Camera,
    /// A frame counter that increments each frame
    FrameCounter,
    /// A dynamic texture that can be updated between frames
    DynamicTexture(ErasedTextureRenderSide),
    /// A static texture with optional sampler configuration
    #[allow(dead_code)] //nop implementation does not use
    StaticTexture(crate::imp::TextureRenderSide, Option<SamplerType>),
    /// A texture sampler configuration
    #[allow(dead_code)] //nop implementation does not use
    Sampler(SamplerType),
    /// A static vertex buffer with its layout description
    #[allow(dead_code)] //nop implementation does not use
    VB(VertexLayout, crate::imp::GPUableBuffer),
    /// A dynamic vertex buffer with its layout description
    #[allow(dead_code)] //nop implementation does not use
    DynamicVB(VertexLayout, ErasedRenderSide),
}

/// Information about a single resource binding.
///
/// This struct pairs a binding target with the shader stage it should be bound to.
#[derive(Debug, Clone, PartialEq)]
pub struct BindInfo {
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) stage: Stage,
    pub(crate) target: BindTarget,
}

/// Configuration for a texture sampler binding.
///
/// When binding a texture, you can optionally specify sampler settings that control
/// how the texture is sampled (filtering, wrapping modes, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SamplerInfo {
    /// The slot to bind the sampler to.
    pub pass_index: u32,
    /// The sampler configuration to use.
    pub sampler_type: SamplerType,
}
impl Default for BindStyle {
    fn default() -> Self {
        Self::new()
    }
}

impl BindStyle {
    /// Creates a new, empty `BindStyle`.
    ///
    /// The returned instance has no bindings configured. Use the various
    /// `bind_*` methods to add resource bindings.
    pub fn new() -> Self {
        BindStyle {
            binds: HashMap::new(),
            index_buffer: None,
        }
    }

    /// Internal method to bind a resource to a slot.
    ///
    /// # Panics
    ///
    /// Panics if a resource is already bound to the specified slot.
    fn bind(&mut self, slot: BindSlot, stage: Stage, target: BindTarget) {
        let old = self
            .binds
            .insert(slot.pass_index, BindInfo { stage, target });
        assert!(old.is_none(), "Already bound to slot {:?}", slot);
    }

    /// Binds the camera transformation matrix to the specified slot.
    ///
    /// The camera matrix is a special binding that is resolved at render time
    /// based on the current camera configuration. This is typically used in
    /// vertex shaders for transforming vertices from model space to clip space.
    ///
    /// # Parameters
    ///
    /// * `slot` - The binding slot to use
    /// * `stage` - The shader stage (typically `Stage::Vertex`)
    pub fn bind_camera_matrix(&mut self, slot: BindSlot, stage: Stage) {
        self.bind(slot, stage, BindTarget::Camera);
    }

    /// Binds a frame counter to the specified slot.
    ///
    /// The frame counter is a 32-bit unsigned integer that increments each frame.
    /// This is useful for animation effects, debugging, or any shader logic that
    /// needs to vary over time.
    ///
    /// The counter starts at 0 and increments up to a maximum value, then wraps
    /// back to 0. The exact maximum value is implementation-defined.
    ///
    /// # Parameters
    ///
    /// * `slot` - The binding slot to use
    /// * `stage` - The shader stage where the counter will be accessible
    pub fn bind_frame_counter(&mut self, slot: BindSlot, stage: Stage) {
        self.bind(slot, stage, BindTarget::FrameCounter);
    }

    /// Binds a static buffer to the specified slot.
    ///
    /// Static buffers contain data that doesn't change during rendering. They are
    /// typically uploaded to the GPU once and reused across many frames.
    ///
    /// # Parameters
    ///
    /// * `slot` - The binding slot to use
    /// * `stage` - The shader stage where the buffer will be accessible
    /// * `buffer` - The static buffer to bind
    ///
    /// # Type Parameters
    ///
    /// * `Element` - The type of elements stored in the buffer
    pub fn bind_static_buffer<Element>(
        &mut self,
        slot: BindSlot,
        stage: Stage,
        buffer: &crate::bindings::forward::r#static::buffer::Buffer<Element>,
    ) {
        self.bind(slot, stage, BindTarget::StaticBuffer(buffer.imp.clone()));
    }

    /// Binds a dynamic buffer to the specified slot.
    ///
    /// Dynamic buffers can be updated between frames from the CPU side. They are
    /// useful for data that changes frequently, such as per-frame uniforms or
    /// instance data.
    ///
    /// # Parameters
    ///
    /// * `slot` - The binding slot to use
    /// * `stage` - The shader stage where the buffer will be accessible
    /// * `buffer` - The dynamic buffer to bind
    ///
    /// # Type Parameters
    ///
    /// * `Element` - The type of elements stored in the buffer (must be `Send + Sync + 'static`)
    pub fn bind_dynamic_buffer<Element>(
        &mut self,
        slot: BindSlot,
        stage: Stage,
        buffer: &crate::bindings::forward::dynamic::buffer::Buffer<Element>,
    ) where
        Element: BackendSend + BackendSync + 'static,
    {
        self.bind(
            slot,
            stage,
            BindTarget::DynamicBuffer(buffer.render_side().erased_render_side()),
        );
    }

    /// Binds a static texture to the specified slot.
    ///
    /// Static textures contain image data that doesn't change during rendering.
    /// You can optionally specify sampler settings to control how the texture
    /// is sampled in shaders.
    ///
    /// # Parameters
    ///
    /// * `slot` - The binding slot for the texture
    /// * `stage` - The shader stage where the texture will be accessible
    /// * `texture` - The static texture to bind
    /// * `sampler_type` - Optional sampler configuration. If provided, the sampler
    ///   will be bound to the slot specified in `SamplerInfo::pass_index`
    ///
    /// # Type Parameters
    ///
    /// * `Format` - The pixel format of the texture
    pub fn bind_static_texture<Format: crate::pixel_formats::sealed::PixelFormat>(
        &mut self,
        slot: BindSlot,
        stage: Stage,
        texture: &crate::bindings::forward::r#static::texture::Texture<Format>,
        sampler_type: Option<SamplerInfo>,
    ) {
        self.bind(
            slot,
            stage.clone(),
            BindTarget::StaticTexture(
                texture.imp.render_side(),
                sampler_type.as_ref().map(|x| x.sampler_type),
            ),
        );
        if let Some(sampler) = sampler_type {
            self.bind(
                BindSlot::new(sampler.pass_index),
                stage,
                BindTarget::Sampler(sampler.sampler_type),
            );
        }
    }

    /// Binds a dynamic texture to the specified slot.
    ///
    /// Dynamic textures can be updated between frames. They are useful for
    /// render targets, procedurally generated textures, or any texture data
    /// that changes frequently.
    ///
    /// # Parameters
    ///
    /// * `slot` - The binding slot to use
    /// * `stage` - The shader stage where the texture will be accessible
    /// * `texture` - The dynamic texture to bind
    ///
    /// # Type Parameters
    ///
    /// * `Format` - The pixel format of the texture (must be `'static`)
    pub fn bind_dynamic_texture<Format>(
        &mut self,
        slot: BindSlot,
        stage: Stage,
        texture: &crate::bindings::forward::dynamic::frame_texture::FrameTexture<Format>,
    ) where
        Format: crate::pixel_formats::sealed::PixelFormat + 'static,
    {
        self.bind(
            slot,
            stage,
            BindTarget::DynamicTexture(texture.render_side().erased()),
        );
    }
    /// Binds a static vertex buffer to the specified slot.
    ///
    /// Vertex buffers contain per-vertex data (positions, normals, texture coordinates, etc.)
    /// and are always bound to the vertex shader stage. The layout parameter describes
    /// how the buffer data should be interpreted.
    ///
    /// # Parameters
    ///
    /// * `slot` - The binding slot to use
    /// * `buffer` - The static buffer containing vertex data
    /// * `layout` - Description of the vertex data layout
    ///
    /// # Type Parameters
    ///
    /// * `Element` - The type of vertex data stored in the buffer
    pub fn bind_static_vertex_buffer<Element>(
        &mut self,
        slot: BindSlot,
        buffer: &crate::bindings::forward::r#static::buffer::Buffer<Element>,
        layout: VertexLayout,
    ) {
        self.bind(
            slot,
            Stage::Vertex,
            BindTarget::VB(layout, buffer.imp.clone()),
        );
    }

    /// Binds a dynamic vertex buffer to the specified slot.
    ///
    /// Dynamic vertex buffers can be updated between frames, useful for animated
    /// geometry or procedurally generated meshes.
    ///
    /// # Parameters
    ///
    /// * `slot` - The binding slot to use
    /// * `buffer` - The dynamic buffer containing vertex data
    /// * `layout` - Description of the vertex data layout
    ///
    /// # Type Parameters
    ///
    /// * `Element` - The type of vertex data stored in the buffer (must be `Send + Sync + 'static`)
    pub fn bind_dynamic_vertex_buffer<Element>(
        &mut self,
        slot: BindSlot,
        buffer: &crate::bindings::forward::dynamic::buffer::Buffer<Element>,
        layout: VertexLayout,
    ) where
        Element: BackendSend + BackendSync + 'static,
    {
        self.bind(
            slot,
            Stage::Vertex,
            BindTarget::DynamicVB(layout, buffer.render_side().erased_render_side()),
        );
    }

    /// Binds a static index buffer for indexed drawing.
    ///
    /// Index buffers contain indices that reference vertices in vertex buffers,
    /// allowing for efficient reuse of vertex data. This method currently only
    /// supports 16-bit indices.
    ///
    /// Unlike other bindings, index buffers don't use slots - there can only be
    /// one index buffer per draw call.
    ///
    /// # Parameters
    ///
    /// * `buffer` - The buffer containing 16-bit indices
    pub fn bind_static_index_buffer(
        &mut self,
        buffer: &crate::bindings::forward::r#static::buffer::Buffer<u16>,
    ) {
        self.index_buffer = Some(buffer.imp.clone())
    }
}

/// Specifies which shader stage a resource should be bound to.
///
/// Resources can be made available to different stages of the graphics pipeline.
/// This enum allows you to specify whether a resource should be accessible from
/// the vertex shader, fragment shader, or both.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Stage {
    /// Resource will be bound to fragment (pixel) shaders.
    Fragment,
    /// Resource will be bound to vertex shaders.
    Vertex,
}

/// Represents a binding slot where a resource can be bound.
///
/// Binding slots are numbered locations that correspond to binding points in
/// shaders. When you bind a resource to a slot, it becomes available at that
/// binding location in the shader.
///
/// # Example
///
/// ```
/// # if cfg!(not(feature="backend_wgpu")) { return; }
/// # #[cfg(feature = "testing")]
/// # {
/// use images_and_words::bindings::bind_style::{BindStyle, BindSlot, Stage};
/// use images_and_words::bindings::visible_to::TextureUsage;
/// use images_and_words::images::projection::WorldCoord;
/// use images_and_words::images::view::View;
/// use images_and_words::pixel_formats::{BGRA8UNormSRGB, BGRA8UnormPixelSRGB};
/// use images_and_words::Priority;
///  # test_executors::sleep_on(async {
/// # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
/// # let bound_device = engine.bound_device().clone();
/// let mut bind_style = BindStyle::new();
/// let config = images_and_words::bindings::visible_to::TextureConfig {
///     width: 1024,
///     height: 1024,
///     visible_to: TextureUsage::VertexShaderRead,
///     debug_name: "my_texture",
///     priority: Priority::unit_test(),
///     cpu_strategy: images_and_words::bindings::visible_to::CPUStrategy::WontRead,
///     mipmaps: true,
/// };
/// let texture = images_and_words::bindings::forward::r#static::texture::Texture::<BGRA8UNormSRGB>::new(
/// &bound_device,
/// config,
/// |_| BGRA8UnormPixelSRGB { r: 255, g: 0, b: 0, a: 255 }
/// ).await.expect("can't create texture");
///
///
///
/// // In your shader:
/// // layout(binding = 0) uniform CameraData { ... };
/// // layout(binding = 1) uniform sampler2D myTexture;
///
/// // In Rust:
/// bind_style.bind_camera_matrix(BindSlot::new(0), Stage::Vertex);
/// bind_style.bind_static_texture(BindSlot::new(1), Stage::Fragment, &texture, None);
/// # });
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct BindSlot {
    pub(crate) pass_index: u32,
}

impl BindSlot {
    /// Creates a new binding slot with the specified index.
    ///
    /// # Parameters
    ///
    /// * `pass_index` - The numeric index of the binding slot
    pub fn new(pass_index: u32) -> Self {
        Self { pass_index }
    }
}

use crate::images::vertex_layout::VertexLayout;
use crate::imp::{BackendSend, BackendSync};
