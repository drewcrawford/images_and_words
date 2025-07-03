// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0

//! Buffer copy operations for the wgpu backend.
//!
//! This module provides three levels of buffer copying functionality:
//!
//! ## Copy Functions Overview
//!
//! 1. **`copy_from_buffer_internal`** - Low-level unsafe implementation
//!    - Records copy command without resource management
//!    - Used internally by higher-level functions
//!    - Requires manual lifetime and synchronization management
//!
//! 2. **`copy_from_buffer`** - Standalone async operation
//!    - Takes ownership of source buffer for lifetime safety
//!    - Creates own command encoder and waits for completion
//!    - Best for one-off operations like static buffer initialization
//!
//! 3. **`copy_mappable_to_gpuable_buffer`** - Batched render pipeline operation  
//!    - Uses existing command encoder from render pipeline
//!    - Allows batching multiple copies for efficiency
//!    - Best for dynamic buffer updates during render passes
//!
//! ## Usage Guidelines
//!
//! - Use `copy_from_buffer` when you need guaranteed completion and can transfer ownership
//! - Use `copy_mappable_to_gpuable_buffer` when batching operations in render pipelines
//! - The choice depends on whether you need immediate completion vs. batched efficiency

use crate::bindings::buffer_access::MapType;
use crate::bindings::visible_to::GPUBufferUsage;
use crate::images::BoundDevice;
use app_window::wgpu::{WgpuCell, wgpu_begin_context, wgpu_in_context, wgpu_smuggle};
use send_cells::SyncCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use wgpu::MapMode;
use wgpu::{BufferDescriptor, BufferUsages, CommandEncoder, Label};

/**
Copy buffer data in a thread-safe manner.
This function takes owned/cloned data that can be moved across thread boundaries.
*/
async fn copy_buffer_data_threadsafe(
    internal_buffer_data: Box<[u8]>,
    wgpu_buffer: Arc<SyncCell<WgpuCell<wgpu::Buffer>>>,
    bound_device: Arc<BoundDevice>,
) {
    // logwise::info_sync!(
    //     "copy_buffer_data_threadsafe called with {f} bytes",
    //     f = internal_buffer_data.len()
    // );
    let copy = logwise::perfwarn_begin!("copy_buffer_data_threadsafe");
    let specified_length = internal_buffer_data.len() as u64;
    let (s, r) = r#continue::continuation();
    wgpu_buffer.with(|wgpu_cell| {
        wgpu_cell.map_async(MapMode::Write, 0..specified_length, |c| {
            logwise::warn_sync!("In callback");
            c.unwrap();
            s.send(());
        });
    });
    // Signal the polling thread that we need to poll
    bound_device.0.set_needs_poll();
    //logwise::info_sync!("will await");
    r.await;
    //logwise::warn_sync!("Resuming from await");
    wgpu_buffer.with(|wgpu_cell| {
        let mut entire_map = wgpu_cell
            .get()
            .slice(0..specified_length)
            .get_mapped_range_mut();
        //copy all data
        entire_map.copy_from_slice(&internal_buffer_data);
        drop(entire_map);
        wgpu_cell.get().unmap();
    });
    drop(copy);
}

/**
A buffer that can be mapped onto the host.
*/
#[derive(Debug)]
pub struct MappableBuffer {
    //In wgpu, buffers are not sendable.
    //Accordingly we need to emulate this somewhat terribly.
    internal_buffer: Box<[u8]>,
    //note that wgpu also requires us to use this 'staging' buffer since MAP_WRITE is only
    //compatible with COPY_SRC.
    wgpu_buffer: Arc<SyncCell<WgpuCell<wgpu::Buffer>>>,
    /// Whether the buffer is dirty and needs to be written back to the GPU.
    wgpu_buffer_is_dirty: bool,

    bound_device: Arc<BoundDevice>,
    debug_label: String,
}

