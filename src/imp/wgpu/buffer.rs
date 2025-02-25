use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, Index, IndexMut};
use wgpu::{BufferDescriptor, BufferUsages, CommandEncoder};
use crate::bindings::buffer_access::MapType;
use crate::bindings::forward::dynamic::buffer::{IndividualBuffer, WriteFrequency};
use crate::bindings::resource_tracking::GPUGuard;
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::visible_to::CPUStrategy;
use crate::imp;
use crate::multibuffer::sealed::{CPUMultibuffer, GPUMultibuffer};

/**
A buffer that can be mapped onto the host.
*/
#[derive(Debug)]
pub struct MappableBuffer{
    //not actually static!
    mapped: Option<(*const u8, usize)>,
    mapped_mut: Option<(*mut u8, usize)>,

    pub(super) buffer: wgpu::Buffer,
}
//ignore the mapped raw pointers!
unsafe impl Send for MappableBuffer {}
unsafe impl Sync for MappableBuffer{}

#[derive(Debug)]
pub struct BindTargetBufferImp {
    pub(crate) element_size: usize,
}

impl BindTargetBufferImp {
    pub fn new(element_size: usize) -> Self {
        BindTargetBufferImp {
            element_size,
        }
    }
}



impl MappableBuffer {
    pub fn new<Initializer: Fn(&mut [MaybeUninit<u8>]) -> &[u8]> (bound_device: &crate::images::BoundDevice, requested_size: usize, map_type: crate::bindings::buffer_access::MapType, debug_name: &str, initialize_with: Initializer) -> Result<Self,crate::imp::Error> {
        let buffer_usage = match map_type {
            MapType::None => { BufferUsages::empty()}
            MapType::Read => { BufferUsages::MAP_READ }
            MapType::Write => {BufferUsages::MAP_WRITE }
            MapType::ReadWrite => {BufferUsages::MAP_WRITE }
        };

        //I think in order to make wgpu happy we need to round up to the nearest COPY_BUFFER_ALIGNMENT
        let allocated_size = (requested_size as u64 + wgpu::COPY_BUFFER_ALIGNMENT - 1) & !(wgpu::COPY_BUFFER_ALIGNMENT - 1);

        let descriptor = BufferDescriptor {
            label: Some(debug_name),
            size: allocated_size,
            usage: buffer_usage,
            mapped_at_creation: true,
        };
        let mut buffer = bound_device.0.device.create_buffer(&descriptor);

        //data we access is only up to the requested size, omitting any padding
        let mut entire_map = buffer.slice(..).get_mapped_range_mut();

        let mut map_entire_view = entire_map.as_mut();
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
            }
        )
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe{
            let (ptr, len) = self.mapped.as_ref().expect("Map first");
            std::slice::from_raw_parts(*ptr, *len)
        }
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe{
            let (ptr, len) = self.mapped_mut.as_ref().expect("Map first");
            std::slice::from_raw_parts_mut(*ptr, *len)
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
        self.buffer.slice(..).map_async(wgpu::MapMode::Write, |r|{
            r.unwrap();
            s.send(());
        });
        r.await;
        let mut range = self.buffer.slice(..).get_mapped_range_mut();
        self.mapped_mut = Some((range.as_mut_ptr(), range.len()));
    }
    pub fn unmap(&mut self) {
        self.buffer.unmap();
        self.mapped = None;
        self.mapped_mut = None;
    }

}


/**
A buffer that can (only) be mapped to GPU.
*/
#[derive(Debug)]
pub struct GPUableBuffer {
    pub(super) buffer: wgpu::Buffer,
}

impl GPUableBuffer {
    pub fn new(bound_device: &crate::images::BoundDevice, size: usize, debug_name: &str) -> Self {
        let descriptor = BufferDescriptor {
            label: Some(debug_name),
            size: size as u64,
            usage: BufferUsages::COPY_DST,
            mapped_at_creation: false,
        };
        let buffer = bound_device.0.device.create_buffer(&descriptor);
        GPUableBuffer {
            buffer,
        }
    }
    /**
    copies from the source buffer to this buffer

    # Warning
    This does no resource tracking - ensure that the source buffer is not deallocated before the copy is complete!
    */
    fn copy_from_buffer_internal(&self, source: &MappableBuffer, source_offset: usize, dest_offset: usize, copy_len: usize, command_encoder: &mut CommandEncoder) {
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
pub struct CopyGuard<Guard> {
    guard: Guard,
}

impl<Guard> Drop for CopyGuard<Guard> {
    fn drop(&mut self) {
        todo!()
    }
}



impl GPUMultibuffer for GPUableBuffer {
    type ItsMappedBuffer = MappableBuffer;
    type OutGuard<InGuard> = CopyGuard<InGuard>;

    fn copy_from_buffer<'a,Guarded>(&self, source_offset: usize, dest_offset: usize, copy_len: usize, info: &mut CopyInfo<'a>, guard: GPUGuard<Guarded>) -> CopyGuard<GPUGuard<Guarded>> where Guarded: AsRef<Self::ItsMappedBuffer>
    ,Guarded: Mappable /* required to appear inside the GPUGuard */
    {
        //somehow we need to get a MappableBuffer
        let m: &MappableBuffer = &guard.as_ref();
        self.copy_from_buffer_internal(m, source_offset, dest_offset, copy_len, info.command_encoder);

        todo!()
    }
}


