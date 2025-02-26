use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use crate::bindings::forward::dynamic::buffer::IndividualBuffer;
use crate::bindings::resource_tracking;
use crate::bindings::resource_tracking::{ResourceTracker};
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::imp;
use crate::imp::CopyInfo;
use crate::multibuffer::sealed::{CPUMultibuffer, GPUMultibuffer};

//We need to wrap the ResourceTracker types in a newtype so that we can implement multibuffering behaviors.
//primarily, we want to mark things dirty.
pub struct CPUReadGuard<'a, Element> where Element: Mappable {
    imp: crate::bindings::resource_tracking::CPUReadGuard<'a, Element>,
}

impl<'a, Element> Deref for CPUReadGuard<'a, Element> where Element: Mappable {
    type Target = Element;
    fn deref(&self) -> &Self::Target {
        &self.imp
    }
}

pub struct CPUWriteGuard<'a, Element> where Element: Mappable {
    imp: crate::bindings::resource_tracking::CPUWriteGuard<'a, Element>,
    dirty_needs_copy: &'a AtomicBool,
}

impl<'a, Element> Drop for CPUWriteGuard<'a, Element> where Element: Mappable {
    fn drop(&mut self) {
        //set dirty
        self.dirty_needs_copy.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl<'a, Element> DerefMut for CPUWriteGuard<'a, Element> where Element: Mappable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.imp
    }
}

impl<'a, Element> Deref for CPUWriteGuard<'a, Element> where Element: Mappable {
    type Target = Element;
    fn deref(&self) -> &Self::Target {
        &self.imp
    }
}

#[derive(Debug)]
pub struct GPUGuard<GPUSide,CPUSide> where GPUSide: GPUMultibuffer, CPUSide: Mappable {
    pub(crate) imp: GPUSide::OutGuard<resource_tracking::GPUGuard<CPUSide>>,
    shared: Arc<Shared>,
}

impl<GPUSide, CPUSide> Drop for GPUGuard<GPUSide, CPUSide> where GPUSide: GPUMultibuffer, CPUSide: Mappable {
    fn drop(&mut self) {
        //mark the GPU buffer as clean
        let old = self.shared.gpu_dirty_needs_copy.swap(false, std::sync::atomic::Ordering::Relaxed);
        assert!(old, "GPU buffer was not dirty??");
    }
}

///shared between CPU/GPU
#[derive(Debug)]
struct Shared {
    //we want to be able to access this without relying on the underlying lock
    //in particular, we only want to reserve the lock for the duration of the copy operation
    //and we need to know if we even need one or not.
    //Generally it is ok to use relaxed here, since we are not synchronizing any other memory (ResourceTracker handles that).
    gpu_dirty_needs_copy: AtomicBool,
}

//safe to send/sync, just not safe to use!
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

pub(crate) mod sealed {
    use std::ops::Deref;
    use crate::bindings::forward::dynamic::buffer::IndividualBuffer;
    use crate::bindings::resource_tracking::GPUGuard;
    use crate::bindings::resource_tracking::sealed::Mappable;
    use crate::imp::{CopyInfo, CopyGuard};



    pub trait GPUMultibuffer {
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
        type ItsMappedBuffer;
        type OutGuard<InGuard>;

        /**
        Safety: Caller must guarantee that the guard is live for the duration of the GPU read.
*/

        unsafe fn copy_from_buffer<'a,Guarded>(&self, source_offset: usize, dest_offset: usize, copy_len: usize, info: &mut CopyInfo<'a>, guard: GPUGuard<Guarded>) -> Self::OutGuard<GPUGuard<Guarded>> where Guarded: AsRef<Self::ItsMappedBuffer>, Guarded: Mappable;

    }
    /**
    Indicates that the type can be a source of a multibuffer copy operation
*/
    pub trait CPUMultibuffer {
        type Source;
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
pub struct Multibuffer<T,U> {
    //right now, not really a multibuffer!
    mappable: ResourceTracker<T>,
    wake_list: Mutex<Vec<r#continue::Sender<()>>>,
    gpu: U,
    shared: Arc<Shared>,
}

impl<T,U> Multibuffer<T,U> {
    pub fn new(element: T, gpu: U) -> Self {
        let tracker = ResourceTracker::new(element, || {
            todo!()
        });
        Multibuffer {
            mappable: tracker,
            gpu,
            wake_list: Mutex::new(Vec::new()),
            //initially, GPU buffer type is probably dirty
            shared: Arc::new(Shared {
                gpu_dirty_needs_copy: AtomicBool::new(true),
            })
        }
    }

    pub async fn access_read(&self) -> CPUReadGuard<T> where T: Mappable {
        loop {
            //insert first
            let (s,f) = r#continue::continuation();
            self.wake_list.lock().unwrap().push(s);
            //then check
            match self.mappable.cpu_read().await {
                Ok(guard) => return CPUReadGuard{ imp: guard },
                Err(_) => f.await
            }
        }
    }

    pub async fn access_write(&self) -> CPUWriteGuard<T> where T: Mappable {
        loop {
            //insert first
            let (s, f) = r#continue::continuation();
            self.wake_list.lock().unwrap().push(s);
            //then check
            match self.mappable.cpu_write().await {
                Ok(guard) => return CPUWriteGuard{ imp: guard, dirty_needs_copy: &self.shared.gpu_dirty_needs_copy },
                Err(_) => f.await
            }
        }
    }

    /**
    Accesses the underlying GPU data.

    Returns a guard type providing access to the data.

    # Safety
    Caller must guarantee that the guard is live for the duration of the GPU read.
    */
    pub (crate) unsafe fn access_gpu(&self, copy_info: &mut CopyInfo) -> GPUGuard<U,T> where T: Mappable, U: GPUMultibuffer, T: AsRef<U::ItsMappedBuffer> {
        let dirty = self.shared.gpu_dirty_needs_copy.load(Ordering::Relaxed);

        //now can read dirty flag
        if dirty {
            let imp_guard = self.mappable.gpu().expect("multibuffer access_gpu");
            let copy_guard = self.gpu.copy_from_buffer(0, 0, imp_guard.byte_len(), copy_info, imp_guard);
            GPUGuard {
                imp: copy_guard,
                shared: self.shared.clone(),
            }
        }
        else {
            todo!()
        }

    }
}