// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*! Resources that pass data in the 'forward' direction, that is, CPU=>GPU.

# Overview

Forward resources enable data transfer from CPU memory to GPU memory, the most common
pattern in graphics programming. This module provides both static and dynamic variants
to optimize for different update patterns.

## What is Forward Data Flow?

Forward data flow (CPU→GPU) is used for:
- Uploading vertex and index buffers for rendering
- Sending uniform data to shaders
- Loading texture data for sampling
- Providing compute shader inputs

This is distinguished from other data flow directions:
- **Reverse** (GPU→CPU): Reading back rendered images or compute results
- **Sideways** (GPU→GPU): Render-to-texture, compute pipeline chaining
- **Omnidirectional** (CPU↔GPU): Interactive simulations with feedback

## Choosing Between Static and Dynamic

Within forward resources, you must choose between static and dynamic variants:

### Use `static` when:
- Data is uploaded once and used many times
- Examples: mesh geometry, texture atlases, lookup tables
- Optimized for GPU performance over CPU flexibility

### Use `dynamic` when:
- Data changes frequently (per frame or per draw)
- Examples: camera matrices, animation data, particle positions
- Optimized for CPU updates with multibuffering

## Architecture

Forward resources handle:
- **Memory allocation**: Appropriate GPU memory for access patterns
- **Synchronization**: Ensuring data is ready when GPU needs it
- **Format conversion**: Converting CPU data to GPU-optimal formats
- **Lifetime management**: Safe resource cleanup

## Examples

```
# if cfg!(not(feature="backend_wgpu")) { return; }
# #[cfg(feature = "testing")]
# {
# use images_and_words::bindings::forward;
# use images_and_words::bindings::visible_to::{GPUBufferUsage, TextureUsage, CPUStrategy};
# use images_and_words::bindings::software::texture::Texel;
# use images_and_words::images::projection::WorldCoord;
# use images_and_words::images::view::View;
# use images_and_words::pixel_formats::RGBA8UNorm;
# use images_and_words::bindings::forward::dynamic::buffer::CRepr;
# app_window::wgpu::wgpu_begin_context(async {
# app_window::wgpu::wgpu_in_context(async {
# let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
# let device = engine.bound_device();
# #[derive(Copy, Clone)]
# #[repr(C)]
# struct Vertex { x: f32, y: f32, z: f32 }
# unsafe impl CRepr for Vertex {}
# let mesh_vertices = vec![Vertex { x: 0.0, y: 0.0, z: 0.0 }; 100];
# #[derive(Copy, Clone)]
# #[repr(C)]
# struct CameraMatrix { view: [[f32; 4]; 4], proj: [[f32; 4]; 4] }
# unsafe impl CRepr for CameraMatrix {}
# let camera_matrix = CameraMatrix { view: [[0.0; 4]; 4], proj: [[0.0; 4]; 4] };
#
// Static mesh data - uploaded once
let vertices = forward::r#static::buffer::Buffer::new(
    device.clone(),
    mesh_vertices.len(),
    GPUBufferUsage::VertexShaderRead,
    "vertices",
    |i| mesh_vertices[i]
).await.expect("Failed to create vertex buffer");

// Dynamic uniforms - updated per frame
let uniforms = forward::dynamic::buffer::Buffer::<CameraMatrix>::new(
    device.clone(),
    1,
    GPUBufferUsage::VertexShaderRead,
    "uniforms",
    |_i| camera_matrix
).await.expect("Failed to create uniform buffer");
let mut uniform_write = uniforms.access_write().await;
uniform_write.write(&[camera_matrix], 0);
uniform_write.async_drop().await;

// Static texture - loaded from file
# let width = 256;
# let height = 256;
let static_config = images_and_words::bindings::visible_to::TextureConfig {
    width,
    height,
    visible_to: TextureUsage::FragmentShaderRead,
    debug_name: "texture",
    priority: images_and_words::Priority::unit_test(),
    cpu_strategy: images_and_words::bindings::visible_to::CPUStrategy::WontRead,
    mipmaps: false,
};
let texture = forward::r#static::texture::Texture::<RGBA8UNorm>::new(
    &device,
    static_config,
    |_texel| images_and_words::pixel_formats::Unorm4 { r: 255, g: 0, b: 0, a: 255 } // Red texture
).await.expect("Failed to create texture");

// Dynamic render target - rendered each frame
let dynamic_config = images_and_words::bindings::visible_to::TextureConfig {
    width,
    height,
    visible_to: TextureUsage::FragmentShaderRead,
    debug_name: "render_target",
    priority: images_and_words::Priority::unit_test(),
    cpu_strategy: images_and_words::bindings::visible_to::CPUStrategy::WontRead,
    mipmaps: false,
};
let target = forward::dynamic::frame_texture::FrameTexture::<RGBA8UNorm>::new(
    &device,
    dynamic_config,
    |_texel| images_and_words::pixel_formats::Unorm4 { r: 0, g: 0, b: 0, a: 255 }, // Black background
).await;
# });
# });
# }
```

## Module Contents

- `static/` - Immutable forward resources (see module docs)
- `dynamic/` - Mutable forward resources (see module docs)

*/

pub mod dynamic;
pub mod r#static;
