/*! Immutable forward resources optimized for write-once, read-many patterns.

# Overview

Static resources are immutable after creation, making them ideal for data that doesn't
change during runtime. The GPU can optimize these resources for fast repeated access
since it knows the data won't be modified.

## When to Use Static Resources

Static resources excel when:
- **Data uploaded once**: Mesh geometry, texture atlases, lookup tables
- **Read frequently**: Accessed many times per frame or across frames
- **Performance critical**: GPU can cache and optimize immutable data
- **Memory efficient**: No multibuffering overhead needed

## Static vs Dynamic Trade-offs

| Aspect | Static | Dynamic |
|--------|--------|---------|
| CPU updates | Not allowed | Frequent updates |
| GPU performance | Optimized | May have overhead |
| Memory usage | Minimal | Multibuffering |
| Use case | Assets, geometry | Per-frame data |

## Available Types

This module provides two static resource types:

### `Buffer` - Structured data storage
- Vertex and index buffers for meshes
- Large lookup tables for shaders
- Precomputed animation data
- Any data with programmer-defined layout

### `Texture` - Image data storage
- Texture atlases and sprite sheets
- Environment maps and skyboxes
- Normal maps and material textures
- Any data needing GPU sampling/filtering

## Choosing Between Buffer and Texture

Use **Buffer** when you need:
- Direct indexed access to elements
- Custom data structures (via `CRepr`)
- Vertex attributes or compute data

Use **Texture** when you need:
- Spatial 2D/3D data access
- Hardware filtering/sampling
- Standardized pixel formats
- Shader texture sampling

## Examples

```
# if cfg!(not(feature="backend_wgpu")) { return; }
# #[cfg(feature = "testing")]
# {
# use images_and_words::bindings::forward::r#static;
# use images_and_words::bindings::visible_to::{GPUBufferUsage, TextureUsage};
# use images_and_words::bindings::software::texture::Texel;
# use images_and_words::images::projection::WorldCoord;
# use images_and_words::images::view::View;
# use images_and_words::pixel_formats::RGBA8UNorm;
# use images_and_words::bindings::forward::dynamic::buffer::CRepr;
# test_executors::sleep_on(async {
# let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
# let device = engine.bound_device();
# #[derive(Copy, Clone)]
# #[repr(C)]
# struct Vertex { x: f32, y: f32, z: f32 }
# unsafe impl CRepr for Vertex {}
# struct Mesh { vertices: Vec<Vertex>, indices: Vec<u16> }
# let mesh = Mesh {
#     vertices: vec![Vertex { x: 0.0, y: 0.0, z: 0.0 }; 100],
#     indices: vec![0, 1, 2, 3, 4, 5] // At least 6 indices for alignment
# };
#
// Load a mesh once
let vertex_buffer = r#static::buffer::Buffer::new(
    device.clone(),
    mesh.vertices.len(),
    GPUBufferUsage::VertexShaderRead,
    "vertices",
    |i| mesh.vertices[i]
).await.expect("Failed to create vertex buffer");

let index_buffer = r#static::buffer::Buffer::new(
    device.clone(),
    mesh.indices.len(),
    GPUBufferUsage::Index,
    "indices",
    |i| mesh.indices[i]
).await.expect("Failed to create index buffer");

// Load textures from files
let diffuse_config = images_and_words::bindings::visible_to::TextureConfig {
    width: 256,
    height: 256,
    visible_to: TextureUsage::FragmentShaderRead,
    debug_name: "diffuse",
    priority: images_and_words::Priority::unit_test(),
    cpu_strategy: images_and_words::bindings::visible_to::CPUStrategy::WontRead,
    mipmaps: false,
};
let diffuse_map = r#static::texture::Texture::<RGBA8UNorm>::new(
    &device,
    diffuse_config,
    |_texel| images_and_words::pixel_formats::Unorm4 { r: 255, g: 255, b: 255, a: 255 } // White texture
).await.expect("Failed to create diffuse map");

let normal_config = images_and_words::bindings::visible_to::TextureConfig {
    width: 256,
    height: 256,
    visible_to: TextureUsage::FragmentShaderRead,
    debug_name: "normal",
    priority: images_and_words::Priority::unit_test(),
    cpu_strategy: images_and_words::bindings::visible_to::CPUStrategy::WontRead,
    mipmaps: false,
};
let normal_map = r#static::texture::Texture::<RGBA8UNorm>::new(
    &device,
    normal_config,
    |_texel| images_and_words::pixel_formats::Unorm4 { r: 128, g: 128, b: 255, a: 255 } // Default normal
).await.expect("Failed to create normal map");

// Create lookup tables
# let gradient_data = vec![0.0f32; 256];
let gradient_lut = r#static::buffer::Buffer::new(
    device.clone(),
    gradient_data.len(),
    GPUBufferUsage::VertexShaderRead,
    "gradient_lut",
    |i| gradient_data[i]
).await.expect("Failed to create gradient LUT");
# });
# }
```

## Performance Notes

Static resources may be placed in GPU-only memory for optimal performance.
This means:
- Fastest possible GPU access
- No CPU access after creation
- Most efficient memory usage
- Best caching behavior

For resources that need updates, see the `dynamic` module instead.

*/

pub mod buffer;
pub mod texture;
