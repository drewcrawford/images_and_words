//SPDX-License-Identifier: MPL-2.0

use super::context::{WGPU_STRATEGY, WGPUStrategy, begin, smuggle, smuggle_async};
use send_cells::UnsafeSendCell;
use send_cells::unsafe_sync_cell::UnsafeSyncCell;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug)]
struct Shared<T: 'static> {
    inner: Option<UnsafeSendCell<UnsafeSyncCell<T>>>,
    mutex: Mutex<()>,
}

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        //when we're dropping the last value,
        //we need to do so on the right thread
        let take = self.inner.take().unwrap();
        begin(|| {
            drop(take);
        })
    }
}

pub struct WgpuGuard<'a, T: 'static> {
    _guard: MutexGuard<'a, ()>,
    value: &'a mut T,
}

impl<'a, T> Deref for WgpuGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.value
    }
}

impl<'a, T> DerefMut for WgpuGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.value
    }
}

impl<'a, T: Debug> Debug for WgpuGuard<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuGuard")
            .field("value", &*self.value)
            .finish()
    }
}

pub struct WgpuCell<T: 'static> {
    //option is so we can consume + custom drop
    shared: Option<Arc<Shared<T>>>,
}

impl<T> PartialEq for WgpuCell<T> {
    fn eq(&self, other: &Self) -> bool {
        let s = self.shared.as_ref().unwrap();
        let o = other.shared.as_ref().unwrap();
        Arc::ptr_eq(s, o)
    }
}

impl<T> Eq for WgpuCell<T> {}

impl<T> std::hash::Hash for WgpuCell<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let s = self.shared.as_ref().unwrap();
        (s.as_ref() as *const Shared<T>).hash(state);
    }
}

impl<T> Clone for WgpuCell<T> {
    fn clone(&self) -> Self {
        WgpuCell {
            shared: self.shared.clone(),
        }
    }
}

impl<T> WgpuCell<T> {
    #[inline]
    pub fn new(t: T) -> Self {
        //I don't think we actually need to verify the thread here?
        //we promise drop is correctly handled
        let cell = unsafe { UnsafeSendCell::new_unchecked(UnsafeSyncCell::new(t)) };
        WgpuCell {
            shared: Some(Arc::new(Shared {
                inner: Some(cell),
                mutex: Mutex::new(()),
            })),
        }
    }

    #[inline]
    pub fn verify_thread() {
        match WGPU_STRATEGY {
            #[cfg(feature = "app_window")]
            WGPUStrategy::MainThread => {
                assert!(
                    app_window::application::is_main_thread(),
                    "WgpuCell accessed from non-main thread when strategy is MainThread"
                );
            }
            #[cfg(feature = "app_window")]
            WGPUStrategy::NotMainThread => {
                assert!(
                    !app_window::application::is_main_thread(),
                    "WgpuCell accessed from main thread when strategy is NotMainThread"
                );
            }
            WGPUStrategy::Relaxed => {
                // No verification needed
            }
        }
    }

    pub fn lock(&self) -> WgpuGuard<'_, T> {
        Self::verify_thread();
        let guard = self.shared.as_ref().unwrap().mutex.lock().unwrap();
        let value = unsafe {
            let inner = self.shared.as_ref().unwrap().inner.as_ref().unwrap();
            inner.get().get_mut_unchecked()
        };
        WgpuGuard {
            _guard: guard,
            value,
        }
    }

    pub fn assume<C, R>(&self, c: C) -> R
    where
        C: FnOnce(&T) -> R,
    {
        Self::verify_thread();
        let guard = self.shared.as_ref().unwrap().mutex.lock().unwrap();
        let r = c(unsafe {
            self.shared
                .as_ref()
                .unwrap()
                .inner
                .as_ref()
                .unwrap()
                .get()
                .get()
        });
        drop(guard);
        r
    }

    /**
    Be careful with this - it allows holding the lock while awaiting.
    */
    #[allow(clippy::await_holding_lock)]
    pub async fn assume_async<C, R>(&self, c: C) -> R
    where
        C: AsyncFnOnce(&T) -> R,
    {
        Self::verify_thread();
        let guard = self.shared.as_ref().unwrap().mutex.lock().unwrap();
        let r = c(unsafe {
            self.shared
                .as_ref()
                .unwrap()
                .inner
                .as_ref()
                .unwrap()
                .get()
                .get()
        })
        .await;
        drop(guard);
        r
    }

    /**
    Runs a closure with the inner value of the WgpuCell, ensuring that the closure is executed
    on the correct thread based on the WGPU_STRATEGY.

    # Panics
    For the duration of this function, the cell may not be otherwise used.
    */
    pub async fn with<C, R>(&self, c: C) -> R
    where
        C: FnOnce(&T) -> R + Send + 'static,
        R: Send + 'static,
        T: 'static,
    {
        let shared = self.shared.clone();
        smuggle("WgpuCell::with".to_string(), move || {
            Self::verify_thread();
            let guard = shared.as_ref().unwrap().mutex.lock().unwrap();
            let r = c(unsafe { shared.as_ref().unwrap().inner.as_ref().unwrap().get().get() });
            drop(guard);
            r
        })
        .await
    }

    /**
    Runs a closure with the inner value of the WgpuCell, ensuring that the closure is executed
    on the correct thread based on the WGPU_STRATEGY.

    # Panics
    For the duration of this function, the cell may not be otherwise used.
    */
    pub async fn with_async<C, R>(&self, c: C) -> R
    where
        C: AsyncFnOnce(&T) -> R + Send + 'static,
        R: Send + 'static,
        T: 'static,
    {
        let shared = self.shared.clone();
        smuggle_async("WgpuCell::with".to_string(), move || async move {
            Self::verify_thread();
            let guard = shared.as_ref().unwrap().mutex.lock().unwrap();
            let r =
                c(unsafe { shared.as_ref().unwrap().inner.as_ref().unwrap().get().get() }).await;
            drop(guard);
            r
        })
        .await
    }

    /**
    Creates a new WgpuCell by running a constructor closure on the correct thread
    based on the WGPU_STRATEGY.

    This function works like `with_mut` but for construction - it ensures the value
    is created on the appropriate thread for the current platform's wgpu strategy.
    */
    pub async fn new_on_thread<C, F>(c: C) -> WgpuCell<T>
    where
        C: FnOnce() -> F + Send + 'static,
        F: Future<Output = T> + 'static,
    {
        // logwise::info_sync!("WgpuCell::new_on_thread() started");
        // logwise::info_sync!("About to call smuggle_async...");
        let value = smuggle_async("WgpuCell::new_on_thread".to_string(), move || async move {
            // logwise::info_sync!("Inside smuggle_async closure");
            let f = c();
            // logwise::info_sync!("Calling provided closure f()...");
            let r = f.await;
            // logwise::info_sync!("Closure completed, creating WgpuCell...");
            WgpuCell::new(r)
        })
        .await;
        // logwise::info_sync!("smuggle_async completed, returning value");
        value
    }
}
unsafe impl<T> Send for WgpuCell<T> {}

