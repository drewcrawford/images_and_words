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
use crate::imp::wgpu::context::{smuggle, smuggle_async};
use std::sync::Arc;
use wgpu::MapMode;
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
A simplified buffer that can be mapped onto the host using only Box<[u8]>.
Unlike MappableBuffer, this doesn't use any wgpu types.
*/
#[derive(Debug)]
pub struct MappableBuffer2 {
    internal_buffer: Box<[u8]>,
    _debug_label: String,
}

impl MappableBuffer2 {
    pub async fn new<Initializer: FnOnce(&mut [std::mem::MaybeUninit<u8>]) -> &[u8]>(
        _bound_device: Arc<crate::images::BoundDevice>,
        requested_size: usize,
        _map_type: crate::bindings::buffer_access::MapType,
        debug_name: &str,
        initialize_with: Initializer,
    ) -> Result<Self, crate::imp::Error> {
        let mut data = vec![std::mem::MaybeUninit::uninit(); requested_size];
        let data_ptr = data.as_ptr();
        let initialized = initialize_with(&mut data);

        // Safety: we ensure that the data is initialized and has the correct length
        // Very dumb check that they were the same pointer
        assert_eq!(initialized.as_ptr(), data_ptr as *const u8);
        // And have same length as requested
        assert_eq!(initialized.len(), data.len());

        // Convert to Vec<u8>
        let initialized_data =
            unsafe { std::mem::transmute::<Vec<std::mem::MaybeUninit<u8>>, Vec<u8>>(data) };
        let internal_buffer = initialized_data.into_boxed_slice();

        Ok(MappableBuffer2 {
            internal_buffer,
            _debug_label: debug_name.to_string(),
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
        let slice = &mut self.internal_buffer[dst_offset..dst_offset + data.len()];
        slice.copy_from_slice(data);
    }

    pub async fn map_read(&mut self) {
        // Since we use a CPU view, this is a no-op
    }

    pub async fn map_write(&mut self) {
        // Since we use a CPU view, this is a no-op
    }

    pub async fn unmap(&mut self) {
        // No-op as requested - we don't use wgpu types
    }

    pub fn byte_len(&self) -> usize {
        self.internal_buffer.len()
    }
}

impl crate::bindings::resource_tracking::sealed::Mappable for MappableBuffer2 {
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

impl AsRef<MappableBuffer2> for MappableBuffer2 {
    fn as_ref(&self) -> &MappableBuffer2 {
        self
    }
}

/**
A buffer that holds two wgpu::Buffers for explicit staging operations.
Contains a staging buffer with MAPPABLE | COPY_SRC and a private device buffer.
*/
#[derive(Debug, Clone)]
pub struct GPUableBuffer {
    staging_buffer: WgpuCell<wgpu::Buffer>,
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
    pub(super) async fn new_imp(
        bound_device: Arc<crate::images::BoundDevice>,
        size: usize,
        debug_name: &str,
        storage_type: StorageType,
    ) -> Self {
        let staging_usage = BufferUsages::MAP_WRITE | BufferUsages::COPY_SRC;
        let device_usage = BufferUsages::COPY_DST
            | match storage_type {
                StorageType::Uniform => BufferUsages::UNIFORM,
                StorageType::Storage => BufferUsages::STORAGE,
                StorageType::Vertex => BufferUsages::VERTEX,
                StorageType::Index => BufferUsages::INDEX,
            };

        let staging_debug_name = format!("{}_staging", debug_name);
        let device_debug_name = format!("{}_device", debug_name);
        let move_device = bound_device.clone();
        let move_device2 = bound_device.clone();

        // Create staging buffer
        let staging_buffer = WgpuCell::new_on_thread(move || async move {
            let buffer = move_device
                .0
                .device
                .with(move |device| {
                    let descriptor = BufferDescriptor {
                        label: Some(&staging_debug_name),
                        size: size as u64,
                        usage: staging_usage,
                        mapped_at_creation: false,
                    };
                    device.create_buffer(&descriptor)
                })
                .await;
            buffer
        })
        .await;

        // Create device buffer
        let device_buffer = WgpuCell::new_on_thread(move || async move {
            let buffer = move_device2
                .0
                .device
                .with(move |device| {
                    let descriptor = BufferDescriptor {
                        label: Some(&device_debug_name),
                        size: size as u64,
                        usage: device_usage,
                        mapped_at_creation: false,
                    };
                    device.create_buffer(&descriptor)
                })
                .await;
            buffer
        })
        .await;

        GPUableBuffer {
            staging_buffer,
            device_buffer,
            bound_device,
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
        let move_bound_device = bound_device.clone();
        let storage_type = smuggle("create buffer".to_string(), move || match usage {
            GPUBufferUsage::VertexShaderRead | GPUBufferUsage::FragmentShaderRead => {
                if move_bound_device
                    .0
                    .device
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

        Self::new_imp(bound_device, size, &debug_name, storage_type).await
    }

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

    /// Copy from a MappableBuffer2 to this GPUableBuffer2 using a CommandEncoder.
    ///
    /// This operation:
    /// 1. Maps the staging buffer
    /// 2. Copies data from MappableBuffer2 into the staging buffer
    /// 3. Unmaps the staging buffer
    /// 4. Schedules the copy from staging to device buffer with the encoder
    ///
    /// # Arguments
    /// * `source` - The MappableBuffer2 to copy from
    /// * `command_encoder` - The CommandEncoder to record the copy operation
    pub(crate) async fn copy_from_mappable_buffer2(
        &self,
        source: &MappableBuffer2,
        command_encoder: &mut CommandEncoder,
    ) {
        let bound_device = self.bound_device.clone();
        let staging_buffer_for_mapping = self.staging_buffer.clone();
        let source_data = source.as_slice();
        let copy_len = source_data.len();

        // Copy the source data to avoid borrowing issues
        let source_data_owned = source_data.to_vec();

        // We need to capture the command encoder in a way that can be moved into the smuggle block
        // Since we can't move the mutable reference, we'll record the copy command immediately
        // but ensure it happens after the staging buffer is ready

        smuggle_async(
            "copy_from_mappable_buffer2".to_string(),
            move || async move {
                // Map the staging buffer
                let specified_length = copy_len as u64;
                let (s, r) = r#continue::continuation();
                staging_buffer_for_mapping.assume(|buffer| {
                    buffer.map_async(MapMode::Write, 0..specified_length, |c| {
                        c.unwrap();
                        s.send(());
                    });
                });

                // Signal the polling thread that we need to poll
                bound_device.0.set_needs_poll();
                r.await;

                // Copy data into the staging buffer
                staging_buffer_for_mapping.assume(|buffer| {
                    let mut entire_map = buffer.slice(0..specified_length).get_mapped_range_mut();
                    entire_map.copy_from_slice(&source_data_owned);
                    drop(entire_map);
                    buffer.unmap();
                });
            },
        )
        .await;

        // Now that the staging buffer is ready, schedule the copy from staging to device buffer
        self.staging_buffer.assume(|staging| {
            self.device_buffer.assume(|device| {
                command_encoder.copy_buffer_to_buffer(staging, 0, device, 0, copy_len as u64);
            });
        });
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
                        .device
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

        let device_debug_name = format!("{}_static_with_data", debug_name);
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
            let buffer = move_device
                .0
                .device
                .with(move |device| {
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
                .await;
            buffer
        })
        .await;

        Ok(GPUableBufferStatic {
            device_buffer,
            bound_device,
            storage_type,
        })
    }
}
