use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut, Index};
use std::sync::atomic::AtomicU8;

const UNUSED: u8 = 0;
const CPU_READ: u8 = 1;
const CPU_WRITE: u8 = 2;
const GPU: u8 = 3;
#[derive(Debug)]
pub struct CPUReadGuard<'a, Resource> {
    tracker: &'a ResourceTracker<Resource>,
}

impl<Resource> Deref for CPUReadGuard<'_, Resource> {
    type Target = Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.tracker.resource.get() }
    }
}
impl <Resource> Drop for CPUReadGuard<'_, Resource> {
    fn drop(&mut self) {
        todo!()
    }
}

#[derive(Debug)]
pub struct CPUWriteGuard<'a, Resource> {
    tracker: &'a ResourceTracker<Resource>,
}

impl<Resource> Deref for CPUWriteGuard<'_, Resource> {
    type Target = Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.tracker.resource.get() }
    }
}

impl<Resource> DerefMut for CPUWriteGuard<'_, Resource> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.tracker.resource.get() }
    }
}

impl <Resource> Drop for CPUWriteGuard<'_, Resource> {
    fn drop(&mut self) {
        todo!()
    }
}

pub struct GPUGuard<'a, Resource> {
    tracker: &'a ResourceTracker<Resource>,

}

#[derive(Debug)]
pub struct NotAvailable {
    read_state: u8,
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
    pub fn cpu_read(&self) -> Result<CPUReadGuard<Resource>,NotAvailable> {
        match self.state.compare_exchange(UNUSED, CPU_READ, std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => Ok(CPUReadGuard { tracker: self }),
            Err(other) => Err(NotAvailable { read_state: other }),
        }
    }
    /// Returns the resource if it is not in use by the CPU or GPU.
    pub fn cpu_write(&self) -> Result<CPUWriteGuard<Resource>,NotAvailable> {
        match self.state.compare_exchange(UNUSED, CPU_WRITE, std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => Ok(CPUWriteGuard { tracker: self }),
            Err(other) => Err(NotAvailable { read_state: other }),
        }
    }
    /// Returns the resource if it is not in use by the CPU or GPU.
    pub fn gpu(&self) -> Result<GPUGuard<Resource>,NotAvailable> {
        match self.state.compare_exchange(UNUSED, GPU, std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => Ok(GPUGuard { tracker: self }),
            Err(other) => Err(NotAvailable { read_state: other }),
        }
    }
}