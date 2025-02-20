use std::marker::PhantomData;
use crate::bindings::resource_tracking::{CPUReadGuard, CPUWriteGuard, GPUGuard};

pub struct Multibuffer<T> {
    t: PhantomData<T>
}

impl<T> Multibuffer<T> {
    pub fn new() -> Self {
        Multibuffer {
            t: PhantomData
        }
    }

    pub async fn access_read(&self) -> CPUReadGuard<T> {
        todo!()
    }

    pub async fn access_write(&self) -> CPUWriteGuard<T> {
        todo!()
    }

    pub async fn access_gpu(&self) -> GPUGuard<T> {
        todo!()
    }
}