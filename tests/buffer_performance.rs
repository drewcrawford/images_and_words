#![cfg(feature = "backend_wgpu")]

//for the time being, wasm_thread only works in browser
//see https://github.com/rustwasm/wasm-bindgen/issues/4534,
//though we also need wasm_thread support.
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

use images_and_words::bindings::BindStyle;
use images_and_words::bindings::bind_style::BindSlot;
use images_and_words::bindings::forward::dynamic::buffer::Buffer;
use images_and_words::bindings::forward::dynamic::buffer::CRepr;
use images_and_words::bindings::visible_to::GPUBufferUsage;
use images_and_words::images::Engine;
use images_and_words::images::projection::WorldCoord;
use images_and_words::images::render_pass::{DrawCommand, PassDescriptor};
use images_and_words::images::shader::{FragmentShader, VertexShader};
use images_and_words::images::vertex_layout::{VertexFieldType, VertexLayout};
use images_and_words::images::view::View;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};
#[cfg(target_arch = "wasm32")]
use web_time::{Duration, Instant};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
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
#[cfg(feature = "backend_wgpu")]
#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
fn main() {
    test_executors::spawn_local(
        async move {
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

            // Create a test buffer similar to what the reproducer uses
            let test_buffer = Buffer::new(
                device.clone(),
                10, // Small buffer size for testing
                GPUBufferUsage::VertexBuffer,
                "test_buffer_performance",
                |_| TestData {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
            )
            .await
            .expect("Failed to create test buffer");

            let mut bind_style = BindStyle::new();
            let mut layout = VertexLayout::new();
            layout.add_field("x", VertexFieldType::F32);

            bind_style.bind_dynamic_vertex_buffer(BindSlot::new(0), &test_buffer, layout);

            let vertex_shader = VertexShader::new(
                "texture_alignment_test",
                r#"
        struct VertexInput {
            @location(0) x: f32,
        };

        @vertex
        fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
            // Create a full-screen triangle
            var pos = array<vec2<f32>, 3>(
                vec2<f32>(-1.0, -1.0),
                vec2<f32>( 3.0, -1.0),
                vec2<f32>(-1.0,  3.0)
            );
            return vec4<f32>(pos[vertex_index], 0.0, 1.0);
        }
        "#
                .to_string(),
            );
            let fragment_shader = FragmentShader::new(
                "texture_alignment_test",
                r#"
        @fragment
        fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
            // Output a solid color for testing
            return vec4<f32>(1.0, 0.0, 0.0, 1.0); // Red color
        }
        "#
                .to_string(),
            );
            let descriptor = PassDescriptor::new(
                "buffer_performance".to_string(),
                vertex_shader,
                fragment_shader,
                bind_style,
                DrawCommand::TriangleList(3),
                false,
                false,
            );
            engine.main_port_mut().add_fixed_pass(descriptor).await;

            logwise::info_sync!("=== Testing buffer write performance ===");

            // Test multiple buffer write operations and measure timing
            let mut total_time = Duration::ZERO;
            let iterations = 3;

            for i in 0..iterations {
                engine.main_port_mut().force_render().await;
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
                logwise::info_sync!(
                    "  Buffer write iteration {iteration} took: {elapsed}",
                    iteration = i + 1,
                    elapsed = logwise::privacy::LogIt(elapsed)
                );
                total_time += elapsed;

                // Small delay between operations like in the reproducer
                portable_async_sleep::async_sleep(Duration::from_millis(1)).await;
            }

            let avg_time = total_time / iterations as u32;
            logwise::info_sync!("Average buffer write time: {:?}", avg_time);

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

            println!(
                "✅ Buffer write performance is acceptable (avg: {:?})",
                avg_time
            );

            // Exit handled by test framework
        },
        "Testing buffer write performance",
    );
}