impl MappableBuffer {
    pub(crate) fn wgpu_buffer(&self) -> &SyncCell<WgpuCell<wgpu::Buffer>> {
        &*self.wgpu_buffer
    }
    pub(crate) async fn new<Initializer: FnOnce(&mut [MaybeUninit<u8>]) -> &[u8]>(
        bound_device: Arc<crate::images::BoundDevice>,
        requested_size: usize,
        map_type: crate::bindings::buffer_access::MapType,
        debug_name: &str,
        initialize_with: Initializer,
    ) -> Result<Self, crate::imp::Error> {
        let mut data = vec![MaybeUninit::uninit(); requested_size];
        let data_ptr = data.as_ptr();
        let initialized = initialize_with(&mut data);
        // Safety: we ensure that the data is initialized and has the correct length
        //very dumb check that they were the same pointer
        assert_eq!(initialized.as_ptr(), data_ptr as *const u8);
        //and have same length as requested
        assert_eq!(initialized.len(), data.len());
        //convert to Vec<u8>
        let initialed_data = unsafe { std::mem::transmute::<Vec<MaybeUninit<u8>>, Vec<u8>>(data) };
        let internal_buffer = initialed_data.into_boxed_slice();

        let buffer_usage = match map_type {
            MapType::Read => BufferUsages::MAP_READ,
            MapType::Write => BufferUsages::MAP_WRITE | BufferUsages::COPY_SRC,
        };

        //I think in order to make wgpu happy we need to round up to the nearest COPY_BUFFER_ALIGNMENT
        let allocated_size = (requested_size as u64 + wgpu::COPY_BUFFER_ALIGNMENT - 1)
            & !(wgpu::COPY_BUFFER_ALIGNMENT - 1);

        //prepare for port
        let debug_name = debug_name.to_string();
        let debug_name_2 = debug_name.clone();
        let move_device = bound_device.clone();
        let internal_buffer = Arc::new(internal_buffer);
        let move_internal_buffer = internal_buffer.clone();
        //here we must use threads
        let buffer = WgpuCell::new_on_thread(move || async move {
            let descriptor = BufferDescriptor {
                label: Some(&debug_name),
                size: allocated_size,
                usage: buffer_usage,
                mapped_at_creation: true,
            };
            let buffer = move_device
                .0
                .wgpu
                .with(|wgpu_cell| wgpu_cell.get().device.create_buffer(&descriptor));
            let mut entire_map = buffer
                .slice(0..requested_size as u64)
                .get_mapped_range_mut();
            //copy all data
            entire_map.copy_from_slice(&internal_buffer);
            drop(internal_buffer);
            drop(entire_map);
            buffer.unmap();
            buffer
        })
        .await;
        let internal_buffer = Arc::try_unwrap(move_internal_buffer).unwrap();
        Ok(MappableBuffer {
            internal_buffer,
            wgpu_buffer: Arc::new(SyncCell::new(buffer)),
            bound_device,
            wgpu_buffer_is_dirty: false,
            debug_label: debug_name_2,
        })
    }

    pub fn as_slice(&self) -> &[u8] {
        self.internal_buffer.as_ref()
    }

    pub fn write(&mut self, data: &[u8], dst_offset: usize) {
        assert!(
            dst_offset + data.len() <= self.internal_buffer.len(),
            "Write out of bounds"
        );
        self.wgpu_buffer_is_dirty = true;
        let slice = &mut self.internal_buffer[dst_offset..dst_offset + data.len()];
        slice.copy_from_slice(data);
    }

    pub async fn map_read(&mut self) {
        //since we use a CPU view, this is a no-op
    }
    pub async fn map_write(&mut self) {
        //since we use a CPU view, this is a no-op
    }
    pub async fn unmap(&mut self) {
        let unmap_perf = logwise::perfwarn_begin!("wgpu::MappableBuffer::unmap");
        // logwise::info_sync!(
        //     "wgpu::MappableBuffer::unmap called on {f}",
        //     f = self.debug_label.clone()
        // );
        if !self.wgpu_buffer_is_dirty {
            return;
        }
        self.wgpu_buffer_is_dirty = false;

        // Clone the data we need to move across the thread boundary
        let internal_buffer_data = self.internal_buffer.clone();
        let wgpu_buffer = self.wgpu_buffer.clone();
        let bound_device = self.bound_device.clone();
        //we need to wait for the unmap to complete here
        let (s, r) = r#continue::continuation();
        let context_calls = logwise::perfwarn_begin!("wgpu::MappableBuffer::context calls");
        let begin_context_begun = logwise::perfwarn_begin!("wgpu::MappableBuffer::begin_context");
        let begin_in_context = logwise::perfwarn_begin!("wgpu::MappableBuffer::in_context");
        app_window::wgpu::wgpu_begin_context(async move {
            drop(begin_context_begun);
            app_window::wgpu::wgpu_in_context(async move {
                drop(begin_in_context);
                copy_buffer_data_threadsafe(internal_buffer_data, wgpu_buffer, bound_device).await;
                s.send(());
            });
        });
        drop(context_calls);
        // logwise::info_sync!(
        //     "wgpu::MappableBuffer::unmap awaiting... on {f}",
        //     f = self.debug_label.clone()
        // );
        let await_perf = logwise::perfwarn_begin!("wgpu::MappableBuffer::unmap await");
        r.await; //wait for unmap to finish
        drop(await_perf);
        // logwise::info_sync!(
        //     "wgpu::MappableBuffer::unmap finished on {f}",
        //     f = self.debug_label.clone()
        // );
        drop(unmap_perf);
    }

