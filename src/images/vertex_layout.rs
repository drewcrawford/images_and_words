/**
Describes the layout of a vertex buffer.

This information is passed to the GPU to help it interpret the data in the buffer.
 */
#[derive(Debug,Clone)]
pub struct VertexLayout {

}

impl VertexLayout {
    pub fn element_stride(&self) -> usize {
        todo!()
    }
}