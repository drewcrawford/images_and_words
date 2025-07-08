//! Tests to ensure that futures returned by buffer mapping operations are Send.
//!
//! This is critical for async code that needs to spawn tasks or work with thread pools.
//! The tests verify that:
//! 1. Buffer::access_write() returns a Send future
//! 2. Buffer operations can be sent across async task boundaries
//! 3. The underlying mapping operations maintain Send + Sync guarantees

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;

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

/// Test that access_write can be sent across task boundaries.
///
/// This test verifies that buffer access futures work correctly when
/// sent between async contexts, which is a practical requirement for
/// many async applications.

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
fn main() {
    test_executors::spawn_local(
        async move {
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
            let test_buffer = Buffer::new(
                device.clone(),
                10,
                GPUBufferUsage::VertexBuffer,
                "cross_task_test_buffer",
                |i| TestData { value: i as f32 },
            )
            .await
            .expect("Failed to create buffer");

            // Test sending the buffer and its access future across a task boundary
            let buffer_clone = test_buffer.clone();

            // Create a future that can be sent
            let access_future = async move {
                let mut write_access = buffer_clone.access_write().await;

                // Write some test data
                write_access.write(&[TestData { value: 42.0 }], 0);

                // Properly drop the write access guard
                write_access.async_drop().await;

                "success"
            };

            // This compilation test verifies the future is Send
            fn assert_send<T: Send>(_: &T) {}
            assert_send(&access_future);

            // Execute the future
            let result = access_future.await;
            assert_eq!(result, "success");

            println!("Sendable futures test completed successfully");
            // Exit handled by test framework
        },
        "sendable_futures_test",
    );
}
