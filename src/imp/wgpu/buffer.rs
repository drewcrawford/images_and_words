// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::bindings::buffer_access::MapType;
use crate::bindings::visible_to::GPUBufferUsage;
use crate::images::BoundDevice;
use app_window::wgpu::WgpuCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use wgpu::{BufferDescriptor, BufferUsages, CommandEncoder, Label};
use wgpu::{MapMode, PollType};

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
    wgpu_buffer: WgpuCell<wgpu::Buffer>,
    /// Whether the buffer is dirty and needs to be written back to the GPU.
    wgpu_buffer_is_dirty: bool,

    bound_device: Arc<BoundDevice>,
}

impl MappableBuffer {
    pub(crate) fn wgpu_buffer(&self) -> &WgpuCell<wgpu::Buffer> {
        &self.wgpu_buffer
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
                .lock()
                .unwrap()
                .device
                .create_buffer(&descriptor);
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
            wgpu_buffer: buffer,
            bound_device,
            wgpu_buffer_is_dirty: false,
        })
    }

    pub fn as_slice(&self) -> &[u8] {
        self.internal_buffer.as_ref()
    }

    async fn copy_data(&mut self) {
        if !self.wgpu_buffer_is_dirty {
            return;
        }
        self.wgpu_buffer_is_dirty = false;
        let specified_length = self.internal_buffer.len() as u64; //as opposed to the allocated length
        let (s, r) = r#continue::continuation();
        self.wgpu_buffer
            .map_async(MapMode::Write, 0..specified_length, |c| {
                c.unwrap();
                s.send(());
            });
        self.bound_device
            .0
            .wgpu
            .lock()
            .unwrap()
            .device
            .poll(PollType::Wait)
            .unwrap();
        r.await;
        let mut entire_map = self
            .wgpu_buffer
            .get()
            .slice(0..specified_length)
            .get_mapped_range_mut();
        //copy all data
        entire_map.copy_from_slice(&self.internal_buffer);
        drop(entire_map);
        self.wgpu_buffer.get().unmap();
    }

    pub fn write(&mut self, data: &[u8], dst_offset: usize) {
        assert!(
            dst_offset + data.len() <= self.internal_buffer.len(),
            "Write out of bounds"
        );
        self.wgpu_buffer_is_dirty = true;
        // Safety: we ensure that the data is within bounds
        unsafe {
            let slice = &mut self.internal_buffer[dst_offset..dst_offset + data.len()];
            slice.copy_from_slice(data);
        }
    }

    pub async fn map_read(&mut self) {
        //since we use a CPU view, this is a no-op
    }
    pub async fn map_write(&mut self) {
        //since we use a CPU view, this is a no-op
    }
    pub async fn unmap(&mut self) {
        self.copy_data().await;
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

impl AsRef<MappableBuffer> for MappableBuffer {
    fn as_ref(&self) -> &MappableBuffer {
        self
    }
}

/**
A buffer that can (only) be mapped to GPU.
*/
#[derive(Debug, Clone)]
pub struct GPUableBuffer {
    pub(super) buffer: wgpu::Buffer,
    bound_device: Arc<BoundDevice>,
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
    fn eq(&self, _other: &Self) -> bool {
        self.buffer == _other.buffer
    }
}

impl GPUableBuffer {
    //only visible to wgpu backend
    pub(super) fn new_imp(
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
        let descriptor = BufferDescriptor {
            label: Some(debug_name),
            size: size as u64,
            usage: usage_type,
            mapped_at_creation: false,
        };
        let buffer = bound_device
            .0
            .wgpu
            .lock()
            .unwrap()
            .device
            .create_buffer(&descriptor);
        GPUableBuffer {
            buffer,
            bound_device,
            storage_type,
        }
    }
    pub(crate) fn new(
        bound_device: Arc<crate::images::BoundDevice>,
        size: usize,
        usage: GPUBufferUsage,
        debug_name: &str,
    ) -> Self {
        let storage_type = match usage {
            GPUBufferUsage::VertexShaderRead | GPUBufferUsage::FragmentShaderRead => {
                if bound_device
                    .0
                    .wgpu
                    .lock()
                    .unwrap()
                    .device
                    .limits()
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
        Self::new_imp(bound_device, size, debug_name, storage_type)
    }
    pub(super) fn storage_type(&self) -> StorageType {
        self.storage_type
    }
    /// Copy from buffer, taking the source buffer to ensure it lives long enough
    ///
    /// This creates a command encoder to do the copy.
    ///
    /// This function suspends until the copy operation is completed.
    pub(crate) async fn copy_from_buffer(
        &self,
        mut source: MappableBuffer,
        source_offset: usize,
        dest_offset: usize,
        copy_len: usize,
    ) {
        let mut encoder = self
            .bound_device
            .0
            .wgpu
            .lock()
            .unwrap()
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Label::from("wgpu::GPUableBuffer::copy_from_buffer"),
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
        let submission_index = self
            .bound_device
            .0
            .wgpu
            .lock()
            .unwrap()
            .queue
            .submit(std::iter::once(command));
        let (s, r) = r#continue::continuation();
        self.bound_device
            .0
            .wgpu
            .lock()
            .unwrap()
            .queue
            .on_submitted_work_done(|| {
                s.send(());
            });
        self.bound_device
            .0
            .wgpu
            .lock()
            .unwrap()
            .device
            .poll(PollType::WaitForSubmissionIndex(submission_index))
            .expect("Poll failed");
        r.await;
    }
    /**
    copies from the source buffer to this buffer

    # Warning
    This does no resource tracking - ensure that the source buffer is not deallocated before the copy is complete!
    */
    unsafe fn copy_from_buffer_internal(
        &self,
        source: &MappableBuffer,
        source_offset: usize,
        dest_offset: usize,
        copy_len: usize,
        command_encoder: &mut CommandEncoder,
    ) {
        command_encoder.copy_buffer_to_buffer(
            &source.wgpu_buffer.get(),
            source_offset as u64,
            &self.buffer,
            dest_offset as u64,
            copy_len as u64,
        );
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
