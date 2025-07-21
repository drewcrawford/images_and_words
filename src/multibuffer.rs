// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*!
Multibuffering implementation.

This implements a generic multibucffering algorithm. The main idea is,

1.  We have one or more CPU-visible objects
2.  We have one (or more) GPU-visible objects
3.  We write to the CPU side and it triggers a copy to the GPU side.

The objects here are fully generic, and may support buffers or textures.

*/

use crate::bindings::dirty_tracking::{DirtyReceiver, DirtySender};
use crate::bindings::resource_tracking;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::resource_tracking::{NotAvailable, ResourceTracker};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

//We need to wrap the ResourceTracker types in a newtype so that we can implement multibuffering behaviors.
//primarily, we want to mark things dirty.
pub struct CPUReadGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    //option so we can take on drop
    imp: Option<crate::bindings::resource_tracking::CPUReadGuard<'a, Element>>,
    _buffer: &'a Multibuffer<Element, U>,
}

impl<'a, Element, U> Deref for CPUReadGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    type Target = Element;
    fn deref(&self) -> &Self::Target {
        self.imp.as_ref().unwrap()
    }
}

impl<'a, Element, U> Drop for CPUReadGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    fn drop(&mut self) {
        // If we haven't taken the guard, panic
        if self.imp.is_some() && !std::thread::panicking() {
            panic!("Dropped CPUReadGuard without calling async_drop");
        }
    }
}

#[derive(Debug)]
pub struct CPUWriteGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    imp: crate::bindings::resource_tracking::CPUWriteGuard<'a, Element>,
    buffer: &'a Multibuffer<Element, U>,
}

impl<'a, Element, U> DerefMut for CPUWriteGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.imp
    }
}

impl<'a, Element, U> Deref for CPUWriteGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    type Target = Element;
    fn deref(&self) -> &Self::Target {
        &self.imp
    }
}

/**
Represents a bindable GPU resource.

Multibuffer type.
*/
#[derive(Debug)]
pub(crate) struct GPUGuard<T: Mappable, U: Clone> {
    dirty_guard: Option<resource_tracking::GPUGuard<T>>,
    debug_label: String,
    gpu_buffer: U,
}

//drop impl for GPUGuard
impl<T: Mappable, U: Clone> Drop for GPUGuard<T, U> {
    fn drop(&mut self) {
        logwise::info_sync!(
            "GPUGuard::drop() called for {f}",
            f = logwise::privacy::LogIt(self.debug_label.clone())
        );
        // Drop the dirty guard if present
        let _ = self.dirty_guard.take();
        logwise::info_sync!(
            "GPUGuard::drop() finished for {f}",
            f = logwise::privacy::LogIt(self.debug_label.clone())
        );
    }
}

impl<T: Mappable, U: Clone> GPUGuard<T, U> {
    pub fn as_imp(&self) -> &U {
        &self.gpu_buffer
    }

    /// Takes the dirty guard if present, indicating that a copy is needed
    pub fn take_dirty_guard(&mut self) -> Option<resource_tracking::GPUGuard<T>> {
        self.dirty_guard.take()
    }
}

/**

Implements multibuffering.

# type parameters
`T` - the CPU type
`U` - the GPU type.  wgpu and similar don't allow GPU-side buffers to be mapped.
*/
#[derive(Debug)]
pub struct Multibuffer<T, U>
where
    T: Mappable,
    U: Clone,
{
    //right now, not really a multibuffer!
    mappable: ResourceTracker<T>,
    gpu: U,
    debug_label: String,
}

impl<T, U> Multibuffer<T, U>
where
    T: Mappable,
    U: Clone,
{
    pub fn new(element: T, gpu: U, initial_write_to_gpu: bool, debug_label: String) -> Self {
        let tracker = ResourceTracker::new(element, initial_write_to_gpu, debug_label.clone());
        // Don't immediately lock for GPU - start in UNUSED state
        // The resource will transition to PENDING_WRITE_TO_GPU when first written

        Multibuffer {
            mappable: tracker,
            gpu,
            debug_label,
        }
    }

    // pub async fn access_read(&self) -> CPUReadGuard<T,U> where T: Mappable, U: GPUMultibuffer {
    //     loop {
    //         //insert first
    //         let (s,f) = r#continue::continuation();
    //         self.wake_list.lock().unwrap().push(s);
    //         //then check
    //         match self.mappable.cpu_read().await {
    //             Ok(guard) => return CPUReadGuard{ imp: Some(guard), buffer: self },
    //             Err(_) => f.await
    //         }
    //     }
    // }

    /**
    Accesses the underlying data.

    This function is unsafe because we perform no locking or checks.
    */
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) unsafe fn access_gpu_unsafe(&self) -> &U {
        &self.gpu
    }

    pub async fn access_write(&self) -> CPUWriteGuard<T, U>
    where
        T: Mappable,
    {
        let underlying = self.mappable.cpu_write().await;
        CPUWriteGuard {
            imp: underlying,
            buffer: self,
        }
    }

    /**
    Accesses the underlying GPU data.

    Returns a guard type providing access to the data.

    # Safety
    Caller must guarantee that the guard is live for the duration of the GPU access.
    */
    pub(crate) unsafe fn access_gpu(&self) -> GPUGuard<T, U>
    where
        T: Mappable,
        U: Clone,
    {
        // // Try to acquire GPU resource if it's in PENDING_WRITE_TO_GPU state
        match self.mappable.poll_gpu() {
            Ok(gpu_guard) => {
                // Resource was in PENDING_WRITE_TO_GPU state, need to copy
                logwise::info_sync!(
                    "Multibuffer: GPU resource {f} is dirty, copying to GPU",
                    f = self.debug_label.clone()
                );

                // TODO: This copy will be pushed down to the callers
                // Previously: copy_from_buffer(0, 0, gpu_guard.byte_len(), copy_info, gpu_guard)

                // Store the dirty guard - callers will handle the copy
                logwise::info_sync!(
                    "Access resource {label} for GPU access (copying)",
                    label = logwise::privacy::LogIt(self.debug_label.clone())
                );
                GPUGuard {
                    dirty_guard: Some(gpu_guard),
                    gpu_buffer: self.gpu.clone(),
                    debug_label: self.debug_label.clone(),
                }
            }
            Err(_) => {
                // Resource is not in PENDING_WRITE_TO_GPU state, no copy needed
                // logwise::info_sync!(
                //     "Multibuffer: GPU resource {f} not dirty, no copy needed",
                //     f = self.debug_label.clone()
                // );
                logwise::info_sync!(
                    "Access resource {label} for GPU access (no copy)",
                    label = logwise::privacy::LogIt(self.debug_label.clone())
                );
                GPUGuard {
                    dirty_guard: None,
                    gpu_buffer: self.gpu.clone(),
                    debug_label: self.debug_label.clone(),
                }
            }
        }
    }
    ///Returns a [DirtyReceiver] that activates when the GPU side is dirty.
    pub(crate) fn gpu_dirty_receiver(&self) -> DirtyReceiver {
        DirtyReceiver::new(&self.mappable.dirty_pending_cpu_to_gpu())
    }
}
