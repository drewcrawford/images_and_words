use std::cell::UnsafeCell;
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::AtomicU8;

const UNUSED: u8 = 0;
const CPU_READ: u8 = 1;
const CPU_WRITE: u8 = 2;
const GPU: u8 = 3;
const PENDING_WRITE_TO_GPU: u8 = 4;
#[derive(Debug)]
pub struct CPUReadGuard<'a, Resource> where Resource: sealed::Mappable {
    tracker: &'a ResourceTrackerInternal<Resource>,
}

impl<Resource> Deref for CPUReadGuard<'_, Resource> where Resource: sealed::Mappable {
    type Target = Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.tracker.resource.get() }
    }
}
impl <Resource> Drop for CPUReadGuard<'_, Resource> where Resource: sealed::Mappable {
    fn drop(&mut self) {
        //safety: it's the guard's responsibility to ensure the lock is held
        unsafe {
            self.tracker.unuse_cpu();
        }
    }
}

#[derive(Debug)]
pub struct CPUWriteGuard<'a, Resource> where Resource: sealed::Mappable  {
    tracker: &'a ResourceTrackerInternal<Resource>,
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
        //safety: it's the guard's responsibility to ensure the lock is held
        unsafe {
            self.tracker.unuse_cpu();

        }
    }
}

#[derive(Debug)]
pub struct GPUGuard<Resource> where Resource: sealed::Mappable {
    tracker: Arc<ResourceTrackerInternal<Resource>>,
}

impl<Resource> Deref for GPUGuard<Resource> where Resource: sealed::Mappable {
    type Target = Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.tracker.resource.get() }
    }
}

impl<Resource> DerefMut for GPUGuard<Resource> where Resource: sealed::Mappable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.tracker.resource.get() }
    }
}

impl<Resource> Drop for GPUGuard<Resource> where Resource: sealed::Mappable {
    fn drop(&mut self) {
        println!("DEBUG: GPUGuard::drop releasing GPU resource");
        self.tracker.unuse_gpu();
        println!("DEBUG: GPUGuard::drop completed");
    }
}

pub struct NotAvailable {
    read_state: u8,
}

impl Debug for NotAvailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = match self.read_state {
            UNUSED => "UNUSED",
            CPU_READ => "CPU_READ",
            CPU_WRITE => "CPU_WRITE",
            GPU => "GPU",
            PENDING_WRITE_TO_GPU => "PENDING_WRITE_TO_GPU",
            _ => "UNKNOWN",
        };
        write!(f, "NotAvailable {{ read_state: {} }}", state)
    }
}




pub(crate) mod sealed {
    use std::future::Future;
    
    pub trait Mappable {
        fn map_read(&mut self) -> impl Future<Output = ()> + Send;
        fn map_write(&mut self) -> impl Future<Output = ()> + Send;

        fn byte_len(&self) -> usize;

        fn unmap(&mut self);

    }
}

struct ResourceTrackerInternal<Resource> {
    state: AtomicU8,
    resource: UnsafeCell<Resource>,
}

impl<Resource> Debug for ResourceTrackerInternal<Resource> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceTrackerInternal")
            .field("state", &self.state)
            .finish()
    }
}



/**
Tracks whether the resource is in use by the CPU or GPU, etc.
*/
#[derive(Debug)]
pub(crate) struct ResourceTracker<Resource> {
    internal: Arc<ResourceTrackerInternal<Resource>>,
}

//todo: do we need these underlying constraints on Resource?
unsafe impl<Resource: Send> Send for ResourceTrackerInternal<Resource> {}
unsafe impl<Resource: Sync> Sync for ResourceTrackerInternal<Resource> {}


impl<Resource> ResourceTrackerInternal<Resource> {
    pub fn new(resource: Resource, initial_state: u8) -> Self {
        Self {
            state: AtomicU8::new(initial_state),
            resource: UnsafeCell::new(resource),
        }
    }
    /// Returns the resource if it is not in use by the CPU or GPU.
    pub async fn cpu_read(&self) -> Result<CPUReadGuard<Resource>,NotAvailable> where Resource: sealed::Mappable {
        match self.state.fetch_update(std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed, |current| {
            match current {
                UNUSED | PENDING_WRITE_TO_GPU => Some(CPU_READ),
                _ => None,
            }
        }) {
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
        match self.state.fetch_update(std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed, |current| {
            match current {
                UNUSED | PENDING_WRITE_TO_GPU => Some(CPU_WRITE),
                _ => None,
            }
        }) {
            Ok(_) => {},
            Err(other) => {
                return Err(NotAvailable { read_state: other });
            },
        }
        unsafe {
            self.resource.get().as_mut().unwrap().map_write().await;
            Ok(CPUWriteGuard { tracker: self })
        }
    }
    /// Returns the resource if it is not in use by the CPU or GPU.
    pub fn gpu(self: &Arc<Self>) -> Result<GPUGuard<Resource>,NotAvailable> where Resource: sealed::Mappable {
        match self.state.compare_exchange(PENDING_WRITE_TO_GPU, GPU, std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => Ok(GPUGuard { tracker: self.clone() }),
            Err(other) => Err(NotAvailable { read_state: other }),
        }
    }


    ///safety: ensure lock is held
    unsafe fn unuse_cpu(&self) where Resource: sealed::Mappable { unsafe {
        (*self.resource.get()).unmap();
        let old_state = self.state.fetch_update(std::sync::atomic::Ordering::Release, std::sync::atomic::Ordering::Relaxed, |current| {
            match current {
                CPU_READ => Some(UNUSED),
                CPU_WRITE => Some(PENDING_WRITE_TO_GPU),
                _ => panic!("unuse_cpu called from invalid state: {}", current),
            }
        }).expect("unuse_cpu state transition failed");
        assert!(old_state == CPU_READ || old_state == CPU_WRITE, "Resource was not in CPU use");
    }}
    fn unuse_gpu(&self) {
        let o = self.state.swap(UNUSED, std::sync::atomic::Ordering::Release);
        assert_ne!(o, UNUSED, "Resource was not in use");
    }
}

impl<Resource> ResourceTracker<Resource> {
    pub fn new(resource: Resource, initial_pending_gpu: bool) -> Self {
        //initially the CPU-side is populated but GPU side is not.
        let state = if initial_pending_gpu {
            PENDING_WRITE_TO_GPU
        } else {
            UNUSED
        };
        Self {
            internal: Arc::new(ResourceTrackerInternal::new(resource, state)),
        }
    }
    pub async fn cpu_read(&self) -> Result<CPUReadGuard<Resource>,NotAvailable> where Resource: sealed::Mappable {
        self.internal.cpu_read().await
    }
    pub async fn cpu_write(&self) -> Result<CPUWriteGuard<Resource>,NotAvailable> where Resource: sealed::Mappable {
        self.internal.cpu_write().await
    }
    pub(crate) fn gpu(&self) -> Result<GPUGuard<Resource>,NotAvailable> where Resource: sealed::Mappable {
        self.internal.gpu()
    }
    /**
    Unsafely accesses the underlying resource.
    
    This is unsafe because it does not check if the resource is in use.
    */
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn access_unsafe(&self) -> &Resource {
        unsafe { &*self.internal.resource.get() }
    }
}