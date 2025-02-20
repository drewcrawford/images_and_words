#[derive(Debug)]

pub struct FragmentShader {
    //may need additional type design for future backends
    wgsl_code: String,
}
#[derive(Debug)]
pub struct VertexShader {
    //may need additional type design for future backends
    wgsl_code: String,
}

impl FragmentShader {
    pub fn new(wgsl_code: String) -> Self {
        Self {
            wgsl_code
        }
    }
}

impl VertexShader {
    pub fn new(wgsl_code: String) -> Self {
        Self {
            wgsl_code
        }
    }
}