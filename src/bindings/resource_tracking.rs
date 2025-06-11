// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Resource tracking for CPU/GPU synchronization
//!
//! This module provides thread-safe tracking of resource usage across CPU and GPU,
//! ensuring proper synchronization and preventing data races when resources are accessed
//! from multiple contexts.
//!
//! # Overview
//!
//! The resource tracking system uses atomic state transitions to coordinate access between:
//! - CPU read operations (immutable access)
//! - CPU write operations (mutable access)
//! - GPU operations (exclusive access during rendering)
//!
//! Resources transition through these states:
//! - `UNUSED`: Resource is not in use and can be acquired for any operation
//! - `CPU_READ`: Resource is mapped for CPU read access
//! - `CPU_WRITE`: Resource is mapped for CPU write access
//! - `GPU`: Resource is in use by the GPU
//! - `PENDING_WRITE_TO_GPU`: CPU write completed, awaiting GPU transfer
//!
//! # Internal Usage
//!
//! This module is used internally by the multibuffer system to track resource state.
//! Resources must implement the `sealed::Mappable` trait which provides async mapping
//! operations for CPU access.
//!
//! The tracking system ensures:
//! - No data races between CPU and GPU access
//! - Proper state transitions through atomic operations
//! - Automatic unmapping when guards are dropped
//! - Thread-safe access through Arc-wrapped internals

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

/// Guard providing immutable CPU access to a tracked resource
///
/// This guard ensures exclusive read access to the resource while held.
/// The resource is automatically unmapped when the guard is dropped.
///
/// # Safety
///
/// The guard maintains the invariant that the resource is in `CPU_READ` state
/// for its entire lifetime, preventing concurrent GPU or CPU write access.
#[derive(Debug)]
pub struct CPUReadGuard<'a, Resource>
where
    Resource: sealed::Mappable,
{
    tracker: &'a ResourceTrackerInternal<Resource>,
}

impl<Resource> Deref for CPUReadGuard<'_, Resource>
where
    Resource: sealed::Mappable,
{
    type Target = Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.tracker.resource.get() }
    }
}
impl<Resource> Drop for CPUReadGuard<'_, Resource>
where
    Resource: sealed::Mappable,
{
    fn drop(&mut self) {
        //safety: it's the guard's responsibility to ensure the lock is held
        unsafe {
            self.tracker.unuse_cpu();
        }
    }
}

/// Guard providing mutable CPU access to a tracked resource
///
/// This guard ensures exclusive write access to the resource while held.
/// When dropped, the resource automatically transitions to `PENDING_WRITE_TO_GPU` state,
/// indicating that the GPU needs to be updated with the modified data.
///
/// # Safety
///
/// The guard maintains the invariant that the resource is in `CPU_WRITE` state
/// for its entire lifetime, preventing any concurrent access.
#[derive(Debug)]
pub struct CPUWriteGuard<'a, Resource>
where
    Resource: sealed::Mappable,
{
    tracker: &'a ResourceTrackerInternal<Resource>,
}

impl<Resource> Deref for CPUWriteGuard<'_, Resource>
where
    Resource: sealed::Mappable,
{
    type Target = Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.tracker.resource.get() }
    }
}

impl<Resource> DerefMut for CPUWriteGuard<'_, Resource>
where
    Resource: sealed::Mappable,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.tracker.resource.get() }
    }
}

impl<Resource> Drop for CPUWriteGuard<'_, Resource>
where
    Resource: sealed::Mappable,
{
    fn drop(&mut self) {
        //safety: it's the guard's responsibility to ensure the lock is held
        unsafe {
            self.tracker.unuse_cpu();
        }
    }
}

/// Guard providing GPU access to a tracked resource
///
/// This guard represents exclusive GPU ownership of the resource.
/// Unlike CPU guards, this holds an `Arc` to ensure the resource
/// remains alive even if passed across thread boundaries during GPU operations.
///
/// # State Transitions
///
/// Can only be acquired when the resource is in `PENDING_WRITE_TO_GPU` state.
/// Transitions to `GPU` state when acquired and back to `UNUSED` when dropped.
#[derive(Debug)]
pub struct GPUGuard<Resource>
where
    Resource: sealed::Mappable,
{
    tracker: Arc<ResourceTrackerInternal<Resource>>,
}

impl<Resource> Deref for GPUGuard<Resource>
where
    Resource: sealed::Mappable,
{
    type Target = Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.tracker.resource.get() }
    }
}

impl<Resource> DerefMut for GPUGuard<Resource>
where
    Resource: sealed::Mappable,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.tracker.resource.get() }
    }
}

impl<Resource> Drop for GPUGuard<Resource>
where
    Resource: sealed::Mappable,
{
    fn drop(&mut self) {
        //println!("DEBUG: GPUGuard::drop releasing GPU resource");
        self.tracker.unuse_gpu();
        //println!("DEBUG: GPUGuard::drop completed");
    }
}

