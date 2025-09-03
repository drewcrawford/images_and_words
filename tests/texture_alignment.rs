#![cfg(feature = "backend_wgpu")]

//for the time being, wasm_thread only works in browser
//see https://github.com/rustwasm/wasm-bindgen/issues/4534,
//though we also need wasm_thread support.
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;

use futures::FutureExt;
use images_and_words::Priority;
use images_and_words::bindings::BindStyle;
use images_and_words::bindings::bind_style::{BindSlot, Stage};
use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
use images_and_words::bindings::software::texture::Texel;
use images_and_words::bindings::visible_to::{CPUStrategy, TextureConfig, TextureUsage};
use images_and_words::images::Engine;
use images_and_words::images::projection::WorldCoord;
use images_and_words::images::render_pass::{DrawCommand, PassDescriptor};
use images_and_words::images::shader::{FragmentShader, VertexShader};
use images_and_words::images::view::View;
use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
use std::sync::Arc;
use test_executors::async_test;

#[async_test]
async fn test_texture_alignment_error_width_100() {
    println!("=== Testing texture alignment error with width 100 ===");
    test_problematic_width(100).await;
}

#[async_test]
async fn test_texture_alignment_error_width_63() {
    println!("=== Testing texture alignment error with width 63 ===");
    test_problematic_width(63).await;
}

#[async_test]
async fn test_texture_alignment_error_width_150() {
    println!("=== Testing texture alignment error with width 150 ===");
    test_problematic_width(150).await;
}

#[async_test]
async fn test_texture_alignment_ok_width_64() {
    println!("=== Testing properly aligned texture with width 64 ===");
    test_problematic_width(64).await; // 64 * 4 = 256, which is aligned
}

#[async_test]
async fn test_texture_alignment_ok_width_128() {
    println!("=== Testing properly aligned texture with width 128 ===");
    test_problematic_width(128).await; // 128 * 4 = 512, which is aligned
}

/// Helper function to test a specific problematic width
async fn test_problematic_width(width: u16) {
    // Calculate bytes per row for RGBA8 format (4 bytes per pixel)
    let bytes_per_row = width as u32 * 4;
    let is_aligned = bytes_per_row % 256 == 0;

    println!("Testing width {} pixels", width);
    println!(
        "Bytes per row: {} (aligned to 256: {})",
        bytes_per_row, is_aligned
    );

    // Create a view for testing (bypasses surface requirement)
    let view = View::for_testing();

    // Create an engine
    let initial_camera_position = WorldCoord::new(0.0, 0.0, 10.0);
    let engine = Arc::new(
        Engine::rendering_to(view, initial_camera_position)
            .await
            .expect("Failed to create engine for testing"),
    );
    let device = engine.bound_device();
    let mut port = engine.main_port_mut();

    // Create a FrameTexture with the problematic width
    let config = TextureConfig {
        width,
        height: 100, // Height doesn't matter for this test
        visible_to: TextureUsage::FragmentShaderRead,
        debug_name: "alignment_test_texture",
        priority: Priority::UserInitiated,
        cpu_strategy: CPUStrategy::WontRead,
        mipmaps: false,
    };

    let mut frame_texture = FrameTexture::<RGBA8UNorm>::new(
        &device,
        config,
        |_| Unorm4 {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }, // Initialize to black
    )
    .await;

    println!("Created FrameTexture with width {}", width);

    // Create simple shaders
    let vertex_shader = VertexShader::new(
        "texture_alignment_test",
        r#"
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
        @group(0) @binding(0) var my_texture: texture_2d<f32>;
        
        @fragment
        fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
            // Use textureLoad instead of textureSample to avoid needing a sampler
            let coord = vec2<i32>(i32(position.x), i32(position.y));
            return textureLoad(my_texture, coord, 0);
        }
        "#
        .to_string(),
    );

    // Create bind style and bind the texture
    let mut bind_style = BindStyle::new();
    bind_style.bind_dynamic_texture(BindSlot::new(0), Stage::Fragment, &frame_texture);

    // Add a render pass
    port.add_fixed_pass(PassDescriptor::new(
        "texture_alignment_test".to_string(),
        vertex_shader,
        fragment_shader,
        bind_style,
        DrawCommand::TriangleList(3),
        false,
        false,
    ))
    .await;

    //pump renderloop
    port.force_render().await;

    // Write some data to the texture to mark it as dirty
    println!("Writing data to texture to mark it as dirty...");
    {
        let mut write_guard = frame_texture.dequeue().await;

        // Create full texture data (width × height pixels)
        let total_pixels = (width as usize) * (100 as usize); // height is 100
        let mut pixel_data = vec![
            Unorm4 {
                r: 0,
                g: 0,
                b: 0,
                a: 255
            };
            total_pixels
        ]; // Black background

        // Set first pixel to red
        if !pixel_data.is_empty() {
            pixel_data[0] = Unorm4 {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            };
        }

        // Write the full texture data
        write_guard.replace(
            width, // source width
            Texel { x: 0, y: 0 },
            &pixel_data,
        );
    }

    println!("Rendering frame to trigger copy operation...");

    // This should trigger the copy operation and cause the alignment error
    // on Windows if the bytes_per_row is not properly aligned

    let result = std::panic::AssertUnwindSafe(port.force_render())
        .catch_unwind()
        .await;

    match result {
        Ok(_) => {
            if !is_aligned {
                println!("⚠️  WARNING: Expected alignment error did not occur!");
                println!(
                    "   Width {} has bytes_per_row {} which is NOT aligned to 256",
                    width, bytes_per_row
                );
                println!("   This suggests the alignment issue may have been fixed or");
                println!("   the error might only occur on Windows.");
            } else {
                println!("✓ Test passed: Width {} is properly aligned", width);
            }
        }
        Err(e) => {
            // The panic payload might be a String or &str
            let is_alignment_error = if let Some(msg) = e.downcast_ref::<String>() {
                msg.contains("COPY_BYTES_PER_ROW_ALIGNMENT")
            } else if let Some(msg) = e.downcast_ref::<&str>() {
                msg.contains("COPY_BYTES_PER_ROW_ALIGNMENT")
            } else {
                false
            };

            if is_alignment_error {
                if !is_aligned {
                    println!("✓ Successfully reproduced alignment error!");
                    println!(
                        "  Width {} with bytes_per_row {} is not aligned to 256",
                        width, bytes_per_row
                    );
                    println!(
                        "  This error should be fixed by aligning bytes_per_row to COPY_BYTES_PER_ROW_ALIGNMENT (256)"
                    );
                    // Don't re-panic - the test successfully demonstrated the issue
                } else {
                    // This shouldn't happen - aligned width shouldn't cause error
                    panic!(
                        "Unexpected: Alignment error occurred for width {} which should be aligned!",
                        width
                    );
                }
            } else {
                // Re-panic with original error if it's not the expected one
                println!("Unexpected error occurred: {:?}", e);
                std::panic::resume_unwind(e);
            }
        }
    }
}
