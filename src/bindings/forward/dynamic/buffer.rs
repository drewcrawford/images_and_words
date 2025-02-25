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
use crate::multibuffer::{CPUReadGuard, CPUWriteGuard, GPUGuard};
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::visible_to::CPUStrategy;
use crate::images::BoundDevice;
use crate::imp;
use crate::imp::{CopyInfo, GPUableBuffer};
use crate::multibuffer::Multibuffer;
use crate::multibuffer::sealed::CPUMultibuffer;

pub enum WriteFrequency {
    ///Significantly less than once per frame.
    Infrequent,
    ///Roughly once per frame.
    Frequent,
}

//shared between CPU and render-side
#[derive(Debug)]
struct Shared<Element> {
    multibuffer: Multibuffer<IndividualBuffer<Element>, imp::GPUableBuffer>,
}
pub struct Buffer<Element> {
    shared: Arc<Shared<Element>>,
}

#[derive(Debug)]
pub struct IndividualBuffer<Element> {
    pub(crate) imp: imp::MappableBuffer,
    _marker: PhantomData<Element>,
    count: usize,
}


impl<Element> Index<usize> for IndividualBuffer<Element> {
    type Output = Element;
    fn index(&self, index: usize) -> &Self::Output {
        let offset = index * std::mem::size_of::<Element>();
        let bytes: &[u8] = &self.imp.as_slice()[offset..offset+std::mem::size_of::<Element>()];
        unsafe {
            &*(bytes.as_ptr() as *const Element)
        }
    }
}

impl<Element> IndexMut<usize> for IndividualBuffer<Element> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let offset = index * std::mem::size_of::<Element>();
        let bytes: &mut [u8] = &mut self.imp.as_slice_mut()[offset..offset+std::mem::size_of::<Element>()];
        unsafe {
            &mut *(bytes.as_mut_ptr() as *mut Element)
        }
    }
}
impl<Element> Mappable for IndividualBuffer<Element> {
    async fn map_read(&mut self) {
        self.imp.map_read().await;
    }
    async fn map_write(&mut self) {
        self.imp.map_write().await;
    }
    fn unmap(&mut self) {
        self.imp.unmap();
    }
    fn byte_len(&self) -> usize {
        self.count * std::mem::size_of::<Element>()
    }
}

//in order to support GPU copy, we also need to implement AsRef to the imp type
//see GPUMultibuffer definition for details
impl<Element> AsRef<imp::MappableBuffer> for IndividualBuffer<Element> {
    fn as_ref(&self) -> &imp::MappableBuffer {
        &self.imp
    }
}

impl<Element> CPUMultibuffer for IndividualBuffer<Element> {
    type Source = imp::MappableBuffer;
    fn as_source(&self) -> &Self::Source {
        &self.imp
    }
}
#[derive(Debug)]
pub struct RenderSide<Element> {
    shared: Arc<Shared<Element>>,
}

pub struct GPUAccess<Element> {
    imp: crate::multibuffer::GPUGuard<imp::GPUableBuffer, IndividualBuffer<Element>>
}
impl<Element> GPUAccess<Element> {
    pub(crate) fn as_ref(&self) -> &imp::GPUableBuffer {
        let out_guard = &self.imp.imp;
        &out_guard.deref()
    }
}
impl<Element> RenderSide<Element> {

    pub(crate) fn erased_render_side(&self) -> ErasedRenderSide {
        todo!()
    }
    /**
    # Safety

    Caller must guarantee that the return value is live for the duration of the GPU read.
     */
    pub(crate) unsafe fn acquire_gpu_buffer(&self, copy_info: &mut CopyInfo) -> GPUAccess<Element> {
        let t = self.shared.multibuffer.access_gpu(copy_info);
        GPUAccess {
            imp: t,
        }
    }
}

pub struct ErasedRenderSide {

}



#[derive(thiserror::Error, Debug)]
pub struct Error(#[from] imp::Error);
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}




impl<Element> Buffer<Element> {
    pub fn new<I: Fn(usize) -> Element>(bound_device: &Arc<BoundDevice>, size: usize, debug_name: &str, initialize_with:I) -> Result<Self,Error> {
        let byte_size = size * std::mem::size_of::<Element>();
        assert_ne!(byte_size,0, "Zero-sized buffers are not allowed");

        let map_type = crate::bindings::buffer_access::MapType::Write; //todo: optimize for read vs write, etc.

        let buffer = imp::MappableBuffer::new(bound_device, byte_size, map_type, debug_name, |byte_array| {
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
            count: size,
        };
        let gpu_buffer = imp::GPUableBuffer::new(bound_device,byte_size, debug_name);

        Ok(Self {
            shared: Arc::new(Shared {
                multibuffer: Multibuffer::new(individual_buffer, gpu_buffer),
            })
        })
    }
    /**
    Dequeues a texture.  Resumes when a texture is available.
     */
    pub async fn access_read(&self) -> CPUReadGuard<IndividualBuffer<Element>> {
        self.shared.multibuffer.access_read().await
    }
    pub async fn access_write(&self) -> CPUWriteGuard<IndividualBuffer<Element>> {
        self.shared.multibuffer.access_write().await
    }

    /**An opaque type that can be bound into a [crate::bindings::bind_style::BindStyle]. */
    pub fn render_side(&self) -> RenderSide<Element> {
        RenderSide {
            shared: self.shared.clone(),
        }
    }


}