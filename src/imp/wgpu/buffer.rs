use std::mem::MaybeUninit;
use std::sync::Arc;
use wgpu::{BufferDescriptor, BufferUsages, CommandEncoder, Label};
use crate::bindings::buffer_access::MapType;
use crate::bindings::resource_tracking::GPUGuard;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::visible_to::GPUBufferUsage;
use crate::images::BoundDevice;
use crate::multibuffer::sealed::GPUMultibuffer;

/**
A buffer that can be mapped onto the host.
*/
#[derive(Debug)]
pub struct MappableBuffer{
    //not actually static!
    mapped: Option<(*const u8, usize)>,
    mapped_mut: Option<(*mut u8, usize)>,

    pub(super) buffer: wgpu::Buffer,
    bound_device: Arc<BoundDevice>,
}
//ignore the mapped raw pointers!
unsafe impl Send for MappableBuffer {}
unsafe impl Sync for MappableBuffer{}

impl MappableBuffer {
    pub(crate) fn new<Initializer: FnOnce(&mut [MaybeUninit<u8>]) -> &[u8]> (bound_device: Arc<crate::images::BoundDevice>, requested_size: usize, map_type: crate::bindings::buffer_access::MapType, debug_name: &str, initialize_with: Initializer) -> Result<Self,crate::imp::Error> {
        let buffer_usage = match map_type {
            MapType::Read => { BufferUsages::MAP_READ }
            MapType::Write => {BufferUsages::MAP_WRITE | BufferUsages::COPY_SRC }
        };

        //I think in order to make wgpu happy we need to round up to the nearest COPY_BUFFER_ALIGNMENT
        let allocated_size = (requested_size as u64 + wgpu::COPY_BUFFER_ALIGNMENT - 1) & !(wgpu::COPY_BUFFER_ALIGNMENT - 1);

        let descriptor = BufferDescriptor {
            label: Some(debug_name),
            size: allocated_size,
            usage: buffer_usage,
            mapped_at_creation: true,
        };
        let buffer = bound_device.0.device.create_buffer(&descriptor);

        //data we access is only up to the requested size, omitting any padding
        let mut entire_map = buffer.slice(..).get_mapped_range_mut();

        let map_entire_view = entire_map.as_mut();
        //ensure we only access the requested size
        let map_requested_view = &mut map_entire_view[..requested_size];
        //These bytes are probably uninitialized and we should represent that to callers
        let map_view: &mut [MaybeUninit<u8>] = unsafe { std::mem::transmute(map_requested_view) };
        let map_view_ptr = map_view.as_ptr();
        let map_view_len = map_view.len();
        //initialize them
        let initialized = initialize_with(map_view);

        //very dumb check that they were the same pointer
        assert_eq!(initialized.as_ptr() as *const u8, map_view_ptr as *const u8);
        //and have same length as requested
        assert_eq!(initialized.len(), map_view_len);
        std::mem::drop(entire_map);

        buffer.unmap();
        Ok(
            MappableBuffer {
                buffer,
                mapped: None,
                mapped_mut: None,
                bound_device,
            }
        )
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe{
            let (ptr, len) = self.mapped.as_ref().expect("Map first");
            std::slice::from_raw_parts(*ptr, *len)
        }
    }

    pub fn write(&mut self, data: &[u8],  dst_offset: usize) {
        unsafe{
            let (ptr, len) = self.mapped_mut.as_ref().expect("Map first");
            assert!(*len >= data.len() + dst_offset, "Buffer too small");
            std::ptr::copy(
                data.as_ptr(),
                (*ptr).add(dst_offset),
                data.len()
            );
        }
    }

    pub async fn map_read(&mut self) {
        let (s,r) = r#continue::continuation();
        let slice = self.buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r|{
            r.unwrap();
            s.send(());
        });
        r.await;
        let range = slice.get_mapped_range();
        self.mapped = Some((range.as_ptr(), range.len()));
    }
    pub async fn map_write(&mut self) {
        let (s,r) = r#continue::continuation();
        let slice = self.buffer.slice(..);
        slice.map_async(wgpu::MapMode::Write, |r|{
            r.unwrap();
            s.send(());
        });

        // Use blocking poll to wait for map completion, avoiding VSync timing issues
        let maintain_result = self.bound_device.0.device.poll(wgpu::Maintain::Wait);
        println!("maintain_result after map_write: {:?}", maintain_result.is_queue_empty());
        r.await;
        let mut range = slice.get_mapped_range_mut();
        self.mapped_mut = Some((range.as_mut_ptr(), range.len()));
    }
    pub fn unmap(&mut self) {
        self.buffer.unmap();
        self.mapped = None;
        self.mapped_mut = None;
    }
    
    pub fn byte_len(&self) -> usize {
        self.buffer.size() as usize
    }

}


/**
A buffer that can (only) be mapped to GPU.
*/
#[derive(Debug,Clone)]
pub struct GPUableBuffer {
    pub(super) buffer: wgpu::Buffer,
    bound_device: Arc<BoundDevice>,
    storage_type: StorageType,
}

#[derive(Debug,Clone,Copy)]
pub(super) enum StorageType {
    Uniform,
    Storage,
    Vertex,
    Index,
}

