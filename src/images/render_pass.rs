use std::fmt::Debug;
use crate::bindings::BindStyle;
use crate::images::shader::{FragmentShader, VertexShader};

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
    ///
    /// # parameters
    /// shader_name:
    /// `depth`: Whether to bind a depth texture to the render pass.
    /// ## Design note:
    /// We use Rust strings because we end up manipulating strings before passing to OS methods
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
}
#[derive(Debug,Clone)]
pub enum DrawCommand {
    ///payload is the number of primitives (e.g., triangles)
    TriangleStrip(u32),
    ///payload is the number of primitives (e.g., triangles)
    TriangleList(u32),
}