/// Error returned when a resource cannot be acquired
///
/// Contains the current state of the resource to help with debugging
/// and potentially implementing retry logic.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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

    /// Trait for resources that can be mapped for CPU access
    ///
    /// This trait must be implemented by any resource type that needs
    /// to be tracked by the resource tracking system. It provides the
    /// necessary operations for mapping/unmapping memory for CPU access.
    pub trait Mappable {
        /// Maps the resource for read-only CPU access
        ///
        /// This operation is asynchronous as it may need to wait for
        /// GPU operations to complete or for data to be transferred.
        fn map_read(&mut self) -> impl Future<Output = ()> + Send;

        /// Maps the resource for read-write CPU access
        ///
        /// This operation is asynchronous as it may need to wait for
        /// GPU operations to complete or for data to be transferred.
        fn map_write(&mut self) -> impl Future<Output = ()> + Send;

        /// Returns the size of the resource in bytes
        fn byte_len(&self) -> usize;

        /// Unmaps the resource from CPU memory
        ///
        /// Called automatically when CPU access guards are dropped.
        fn unmap(&mut self);
    }
}

/// Internal implementation of resource tracking
///
/// Uses atomic operations to ensure thread-safe state transitions
/// and UnsafeCell for interior mutability of the tracked resource.
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

/// Thread-safe resource tracker for CPU/GPU synchronization
///
/// This is the main interface for resource tracking. It wraps
/// the internal tracker in an Arc to allow sharing across threads
/// and provides safe methods for acquiring CPU or GPU access.
///
/// # Thread Safety
///
/// All methods are thread-safe and use atomic operations to ensure
/// correct state transitions even under concurrent access.
///
/// # Usage Pattern
///
/// 1. Create a tracker with a resource and initial state
/// 2. Use `cpu_read()` or `cpu_write()` for CPU access
/// 3. Use `gpu()` to acquire for GPU operations
/// 4. Guards automatically handle state transitions on drop
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
    // /// Acquires the resource for CPU read access
    // ///
    // /// # Returns
    // ///
    // /// - `Ok(CPUReadGuard)` if the resource can be acquired for reading
    // /// - `Err(NotAvailable)` if the resource is currently in use
    // ///
    // /// # State Transitions
    // ///
    // /// Can acquire from: `UNUSED`, `PENDING_WRITE_TO_GPU`
    // /// Transitions to: `CPU_READ`
    // ///
    // /// # Async Behavior
    // ///
    // /// This method is async because it calls `map_read()` on the resource,
    // /// which may need to wait for GPU operations or data transfers.
    // pub async fn cpu_read(&self) -> Result<CPUReadGuard<Resource>,NotAvailable> where Resource: sealed::Mappable {
    //     match self.state.fetch_update(std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed, |current| {
    //         match current {
    //             UNUSED | PENDING_WRITE_TO_GPU => Some(CPU_READ),
    //             _ => None,
    //         }
    //     }) {
    //         Ok(_) => {},
    //         Err(other) => return Err(NotAvailable { read_state: other }),
    //     }
    //     unsafe {
    //         self.resource.get().as_mut().unwrap().map_read().await;
    //         Ok(CPUReadGuard { tracker: self })
    //     }
    // }
    /// Acquires the resource for CPU write access
    ///
    /// # Returns
    ///
    /// - `Ok(CPUWriteGuard)` if the resource can be acquired for writing
    /// - `Err(NotAvailable)` if the resource is currently in use
    ///
    /// # State Transitions
    ///
    /// Can acquire from: `UNUSED`, `PENDING_WRITE_TO_GPU`
    /// Transitions to: `CPU_WRITE`
    /// On guard drop: Transitions to `PENDING_WRITE_TO_GPU`
    ///
    /// # Async Behavior
    ///
    /// This method is async because it calls `map_write()` on the resource,
    /// which may need to wait for GPU operations or data transfers.
    pub async fn cpu_write(&self) -> Result<CPUWriteGuard<Resource>, NotAvailable>
    where
        Resource: sealed::Mappable,
    {
        match self.state.fetch_update(
            std::sync::atomic::Ordering::Acquire,
            std::sync::atomic::Ordering::Relaxed,
            |current| match current {
                UNUSED | PENDING_WRITE_TO_GPU => Some(CPU_WRITE),
                _ => None,
            },
        ) {
            Ok(_) => {}
            Err(other) => {
                return Err(NotAvailable { read_state: other });
            }
        }
        unsafe {
            self.resource.get().as_mut().unwrap().map_write().await;
            Ok(CPUWriteGuard { tracker: self })
        }
    }
    /// Acquires the resource for GPU use
    ///
    /// # Returns
    ///
    /// - `Ok(GPUGuard)` if the resource can be acquired for GPU use
    /// - `Err(NotAvailable)` if the resource is not in the correct state
    ///
    /// # State Transitions
    ///
    /// Can only acquire from: `PENDING_WRITE_TO_GPU`
    /// Transitions to: `GPU`
    /// On guard drop: Transitions to `UNUSED`
    ///
    /// # Note
    ///
    /// This method requires `&Arc<Self>` because the returned guard needs
    /// to hold an Arc reference to ensure the tracker stays alive.
    pub fn gpu(self: &Arc<Self>) -> Result<GPUGuard<Resource>, NotAvailable>
    where
        Resource: sealed::Mappable,
    {
        match self.state.compare_exchange(
            PENDING_WRITE_TO_GPU,
            GPU,
            std::sync::atomic::Ordering::Acquire,
            std::sync::atomic::Ordering::Relaxed,
        ) {
            Ok(_) => Ok(GPUGuard {
                tracker: self.clone(),
            }),
            Err(other) => Err(NotAvailable { read_state: other }),
        }
    }

    /// Releases CPU access to the resource
    ///
    /// # Safety
    ///
    /// Caller must ensure that they hold a valid CPU lock (read or write).
    /// This is enforced by the guard types which are the only callers.
    unsafe fn unuse_cpu(&self)
    where
        Resource: sealed::Mappable,
    {
        unsafe {
            (*self.resource.get()).unmap();
            let old_state = self
                .state
                .fetch_update(
                    std::sync::atomic::Ordering::Release,
                    std::sync::atomic::Ordering::Relaxed,
                    |current| match current {
                        CPU_READ => Some(UNUSED),
                        CPU_WRITE => Some(PENDING_WRITE_TO_GPU),
                        _ => panic!("unuse_cpu called from invalid state: {}", current),
                    },
                )
                .expect("unuse_cpu state transition failed");
            assert!(
                old_state == CPU_READ || old_state == CPU_WRITE,
                "Resource was not in CPU use"
            );
        }
    }
    /// Releases GPU access to the resource
    ///
    /// Transitions the resource from `GPU` state back to `UNUSED`.
    /// Panics if the resource was not in GPU state.
    fn unuse_gpu(&self) {
        let o = self
            .state
            .swap(UNUSED, std::sync::atomic::Ordering::Release);
        assert_ne!(o, UNUSED, "Resource was not in use");
    }
}

