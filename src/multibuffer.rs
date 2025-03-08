/*!
Multibuffering implementation.

This implements a generic multibucffering algorithm. The main idea is,

1.  We have one or more CPU-visible objects
2.  We have one (or more) GPU-visible objects
3.  We write to the CPU side and it triggers a copy to the GPU side.

The objects here are fully generic, and may support buffers or textures.

*/

use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use crate::bindings::dirty_tracking::DirtyReceiver;
use crate::bindings::forward::dynamic::buffer::{IndividualBuffer};
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

pub struct CPUWriteGuard<'a, Element, U> where Element: Mappable, U: GPUMultibuffer {
    imp: Option<crate::bindings::resource_tracking::CPUWriteGuard<'a, Element>>, //option for drop!
    buffer: &'a Multibuffer<Element, U>
}

impl<'a, Element, U> Drop for CPUWriteGuard<'a, Element, U> where Element: Mappable, U: GPUMultibuffer {
    fn drop(&mut self) {
        let take = self.imp.take().expect("Dropped CPUWriteGuard already");
        let gpu = self.buffer.mappable.convert_to_gpu(take);
        *self.buffer.shared.needs_gpu_copy.lock().unwrap() = Some(gpu);
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
    imp: U::OutGuard<resource_tracking::GPUGuard<T>>,
}

impl<T: Mappable, U: GPUMultibuffer> GPUGuard<T,U> {
    pub fn as_imp(&self) -> &U {
        self.imp.as_ref()
    }
}

///shared between CPU/GPU
#[derive(Debug)]
struct Shared<CPUSide> where
//since we need to hold the guard type, we need the constraints it will require..
CPUSide: Mappable
{
    needs_gpu_copy: Mutex<Option<resource_tracking::GPUGuard<CPUSide>>>,
}

//safe to send/sync, just not safe to use!
unsafe impl<T> Send for Shared<T> where T: Mappable  {}
unsafe impl<T> Sync for Shared<T> where T: Mappable {}

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
    wake_list: Mutex<Vec<r#continue::Sender<()>>>,
    gpu: U,
    shared: Arc<Shared<T>>,
}

impl<T,U> Multibuffer<T,U> where T: Mappable, U: GPUMultibuffer {
    pub fn new(element: T, gpu: U) -> Self {
        let tracker = ResourceTracker::new(element, || {
            todo!()
        });
        let dirty_copy_to_gpu = tracker.gpu().expect("multibuffer new");

        Multibuffer {
            mappable: tracker,
            gpu,
            wake_list: Mutex::new(Vec::new()),
            //initially, GPU buffer type is probably dirty
            shared: Arc::new(Shared {
                needs_gpu_copy: Mutex::new(Some(dirty_copy_to_gpu))
            })
        }
    }

    pub async fn access_read(&self) -> CPUReadGuard<T> where T: Mappable, U: GPUMultibuffer {
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

    pub async fn access_write(&self) -> CPUWriteGuard<T, U> where T: Mappable, U: GPUMultibuffer {
        loop {
            //insert first
            let (s, f) = r#continue::continuation();
            self.wake_list.lock().unwrap().push(s);
            //then check
            match self.mappable.cpu_write().await {
                Ok(guard) => return CPUWriteGuard{ imp: Some(guard), buffer: &self },
                Err(_) => f.await
            }
        }
    }

    /**
    Accesses the underlying GPU data.

    Returns a guard type providing access to the data.

    # Safety
    Caller must guarantee that the guard is live for the duration of the GPU copy.
    */
    pub (crate) unsafe fn access_gpu(&self, copy_info: &mut CopyInfo) -> GPUGuard<T,U> where T: Mappable, U: GPUMultibuffer, T: AsRef<U::CorrespondingMappedType> {
        let take_dirty = self.shared.needs_gpu_copy.lock().unwrap().take();
        if let Some(imp_guard) = take_dirty {
            let copy_guard = self.gpu.copy_from_buffer(0, 0, imp_guard.byte_len(), copy_info, imp_guard);
            GPUGuard {
                imp: copy_guard,
                // shared: self.shared.clone(),
            }
        }
        else {
            todo!()
        }

    }
    ///Returns a [DirtyReceiver] that activates when the GPU side is dirty.
    pub(crate) fn gpu_dirty_receiver(&self) -> DirtyReceiver {
        todo!()
    }
}