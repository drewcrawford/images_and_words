// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//
// Benchmarks for frame texture dequeue operations.
// Native uses Criterion; WASM uses wasm-bindgen-test's Criterion.

#![cfg(feature = "backend_wgpu")]

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
use std::time::Duration;

// ============================================================================
// Benchmark setup
// ============================================================================

async fn setup_benchmark() -> (Arc<Engine>, FrameTexture<RGBA8UNorm>) {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let view = View::for_testing();
    let initial_camera_position = WorldCoord::new(0.0, 0.0, 10.0);
    let engine = Engine::rendering_to(view, initial_camera_position)
        .await
        .expect("Failed to create engine for testing");
    let device = engine.bound_device();

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

    let mut bind_style = BindStyle::new();
    bind_style.bind_dynamic_texture(BindSlot::new(0), Stage::Fragment, &frame_texture);

    {
        let port = engine.main_port();
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

        port.force_render().await;
    }

    // Spawn render loop.
    // On WASM, GPUDevice cannot be shared across web workers, so we use spawn_local
    // to stay on the same thread. On native, we spawn a separate thread.
    let engine_for_render = engine.clone();
    #[cfg(target_arch = "wasm32")]
    test_executors::spawn_local(
        async move {
            engine_for_render.main_port().start().await.unwrap();
        },
        "render_loop",
    );
    #[cfg(not(target_arch = "wasm32"))]
    test_executors::spawn_on("render_loop", async move {
        engine_for_render.main_port().start().await.unwrap();
    });

    (engine, frame_texture)
}

// ============================================================================
// Native benchmarks using Criterion
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
use criterion::{Criterion, criterion_group, criterion_main};

#[cfg(not(target_arch = "wasm32"))]
fn bench_dequeue(c: &mut Criterion) {
    #[cfg(feature = "exfiltrate")]
    exfiltrate::begin();

    // Set up the benchmark environment
    let (_engine, frame_texture) = test_executors::sleep_on(setup_benchmark());
    let frame_texture = Rc::new(RefCell::new(frame_texture));

    let mut group = c.benchmark_group("frame_texture");
    group.measurement_time(Duration::from_secs(10));

    let ft = frame_texture.clone();
    group.bench_function("dequeue", |b| {
        b.iter_custom(|iters| {
            let ft = ft.clone();
            test_executors::sleep_on(async move {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    //wait a bit between iterations
                    portable_async_sleep::async_sleep(Duration::from_millis(8)).await;
                    let start = std::time::Instant::now();
                    {
                        let mut frame_texture = ft.borrow_mut();
                        std::hint::black_box(frame_texture.dequeue().await);
                    }
                    total += start.elapsed();
                }
                total
            })
        })
    });

    group.finish();
}

#[cfg(not(target_arch = "wasm32"))]
criterion_group!(benches, bench_dequeue);

#[cfg(not(target_arch = "wasm32"))]
criterion_main!(benches);

// ============================================================================
// WASM benchmarks using wasm-bindgen-test's Criterion
// ============================================================================

#[cfg(target_arch = "wasm32")]
mod wasm_bench {
    use super::*;
    use wasm_bindgen_test::{Criterion, Instant, wasm_bindgen_bench};
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    // Choose wait duration to stay under frame budget (60fps = 16.667ms, 120fps = 8.333ms)
    const WAIT_DURATION: Duration = Duration::from_millis(12);

    #[wasm_bindgen_bench]
    async fn bench_with_sleep(c: &mut Criterion) {
        #[cfg(feature = "exfiltrate")]
        exfiltrate::begin();

        *c = std::mem::take(c).measurement_time(Duration::from_secs(15));
        #[allow(unused_variables)]
        let (engine, frame_texture) = setup_benchmark().await;
        let frame_texture = Rc::new(RefCell::new(frame_texture));

        let ft = frame_texture.clone();
        c.bench_async_function("dequeue_with_sleep", move |b| {
            let ft = ft.clone();
            Box::pin(b.iter_custom_future(move |iters| {
                let ft = ft.clone();
                async move {
                    let mut accum = Duration::ZERO;
                    for _ in 0..iters {
                        // Sleep to avoid throttling - NOT timed
                        portable_async_sleep::async_sleep(WAIT_DURATION).await;

                        // Only measure the actual operation
                        let start = Instant::now();
                        let mut frame_texture = ft.borrow_mut();
                        std::hint::black_box(frame_texture.dequeue().await);
                        accum += start.elapsed();
                    }
                    accum
                }
            }))
        })
        .await;
        // Cleanup by stopping our port
        engine.main_port().stop();
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