impl<Resource> ResourceTracker<Resource> {
    /// Creates a new resource tracker
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource to track
    /// * `initial_pending_gpu` - If true, starts in `PENDING_WRITE_TO_GPU` state,
    ///   otherwise starts in `UNUSED` state
    ///
    /// # Initial State
    ///
    /// - `true`: Start in `PENDING_WRITE_TO_GPU` state, indicating the CPU has written
    ///   data that needs to be transferred to GPU. Common for newly created resources
    ///   with initial data.
    /// - `false`: Start in `UNUSED` state, indicating the resource is not in use and
    ///   can be acquired for any operation.
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
    // /// Acquires the resource for CPU read access
    // ///
    // /// See [`ResourceTrackerInternal::cpu_read`] for details.
    // pub async fn cpu_read(&self) -> Result<CPUReadGuard<Resource>,NotAvailable> where Resource: sealed::Mappable {
    //     self.internal.cpu_read().await
    // }

    /// Acquires the resource for CPU write access
    ///
    /// See [`ResourceTrackerInternal::cpu_write`] for details.
    pub async fn cpu_write(&self) -> Result<CPUWriteGuard<Resource>, NotAvailable>
    where
        Resource: sealed::Mappable,
    {
        self.internal.cpu_write().await
    }

    /// Acquires the resource for GPU use
    ///
    /// See [`ResourceTrackerInternal::gpu`] for details.
    pub(crate) fn gpu(&self) -> Result<GPUGuard<Resource>, NotAvailable>
    where
        Resource: sealed::Mappable,
    {
        self.internal.gpu()
    }

    /// Unsafely accesses the underlying resource
    ///
    /// # Safety
    ///
    /// This method bypasses all synchronization checks. The caller must ensure:
    /// - The resource is not currently in use by CPU or GPU
    /// - No concurrent access will occur
    /// - The resource state remains consistent
    ///
    /// Use this only when you have external guarantees about resource state.
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) fn access_unsafe(&self) -> &Resource {
        unsafe { &*self.internal.resource.get() }
    }
}

// Boilerplate implementations
impl std::fmt::Display for NotAvailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = match self.read_state {
            UNUSED => "UNUSED",
            CPU_READ => "CPU_READ",
            CPU_WRITE => "CPU_WRITE",
            GPU => "GPU",
            PENDING_WRITE_TO_GPU => "PENDING_WRITE_TO_GPU",
            _ => "UNKNOWN",
        };
        write!(f, "resource not available; current state: {}", state)
    }
}

impl std::error::Error for NotAvailable {}
