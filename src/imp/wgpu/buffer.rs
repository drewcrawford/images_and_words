use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Index, IndexMut};
use wgpu::{BufferDescriptor, BufferUsages};
use crate::bindings::buffer_access::MapType;
use crate::bindings::forward::dynamic::buffer::WriteFrequency;
use crate::bindings::visible_to::CPUStrategy;

pub struct Buffer{
    //not actually static!
    view_mut_unsafe: Option<wgpu::BufferViewMut<'static>>,
    buffer: wgpu::Buffer,
}



impl Buffer {
    pub fn new<Initializer: Fn(&mut [MaybeUninit<u8>]) -> &[u8]> (bound_device: &crate::images::BoundDevice, requested_size: usize, map_type: crate::bindings::buffer_access::MapType, debug_name: &str, initialize_with: Initializer) -> Result<Self,crate::imp::Error> {
        let buffer_usage = match map_type {
            MapType::None => { BufferUsages::empty()}
            MapType::Read => { BufferUsages::MAP_READ }
            MapType::Write => {BufferUsages::MAP_WRITE }
            MapType::ReadWrite => {BufferUsages::MAP_READ | BufferUsages::MAP_WRITE }
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
            Buffer {
                buffer,
                view_mut_unsafe: None,
            }
        )
    }

    pub fn as_slice(&self) -> &[u8] {
        self.view_mut_unsafe.as_ref().expect("Map first")
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        self.view_mut_unsafe.as_mut().expect("Map first")
    }
}






