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
use crate::bindings::resource_tracking::ResourceTracker;
use crate::bindings::resource_tracking::sealed::Mappable;
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
    buffer: &'a Multibuffer<Element, U>,
}

impl<'a, Element, U> Drop for CPUReadGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    fn drop(&mut self) {
        _ = self.imp.take().expect("Dropped CPUReadGuard already");
        //wake up the waiting threads
        // Step 1: Acquire lock, drain wakers into a temporary Vec, then release lock.
        let wakers_to_send: Vec<r#continue::Sender<()>> = {
            let mut locked_wake_list = self.buffer.wake_list.lock().unwrap();
            locked_wake_list.drain(..).collect()
        }; // MutexGuard is dropped here, so the lock is released.

        // Step 2: Iterate and send notifications *after* the lock is released.
        for waker in wakers_to_send {
            waker.send(());
        }
    }
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

impl<'a, Element, U> CPUReadGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    /// Asynchronously drops the guard, properly unmapping the resource
    ///
    /// This method must be called before the guard is dropped. Failure to call
    /// this method will result in a panic when the guard's Drop implementation runs.
    pub async fn async_drop(mut self) {
        if let Some(inner_guard) = self.imp.take() {
            inner_guard.async_drop().await;
        }

        // Handle the wake list notifications
        let wakers_to_send: Vec<r#continue::Sender<()>> = {
            let mut locked_wake_list = self.buffer.wake_list.lock().unwrap();
            locked_wake_list.drain(..).collect()
        };

        for waker in wakers_to_send {
            waker.send(());
        }
    }
}

#[derive(Debug)]
pub struct CPUWriteGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    imp: Option<crate::bindings::resource_tracking::CPUWriteGuard<'a, Element>>, //option for drop!
    buffer: &'a Multibuffer<Element, U>,
}

impl<'a, Element, U> DerefMut for CPUWriteGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.imp.as_mut().expect("No imp??")
    }
}

impl<'a, Element, U> Deref for CPUWriteGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    type Target = Element;
    fn deref(&self) -> &Self::Target {
        self.imp.as_ref().expect("No imp??")
    }
}

impl<'a, Element, U> CPUWriteGuard<'a, Element, U>
where
    Element: Mappable,
    U: Clone,
{
    /// Asynchronously drops the guard, properly unmapping the resource
    ///
    /// This method must be called before the guard is dropped. Failure to call
    /// this method will result in a panic when the guard's Drop implementation runs.
    pub async fn async_drop(mut self) {
        if let Some(inner_guard) = self.imp.take() {
            inner_guard.async_drop().await;
        }

        // Mark that GPU side needs updating
        self.buffer.gpu_side_is_dirty.mark_dirty(true);

        // Handle the wake list notifications
        let wakers_to_send: Vec<r#continue::Sender<()>> = {
            let mut locked_wake_list = self.buffer.wake_list.lock().unwrap();
            locked_wake_list.drain(..).collect()
        };

        for waker in wakers_to_send {
            waker.send(());
        }
    }
}

/**
Represents a bindable GPU resource.

Multibuffer type.
*/
#[derive(Debug)]
pub(crate) struct GPUGuard<T: Mappable, U: Clone> {
    wake_list: Arc<Mutex<Vec<r#continue::Sender<()>>>>,
    dirty_guard: Option<resource_tracking::GPUGuard<T>>,
    gpu_buffer: U,
}

//drop impl for GPUGuard
impl<T: Mappable, U: Clone> Drop for GPUGuard<T, U> {
    fn drop(&mut self) {
        // Drop the dirty guard if present
        let _ = self.dirty_guard.take();
        //wake up the waiting threads
        // Step 1: Acquire lock, drain wakers into a temporary Vec, then release lock.
        let wakers_to_send: Vec<r#continue::Sender<()>> = {
            let mut locked_wake_list = self.wake_list.lock().unwrap();
            locked_wake_list.drain(..).collect()
        }; // MutexGuard is dropped here, so the lock is released.

        // Step 2: Iterate and send notifications *after* the lock is released.
        for waker in wakers_to_send {
            waker.send(());
        }
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
    wake_list: Arc<Mutex<Vec<r#continue::Sender<()>>>>,
    gpu: U,
    gpu_side_is_dirty: DirtySender,
}

impl<T, U> Multibuffer<T, U>
where
    T: Mappable,
    U: Clone,
{
    pub fn new(element: T, gpu: U, initial_write_to_gpu: bool) -> Self {
        let tracker = ResourceTracker::new(element, initial_write_to_gpu);
        // Don't immediately lock for GPU - start in UNUSED state
        // The resource will transition to PENDING_WRITE_TO_GPU when first written

        Multibuffer {
            mappable: tracker,
            gpu,
            wake_list: Arc::new(Mutex::new(Vec::new())),
            gpu_side_is_dirty: DirtySender::new(false),
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
        loop {
            // FIRST, insert into the list.  Think very carefully before changing this order.
            let (s, f) = r#continue::continuation();
            self.wake_list.lock().unwrap().push(s);
            // THEN, try to acquire the write lock.
            let r = self.mappable.cpu_write().await;
            match r {
                Ok(guard) => {
                    //Someone else will send a nonsense value to the sender later, that's fine.
                    return CPUWriteGuard {
                        imp: Some(guard),
                        buffer: self,
                    };
                }
                Err(_) => f.await, //if we fail, wait for the next time
            }
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
        // Try to acquire GPU resource if it's in PENDING_WRITE_TO_GPU state
        match self.mappable.gpu() {
            Ok(gpu_guard) => {
                // Resource was in PENDING_WRITE_TO_GPU state, need to copy
                self.gpu_side_is_dirty.mark_dirty(false); //clear dirty bit

                // TODO: This copy will be pushed down to the callers
                // Previously: copy_from_buffer(0, 0, gpu_guard.byte_len(), copy_info, gpu_guard)

                // Store the dirty guard - callers will handle the copy
                GPUGuard {
                    wake_list: self.wake_list.clone(),
                    dirty_guard: Some(gpu_guard),
                    gpu_buffer: self.gpu.clone(),
                }
            }
            Err(_) => {
                // Resource is not in PENDING_WRITE_TO_GPU state, no copy needed
                GPUGuard {
                    wake_list: self.wake_list.clone(),
                    dirty_guard: None,
                    gpu_buffer: self.gpu.clone(),
                }
            }
        }
    }
    ///Returns a [DirtyReceiver] that activates when the GPU side is dirty.
    pub(crate) fn gpu_dirty_receiver(&self) -> DirtyReceiver {
        DirtyReceiver::new(&self.gpu_side_is_dirty)
    }
}
