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

use crate::bindings::visible_to::GPUBufferUsage;
use crate::images::BoundDevice;
use crate::imp::wgpu::cell::WgpuCell;
use crate::imp::wgpu::context::smuggle;
use std::sync::Arc;
use wgpu::{BufferDescriptor, BufferUsages, CommandEncoder};

/**
Backend-specific information for copying between buffers.
*/
pub struct CopyInfo<'a> {
    pub(crate) command_encoder: &'a mut CommandEncoder,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum StorageType {
    Uniform,
    Storage,
    Vertex,
    Index,
}

/**
A buffer that writes directly to GPU via queue.write_buffer_with().

This implementation uses `write_buffer_with` to allow writing directly into
the staging buffer, eliminating an intermediate copy. The staging buffer is
the queue's internal buffer mapped into user space.
*/
#[derive(Debug)]
pub struct MappableBuffer2 {
    bound_device: Arc<BoundDevice>,
    device_buffer: WgpuCell<wgpu::Buffer>,
    size: usize,
    _debug_label: String,
}

impl MappableBuffer2 {
    /// Creates a new MappableBuffer2 that writes directly to a GPU buffer.
    ///
    /// Note: This constructor doesn't take an initializer - initialization
    /// should be done via GPUableBuffer::new_with_data() which uses
    /// mapped_at_creation for efficient initial data upload.
    pub async fn new_for_gpu_buffer(
        bound_device: Arc<crate::images::BoundDevice>,
        device_buffer: WgpuCell<wgpu::Buffer>,
        size: usize,
        debug_name: &str,
    ) -> Result<Self, crate::imp::Error> {
        Ok(MappableBuffer2 {
            bound_device,
            device_buffer,
            size,
            _debug_label: debug_name.to_string(),
        })
    }

    /// Writes data directly to the GPU buffer via queue.write_buffer_with().
    ///
    /// This writes directly into the queue's staging buffer, eliminating
    /// an intermediate copy compared to queue.write_buffer().
    #[logwise::profile]
    pub async fn write(&mut self, data: &[u8], dst_offset: usize) {
        assert!(
            dst_offset + data.len() <= self.size,
            "Write out of bounds: offset {} + len {} > size {}",
            dst_offset,
            data.len(),
            self.size
        );

        // Copy data to owned Vec so it can be sent across threads
        let data = data.to_vec();
        let dst_offset = dst_offset as u64;

        // Use WgpuCell::with() to run on the correct thread via smuggle
        let queue = self.bound_device.0.queue();
        let device_buffer = self.device_buffer.clone();

        queue
            .with(move |queue| {
                device_buffer.assume(|device_buffer| {
                    if let Some(mut view) = queue.write_buffer_with(
                        device_buffer,
                        dst_offset,
                        std::num::NonZero::new(data.len() as u64).unwrap(),
                    ) {
                        view.copy_from_slice(&data);
                        // View is dropped here, queuing the transfer
                    } else {
                        // Fallback to write_buffer if write_buffer_with fails
                        logwise::warn_sync!(
                            "write_buffer_with returned None, falling back to write_buffer"
                        );
                        queue.write_buffer(device_buffer, dst_offset, &data);
                    }
                });
            })
            .await;
    }

    pub async fn map_write(&mut self) {
        // No-op: write_buffer_with doesn't require explicit mapping
    }

    pub fn unmap(&mut self) {
        // No-op: write_buffer_with handles this automatically
    }
}

impl crate::bindings::resource_tracking::sealed::Mappable for MappableBuffer2 {
    // async fn map_read(&mut self) {
    //     self.map_read().await
    // }

    async fn map_write(&mut self) {
        self.map_write().await
    }

    fn unmap(&mut self) {
        self.unmap();
    }

    // fn byte_len(&self) -> usize {
    //     self.byte_len()
    // }
}

impl AsRef<MappableBuffer2> for MappableBuffer2 {
    fn as_ref(&self) -> &MappableBuffer2 {
        self
    }
}

/**
A buffer that holds a GPU device buffer.
Uses queue.write_buffer() for efficient CPU-to-GPU transfers.
*/
#[derive(Debug, Clone)]
pub struct GPUableBuffer {
    device_buffer: WgpuCell<wgpu::Buffer>,
    bound_device: Arc<BoundDevice>,
    storage_type: StorageType,
}

impl PartialEq for GPUableBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.device_buffer == other.device_buffer
    }
}

impl GPUableBuffer {
    pub(super) fn storage_type(&self) -> StorageType {
        self.storage_type
    }

