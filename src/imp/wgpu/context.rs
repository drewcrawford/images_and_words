/*!
Holds threading primitives on wgpu */

#[cfg(feature = "app_window")]
use app_window::application::submit_to_main_thread;
use r#continue::continuation;
use logwise::context::Context;
use some_executor::task::{Configuration, Task};

pub(crate) enum WGPUStrategy {
    #[cfg(feature = "app_window")]
    MainThread,
    #[cfg(feature = "app_window")]
    NotMainThread,
    Relaxed,
}

impl WGPUStrategy {
    #[cfg(feature = "app_window")]
    const fn from_appwindow_strategy(strategy: app_window::WGPUStrategy) -> Self {
        match strategy {
            app_window::WGPUStrategy::MainThread => WGPUStrategy::MainThread,
            app_window::WGPUStrategy::NotMainThread => WGPUStrategy::NotMainThread,
            app_window::WGPUStrategy::Relaxed => WGPUStrategy::Relaxed,
            _ => panic!("non-exhaustive match must be updated"),
        }
    }
}

/**
Begins a context for wgpu operations.

# Context

This function begins a wgpu execution context, allowing you to run futures that interact with wgpu.

The type of context depends on the platform's wgpu strategy, which is defined by the `WGPU_STRATEGY` constant.

* `WGPUStrategy::MainThread`: Executes the future on the main thread via app_window's main thread executor.
* `WGPUStrategy::NotMainThread`: If we're not on the main thread, use [some_executor::thread_executor].  If we're on the main thread,
   spin up a new thread with a local executor.
* `WGPUStrategy::Relaxed`: If we're on the main thread, use the main thread executor.
   If we're not on the main thread, use the thread executor.

# A brief digression on Sendability

In Rust the `Send` trait indicates that a type can be transferred between threads. For a Future,
this means the future can arbitrarily be sent between polls (so you can wake up on a different
thread every time).

Meanwhile, GPU backends often require you to call their APIs "in context".  This is typically,
though not always, from a certain thread.  If so, GPU types tend to be modeled as !Send, complicating
their use in async code.  At the same time, you need Send to get into the "right context" if that
context is another thread.

Usually what we want to model is "you can Send until the future starts running, and not after that",
which is a bit complex to express in Rust.  How we do it is:

* [`wgpu_begin_context`]: Sets up the context (possibly a thread) and runs a Send future in it.
* [`wgpu_in_context`]`: Uses a previously established context to run a future that is not Send.
*/

#[cfg(feature = "app_window")]
pub const WGPU_STRATEGY: WGPUStrategy =
    WGPUStrategy::from_appwindow_strategy(app_window::WGPU_STRATEGY);

#[cfg(not(feature = "app_window"))]
pub const WGPU_STRATEGY: WGPUStrategy = WGPUStrategy::Relaxed;

pub fn begin<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    match WGPU_STRATEGY {
        #[cfg(feature = "app_window")]
        WGPUStrategy::MainThread => {
            if app_window::application::is_main_thread() {
                // If we're on the main thread, we can just call the function directly.
                f();
            } else {
                // If we're not on the main thread, we need to run it on the main thread executor.
                let hop_on_main_thread =
                    logwise::perfwarn_begin!("wgpu_begin_context hop_on_main_thread");
                submit_to_main_thread(|| {
                    drop(hop_on_main_thread);
                    f();
                });
            }
        }
        #[cfg(feature = "app_window")]
        WGPUStrategy::NotMainThread => {
            if app_window::application::is_main_thread() {
                std::thread::Builder::new()
                    .name("wgpu_begin_context".to_string())
                    .spawn(|| {
                        f();
                    })
                    .expect("Failed to spawn wgpu_begin_context thread");
            } else {
                f();
            }
        }
        WGPUStrategy::Relaxed => {
            f();
        }
    }
}

pub async fn smuggle<F, R>(label: String, f: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let prior_context = logwise::context::Context::current();
    let (s, r) = continuation();
    begin(move || {
        let c = logwise::context::Context::new_task(Some(prior_context), "smuggle");
        let id = c.context_id();
        c.set_current();
        logwise::info_sync!("smuggle {label}", label = label);
        let r = f();
        s.send(r);
        Context::pop(id);
    });
    r.await
}

pub async fn smuggle_async<F, C, R>(label: String, c: C) -> R
where
    F: Future<Output = R> + 'static,
    C: FnOnce() -> F + Send + 'static,
    R: Send + 'static,
{
    let (s, r) = continuation();
    begin(|| {
        let f = c();
        Task::without_notifications(label, Configuration::default(), async move {
            let r = f.await;
            s.send(r);
        })
        .spawn_thread_local();
    });
    r.await
}
