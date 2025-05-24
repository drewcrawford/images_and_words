/*!
Static buffer type.
*/

use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Arc;
use crate::bindings::buffer_access::MapType;
use crate::bindings::forward::dynamic::buffer::CRepr;
use crate::images::BoundDevice;
use crate::imp;

pub struct Buffer<Element> {
    pub(crate) imp: imp::GPUableBuffer,
    #[allow(dead_code)] //nop implementation does not use
    count: usize,
    element: PhantomData<Element>,
}

#[derive(Debug,Clone)]
pub struct RenderSide {
    #[allow(dead_code)] //nop implementation does not use
    pub(crate) imp: imp::GPUableBuffer,
}

#[derive(Debug,thiserror::Error)]
#[error("Texture error")]
pub struct Error(#[from] imp::Error);

pub(crate) fn initialize_byte_array_with<Element,I: Fn(usize) -> Element>(element_count: usize, byte_array: &mut [MaybeUninit<u8>], initializer: I) -> &mut [u8] where Element: CRepr {
    let byte_size = element_count * std::mem::size_of::<Element>();
    assert_eq!(byte_array.len(),byte_size);
    //transmute to element type
    let as_elements: &mut [MaybeUninit<Element>] = unsafe {
        std::slice::from_raw_parts_mut(byte_array.as_mut_ptr() as *mut MaybeUninit<Element>, element_count)
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
    pub async fn new(device: Arc<BoundDevice>, count: usize, usage: crate::bindings::visible_to::GPUBufferUsage, debug_name: &str, initializer: impl Fn(usize) -> Element) -> Result<Self,Error> where Element: CRepr {
        let byte_size = std::mem::size_of::<Element>() * count;
        let mappable = imp::MappableBuffer::new(device.clone(), byte_size, MapType::Write, debug_name, |bytes| {
            initialize_byte_array_with(count, bytes, initializer)
        })?;


        let imp = imp::GPUableBuffer::new(device, byte_size, usage, debug_name);

        imp.copy_from_buffer(mappable, 0, 0, byte_size).await;

        Ok(Self {
            imp,
            count,
            element: PhantomData,
        })
    }

    pub fn render_side(&self) -> RenderSide {
        RenderSide {
            imp: self.imp.clone()
        }
    }
}

