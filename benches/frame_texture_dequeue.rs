// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//
// Benchmarks for frame texture dequeue operations.
// Uses a custom harness with iter_custom_future-like support to work around
// browser throttling issues in headless/webdriver contexts.

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
#[cfg(not(target_arch = "wasm32"))]
use std::cell::RefCell;
#[cfg(not(target_arch = "wasm32"))]
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use some_executor::task::Configuration;

// ============================================================================
// Custom benchmark harness
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod custom_bench {
    use std::time::Duration;

    pub struct BenchResult {
        pub name: String,
        pub times_ms: Vec<f64>,
    }

    impl BenchResult {
        pub fn report(&self) {
            use logwise::privacy::LogIt;

            if self.times_ms.is_empty() {
                logwise::mandatory_sync!("BENCH {name}: no samples", name = LogIt(&self.name));
                return;
            }

            let sum: f64 = self.times_ms.iter().sum();
            let mean = sum / self.times_ms.len() as f64;

            let variance: f64 = self
                .times_ms
                .iter()
                .map(|t| (t - mean).powi(2))
                .sum::<f64>()
                / self.times_ms.len() as f64;
            let stddev = variance.sqrt();

            let mut sorted = self.times_ms.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = sorted[sorted.len() / 2];
            let p95 = sorted[(sorted.len() as f64 * 0.95) as usize];
            let min = sorted[0];
            let max = sorted[sorted.len() - 1];

            let msg = format!(
                "BENCH {}: mean={:.3}ms ±{:.3}ms, median={:.3}ms, p95={:.3}ms, min={:.3}ms, max={:.3}ms, n={}",
                self.name,
                mean,
                stddev,
                median,
                p95,
                min,
                max,
                self.times_ms.len()
            );
            logwise::mandatory_sync!("{msg}", msg = LogIt(&msg));
        }
    }

    pub struct CustomBencher {
        pub warmup_iters: usize,
        pub sample_iters: usize,
    }

    impl Default for CustomBencher {
        fn default() -> Self {
            Self {
                warmup_iters: 2,
                sample_iters: 10,
            }
        }
    }

    impl CustomBencher {
        /// Run benchmark with custom timing.
        /// The `routine` receives the iteration count and returns the measured Duration.
        /// Use this to exclude setup/wait time from measurements.
        pub async fn iter_custom_future<F, Fut>(&self, name: &str, mut routine: F) -> BenchResult
        where
            F: FnMut(u64) -> Fut,
            Fut: std::future::Future<Output = Duration>,
        {
            use logwise::privacy::LogIt;

            // Warmup
            logwise::mandatory_sync!(
                "BENCH {name}: warming up ({iters} iters)...",
                name = LogIt(&name),
                iters = LogIt(&self.warmup_iters)
            );
            for _ in 0..self.warmup_iters {
                let _ = routine(1).await;
            }

            // Measurement
            logwise::mandatory_sync!(
                "BENCH {name}: measuring ({iters} iters)...",
                name = LogIt(&name),
                iters = LogIt(&self.sample_iters)
            );
            let mut times_ms = Vec::with_capacity(self.sample_iters);

            for i in 0..self.sample_iters {
                let duration = routine(1).await;
                times_ms.push(duration.as_secs_f64() * 1000.0);

                // Log progress every 10 iterations
                if (i + 1) % 10 == 0 {
                    logwise::mandatory_sync!(
                        "BENCH {name}: {done}/{total} complete",
                        name = LogIt(&name),
                        done = LogIt(&(i + 1)),
                        total = LogIt(&self.sample_iters)
                    );
                }
            }

            BenchResult {
                name: name.to_string(),
                times_ms,
            }
        }
    }
}

// ============================================================================
// Platform-specific timing
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    use std::time::Instant;
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_secs_f64() * 1000.0
}

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

    // Spawn render loop
    let engine_for_render = engine.clone();
    some_executor::task::Task::without_notifications(
        "render_loop".to_string(),
        Configuration::default(),
        async move {
            engine_for_render.main_port().start().await.unwrap();
        },
    )
    .spawn_static_current();

    (engine, frame_texture)
}

// ============================================================================
// Benchmarks
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
async fn bench_dequeue_with_sleep_workaround() {
    use custom_bench::CustomBencher;

    #[cfg(feature = "exfiltrate")]
    exfiltrate::begin();

    logwise::mandatory_sync!("BENCH Setting up benchmark...");
    let (_engine, frame_texture) = setup_benchmark().await;
    let frame_texture = Rc::new(RefCell::new(frame_texture));

    let bencher = CustomBencher::default();

    let wait_duration = Duration::from_secs(1);
    let ft = frame_texture.clone();

    let result = bencher
        .iter_custom_future("dequeue_with_sleep", |_iters| {
            let ft = ft.clone();
            async move {
                // Wait to try to avoid throttling (NOT timed)
                portable_async_sleep::async_sleep(wait_duration).await;

                // Measure only the actual operation
                let start = now_ms();
                {
                    let mut frame_texture = ft.borrow_mut();
                    let _out = frame_texture.dequeue().await;
                }
                let elapsed_ms = now_ms() - start;

                Duration::from_secs_f64(elapsed_ms / 1000.0)
            }
        })
        .await;

    result.report();
}

// ============================================================================
// Main entry point
// ============================================================================

// wasm-bindgen-test-runner requires exported test/bench functions
#[cfg(target_arch = "wasm32")]
mod wasm_bench {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen_test::{Criterion, Instant, wasm_bindgen_bench};

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    //basically we choose this such that we get <16.667 ms (60fps) or <8.333ms (120fps)
    const WAIT_DURATION: Duration = Duration::from_millis(0);

    #[wasm_bindgen_bench]
    async fn bench_with_sleep(c: &mut Criterion) {
        #[cfg(feature = "exfiltrate")]
        exfiltrate::begin();

        *c = std::mem::take(c).measurement_time(Duration::from_secs(15));
        let (engine, frame_texture) = setup_benchmark().await;
        let frame_texture = Rc::new(RefCell::new(frame_texture));

        let ft = frame_texture.clone();
        c.bench_async_function("dequeue_withs_sleep", move |b| {
            let ft = ft.clone();
            Box::pin(b.iter_custom_future(move |iters| {
                let ft = ft.clone();
                async move {
                    let mut accum = crate::Duration::ZERO;
                    logwise::mandatory_sync!("Will do {iters} iters", iters = iters);
                    for _ in 0..iters {
                        // Sleep to avoid throttling - NOT timed
                        portable_async_sleep::async_sleep(WAIT_DURATION).await;

                        // Only measure the actual operation
                        let start = Instant::now();
                        logwise::mandatory_sync!("iter begin");

                        let mut frame_texture = ft.borrow_mut();
                        std::hint::black_box(frame_texture.dequeue().await);
                        logwise::mandatory_sync!("iter end");
                        accum += start.elapsed();
                    }
                    accum
                }
            }))
        })
        .await;
        // Cleanup by stopping our port
        engine.main_port_mut().stop();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    test_executors::sleep_on(async {
        logwise::mandatory_sync!("BENCH === Running dequeue benchmark WITH sleep workaround ===");
        bench_dequeue_with_sleep_workaround().await;

        logwise::mandatory_sync!("BENCH === Running dequeue benchmark WITHOUT sleep ===");
    });
}

#[cfg(target_arch = "wasm32")]
fn main() {}
