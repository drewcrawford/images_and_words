/*!
Multibuffering implementation.

This implements a generic multibucffering algorithm. The main idea is,

1.  We have one or more CPU-visible objects
2.  We have one (or more) GPU-visible objects
3.  We write to the CPU side and it triggers a copy to the GPU side.

The objects here are fully generic, and may support buffers or textures.

*/

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use crate::bindings::dirty_tracking::{DirtyReceiver, DirtySender};
use crate::bindings::resource_tracking;
use crate::bindings::resource_tracking::{ResourceTracker};
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::imp::CopyInfo;
use crate::multibuffer::sealed::GPUMultibuffer;

//We need to wrap the ResourceTracker types in a newtype so that we can implement multibuffering behaviors.
//primarily, we want to mark things dirty.
pub struct CPUReadGuard<'a, Element, U> where Element: Mappable, U: GPUMultibuffer {
    //option so we can take on drop
    imp: Option<crate::bindings::resource_tracking::CPUReadGuard<'a, Element>>,
    buffer: &'a Multibuffer<Element, U>
}

impl<'a, Element, U> Drop for CPUReadGuard<'a, Element, U> where Element: Mappable, U: GPUMultibuffer {
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

impl<'a, Element, U> Deref for CPUReadGuard<'a, Element, U> where Element: Mappable, U: GPUMultibuffer {
    type Target = Element;
    fn deref(&self) -> &Self::Target {
        self.imp.as_ref().unwrap()
    }
}

#[derive(Debug)]
pub struct CPUWriteGuard<'a, Element, U> where Element: Mappable, U: GPUMultibuffer {
    imp: Option<crate::bindings::resource_tracking::CPUWriteGuard<'a, Element>>, //option for drop!
    buffer: &'a Multibuffer<Element, U>
}


impl<'a, Element, U> Drop for CPUWriteGuard<'a, Element, U> where Element: Mappable, U: GPUMultibuffer {
    fn drop(&mut self) {
        let take = self.imp.take().expect("Dropped CPUWriteGuard already");
        let gpu = self.buffer.mappable.convert_to_gpu(take);
        *self.buffer.needs_gpu_copy.lock().unwrap() = Some(gpu);
        self.buffer.gpu_side_is_dirty.mark_dirty(true);
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

impl<'a, Element, U> DerefMut for CPUWriteGuard<'a, Element, U> where Element: Mappable, U: GPUMultibuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.imp.as_mut().expect("No imp??")
    }
}

impl<'a, Element, U> Deref for CPUWriteGuard<'a, Element, U> where Element: Mappable, U: GPUMultibuffer {
    type Target = Element;
    fn deref(&self) -> &Self::Target {
        self.imp.as_ref().expect("No imp??")
    }
}


/**
Represents a bindable GPU resource.

Multibuffer type.
*/
pub(crate) struct GPUGuard<T: Mappable, U: GPUMultibuffer> {
    imp: Option<Result<U,U::OutGuard<resource_tracking::GPUGuard<T>>>>,
    wake_list: Arc<Mutex<Vec<r#continue::Sender<()>>>>,
}

