/*! Dynamic buffer implementation.

A dynamic buffer is data we expect to change dynamically.
It is not necessarily every frame, the exact optimizations are passed by argument.
*/

use std::fmt::{Display, Formatter};
use std::future::Future;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::sync::Arc;
use log::debug;
use crate::bindings::resource_tracking::{CPUReadGuard, CPUWriteGuard, GPUGuard};
use crate::bindings::visible_to::CPUStrategy;
use crate::images::BoundDevice;
use crate::imp;
use crate::multibuffer::Multibuffer;

pub enum WriteFrequency {
    ///Significantly less than once per frame.
    Infrequent,
    ///Roughly once per frame.
    Frequent,
}
pub struct Buffer<Element> {
    multibuffer: Multibuffer<IndividualBuffer<Element>>
}

pub struct IndividualBuffer<Element> {
    imp: imp::Buffer,
    _marker: PhantomData<Element>,
}

impl<Element> Index<usize> for IndividualBuffer<Element> {
    type Output = Element;
    fn index(&self, index: usize) -> &Self::Output {
        todo!()
    }
}

impl<Element> IndexMut<usize> for IndividualBuffer<Element> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        todo!()
    }
}

#[derive(Debug)]
pub struct RenderSide<Element> {
    _marker: PhantomData<Element>,
}
impl<Element> RenderSide<Element> {
    pub(crate) fn dequeue(&mut self) -> GPUGuard<IndividualBuffer<Element>> {
        todo!()
    }
}



#[derive(thiserror::Error, Debug)]
pub struct Error(#[from] imp::Error);
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}




impl<Element> Buffer<Element> {
    pub fn new<I: Fn(usize) -> Element>(bound_device: &Arc<BoundDevice>, size: usize, write_frequency: WriteFrequency, cpu_strategy: CPUStrategy, debug_name: &str, initialize_with:I) -> Result<Self,Error> {
        match write_frequency {
            WriteFrequency::Infrequent => {/* Not sure what to do here but possibly we load into map_type somehow?*/}
            WriteFrequency::Frequent => {/* Not sure what to do here but possibly we load into map_type somehow?*/}
        }
        let map_type = match cpu_strategy {
            CPUStrategy::ReadsFrequently => {
                crate::bindings::buffer_access::MapType::ReadWrite
            }
            CPUStrategy::WontRead => {
                crate::bindings::buffer_access::MapType::Read //read-only!
            }
        };
        let byte_size = size * std::mem::size_of::<Element>();
        assert_ne!(byte_size,0, "Zero-sized buffers are not allowed");

        let buffer = imp::Buffer::new(bound_device, byte_size, map_type, debug_name, |byte_array| {
           assert_eq!(byte_array.len(),byte_size);
            //transmute to element type
            let as_elements: &mut [MaybeUninit<Element>] = unsafe {
                std::slice::from_raw_parts_mut(byte_array.as_mut_ptr() as *mut MaybeUninit<Element>, size)
            };
            for (i,element) in as_elements.iter_mut().enumerate() {
                *element = MaybeUninit::new(initialize_with(i));
            }
            //represent that we initialized the buffer!

            unsafe {
                std::slice::from_raw_parts_mut(byte_array.as_mut_ptr() as *mut u8, byte_size)
            }
        })?;

        let individual_buffer = IndividualBuffer {
            imp: buffer,
            _marker: PhantomData,
        };

        Ok(Self {
            multibuffer: Multibuffer::new(individual_buffer),
        })
    }
    /**
    Dequeues a texture.  Resumes when a texture is available.
     */
    pub async fn access_read(&self) -> CPUReadGuard<IndividualBuffer<Element>> {
        self.multibuffer.access_read().await
    }
    pub async fn access_write(&self) -> CPUWriteGuard<IndividualBuffer<Element>> {
        self.multibuffer.access_write().await
    }

    /**An opaque type that can be bound into a [crate::bindings::bind_style::BindStyle]. */
    pub fn render_side(&mut self) -> RenderSide<Element> {
        RenderSide {
            _marker: PhantomData,
        }
    }


}