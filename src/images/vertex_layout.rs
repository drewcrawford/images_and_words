
/**
Describes the layout of a vertex buffer.

This information is passed to the GPU to help it interpret the data in the buffer.
 */
#[derive(Debug,Clone)]
pub struct VertexLayout {
    pub(crate) fields: Vec<VertexField>,
}

#[derive(Debug,Clone)]
pub(crate) struct VertexField {
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) name: &'static str,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) r#type: VertexFieldType,
}

#[derive(Debug,Clone)]
#[non_exhaustive]
pub enum VertexFieldType {
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
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
        }
    }
    pub fn add_field(&mut self, name: &'static str, r#type: VertexFieldType) {
        self.fields.push(VertexField { name, r#type });
    }
    #[allow(dead_code)] //nop implementation does not use
    pub (crate) fn element_stride(&self) -> usize {
        self.fields.iter().map(|e| e.r#type.stride()).sum()
    }
}