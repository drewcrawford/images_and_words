#[derive(Debug,Clone)]
pub struct FragmentShader {
    //may need additional type design for future backends
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) wgsl_code: String,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) label: &'static str,
}
#[derive(Debug,Clone)]
pub struct VertexShader {
    //may need additional type design for future backends
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) wgsl_code: String,
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) label: &'static str,
}

impl FragmentShader {
    pub fn new(label: &'static str, wgsl_code: String) -> Self {
        Self {
            label,
            wgsl_code
        }
    }
}

impl VertexShader {
    pub fn new(label: &'static str, wgsl_code: String) -> Self {
        Self {
            label,
            wgsl_code
        }
    }
}