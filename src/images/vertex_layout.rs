// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Vertex buffer layout descriptions for GPU rendering.
//!
//! This module provides types for describing the structure and layout of vertex data
//! in GPU buffers. When rendering geometry, the GPU needs to know how to interpret
//! the raw bytes in vertex buffers - this module helps you specify that interpretation.
//!
//! # Overview
//!
//! A vertex typically contains multiple attributes like position, color, texture coordinates,
//! etc. The [`VertexLayout`] type allows you to describe which attributes are present
//! and their types, so the GPU can correctly read the vertex data during rendering.
//!
//! # Example
//!
//! ```
//! use images_and_words::images::vertex_layout::{VertexLayout, VertexFieldType};
//!
//! // Create a vertex layout for a simple 2D vertex with position and color
//! let mut layout = VertexLayout::new();
//! layout.add_field("position", VertexFieldType::F32);
//! layout.add_field("position", VertexFieldType::F32); // x, y
//! layout.add_field("color", VertexFieldType::F32);
//! layout.add_field("color", VertexFieldType::F32);
//! layout.add_field("color", VertexFieldType::F32); // r, g, b
//! ```

/// Describes the layout of a vertex buffer.
///
/// This type specifies how vertex data is structured in memory, including what
/// attributes each vertex contains and their types. This information is passed
/// to the GPU to help it interpret the data in the buffer during rendering.
///
/// # Usage
///
/// Vertex layouts are typically created when setting up vertex buffers for rendering.
/// You build a layout by adding fields that correspond to the attributes in your
/// vertex shader.
///
/// # Example
///
/// ```
/// use images_and_words::images::vertex_layout::{VertexLayout, VertexFieldType};
///
/// // Define a layout for vertices with 3D position and UV coordinates
/// let mut layout = VertexLayout::new();
///
/// // Add position fields (x, y, z)
/// layout.add_field("position_x", VertexFieldType::F32);
/// layout.add_field("position_y", VertexFieldType::F32);
/// layout.add_field("position_z", VertexFieldType::F32);
///
/// // Add texture coordinate fields (u, v)
/// layout.add_field("texcoord_u", VertexFieldType::F32);
/// layout.add_field("texcoord_v", VertexFieldType::F32);
/// ```
#[derive(Debug, Clone)]
pub struct VertexLayout {
    pub(crate) fields: Vec<VertexField>,
}

#[derive(Debug, Clone)]
pub(crate) struct VertexField {
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) name: &'static str,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) r#type: VertexFieldType,
}

/// Specifies the data type of a vertex attribute field.
///
/// This enum defines the possible types for individual fields within a vertex.
/// Currently only 32-bit floating point values are supported, but this may be
/// extended in the future to support other common vertex data types.
///
/// # Example
///
/// ```
/// use images_and_words::images::vertex_layout::VertexFieldType;
///
/// // Currently F32 is the only supported type
/// let field_type = VertexFieldType::F32;
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum VertexFieldType {
    /// A 32-bit floating point value.
    ///
    /// This is the most common type for vertex attributes like positions,
    /// normals, texture coordinates, and colors.
    F32,
}

impl VertexFieldType {
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn stride(&self) -> usize {
        match self {
            VertexFieldType::F32 => 4,
        }
    }
}

impl VertexLayout {
    /// Creates a new, empty vertex layout.
    ///
    /// The returned layout has no fields. Use [`add_field`](Self::add_field) to add
    /// vertex attribute descriptions.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::vertex_layout::VertexLayout;
    ///
    /// let layout = VertexLayout::new();
    /// // Layout is empty until fields are added
    /// ```
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Adds a field to the vertex layout.
    ///
    /// Each field represents one component of a vertex attribute. For multi-component
    /// attributes (like a 3D position or RGB color), you need to add multiple fields.
    ///
    /// Fields are added in the order they appear in memory. This order must match
    /// the actual layout of your vertex data.
    ///
    /// # Parameters
    ///
    /// * `name` - A descriptive name for the field. This is used for debugging and
    ///   must have a `'static` lifetime (typically a string literal).
    /// * `type` - The data type of this field.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::vertex_layout::{VertexLayout, VertexFieldType};
    ///
    /// let mut layout = VertexLayout::new();
    ///
    /// // Add a 2D position attribute
    /// layout.add_field("pos_x", VertexFieldType::F32);
    /// layout.add_field("pos_y", VertexFieldType::F32);
    ///
    /// // Add an RGB color attribute  
    /// layout.add_field("color_r", VertexFieldType::F32);
    /// layout.add_field("color_g", VertexFieldType::F32);
    /// layout.add_field("color_b", VertexFieldType::F32);
    /// ```
    pub fn add_field(&mut self, name: &'static str, r#type: VertexFieldType) {
        self.fields.push(VertexField { name, r#type });
    }

    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn element_stride(&self) -> usize {
        self.fields.iter().map(|e| e.r#type.stride()).sum()
    }
}

impl Default for VertexLayout {
    /// Creates a new, empty vertex layout.
    ///
    /// This is equivalent to calling [`VertexLayout::new()`].
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::vertex_layout::VertexLayout;
    ///
    /// let layout: VertexLayout = Default::default();
    /// ```
    fn default() -> Self {
        Self::new()
    }
}