    pub fn byte_len(&self) -> usize {
        self.internal_buffer.len()
    }
}

impl crate::bindings::resource_tracking::sealed::Mappable for MappableBuffer {
    async fn map_read(&mut self) {
        self.map_read().await
    }

    async fn map_write(&mut self) {
        self.map_write().await
    }

    async fn unmap(&mut self) {
        self.unmap().await
    }

    fn byte_len(&self) -> usize {
        self.byte_len()
    }
}

impl Drop for MappableBuffer {
    fn drop(&mut self) {
        let clone_buffer = self.wgpu_buffer.clone();
        wgpu_begin_context(async move {
            wgpu_in_context(async move {
                drop(clone_buffer); //ensure this is dropped in the wgpu context
            })
        })
    }
}

impl AsRef<MappableBuffer> for MappableBuffer {
    fn as_ref(&self) -> &MappableBuffer {
        self
    }
}

/**
Internal structure for GPUableBuffer containing WGPU resources.
*/
#[derive(Debug)]
struct GPUableBufferInner {
    buffer: wgpu::Buffer,
    bound_device: Arc<BoundDevice>,
}

/**
A buffer that can (only) be mapped to GPU.
*/
#[derive(Debug, Clone)]
pub struct GPUableBuffer {
    inner: Arc<SyncCell<WgpuCell<GPUableBufferInner>>>,
    storage_type: StorageType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum StorageType {
    Uniform,
    Storage,
    Vertex,
    Index,
}

impl PartialEq for GPUableBuffer {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl GPUableBuffer {
    //only visible to wgpu backend
    pub(super) async fn new_imp(
        bound_device: Arc<crate::images::BoundDevice>,
        size: usize,
        debug_name: &str,
        storage_type: StorageType,
    ) -> Self {
        let forward_flags = BufferUsages::COPY_DST;
        let usage_type_only = match storage_type {
            StorageType::Uniform => BufferUsages::UNIFORM,
            StorageType::Storage => BufferUsages::STORAGE,
            StorageType::Vertex => BufferUsages::VERTEX,
            StorageType::Index => BufferUsages::INDEX,
        };
        let usage_type = usage_type_only | forward_flags;

        let debug_name = debug_name.to_string();
        let move_device = bound_device.clone();
        let inner = WgpuCell::new_on_thread(move || async move {
            let descriptor = BufferDescriptor {
                label: Some(&debug_name),
                size: size as u64,
                usage: usage_type,
                mapped_at_creation: false,
            };
            let buffer = move_device
                .0
                .wgpu
                .with(|wgpu_cell| wgpu_cell.get().device.create_buffer(&descriptor));
            GPUableBufferInner {
                buffer,
                bound_device,
            }
        })
        .await;

        GPUableBuffer {
            inner: Arc::new(SyncCell::new(inner)),
            storage_type,
        }
    }
    pub(crate) async fn new(
        bound_device: Arc<crate::images::BoundDevice>,
        size: usize,
        usage: GPUBufferUsage,
        debug_name: &str,
    ) -> Self {
        let debug_name = debug_name.to_string();
        wgpu_smuggle(move || async move {
            let storage_type = match usage {
                GPUBufferUsage::VertexShaderRead | GPUBufferUsage::FragmentShaderRead => {
                    if bound_device
                        .0
                        .wgpu
                        .with(|wgpu_cell| wgpu_cell.get().device.limits())
                        .max_uniform_buffer_binding_size as usize
                        > size
                    {
                        StorageType::Uniform
                    } else {
                        StorageType::Storage
                    }
                }
                GPUBufferUsage::VertexBuffer => StorageType::Vertex,
                GPUBufferUsage::Index => StorageType::Index,
            };
            Self::new_imp(bound_device, size, &debug_name, storage_type).await
        })
        .await
    }
    pub(super) fn storage_type(&self) -> StorageType {
        self.storage_type
    }

    pub(super) fn buffer(&self) -> wgpu::Buffer {
        self.inner.with(|inner| inner.get().buffer.clone())
    }