impl<T> WgpuCell<T> {}

impl<T: Debug> Debug for WgpuCell<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuCell").finish()
    }
}

impl<T: Default> Default for WgpuCell<T> {
    fn default() -> Self {
        WgpuCell::new(Default::default())
    }
}

impl<T> From<T> for WgpuCell<T> {
    fn from(value: T) -> Self {
        WgpuCell::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests that run on platforms where we can access from any thread (Relaxed strategy)
    #[cfg(target_os = "windows")]
    mod relaxed_tests {
        use super::*;
        use std::rc::Rc;

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_wgpu_cell_basic_operations() {
            let value = 42;
            let mut cell = WgpuCell::new(value);
            assert_eq!(*cell.get(), 42);

            *cell.get_mut() = 100;
            assert_eq!(*cell.get(), 100);

            let value = cell.into_inner();
            assert_eq!(value, 100);
        }

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_wgpu_cell_copy() {
            let cell = WgpuCell::new(42);
            let cell2 = cell.copying();
            assert_eq!(*cell.get(), *cell2.get());
        }

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_wgpu_cell_with_non_send_type() {
            // Rc is not Send
            let rc = Rc::new(42);
            let cell = WgpuCell::new(rc);
            assert_eq!(**cell.get(), 42);
        }
    }
    // Test constructors that don't require thread access
    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    fn test_cell_construction() {
        // Just verify we can construct cells
        let _cell = WgpuCell::new(42);
        let _cell_from: WgpuCell<i32> = 42.into();
        let _cell_default: WgpuCell<i32> = Default::default();
    }

    // Test guard functionality on platforms where we can access from any thread (Relaxed strategy)
    #[cfg(target_os = "windows")]
    mod guard_tests {
        use super::*;

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_guard_basic_operations() {
            let cell = WgpuCell::new(42);

            // Test that we can lock and access the value
            {
                let guard = cell.lock();
                assert_eq!(*guard, 42);
            }

            // Test that we can lock again after the guard is dropped
            {
                let mut guard = cell.lock();
                *guard = 100;
                assert_eq!(*guard, 100);
            }

            // Verify the value was actually changed
            {
                let guard = cell.lock();
                assert_eq!(*guard, 100);
            }
        }

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_guard_debug_impl() {
            let cell = WgpuCell::new(42);
            let guard = cell.lock();
            let debug_str = format!("{:?}", guard);
            assert!(debug_str.contains("WgpuGuard"));
            assert!(debug_str.contains("42"));
        }

        #[test]
        #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
        fn test_guard_drop_behavior() {
            use std::sync::Arc;
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::thread;

            let cell = Arc::new(WgpuCell::new(42));
            let locked = Arc::new(AtomicBool::new(false));

            let cell_clone = cell.clone();
            let locked_clone = locked.clone();

            // Spawn a thread that tries to lock the cell
            let handle = thread::spawn(move || {
                let _guard = cell_clone.lock();
                locked_clone.store(true, Ordering::SeqCst);
                // Hold the lock for a bit
                thread::sleep(Duration::from_millis(10));
            });

            // Give the other thread time to acquire the lock
            thread::sleep(Duration::from_millis(5));

            // This should block until the other thread releases the lock
            let start = Instant::now();
            let _guard = cell.lock();
            let elapsed = start.elapsed();

            // We should have waited at least 5ms (remaining time from the 10ms sleep)
            assert!(elapsed.as_millis() >= 4); // Using 4ms to account for timing variations

            handle.join().unwrap();
        }
    }
}
