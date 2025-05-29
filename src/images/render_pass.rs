use std::collections::HashMap;
use std::fmt::Debug;
use crate::bindings::BindStyle;
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
#[derive(Debug,Clone)]
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
#[derive(Debug,Clone)]
pub struct PassDescriptor {
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) name: String,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) vertex_shader: VertexShader,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fragment_shader: FragmentShader,
    #[allow(dead_code)] //todo: mt2-495
    pub(crate) resource_specialize: ResourceSpecialize,
    pub(crate) bind_style: BindStyle,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) draw_command: DrawCommand,
    #[allow(dead_code)] //todo: mt2-496
    pub(crate) depth: bool,
    #[allow(dead_code)] //nop implementation does not use
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
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    #[allow(dead_code)] //nop implementation does not use
    pub(crate) const fn draw_command(&self) -> &DrawCommand {
        &self.draw_command
    }
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) const fn bind_style(&self) -> &BindStyle { &self.bind_style }
    #[allow(dead_code)] //mt2-495
    pub(crate) const fn resource_specialize(&self) -> &ResourceSpecialize { &self.resource_specialize }
}
#[derive(Debug,Clone)]
pub enum DrawCommand {
    ///payload is the number of primitives (e.g., triangles)
    TriangleStrip(u32),
    ///payload is the number of primitives (e.g., triangles)
    TriangleList(u32),
}