    pub(super) fn with_buffer<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&wgpu::Buffer) -> R,
    {
        self.inner.with(|inner| f(&inner.get().buffer))
    }
    /// Copy from a mappable buffer to this GPU buffer with full synchronization.
    ///
    /// This function is designed for standalone operations that need guaranteed completion.
    /// It takes ownership of the source buffer to ensure proper lifetime management,
    /// creates its own command encoder, submits the command, and waits for GPU completion.
    ///
    /// # Use Cases
    /// - Static buffer initialization (uploading data once)
    /// - One-off buffer copies where you need guaranteed completion
    /// - Operations where you want fire-and-forget semantics
    ///
    /// # Contrast with `copy_mappable_to_gpuable_buffer`
    /// - Takes ownership of source buffer (ensures lifetime safety)
    /// - Creates its own command encoder and submits immediately
    /// - Waits for GPU completion before returning (full synchronization)
    /// - Designed for standalone operations
    ///
    /// For batched operations in render pipelines, use `copy_mappable_to_gpuable_buffer` instead.
    ///
    /// # Arguments
    /// * `source` - The mappable buffer to copy from (ownership transferred)
    /// * `source_offset` - Byte offset in the source buffer
    /// * `dest_offset` - Byte offset in this buffer
    /// * `copy_len` - Number of bytes to copy
    pub(crate) async fn copy_from_buffer(
        &self,
        source: MappableBuffer,
        source_offset: usize,
        dest_offset: usize,
        copy_len: usize,
    ) {
        let bound_device = self.inner.with(|inner| inner.get().bound_device.clone());

        let mut encoder = bound_device.0.wgpu.with(|wgpu_cell| {
            wgpu_cell
                .get()
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Label::from("wgpu::GPUableBuffer::copy_from_buffer"),
                })
        });
        //safety: we take the source, so nobody can deallocate it
        unsafe {
            self.copy_from_buffer_internal(
                &source,
                source_offset,
                dest_offset,
                copy_len,
                &mut encoder,
            )
        }

        let command = encoder.finish();
        let _submission_index = bound_device
            .0
            .wgpu
            .with(|wgpu_cell| wgpu_cell.get().queue.submit(std::iter::once(command)));
        let (s, r) = r#continue::continuation();
        bound_device.0.wgpu.with(|wgpu_cell| {
            wgpu_cell.get().queue.on_submitted_work_done(|| {
                s.send(());
            });
        });
        // Signal the polling thread that we need to poll
        bound_device.0.set_needs_poll();
        r.await;
    }
    /// Internal unsafe buffer copy implementation.
    ///
    /// This function records a buffer copy command in the provided command encoder
    /// without any resource management or synchronization. It's the low-level
    /// implementation used by `copy_from_buffer`.
    ///
    /// # Safety
    /// - The caller must ensure the source buffer remains alive until the copy operation completes
    /// - The caller must ensure proper command encoder submission and synchronization
    /// - No bounds checking is performed on offsets or copy length
    ///
    /// # Arguments
    /// * `source` - The mappable buffer to copy from
    /// * `source_offset` - Byte offset in the source buffer
    /// * `dest_offset` - Byte offset in this buffer
    /// * `copy_len` - Number of bytes to copy
    /// * `command_encoder` - The command encoder to record the copy operation
    unsafe fn copy_from_buffer_internal(
        &self,
        source: &MappableBuffer,
        source_offset: usize,
        dest_offset: usize,
        copy_len: usize,
        command_encoder: &mut CommandEncoder,
    ) {
        self.inner.with(|inner| {
            source.wgpu_buffer.with(|source_wgpu_cell| {
                command_encoder.copy_buffer_to_buffer(
                    &source_wgpu_cell.get(),
                    source_offset as u64,
                    &inner.get().buffer,
                    dest_offset as u64,
                    copy_len as u64,
                );
            });
        });
    }
}

impl Drop for GPUableBuffer {
    fn drop(&mut self) {
        // Ensure the buffer is dropped in the wgpu context
        let clone_inner = self.inner.clone();
        wgpu_begin_context(async move {
            wgpu_in_context(async move {
                drop(clone_inner); // Ensure this is dropped in the wgpu context
            });
        });
    }
}

/**
Backend-specific information for copying between buffers.
*/
pub struct CopyInfo<'a> {
    pub(crate) command_encoder: &'a mut CommandEncoder,
}
//wrap the underlying guard type, no particular reason
#[derive(Debug)]
#[must_use = "Ensure this guard lives for the lifetime of the copy!"]
pub struct CopyGuard<SourceGuard> {
    #[allow(dead_code)] //guards work on drop
    source_guard: SourceGuard,
    gpu_buffer: GPUableBuffer,
}

impl<SourceGuard> AsRef<GPUableBuffer> for CopyGuard<SourceGuard> {
    fn as_ref(&self) -> &GPUableBuffer {
        &self.gpu_buffer
    }
}

//I don't think we need to do anything wgpu-specific on CopyGuard's Drop here?
