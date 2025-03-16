/*!
Static buffer type.
*/

use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Arc;
use crate::bindings::buffer_access::MapType;
use crate::images::BoundDevice;
use crate::imp;
use crate::multibuffer::sealed::GPUMultibuffer;

pub struct Buffer<Element> {
    pub(crate) imp: imp::GPUableBuffer,
    count: usize,
    element: PhantomData<Element>,
}

#[derive(Debug,thiserror::Error)]
#[error("Texture error")]
pub struct Error(#[from] imp::Error);

pub(crate) fn initialize_byte_array_with<Element,I: Fn(usize) -> Element>(element_count: usize, byte_array: &mut [MaybeUninit<u8>], initializer: I) -> &mut [u8] {
    let byte_size = element_count * std::mem::size_of::<Element>();
    assert_eq!(byte_array.len(),byte_size);
    //transmute to element type
    let as_elements: &mut [MaybeUninit<Element>] = unsafe {
        std::slice::from_raw_parts_mut(byte_array.as_mut_ptr() as *mut MaybeUninit<Element>, byte_size)
    };
    for (i,element) in as_elements.iter_mut().enumerate() {
        *element = MaybeUninit::new(initializer(i));
    }
    //represent that we initialized the buffer!

    unsafe {
        std::slice::from_raw_parts_mut(byte_array.as_mut_ptr() as *mut u8, byte_size)
    }
}

impl<Element> Buffer<Element> {
    pub fn new(device: &Arc<BoundDevice>, count: usize, debug_name: &str, initializer: impl Fn(usize) -> Element) -> Result<Self,Error> {
        let byte_size = std::mem::size_of::<Element>() * count;
        let mappable = imp::MappableBuffer::new(device, byte_size, MapType::Write, debug_name, |bytes| {
            initialize_byte_array_with(count, bytes, initializer)
        })?;


        let imp = imp::GPUableBuffer::new(device, count, debug_name);

        imp.copy_from_buffer(mappable, 0, 0, byte_size);

        Ok(Self {
            imp,
            count,
            element: PhantomData,
        })
    }
}

