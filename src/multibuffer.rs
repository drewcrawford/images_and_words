use std::marker::PhantomData;
use crate::bindings::resource_tracking::{CPUReadGuard, CPUWriteGuard, GPUGuard, ResourceTracker};

pub struct Multibuffer<T> {
    //right now, not really a multibuffer!
    t: ResourceTracker<T>,
}

impl<T> Multibuffer<T> {
    pub fn new(element: T) -> Self {
        let tracker = ResourceTracker::new(element);
        Multibuffer {
            t: tracker,
        }
    }

    pub async fn access_read(&self) -> CPUReadGuard<T> {
        self.t.cpu_read().expect("multibuffer access_read")
    }

    pub async fn access_write(&self) -> CPUWriteGuard<T> {
        self.t.cpu_write().expect("multibuffer access_write")
    }

    pub async fn access_gpu(&self) -> GPUGuard<T> {
        self.t.gpu().expect("multibuffer access_gpu")
    }
}