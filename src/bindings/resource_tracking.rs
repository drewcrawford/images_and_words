use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut, Index};
use std::sync::atomic::AtomicU8;

const UNUSED: u8 = 0;
const CPU_READ: u8 = 1;
const CPU_WRITE: u8 = 2;
const GPU: u8 = 3;
#[derive(Debug)]
pub struct CPUReadGuard<'a, Resource> where Resource: sealed::Mappable {
    tracker: &'a ResourceTracker<Resource>,
}

impl<Resource> Deref for CPUReadGuard<'_, Resource> where Resource: sealed::Mappable {
    type Target = Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.tracker.resource.get() }
    }
}
impl <Resource> Drop for CPUReadGuard<'_, Resource> where Resource: sealed::Mappable {
    fn drop(&mut self) {
        self.tracker.unuse();
    }
}

#[derive(Debug)]
pub struct CPUWriteGuard<'a, Resource> where Resource: sealed::Mappable  {
    tracker: &'a ResourceTracker<Resource>,
}

impl<Resource> Deref for CPUWriteGuard<'_, Resource> where Resource: sealed::Mappable {
    type Target = Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.tracker.resource.get() }
    }
}

impl<Resource> DerefMut for CPUWriteGuard<'_, Resource> where Resource: sealed::Mappable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.tracker.resource.get() }
    }
}

impl <Resource> Drop for CPUWriteGuard<'_, Resource> where Resource: sealed::Mappable {
    fn drop(&mut self) {
        self.tracker.unuse();
    }
}

pub struct GPUGuard<'a, Resource> {
    tracker: &'a ResourceTracker<Resource>,

}

#[derive(Debug)]
pub struct NotAvailable {
    read_state: u8,
}

pub(crate) mod sealed {
    pub trait Mappable {
        async fn map_read(&mut self);
        async fn map_write(&mut self);

        fn unmap(&mut self);

    }
}


/**
Tracks whether the resource is in use by the CPU or GPU, etc.
*/
#[derive(Debug)]
pub(crate) struct ResourceTracker<Resource> {
    state: AtomicU8,
    resource: UnsafeCell<Resource>,
}

//todo: do we need these constraints?
unsafe impl<Resource: Send> Send for ResourceTracker<Resource> {}
unsafe impl<Resource: Sync> Sync for ResourceTracker<Resource> {}


impl<Resource> ResourceTracker<Resource> {
    pub fn new(resource: Resource) -> Self {
        Self {
            state: AtomicU8::new(UNUSED),
            resource: UnsafeCell::new(resource),
        }
    }
    /// Returns the resource if it is not in use by the CPU or GPU.
    pub async fn cpu_read(&self) -> Result<CPUReadGuard<Resource>,NotAvailable> where Resource: sealed::Mappable {
        match self.state.compare_exchange(UNUSED, CPU_READ, std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => {},
            Err(other) => return Err(NotAvailable { read_state: other }),
        }
        unsafe {
            self.resource.get().as_mut().unwrap().map_read().await;
            Ok(CPUReadGuard { tracker: self })
        }
    }
    /// Returns the resource if it is not in use by the CPU or GPU.
    pub async fn cpu_write(&self) -> Result<CPUWriteGuard<Resource>,NotAvailable> where Resource: sealed::Mappable {
        match self.state.compare_exchange(UNUSED, CPU_WRITE, std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => {},
            Err(other) => return Err(NotAvailable { read_state: other }),
        }
        unsafe {
            self.resource.get().as_mut().unwrap().map_write().await;
            Ok(CPUWriteGuard { tracker: self })
        }
    }
    /// Returns the resource if it is not in use by the CPU or GPU.
    pub fn gpu(&self) -> Result<GPUGuard<Resource>,NotAvailable> {
        match self.state.compare_exchange(UNUSED, GPU, std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => Ok(GPUGuard { tracker: self }),
            Err(other) => Err(NotAvailable { read_state: other }),
        }
    }

    fn unuse(&self) where Resource: sealed::Mappable {
        unsafe{&mut *self.resource.get()}.unmap();
        let o = self.state.swap(UNUSED, std::sync::atomic::Ordering::Release);
        assert_ne!(o, UNUSED, "Resource was not in use");
    }
}