//! Test to reproduce the WgpuCell threading error with CPUWriteAccess
//!
//! This test creates a buffer, obtains write access, and then tries to call
//! async_drop from a non-main thread to trigger the "WgpuCell accessed from
//! non-main thread when strategy is MainThread" error.

#![cfg(all(feature = "testing", feature = "backend_wgpu"))]

use images_and_words::bindings::forward::dynamic::buffer::Buffer;
use images_and_words::bindings::forward::dynamic::buffer::CRepr;
use images_and_words::bindings::visible_to::GPUBufferUsage;
use images_and_words::images::Engine;
use images_and_words::images::projection::WorldCoord;
use images_and_words::images::view::View;
use std::sync::Arc;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct TestData {
    value: f32,
}

unsafe impl CRepr for TestData {}

fn main() {
    println!("=== Testing WgpuCell threading error reproduction ===");
    app_window::application::main(|| {
        app_window::wgpu::wgpu_begin_context(async {
            app_window::wgpu::wgpu_in_context(async {
                // Create a view for testing
                let view = View::for_testing();

                // Create an engine
                let initial_camera_position = WorldCoord::new(0.0, 0.0, 10.0);
                let engine = Arc::new(
                    Engine::rendering_to(view, initial_camera_position)
                        .await
                        .expect("Failed to create engine for testing"),
                );

                let device = engine.bound_device();

                // Create a test buffer
                let test_buffer = Arc::new(
                    Buffer::new(
                        device.clone(),
                        100,
                        GPUBufferUsage::VertexBuffer,
                        "threading_test_buffer",
                        |i| TestData { value: i as f32 },
                    )
                    .await
                    .expect("Failed to create buffer"),
                );

                println!("Created buffer successfully");

                // Now try to spawn a thread that gets write access and calls async_drop from a non-main thread
                // This should trigger the WgpuCell threading error
                let (sender, receiver) = std::sync::mpsc::channel();
                let test_buffer_clone = test_buffer.clone();

                std::thread::spawn(move || {
                    println!("Getting write access from spawned thread (non-main thread)");

                    // This should trigger: "WgpuCell accessed from non-main thread when strategy is MainThread"
                    test_executors::sleep_on(async move {
                        let mut write_access = test_buffer_clone.access_write().await;
                        println!("Obtained write access on non-main thread");

                        // Write some data
                        write_access.write(&[TestData { value: 42.0 }], 0);
                        println!("Wrote data to buffer");

                        println!("Calling async_drop from spawned thread (non-main thread)");
                        write_access.async_drop().await;
                    });

                    let _ = sender.send(0);
                });

                // Wait for the spawned thread to complete
                let result = receiver
                    .recv()
                    .expect("Failed to receive result from spawned thread");

                //exit the process
                std::process::exit(result);
            });
        });
    });
}
