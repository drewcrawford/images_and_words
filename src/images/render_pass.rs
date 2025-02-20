use std::collections::HashMap;
use std::fmt::Debug;
use std::pin::Pin;
use crate::bindings::BindStyle;
use crate::images::port::{PassClient};
use crate::images::shader::{FragmentShader, VertexShader};

/**
This indicates a specialization we will use to hide resource access.

For other forms of specialization, we should use another method,
for details see obsidian://open?vault=mt2&file=IW%2FFunction%20speclialization

# Metal

On Metal, we specialize via... specialization

# Vulkan

On vulkan, we need to implement different entrypoints, mt2-270 and obsidian.

*/
#[derive(Debug)]
pub struct ResourceSpecialize {
    ///if there are more than 255 ways to specialize something we are fucked.
    pub(crate) dict: HashMap<u8,bool>
}
impl ResourceSpecialize {
    pub fn new() -> Self { Self { dict: HashMap::new()} }
    pub fn set(&mut self, index: u8, value: bool) {
        self.dict.entry(index).or_insert(value);
    }
}
#[derive(Debug)]
pub struct PassDescriptor {
    pub(crate) name: String,
    pub(crate) vertex_shader: VertexShader,
    pub(crate) fragment_shader: FragmentShader,
    #[allow(dead_code)] //todo: mt2-495
    pub(crate) resource_specialize: ResourceSpecialize,
    pub(crate) bind_style: BindStyle,
    pub(crate) draw_command: DrawCommand,
    pub(crate) depth: bool, //todo: mt2-496
    pub(crate) alpha: bool,
}
impl PassDescriptor {
    ///
    /// # parameters
    /// shader_name:
    /// `depth`: Whether to bind a depth texture to the render pass.
    /// ## Design note:
    /// We use Rust strings because we end up manipulating strings before passing to OS methods
    pub fn new(name: String, vertex_shader: VertexShader, fragment_shader: FragmentShader, resource_specialize: ResourceSpecialize, bind_style: BindStyle,draw_command: DrawCommand,depth: bool, alpha: bool) -> Self {
        Self {
            name,
            bind_style,
            vertex_shader,
            fragment_shader,
            resource_specialize,
            draw_command,
            depth,
            alpha
        }
    }
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    pub(crate) const fn draw_command(&self) -> &DrawCommand {
        &self.draw_command
    }
    pub(crate) const fn bind_style(&self) -> &BindStyle { &self.bind_style }
    #[allow(dead_code)] //mt2-495
    pub(crate) const fn resource_specialize(&self) -> &ResourceSpecialize { &self.resource_specialize }
}
#[derive(Debug)]
pub enum DrawCommand {
    ///payload is the number of primitives (e.g., triangles)
    TriangleStrip(u32),
}
pub trait PassTrait<const DESCRIPTORS: usize> {
    ///A type returned by the process of making a descriptor.
    ///
    /// You can use this to hold onto e.g. static texture tickets and provide them to future passes.
    /// If you do not need to expose any details of your render pass, you can use the type `()`.
    type DescriptorResult;
    ///A type that contains information about your rendering pass.
    /// The type may be cached by the implementation.
    ///todo: async fn is not permitted in traits
    fn into_descriptor<'a>(self, port: &'a mut PassClient) -> Pin<Box<dyn std::future::Future<Output=([PassDescriptor; DESCRIPTORS],Self::DescriptorResult)> + 'a>>;
}