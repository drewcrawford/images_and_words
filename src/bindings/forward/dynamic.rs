// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*! Mutable forward resources optimized for frequent CPU updates.

# Overview

Dynamic resources support frequent updates from the CPU, making them ideal for data
that changes every frame or multiple times per frame. They use sophisticated multibuffering
and synchronization to maintain performance while allowing mutations.

## When to Use Dynamic Resources

Dynamic resources excel when:
- **Frequent updates**: Data changes every frame or draw call
- **Streaming data**: Continuously feeding new data to GPU
- **Interactive content**: Responding to user input in real-time
- **Animation**: Bone matrices, particle systems, morphing geometry

## Dynamic vs Static Trade-offs

| Aspect | Dynamic | Static |
|--------|---------|--------|
| CPU updates | Supported | Not allowed |
| Update frequency | High | Never |
| Memory usage | Higher (multibuffering) | Minimal |
| GPU performance | Good | Optimal |
| Use case | Per-frame data | Immutable assets |

## Architecture Considerations

Dynamic resources adapt to your hardware:

### Discrete GPUs
- May use special host-visible memory regions
- Optimizes PCI Express transfers
- Balances transfer cost vs GPU access speed

### Integrated GPUs
- Uses shared system memory
- May have suboptimal layouts for GPU access
- Lower transfer cost but potential access penalties

## Available Types

This module provides dynamic resource types:

### `Buffer` - Mutable structured data
- Per-frame uniform buffers
- Streaming vertex data
- Dynamic compute inputs
- Frequently updated lookup tables

### `FrameTexture` - Mutable image data
- Render targets
- Video frames
- Procedural textures
- Dynamic environment maps

## Multibuffering Strategy

Dynamic resources automatically handle multibuffering to prevent:
- GPU stalls waiting for CPU updates
- CPU stalls waiting for GPU to finish reading
- Data races between CPU writes and GPU reads

The multibuffering is transparent - you write to the resource and the system
ensures the GPU sees consistent data.

## Examples

```
# if cfg!(not(feature="backend_wgpu")) { return; }
# #[cfg(feature = "testing")]
# {
# use images_and_words::bindings::forward::dynamic;
# use images_and_words::bindings::visible_to::{GPUBufferUsage, TextureUsage, CPUStrategy};
# use images_and_words::bindings::software::texture::Texel;
# use images_and_words::images::projection::WorldCoord;
# use images_and_words::images::view::View;
# use images_and_words::pixel_formats::RGBA8UNorm;
# use images_and_words::bindings::forward::dynamic::buffer::CRepr;
# use images_and_words::Priority;
# test_executors::spawn_local(async {
# let view = View::for_testing();
# let engine = images_and_words::images::Engine::rendering_to(view, images_and_words::images::projection::WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
# let device = engine.bound_device();
# #[derive(Copy, Clone)]
# #[repr(C)]
# struct CameraUniforms { view: [[f32; 4]; 4], proj: [[f32; 4]; 4] }
# unsafe impl CRepr for CameraUniforms {}
# let current_camera = CameraUniforms { view: [[1.0; 4]; 4], proj: [[1.0; 4]; 4] };
# #[derive(Copy, Clone)]
# #[repr(C)]
# struct Particle { pos: [f32; 3], vel: [f32; 3] }
# unsafe impl CRepr for Particle {}
# let active_particles = vec![Particle { pos: [0.0; 3], vel: [1.0; 3] }; 1000];
#
// Per-frame uniforms
let uniforms = dynamic::buffer::Buffer::<CameraUniforms>::new(
    device.clone(),
    1,
    GPUBufferUsage::VertexShaderRead,
    "camera_uniforms",
    |_i| current_camera
).await.expect("Failed to create uniform buffer");
let mut uniform_write = uniforms.access_write().await;
uniform_write.write(&[current_camera], 0);
uniform_write.async_drop().await;

// Streaming vertices for particles
let particles = dynamic::buffer::Buffer::<Particle>::new(
    device.clone(),
    active_particles.len(),
    GPUBufferUsage::VertexShaderRead,
    "particles",
    |i| active_particles[i]
).await.expect("Failed to create particle buffer");
let mut particle_write = particles.access_write().await;
particle_write.write(&active_particles, 0);
particle_write.async_drop().await;

// Render target for post-processing
# let width = 1920;
# let height = 1080;
let config = images_and_words::bindings::visible_to::TextureConfig {
    width,
    height,
    visible_to: TextureUsage::FragmentShaderRead,
    debug_name: "framebuffer",
    priority: images_and_words::Priority::unit_test(),
    cpu_strategy: CPUStrategy::WontRead,
    mipmaps: false,
};
let framebuffer = dynamic::frame_texture::FrameTexture::<RGBA8UNorm>::new(
    &device,
    config,
    |_texel| images_and_words::pixel_formats::Unorm4 { r: 0, g: 0, b: 0, a: 255 }, // Black background
).await;
# }, "forward_dynamic_doctest");
# }
```

## Performance Considerations

- **Write patterns**: Prefer updating entire resources over partial updates
- **Frame timing**: Updates are synchronized with frame boundaries
- **Memory bandwidth**: Consider update size vs frequency trade-offs
- **Latency**: Multibuffering may introduce 1-2 frames of latency

For immutable data, use `static` resources for better performance.

*/
pub mod buffer;
pub mod frame_texture;
