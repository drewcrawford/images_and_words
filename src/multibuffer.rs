use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
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
    gpu_dirty_needs_copy: bool,
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
            gpu_dirty_needs_copy: true,
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
                Ok(guard) => return CPUWriteGuard{ imp: guard },
                Err(_) => f.await
            }
        }
    }

    pub (crate) fn access_gpu(&self) -> GPUGuard<T> where T: Mappable {
        GPUGuard {
            imp: self.mappable.gpu().expect("multibuffer access_gpu")
        }
    }
}