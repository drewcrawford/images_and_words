/** Explains why we will use some resource.*/
pub enum TextureUsage {
    ///We will read from this resource in the fragment shader.
    FragmentShaderRead,
    ///Will be read in the vertex shader
    VertexShaderRead,
    ///We will read from this resource in Vertex and Fragment shaders.
    VertexAndFragmentShaderRead,
    ///We will sample the resource in fragment shaders.
    FragmentShaderSample,
    ///We will sample the resource in vertex shaders.
    VertexShaderSample,
    ///We will sample in vertex and fragment shaders.
    VertexAndFragmentShaderSample,
}

pub enum GPUBufferUsage {
    ///We will read this resource in the vertex shader.
    VertexShaderRead,
    ///We will read this resource in the fragment shader.
    FragmentShaderRead,
    ///This is a vertex buffer object
    VertexBuffer,
    ///This is an index buffer object
    Index,
}

pub enum CPUStrategy {
    ///CPU will frequently read the resource.
    ReadsFrequently,
    ///CPU will not frequently read the resource.
    WontRead,
}