impl GPUableBuffer {
    //only visible to wgpu backend
    pub(super) fn new_imp(bound_device: Arc<crate::images::BoundDevice>, size: usize, debug_name: &str, storage_type: StorageType) -> Self {
        let usage_type = match storage_type {
            StorageType::Uniform => BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            StorageType::Storage => BufferUsages::STORAGE | BufferUsages::COPY_DST,
            StorageType::Vertex => BufferUsages::VERTEX | BufferUsages::COPY_DST,
            StorageType::Index => BufferUsages::INDEX | BufferUsages::COPY_DST,
        };
        let descriptor = BufferDescriptor {
            label: Some(debug_name),
            size: size as u64,
            usage: usage_type,
            mapped_at_creation: false,
        };
        let buffer = bound_device.0.device.create_buffer(&descriptor);
        GPUableBuffer {
            buffer,
            bound_device,
            storage_type,
        }
    }
    pub(crate) fn new(bound_device: Arc<crate::images::BoundDevice>, size: usize, usage: GPUBufferUsage, debug_name: &str) -> Self {
        let storage_type = match usage {
            GPUBufferUsage::VertexShaderRead | GPUBufferUsage::FragmentShaderRead => {
                if bound_device.0.device.limits().max_uniform_buffer_binding_size as usize > size  {
                    StorageType::Uniform
                }
                else {
                    StorageType::Storage
                }
            }
            GPUBufferUsage::VertexBuffer => {
                StorageType::Vertex
            }
            GPUBufferUsage::Index => {
                StorageType::Index
            }
        };
        Self::new_imp(bound_device, size, debug_name,  storage_type)
    }
    pub(super) fn storage_type(&self) -> StorageType {
        self.storage_type
    }
    /// Copy from buffer, taking the source buffer to ensure it lives long enough
    ///
    /// This creates a command encoder to do the copy.
    ///
    /// This function suspends until the copy operation is completed.
    pub(crate) async fn copy_from_buffer(&self, source: MappableBuffer, source_offset: usize, dest_offset: usize, copy_len: usize) {
        let mut encoder = self.bound_device.0.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Label::from("wgpu::GPUableBuffer::copy_from_buffer") });
        //safety: we take the source, so nobody can deallocate it
        unsafe { self.copy_from_buffer_internal(&source, source_offset, dest_offset, copy_len, &mut encoder) }

        let command = encoder.finish();
        let submission_index = self.bound_device.0.queue.submit(std::iter::once(command));
        let (s,r) = r#continue::continuation();
        self.bound_device.0.queue.on_submitted_work_done(|| {
            s.send(());
        });
        self.bound_device.0.device.poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));
        r.await;
    }
    /**
    copies from the source buffer to this buffer

    # Warning
    This does no resource tracking - ensure that the source buffer is not deallocated before the copy is complete!
    */
    unsafe fn copy_from_buffer_internal(&self, source: &MappableBuffer, source_offset: usize, dest_offset: usize, copy_len: usize, command_encoder: &mut CommandEncoder) {
        command_encoder.copy_buffer_to_buffer(&source.buffer, source_offset as u64, &self.buffer, dest_offset as u64, copy_len as u64);
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




impl GPUMultibuffer for GPUableBuffer {
    type CorrespondingMappedType = MappableBuffer;
    type OutGuard<InGuard> = CopyGuard<InGuard>;

    unsafe fn copy_from_buffer<'a,Guarded>(&self, source_offset: usize, dest_offset: usize, copy_len: usize, info: &mut CopyInfo<'a>, guard: GPUGuard<Guarded>) -> CopyGuard<GPUGuard<Guarded>> where Guarded: AsRef<Self::CorrespondingMappedType>
    ,Guarded: Mappable /* required to appear inside the GPUGuard */
    {
        //somehow we need to get a MappableBuffer
        let m: &MappableBuffer = &guard.as_ref();
        unsafe { self.copy_from_buffer_internal(m, source_offset, dest_offset, copy_len, info.command_encoder); }
        CopyGuard {
            source_guard: guard,
            gpu_buffer: self.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Instead of testing with real MappableBuffer instances (which require GPU setup),
    // let's test the logic by examining the code differences between map_read and map_write
    
    #[test]
    fn test_map_write_missing_implementation() {
        // This test demonstrates that map_write() is missing the crucial implementation
        // It will FAIL, showing that the bug from the reproducer exists
        
        let map_write_source = include_str!("buffer.rs");
        
        // Verify map_read has the correct pattern
        assert!(map_write_source.contains("let range = slice.get_mapped_range();"), 
                "map_read should set mapped field");
        assert!(map_write_source.contains("self.mapped = Some((range.as_ptr(), range.len()));"), 
                "map_read should set mapped field");
        
        // Verify map_write is MISSING the equivalent pattern (this is the bug!)
        let map_write_body_start = map_write_source.find("pub async fn map_write(&mut self) {").unwrap();
        let map_write_body_end = map_write_source[map_write_body_start..].find("pub fn unmap(&mut self)").unwrap() + map_write_body_start;
        let map_write_body = &map_write_source[map_write_body_start..map_write_body_end];
        
        // Check that map_write method body is missing the crucial code
        let executable_lines: Vec<&str> = map_write_body.lines()
            .filter(|line| !line.trim().starts_with("//"))
            .collect();
        let executable_code = executable_lines.join("\n");
        
        // These assertions will FAIL because the code is missing (demonstrating the bug)
        assert!(executable_code.contains("let mut range = slice.get_mapped_range_mut()"), 
                "map_write is missing get_mapped_range_mut - this is the bug from the reproducer!");
        assert!(executable_code.contains("self.mapped_mut = Some((range.as_mut_ptr(), range.len()))"), 
                "map_write is missing mapped_mut assignment - this is the bug from the reproducer!");
    }
    
}