    #[allow(dead_code)]
    pub(super) fn device_buffer(&self) -> &WgpuCell<wgpu::Buffer> {
        &self.device_buffer
    }

    /// Get a reference to the device buffer for GPU operations.
    /// This is the buffer that should be used for binding to shaders.
    pub(super) fn buffer(&self) -> &WgpuCell<wgpu::Buffer> {
        &self.device_buffer
    }

    /// Get a clone of the device buffer cell for write_buffer_with operations.
    pub(crate) fn device_buffer_clone(&self) -> WgpuCell<wgpu::Buffer> {
        self.device_buffer.clone()
    }

    /// Get a clone of the bound device for queue access.
    pub(crate) fn bound_device(&self) -> Arc<BoundDevice> {
        self.bound_device.clone()
    }

    /// Copy from a MappableBuffer2 to this GPUableBuffer.
    ///
    /// With the write_buffer_with design, MappableBuffer2 writes directly
    /// to the GPU buffer, so this is now a no-op.
    ///
    /// # Arguments
    /// * `_source` - Unused, MappableBuffer2 already wrote directly
    /// * `_command_encoder` - Unused
    #[logwise::profile]
    pub(crate) async fn copy_from_mappable_buffer2(
        &self,
        _source: &MappableBuffer2,
        _command_encoder: &mut CommandEncoder,
    ) {
        // No-op: MappableBuffer2 now writes directly via write_buffer_with
        logwise::trace_sync!("buffer_copy_data: no-op (write_buffer_with already performed)");
    }

    /// Creates a new GPUableBuffer with initial data using mapped_at_creation.
    ///
    /// This is the most efficient way to create a buffer with initial data,
    /// as it writes directly to the mapped GPU memory without staging.
    pub(crate) async fn new_with_data<I: FnOnce(&mut [std::mem::MaybeUninit<u8>]) -> &[u8]>(
        bound_device: Arc<crate::images::BoundDevice>,
        size: usize,
        usage: GPUBufferUsage,
        debug_name: &str,
        initializer: I,
    ) -> Self {
        let debug_name = debug_name.to_string();
        let move_bound_device = bound_device.clone();
        let storage_type = smuggle("create buffer with data".to_string(), move || match usage {
            GPUBufferUsage::VertexShaderRead | GPUBufferUsage::FragmentShaderRead => {
                if move_bound_device
                    .0
                    .device()
                    .assume(|c| c.limits())
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
        })
        .await;

        let device_usage = BufferUsages::COPY_DST
            | match storage_type {
                StorageType::Uniform => BufferUsages::UNIFORM,
                StorageType::Storage => BufferUsages::STORAGE,
                StorageType::Vertex => BufferUsages::VERTEX,
                StorageType::Index => BufferUsages::INDEX,
            };

        // Prepare data for initialization
        let mut data = vec![std::mem::MaybeUninit::uninit(); size];
        let data_ptr = data.as_ptr();
        let initialized = initializer(&mut data);

        // Safety: we ensure that the data is initialized and has the correct length
        assert_eq!(initialized.as_ptr(), data_ptr as *const u8);
        assert_eq!(initialized.len(), size);

        // Convert to Vec<u8>
        let initialized_data =
            unsafe { std::mem::transmute::<Vec<std::mem::MaybeUninit<u8>>, Vec<u8>>(data) };
        let internal_buffer = std::sync::Arc::new(initialized_data.into_boxed_slice());

        let device_debug_name = format!("{debug_name}_with_data");
        let move_device = bound_device.clone();

        // Create device buffer with mapped_at_creation=true for direct initialization
        let device_buffer = WgpuCell::new_on_thread(move || async move {
            move_device.0.device().assume(move |device| {
                let descriptor = BufferDescriptor {
                    label: Some(&device_debug_name),
                    size: size as u64,
                    usage: device_usage,
                    mapped_at_creation: true,
                };
                let buffer = device.create_buffer(&descriptor);
                let mut entire_map = buffer.slice(0..size as u64).get_mapped_range_mut();
                // Copy all data
                entire_map.copy_from_slice(&internal_buffer);
                drop(internal_buffer);
                drop(entire_map);
                buffer.unmap();
                buffer
            })
        })
        .await;

        GPUableBuffer {
            device_buffer,
            bound_device,
            storage_type,
        }
    }
}

/**
A static buffer that holds only a single device wgpu::Buffer.
Like GPUableBuffer2 but without the staging buffer - for static data that doesn't change.
*/
#[derive(Debug, Clone)]
pub struct GPUableBufferStatic {
    device_buffer: WgpuCell<wgpu::Buffer>,
    #[allow(dead_code)]
    bound_device: Arc<BoundDevice>,
    storage_type: StorageType,
}

impl PartialEq for GPUableBufferStatic {
    fn eq(&self, other: &Self) -> bool {
        self.device_buffer == other.device_buffer
    }
}

impl Eq for GPUableBufferStatic {}

impl std::hash::Hash for GPUableBufferStatic {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.device_buffer.hash(state);
    }
}