//drop impl for GPUGuard
impl<T: Mappable, U: GPUMultibuffer> Drop for GPUGuard<T,U> {
    fn drop(&mut self) {
        let _ = self.imp.take().unwrap();
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

impl<T: Mappable, U: GPUMultibuffer> GPUGuard<T,U> {
    pub fn as_imp(&self) -> &U {
        match self.imp {
            Some(Ok(ref imp)) => imp,
            Some(Err(ref imp)) => imp.as_ref(),
            None => unreachable!()
        }
    }
}





pub(crate) mod sealed {
    
    
    use crate::bindings::resource_tracking::GPUGuard;
    use crate::bindings::resource_tracking::sealed::Mappable;
    use crate::imp::CopyInfo;



    pub trait GPUMultibuffer: Clone {
        /*
        So I guess the issue is
        1.  Implementation needs e.g. imp::MappedBuffer, possibly indirectly through IndividualBuffer.
        2.  IndividualBuffer has generics, and hard to name the type of the generics.  Really it's "for all"
        3.  Hard to express this idea in the rust typesystem.

        Meanwhile,
        1.  imp::IndividualBuffer has no generics and is easy to name, however,
        2.  The guard type protects IndividualBuffer, not the impl type.
        3.  Can't pass them separately due to borrowing rules.

        So my idea is, maybe we can avoid naming the IndividualBuffer type exactly?
         */
        type CorrespondingMappedType;
        type OutGuard<InGuard>: AsRef<Self>;

        /**
        Safety: Caller must guarantee that the guard is live for the duration of the GPU read.
*/

        unsafe fn copy_from_buffer<'a,Guarded>(&self, source_offset: usize, dest_offset: usize, copy_len: usize, info: &mut CopyInfo<'a>, guard: GPUGuard<Guarded>) -> Self::OutGuard<GPUGuard<Guarded>> where Guarded: AsRef<Self::CorrespondingMappedType>, Guarded: Mappable;

    }
    /**
    Indicates that the type can be a source of a multibuffer copy operation
*/
    pub trait CPUMultibuffer {
        type Source;
        #[allow(dead_code)] //nop implementation does not use
        fn as_source(&self) -> &Self::Source;
    }
}


/**

Implements multibuffering.

# type parameters
`T` - the CPU type
`U` - the GPU type.  wgpu and similar don't allow GPU-side buffers to be mapped.
*/
#[derive(Debug)]
pub struct Multibuffer<T,U> where T: Mappable, U: GPUMultibuffer {
    //right now, not really a multibuffer!
    mappable: ResourceTracker<T>,
    wake_list: Arc<Mutex<Vec<r#continue::Sender<()>>>>,
    gpu: U,
    needs_gpu_copy: Mutex<Option<resource_tracking::GPUGuard<T>>>,
    gpu_side_is_dirty: DirtySender,
}

impl<T,U> Multibuffer<T,U> where T: Mappable, U: GPUMultibuffer {
    pub fn new(element: T, gpu: U) -> Self {
        let tracker = ResourceTracker::new(element);
        let dirty_copy_to_gpu = tracker.gpu().expect("multibuffer new");

        Multibuffer {
            mappable: tracker,
            gpu,
            wake_list: Arc::new(Mutex::new(Vec::new())),
            //initially, GPU buffer type is probably dirty
            needs_gpu_copy: Mutex::new(Some(dirty_copy_to_gpu)),
            gpu_side_is_dirty: DirtySender::new(true) //agrees with needs_gpu_copy property
        }
    }

    pub async fn access_read(&self) -> CPUReadGuard<T,U> where T: Mappable, U: GPUMultibuffer {
        loop {
            //insert first
            let (s,f) = r#continue::continuation();
            self.wake_list.lock().unwrap().push(s);
            //then check
            match self.mappable.cpu_read().await {
                Ok(guard) => return CPUReadGuard{ imp: Some(guard), buffer: self },
                Err(_) => f.await
            }
        }
    }
    
    /**
    Accesses the underlying data.
    
    This function is unsafe because we perform no locking or checks.
    */
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) unsafe fn access_gpu_unsafe(&self) -> &U {
        &self.gpu
    }

    pub async fn access_write(&self) -> CPUWriteGuard<T, U> where T: Mappable, U: GPUMultibuffer {
        loop {
            // Try to acquire first WITHOUT registering waker
            match self.mappable.cpu_write().await {
                Ok(guard) => {
                    return CPUWriteGuard{ imp: Some(guard), buffer: &self };
                },
                Err(_) => {
                    // ONLY register waker AFTER failed attempt to prevent race condition
                    let (s, f) = r#continue::continuation();
                    
                    self.wake_list.lock().unwrap().push(s);
                    
                    // Double-check resource availability after registering waker
                    // This prevents race where resource becomes available between failed attempt and waker registration
                    match self.mappable.cpu_write().await {
                        Ok(guard) => {
                            return CPUWriteGuard{ imp: Some(guard), buffer: &self };
                        },
                        Err(_) => {
                            f.await;
                        }
                    }
                }
            }
        }
    }

    /**
    Accesses the underlying GPU data.

    Returns a guard type providing access to the data.

    # Safety
    Caller must guarantee that the guard is live for the duration of the GPU access.
    */
    pub (crate) unsafe fn access_gpu(&self, copy_info: &mut CopyInfo) -> GPUGuard<T,U> where T: Mappable, U: GPUMultibuffer, T: AsRef<U::CorrespondingMappedType> {
        let take_dirty = self.needs_gpu_copy.lock().unwrap().take();
        self.gpu_side_is_dirty.mark_dirty(false); //clear dirty bit
        if let Some(imp_guard) = take_dirty {
            let copy_guard = unsafe { self.gpu.copy_from_buffer(0, 0, imp_guard.byte_len(), copy_info, imp_guard) };
            GPUGuard {
                imp: Some(Err(copy_guard)),
                wake_list: self.wake_list.clone(),
                // shared: self.shared.clone(),
            }
        }
        else {
            //GPU isn't necessary so no copy is needed
            GPUGuard {
                imp: Some(Ok(self.gpu.clone())),
                wake_list: self.wake_list.clone(),
            }
        }

    }
    ///Returns a [DirtyReceiver] that activates when the GPU side is dirty.
    pub(crate) fn gpu_dirty_receiver(&self) -> DirtyReceiver {
        DirtyReceiver::new(&self.gpu_side_is_dirty)
    }
}