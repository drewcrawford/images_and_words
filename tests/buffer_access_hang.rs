#![cfg(all(feature = "testing", feature = "backend_wgpu"))]

use images_and_words::bindings::BindStyle;
use images_and_words::bindings::bind_style::{BindSlot, Stage};
use images_and_words::bindings::forward::dynamic::buffer::Buffer;
use images_and_words::bindings::forward::dynamic::buffer::CRepr;
use images_and_words::bindings::visible_to::GPUBufferUsage;
use images_and_words::images::Engine;
use images_and_words::images::projection::WorldCoord;
use images_and_words::images::render_pass::{DrawCommand, PassDescriptor};
use images_and_words::images::shader::{FragmentShader, VertexShader};
use images_and_words::images::view::View;
use std::sync::Arc;
use std::time::Duration;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
struct TestData {
    x: f32,
    y: f32,
    z: f32,
}

unsafe impl CRepr for TestData {}

/// Test that reproduces the buffer access hang from the reproducer.
///
/// This test creates an Engine with for_testing() View and repeatedly calls
/// Buffer::access_write().await. We test the scenario where a PortLoop would
/// be running by checking that the buffer properly integrates with the dirty
/// tracking system that the PortLoop uses.
///
/// The hang manifests as Buffer::access_write().await never completing after
/// several successful iterations, likely due to issues with dirty tracking
/// or resource contention in the GPU pipeline.
#[test_executors::async_test]
#[cfg(feature = "backend_wgpu")]
async fn test_buffer_access_hang_reproducer() {
    println!("=== Testing buffer access hang reproducer ===");

    // Create a view for testing (bypasses surface requirement)
    let view = View::for_testing();

    // Create an engine with a stationary camera
    let initial_camera_position = WorldCoord::new(0.0, 0.0, 10.0);
    let engine = Arc::new(
        Engine::rendering_to(view, initial_camera_position)
            .await
            .expect("Failed to create engine for testing"),
    );
    let device = engine.bound_device();
    let mut port = engine.main_port_mut();

    // Create a test buffer similar to what the reproducer uses
    let test_buffer = Arc::new(
        Buffer::new(
            device.clone(),
            1024,
            GPUBufferUsage::FragmentShaderRead,
            "hang_test_buffer",
            |_| TestData {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        )
        .expect("Failed to create test buffer"),
    );

    println!("Created buffer: {:?}", test_buffer);

    let vertex_shader = VertexShader::new("buffer_access_hang_test", "@vertex   fn vs_main()  -> @builtin(position) vec4<f32> { return vec4<f32>(1.0, 1.0, 1.0, 1.0); }".to_string());
    let fragment_shader = FragmentShader::new("buffer_access_hang_test", "@fragment fn fs_main() -> @location(0) vec4<f32>       { return vec4<f32>(1.0, 0.0, 0.0, 1.0); }".to_string());

    let mut bind_style = BindStyle::new();
    bind_style.bind_dynamic_buffer(BindSlot::new(0), Stage::Fragment, &test_buffer);

    port.add_fixed_pass(PassDescriptor::new(
        "buffer_access_hang_test".to_string(),
        vertex_shader,
        fragment_shader,
        bind_style,
        DrawCommand::TriangleStrip(4),
        false,
        false,
    ))
    .await;

    // Simplify the test to avoid Send/Sync issues with executors
    // We'll test buffer access directly and use a timeout at the async function level

    // Test multiple buffer write operations in sequence
    let mut iteration = 0;
    let start_time = std::time::Instant::now();
    let max_test_duration = Duration::from_secs(1);

    println!("Beginning buffer access test...");

    let (send, rend) = std::sync::mpsc::channel();

    test_executors::spawn_on("buffer thread", async move {
        // Reproduce the pattern from the reproducer
        loop {
            iteration += 1;
            println!("=== Iteration {} ===", iteration);

            // Check if we've exceeded our time limit
            if start_time.elapsed() > max_test_duration {
                panic!(
                    "❌ Test timed out after {:?}! This indicates the buffer access hang bug is present. \
                 The Buffer::access_write().await calls are taking too long, \
                 which matches the behavior described in the reproducer.",
                    max_test_duration
                );
            }

            println!("Requesting buffer write access on buffer {:?}", test_buffer);

            let access_start = std::time::Instant::now();

            // This is where the hang occurs in the reproducer
            let mut write = test_buffer.access_write().await;

            let access_time = access_start.elapsed();
            println!("Got buffer write access (took {:?})", access_time);

            // If any single access takes more than 1 second, that's suspicious
            if access_time > Duration::from_secs(1) {
                panic!(
                    "❌ Buffer access took {:?} which is unexpectedly long! \
                 This suggests the hang issue is present.",
                    access_time
                );
            }

            // Write some test data
            for i in 0..4 {
                let data = TestData {
                    x: (i as f32) * 10.0 + (iteration as f32),
                    y: (i as f32) * 20.0 + (iteration as f32),
                    z: (i as f32) * 30.0 + (iteration as f32),
                };
                write.write(&[data], i);
            }

            println!("Completed buffer write, dropping write access");
            drop(write);

            // Small delay like in the reproducer
            portable_async_sleep::async_sleep(Duration::from_millis(10)).await;

            // After a few iterations, exit successfully if no hang occurred
            if iteration >= 3 {
                println!("✅ Completed {} iterations without hanging", iteration);
                send.send(()).unwrap();
                break;
            }
        }

        println!("✅ Test passed - no hang detected in buffer access operations");
    });

    let start = std::time::Instant::now();
    let mut iterations = 0;
    loop {
        if port.needs_render() {
            port.force_render().await;
            iterations += 1;
        }

        if rend.try_recv().is_ok() {
            println!("Received completion signal after {} iterations", iterations);
            break;
        }
        if start.elapsed() > max_test_duration {
            panic!(
                "❌ Test timed out waiting for buffer access operations to complete! This indicates the hang issue is present."
            );
        }
    }
}