impl GPUableBufferStatic {
    pub(super) fn storage_type(&self) -> StorageType {
        self.storage_type
    }

    #[allow(dead_code)]
    pub(super) fn device_buffer(&self) -> &WgpuCell<wgpu::Buffer> {
        &self.device_buffer
    }

    /// Get a reference to the device buffer for GPU operations.
    /// This is the buffer that should be used for binding to shaders.
    pub(super) fn buffer(&self) -> &WgpuCell<wgpu::Buffer> {
        &self.device_buffer
    }

    /// Creates a new static buffer with initial data provided during creation.
    ///
    /// This method creates a GPU buffer with `mapped_at_creation=true` and initializes
    /// it with data using the provided initializer function. This is the most efficient
    /// way to create static buffers since it avoids the need for a separate staging buffer
    /// and copy operation.
    ///
    /// # Arguments
    /// * `bound_device` - The GPU device to create the buffer on
    /// * `size` - Size of the buffer in bytes
    /// * `usage` - How the buffer will be used on the GPU
    /// * `debug_name` - Human-readable name for debugging
    /// * `initializer` - Function to initialize the buffer data
    ///
    /// # Returns
    /// Returns a `GPUableBuffer2Static` with the initialized data.
    pub(crate) async fn new_with_data<I: FnOnce(&mut [std::mem::MaybeUninit<u8>]) -> &[u8]>(
        bound_device: Arc<crate::images::BoundDevice>,
        size: usize,
        usage: GPUBufferUsage,
        debug_name: &str,
        initializer: I,
    ) -> Result<Self, crate::imp::Error> {
        let debug_name = debug_name.to_string();
        let move_bound_device = bound_device.clone();
        let storage_type = smuggle(
            "create static buffer with data".to_string(),
            move || match usage {
                GPUBufferUsage::VertexShaderRead | GPUBufferUsage::FragmentShaderRead => {
                    if move_bound_device
                        .0
                        .device()
                        .assume(|c| c.limits())
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
            },
        )
        .await;

        let device_usage = BufferUsages::COPY_DST
            | match storage_type {
                StorageType::Uniform => BufferUsages::UNIFORM,
                StorageType::Storage => BufferUsages::STORAGE,
                StorageType::Vertex => BufferUsages::VERTEX,
                StorageType::Index => BufferUsages::INDEX,
            };

        let device_debug_name = format!("{debug_name}_static_with_data");
        let move_device = bound_device.clone();

        // Prepare data for initialization
        let mut data = vec![std::mem::MaybeUninit::uninit(); size];
        let data_ptr = data.as_ptr();
        let initialized = initializer(&mut data);

        // Safety: we ensure that the data is initialized and has the correct length
        assert_eq!(initialized.as_ptr(), data_ptr as *const u8);
        assert_eq!(initialized.len(), size);

        // Convert to Vec<u8>
        let initialized_data =
            unsafe { std::mem::transmute::<Vec<std::mem::MaybeUninit<u8>>, Vec<u8>>(data) };
        let internal_buffer = std::sync::Arc::new(initialized_data.into_boxed_slice());
        let _move_internal_buffer = internal_buffer.clone();

        // Create device buffer with mapped_at_creation=true for direct initialization
        let device_buffer = WgpuCell::new_on_thread(move || async move {
            move_device.0.device().assume(move |device| {
                let descriptor = BufferDescriptor {
                    label: Some(&device_debug_name),
                    size: size as u64,
                    usage: device_usage,
                    mapped_at_creation: true,
                };
                let buffer = device.create_buffer(&descriptor);
                let mut entire_map = buffer.slice(0..size as u64).get_mapped_range_mut();
                // Copy all data
                entire_map.copy_from_slice(&internal_buffer);
                drop(internal_buffer);
                drop(entire_map);
                buffer.unmap();
                buffer
            })
        })
        .await;

        Ok(GPUableBufferStatic {
            device_buffer,
            bound_device,
            storage_type,
        })
    }
}
