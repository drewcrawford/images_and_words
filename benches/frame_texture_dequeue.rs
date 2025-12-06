// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//
// Benchmarks for frame texture dequeue operations using wasm-bindgen-test's
// new benchmark feature. These benchmarks only run on wasm32 targets.

#![cfg(all(feature = "backend_wgpu", target_arch = "wasm32"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

// The wasm-bindgen-test-runner provides the actual entry point.
// We need to provide a main function as a placeholder for cargo to compile.
fn main() {}

use images_and_words::Priority;
use images_and_words::bindings::BindStyle;
use images_and_words::bindings::bind_style::{BindSlot, Stage};
use images_and_words::bindings::forward::dynamic::frame_texture::FrameTexture;
use images_and_words::bindings::visible_to::{CPUStrategy, TextureConfig, TextureUsage};
use images_and_words::images::Engine;
use images_and_words::images::projection::WorldCoord;
use images_and_words::images::render_pass::{DrawCommand, PassDescriptor};
use images_and_words::images::shader::{FragmentShader, VertexShader};
use images_and_words::images::view::View;
use images_and_words::pixel_formats::{RGBA8UNorm, Unorm4};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use wasm_bindgen_test::{Criterion, wasm_bindgen_bench};

/// Creates an engine and frame texture for benchmarking.
async fn setup_benchmark() -> (Arc<Engine>, FrameTexture<RGBA8UNorm>) {
    let view = View::for_testing();
    let initial_camera_position = WorldCoord::new(0.0, 0.0, 10.0);
    let engine = Engine::rendering_to(view, initial_camera_position)
        .await
        .expect("Failed to create engine for testing");
    let device = engine.bound_device();
    let mut port = engine.main_port_mut();

    // Create a FrameTexture
    let config = TextureConfig {
        width: 256,
        height: 256,
        visible_to: TextureUsage::FragmentShaderRead,
        debug_name: "benchmark_texture",
        priority: Priority::UserInitiated,
        cpu_strategy: CPUStrategy::WontRead,
        mipmaps: false,
    };

    let frame_texture = FrameTexture::<RGBA8UNorm>::new(&device, config, |_| Unorm4 {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    })
    .await;

    // Create simple shaders
    let vertex_shader = VertexShader::new(
        "benchmark_test",
        r#"
        @vertex
        fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
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
        "benchmark_test",
        r#"
        @group(0) @binding(0) var my_texture: texture_2d<f32>;

        @fragment
        fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
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
        "benchmark_test".to_string(),
        vertex_shader,
        fragment_shader,
        bind_style,
        DrawCommand::TriangleList(3),
        false,
        false,
    ))
    .await;

    // Initial render to put resources in known state
    port.force_render().await;

    // Drop port before returning to release the borrow on engine
    drop(port);

    (engine, frame_texture)
}

/// Benchmark frame texture dequeue with force_render before each iteration.
///
/// This approach calls force_render() before each dequeue to ensure the GPU
/// has released the resource back to UNUSED state.
#[wasm_bindgen_bench]
async fn bench_dequeue_with_force_render(c: &mut Criterion) {
    let (engine, frame_texture) = setup_benchmark().await;
    let frame_texture = Rc::new(RefCell::new(frame_texture));

    c.bench_async_function("dequeue with force_render", |b| {
        let engine = engine.clone();
        let frame_texture = frame_texture.clone();
        Box::pin(b.iter_future(move || {
            let engine = engine.clone();
            let frame_texture = frame_texture.clone();
            async move {
                // Force render to release resources
                engine.main_port_mut().force_render().await;

                // This is the operation we're benchmarking
                let mut ft = frame_texture.borrow_mut();
                let out = ft.dequeue().await;

                // Drop the guard to release back to pending GPU state
                drop(out);
            }
        }))
    })
    .await;
}
