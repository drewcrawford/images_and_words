// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//
// Benchmarks for frame texture dequeue operations using wasm-bindgen-test's
// new benchmark feature. These benchmarks only run on wasm32 targets.

#![cfg(feature = "backend_wgpu")]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(target_arch = "wasm32")]
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
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::{Criterion, wasm_bindgen_bench};

#[cfg(not(target_arch = "wasm32"))]
use criterion::Criterion;

use some_executor::task::Configuration;
#[cfg(not(target_arch = "wasm32"))]
use std::future::Future;

/// Creates an engine and frame texture for benchmarking.
/// Also spawns a background thread that runs the render loop with dirty tracking.
async fn setup_benchmark() -> (Arc<Engine>, FrameTexture<RGBA8UNorm>) {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let view = View::for_testing();
    let initial_camera_position = WorldCoord::new(0.0, 0.0, 10.0);
    let engine = Engine::rendering_to(view, initial_camera_position)
        .await
        .expect("Failed to create engine for testing");
    let device = engine.bound_device();

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

    // Add a render pass (need to hold port temporarily)
    {
        let mut port = engine.main_port_mut();
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
    }

    // Spawn render loop as async task on main thread (no separate thread needed)
    // It will interleave with the benchmark via async yielding
    let engine_for_render = engine.clone();
    some_executor::task::Task::without_notifications(
        "render_loop".to_string(),
        Configuration::default(),
        async move {
            logwise::mandatory_sync!("render_loop async task started on main thread");
            engine_for_render.main_port_mut().start().await.unwrap();
        },
    )
    .spawn_static_current();

    (engine, frame_texture)
}

async fn actual_fn(frame_texture: Rc<RefCell<FrameTexture<RGBA8UNorm>>>) {
    // This is the operation we're benchmarking.
    // The background render thread handles rendering when dirty.
    let mut ft = frame_texture.borrow_mut();
    let out = ft.dequeue().await;

    // Drop the guard to release back to pending GPU state (marks dirty)
    drop(out);
}

/// Benchmark frame texture dequeue with background render thread.
///
/// This approach uses a background thread running port.start() with dirty
/// tracking. The GPU naturally paces rendering based on dirty state, avoiding
/// the backpressure issue of force_render in a tight loop.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_bench]
async fn bench_dequeue_with_dirty_tracking(c: &mut Criterion) {
    exfiltrate::begin();
    let (_engine, frame_texture) = setup_benchmark().await;
    let frame_texture = Rc::new(RefCell::new(frame_texture));
    c.bench_async_function("dequeue with dirty tracking", |b| {
        let frame_texture = frame_texture.clone();
        Box::pin(b.iter_future(move || {
            let frame_texture = frame_texture.clone();
            async move {
                actual_fn(frame_texture).await;
            }
        }))
    })
    .await;
}

#[cfg(not(target_arch = "wasm32"))]
fn bench_native(c: &mut Criterion) {
    exfiltrate::begin();
    let (_engine, frame_texture) = test_executors::sleep_on(async move { setup_benchmark().await });

    let frame_texture = Rc::new(RefCell::new(frame_texture));
    c.bench_function("bench_dequeue_with_dirty_tracking", |b| {
        // Insert a call to `to_async` to convert the bencher to async mode.
        // The timing loops are the same as with the normal bencher.
        b.to_async(Wrap).iter(|| actual_fn(frame_texture.clone()));
    });
}

#[cfg(not(target_arch = "wasm32"))]
criterion::criterion_group!(benches, bench_native);
#[cfg(not(target_arch = "wasm32"))]
criterion::criterion_main!(benches);

//criterion support for test_executors
//consider implementing this properly if we need it again
#[cfg(not(target_arch = "wasm32"))]
struct Wrap;

#[cfg(not(target_arch = "wasm32"))]
impl criterion::async_executor::AsyncExecutor for Wrap {
    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        //need to preserve output I guess
        //this is fine on native
        let (s, r) = std::sync::mpsc::sync_channel(1);
        test_executors::sleep_on(async {
            let t = future.await;
            s.send(t).unwrap();
        });
        r.recv().unwrap()
    }
}
