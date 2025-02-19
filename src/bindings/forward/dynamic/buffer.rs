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
use crate::bindings::visible_to::CPUStrategy;
use crate::images::BoundDevice;
use crate::imp;
use crate::multibuffer::{multibuffer, Producer, ProducerWriteGuard, Receiver, ReceiverReadGuard};

pub enum WriteFrequency {
    ///Significantly less than once per frame.
    Infrequent,
    ///Roughly once per frame.
    Frequent,
}
pub struct Buffer<Element> {
    //?
    element: PhantomData<Element>,
}
#[derive(Debug)]
pub struct RenderSide {

}
impl RenderSide {
    pub(crate) fn dequeue(&mut self) -> GPUBorrow {
        todo!()
    }
}
#[derive(Debug,Clone)]
pub struct GPUBorrow(ReceiverReadGuard<imp::Delivery>);
impl Deref for GPUBorrow {
    type Target = imp::Delivery;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
pub struct CPUBorrow<Element>(Element);
impl<Element> Index<usize> for CPUBorrow<Element> {
    type Output = Element;

    fn index(&self, index: usize) -> &Self::Output {
        todo!()
    }
}

pub struct CPUBorrowMut<Element>(Element);

impl<Element> Index<usize> for CPUBorrowMut<Element> {
    type Output = Element;

    fn index(&self, index: usize) -> &Self::Output {
        todo!()
    }
}

impl<Element> IndexMut<usize> for CPUBorrowMut<Element> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
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

        Ok(Self {
            element: PhantomData,
        })
    }
    /**
    Dequeues a texture.  Resumes when a texture is available.
     */
    pub fn access_read<'s>(&'s mut self) -> impl Future<Output=CPUBorrow<Element>> + 's where Element: Send {
        async {
            todo!()
        }
    }
    pub fn access_write<'s>(&'s mut self) -> impl Future<Output=CPUBorrowMut<Element>> + 's where Element: Send {
        async {
            todo!()
        }
    }

    /**An opaque type that can be bound into a [crate::bindings::bind_style::BindStyle]. */
    pub fn render_side(&mut self) -> RenderSide {
        RenderSide {}
    }

    /**
    Returns the CPUAccess and marks the contents as ready for GPU submission.

    The renderloop will generally re-use each buffer until the next buffer is submitted.
    In this way failing to keep up will not drop the framerate (although it may block your subsystem).

    There is currently no support for atomically submitting two different textures together, mt2-471.
     */
    pub fn submit<'s>(&'s mut self, cpu_access: CPUBorrow<Element>) -> impl Future<Output=()> + 's {
        async { todo!() }
    }

    pub fn submit_mut<'s>(&'s mut self, cpu_access: CPUBorrowMut<Element>) -> impl Future<Output=()> + 's {
        async { todo!() }
    }
}