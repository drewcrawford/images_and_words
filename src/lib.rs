/*! images_and_words is a GPU middleware and abstraction layer for high-performance
  graphics applications and games.

Here is a quick chart to compare IW against many other solutions:

|| Strategy                | Examples               | API style   | API concepts                                                                                | Synchronization concerns | Shaders                                               | Runtime size     | Platform support                               | Development speed | Runtime speed                                                 |
|-------------------------|------------------------|-------------|---------------------------------------------------------------------------------------------|--------------------------|-------------------------------------------------------|------------------|------------------------------------------------|-------------------|---------------------------------------------------------------|
| Game engine             | Unity, Unreal, Godot   | Scene-based | Scene, nodes, camera, materials                                                             | Low                      | Mostly builtin; programmability varies                | Massive          | Excellent                                      | Very high         | Depends on how similar you are to optimized usecases          |
| Low-level APIs          | DX, Vulkan, Metal      | Pass-based  | Passes, shaders, buffers, textures                                                          | High                     | BYO, Extremely customizable                                | None             | Poor; write once, run once                     | Very low          | Extreme                                                       |
| Layered implementations | MoltenVK, Proton, wgpu | Pass-based  | Passes, shaders, buffers, textures                                                          | High                     | BYO, Customizable in theory, translation causes issues     | Some             | Good in theory, varies in practice             | Very low          | Excellent on native platforms, varies on translated platforms |
| Constructed APIs        | WebGPU                 | Pass-based  | Passes, shaders, buffers, textures                                                          | Medium-high              | BYO, Customizable, though many features stuck in committee | It's complicated | Some browser support, some translation support | Medium-low        | Good                                                          |
| GPU middleware          | images_and_words       | Pass-based  | Passes, shaders, camera, higher-order buffers and textures, multibuffering, common patterns | Medium-low               | BYO, Inherit from backends                                 | Some             | Good in theory, varies in practice             | Medium            | Good                                                          |

# Higher-order memory types

The main innovation of IW is providing an obvious family of higher-order kinds of buffers and textures.

These types are layered atop traditional GPU buffers/textures, but are customized
for specific usecases, such as multibuffering or synchronization.  Because each type encodes
its usecase information, the behavior can be optimized in a usecase-specific way.

Examples include:

| Class    | Use case       | Potential optimizations                 | Multibuffering | Synchronization      |
|----------|----------------|-----------------------------------------|----------------|----------------------|
| Static   | Sprites, etc   | Convert to a private, GPU-native format | Not needed     | Not needed           |
| Forward  | Write CPU->GPU | Unified vs discrete memory              | Builtin        | Builtin              |
| Reverse  | Write GPU->CPU | Unified vs discrete memory              | Builtin        | Builtin              |
| Sideways | Write GPU->GPU | private, GPU-native format              | Builtin        | TBD                  |


# Backends

In the interests of getting going, current development targets [wgpu](https://wgpu.rs)
as backend, so we inherit its broad support for DX12, Vulkan, Metal, WebGPU, Angle, WebGL, etc.

On the other hand, I have intentionally designed IW to support multiple backends, and have prototyped
Vulkan and Metal-based approaches myself.  I intend to stand up other backends as I need them.  If
you need them before I do, get in touch.

Longer-term I am skeptical of wgpu as a backend.  I am skeptical I can meet native performance expectations
with a web-based API, I am skeptical of wgpu's guidance on accepting contributions to solve these issues,
and I am skeptical of any single graphics API as I've seen them come and go while I'm supporting an
application.

A substantial motivation for creating IW is to design an API that can solve these problems and
become a practical and performant target for my own applications.  In the short term, I need
features/optimizations that don't happen in design-by-committee APIs.  In the long term, I need to
maintain my applications after APIs have been deprecated.  IW is the middleware to bridge
this gap in one place.

# Contributions

If you are motivated enough to consider writing your own solution, I would love to have your help
here instead.



*/



mod entry_point;
pub mod images;
pub mod bindings;
pub mod pixel_formats;
mod imp;
mod multibuffer;
mod bittricks;
mod stable_address_vec;
mod send_phantom;

pub use vectormatrix;

pub type Priority = some_executor::Priority;

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    
    use crate::bindings::forward::dynamic::buffer::{Buffer, CRepr};
    use crate::bindings::visible_to::GPUBufferUsage;
    use crate::images::projection::WorldCoord;
    use crate::images::{Engine};
    use crate::images::view::View;

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct TestData {
        x: f32,
        y: f32,
        z: f32,
        w: f32,
    }

    unsafe impl CRepr for TestData {}

    /// Test that reproduces the buffer write performance issue from the reproducer.
    /// 
    /// This test creates an Engine with for_testing() View, creates a Buffer, and measures
    /// buffer.access_write().await performance. The test verifies that buffer operations
    /// complete in reasonable time and detects if they're being throttled by the render pipeline.
    /// 
    /// This test should FAIL if the bug exists where buffer writes take seconds instead of milliseconds.
    #[test_executors::async_test]
    async fn test_buffer_write_performance_issue() {
        // Create a view for testing (bypasses surface requirement)
        let view = View::for_testing();
        
        // Create an engine with a stationary camera
        let initial_camera_position = WorldCoord::new(0.0, 0.0, 10.0);
        let engine = Arc::new(
            Engine::rendering_to(view, initial_camera_position)
                .await
                .expect("Failed to create engine for testing")
        );

        let device = engine.bound_device();

        // Create a test buffer similar to what the reproducer uses
        let test_buffer = Buffer::new(
            device.clone(),
            10, // Small buffer size for testing
            GPUBufferUsage::VertexBuffer,
            "test_buffer_performance",
            |_| TestData { x: 0.0, y: 0.0, z: 0.0, w: 0.0 }
        ).expect("Failed to create test buffer");

        println!("=== Testing buffer write performance ===");
        
        // Test multiple buffer write operations and measure timing
        let mut total_time = Duration::ZERO;
        let iterations = 3;
        
        for i in 0..iterations {
            let start = Instant::now();
            
            // This is the operation that was slow in the reproducer
            let mut write_guard = test_buffer.access_write().await;
            let test_data = TestData {
                x: i as f32,
                y: (i * 2) as f32,
                z: (i * 3) as f32,
                w: (i * 4) as f32,
            };
            write_guard.write(&[test_data], 0);
            drop(write_guard);
            
            let elapsed = start.elapsed();
            println!("  Buffer write iteration {} took: {:?}", i + 1, elapsed);
            total_time += elapsed;
            
            // Small delay between operations like in the reproducer
            portable_async_sleep::async_sleep(Duration::from_millis(1)).await;
        }
        
        let avg_time = total_time / iterations as u32;
        println!("Average buffer write time: {:?}", avg_time);

        // The bug manifested as buffer writes taking SECONDS instead of milliseconds
        // If any single operation takes more than 1 second, that indicates the bug
        let max_acceptable_time = Duration::from_secs(1);
        
        assert!(
            avg_time < max_acceptable_time,
            "Buffer write performance issue detected! Average write time ({:?}) exceeds acceptable threshold ({:?}). \
             This suggests buffer operations are being throttled by the rendering pipeline. \
             In the original bug, operations took seconds instead of milliseconds.",
            avg_time,
            max_acceptable_time
        );

        println!("✅ Buffer write performance is acceptable (avg: {:?})", avg_time);
    }
}


