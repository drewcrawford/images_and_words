use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use crate::bindings::resource_tracking::{ResourceTracker};
use crate::bindings::resource_tracking::sealed::Mappable;

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
    dirty_needs_copy: &'a UnsafeCell<bool>,
}

impl<'a, Element> Drop for CPUWriteGuard<'a, Element> where Element: Mappable {
    fn drop(&mut self) {
        //mark the GPU buffer as dirty
        //we are still holding the lock here so it's ok to write
        unsafe {
            *self.dirty_needs_copy.get() = true;
        }
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
pub struct GPUGuard<Element> where Element: Mappable {
    pub(crate) imp: crate::bindings::resource_tracking::GPUGuard<Element>,
    shared: Arc<Shared>,
}

impl<'a, Element> Drop for GPUGuard<Element> where Element: Mappable {
    fn drop(&mut self) {
        //with lock held, we can clear the dirty flag
        unsafe {
            *self.shared.gpu_dirty_needs_copy.get() = false;
        }
    }
}

///shared between CPU/GPU
#[derive(Debug)]
struct Shared {
    gpu_dirty_needs_copy: UnsafeCell<bool>,
}





/**

Implements multibuffering.

# type parameters
`T` - the CPU type
`U` - the GPU type.  wgpu and similar don't allow GPU-side buffers to be mapped.
*/
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
                gpu_dirty_needs_copy: UnsafeCell::new(true),
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
    Returns a guard object that can be used to schedule a GPU copy.

    Returns None if the GPU buffer is not dirty.
    */
    pub (crate) fn copy_gpu_guard_if_needed(&self) -> Option<GPUGuard<T>> where T: Mappable {
        let imp_guard = self.mappable.gpu().expect("multibuffer access_gpu");
        //now can read if dirty
        let dirty = unsafe{ *self.shared.gpu_dirty_needs_copy.get() };
        if dirty {
            Some(
                GPUGuard {
                    imp: imp_guard,
                    shared: self.shared.clone(),
                }
            )
        }
        else {
            None
        }

    }
}