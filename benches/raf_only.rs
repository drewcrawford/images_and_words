// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//
// Benchmark that only tests requestAnimationFrame timing.
// Used to isolate headless Chrome performance issues from WebGPU operations.

#![cfg(target_arch = "wasm32")]

use std::time::Duration;
use wasm_bindgen_test::{Criterion, Instant, wasm_bindgen_bench};

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn log_visibility_state() {
    let document = web_sys::window()
        .expect("No window")
        .document()
        .expect("No document");

    let _hidden = document.hidden();

    // visibility_state() requires web-sys feature, use hidden() which is always available
    logwise::info_sync!("Visibility: hidden={hidden}", hidden = _hidden);
}

async fn request_animation_frame_async() {
    let (c, s) = r#continue::continuation();

    let context = logwise::context::Context::current();
    use web_sys::wasm_bindgen::JsCast;
    use web_sys::wasm_bindgen::closure::Closure;
    web_sys::window()
        .expect("No window found")
        .request_animation_frame(
            Closure::once_into_js(move || {
                context.set_current();
                c.send(());
            })
            .as_ref()
            .unchecked_ref(),
        )
        .expect("Failed to request animation frame");

    s.await
}

#[wasm_bindgen_bench]
async fn bench_raf_only(c: &mut Criterion) {
    *c = std::mem::take(c).measurement_time(Duration::from_secs(15));

    // Log initial visibility state
    log_visibility_state();

    c.bench_async_function("raf_only", move |b| {
        Box::pin(b.iter_custom_future(move |iters| {
            async move {
                let mut accum = Duration::ZERO;
                logwise::info_sync!("RAF bench: Will do {iters} iters", iters = iters);

                for i in 0..iters {
                    let start = Instant::now();
                    // Single RAF call - no nested chains
                    request_animation_frame_async().await;
                    let elapsed = start.elapsed();
                    accum += elapsed;

                    // Log every 10th iteration to track degradation
                    if i % 10 == 0 {
                        logwise::info_sync!(
                            "RAF iter {i}: elapsed {elapsed_ms}ms",
                            i = i,
                            elapsed_ms = elapsed.as_millis()
                        );
                        // Check visibility state periodically
                        log_visibility_state();
                    }
                }

                accum
            }
        }))
    })
    .await;
}
