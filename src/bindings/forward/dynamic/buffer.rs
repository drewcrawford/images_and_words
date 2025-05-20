/*! Dynamic buffer implementation.

A dynamic buffer is data we expect to change dynamically.
It is not necessarily every frame, the exact optimizations are passed by argument.
*/

use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::sync::Arc;
use log::debug;
use crate::bindings::dirty_tracking::DirtyReceiver;
use crate::bindings::resource_tracking::GPUGuard;
use crate::multibuffer::{CPUReadGuard, CPUWriteGuard};
use crate::bindings::resource_tracking::sealed::Mappable;
use crate::bindings::visible_to::{CPUStrategy, GPUBufferUsage};
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

struct Shared<Element> {
    multibuffer: Multibuffer<IndividualBuffer<Element>, imp::GPUableBuffer>,
}
impl<Element> Debug for Shared<Element> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("multibuffer", &self.multibuffer)
            .finish()
    }
}
pub struct Buffer<Element> {
    shared: Arc<Shared<Element>>,
    count: usize,
}

pub struct IndividualBuffer<Element> {
    pub(crate) imp: imp::MappableBuffer,
    _marker: PhantomData<Element>,
    count: usize,
}

impl<Element> Debug for IndividualBuffer<Element> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndividualBuffer")
            .field("imp", &self.imp)
            .field("count", &self.count)
            .finish()
    }
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
pub struct RenderSide<Element> {
    shared: Arc<Shared<Element>>,
    count: usize,
}

impl<Element> Debug for RenderSide<Element> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderSide")
            .field("shared", &self.shared)
            .field("count", &self.count)
            .finish()
    }
}


/*
Guards access to the underlying [GPUableBuffer].  Used for binding in the render pass.
 */
pub struct GPUAccess<Element> {
    imp: crate::multibuffer::GPUGuard<IndividualBuffer<Element>,GPUableBuffer>,
    _phantom: PhantomData<Element>,
}
impl<Element> GPUAccess<Element> {
    pub(crate) fn as_ref(&self) -> &imp::GPUableBuffer {
        &self.imp.as_imp()
    }
}

pub(crate) trait SomeGPUAccess: Send {
    fn as_imp(&self) -> &imp::GPUableBuffer;
}

impl<Element: Send + Sync> SomeGPUAccess for GPUAccess<Element> {
    fn as_imp(&self) -> &imp::GPUableBuffer {
        self.as_ref()
    }
}
impl<Element> RenderSide<Element> {

    pub(crate) fn erased_render_side(self) -> ErasedRenderSide where Element: Send + Sync + 'static {
        ErasedRenderSide {
            element_size: std::mem::size_of::<Element>(),
            byte_size: self.count * std::mem::size_of::<Element>(),
            imp: Arc::new(self),
        }
    }


}

///Erases the RenderSide generics.
pub(crate) trait SomeRenderSide: Send + Sync + Debug {
    ///Safety: keep the guard alive
    unsafe fn acquire_gpu_buffer(&self, copy_info: &mut CopyInfo) -> Box<dyn SomeGPUAccess>;
    fn dirty_receiver(&self) -> DirtyReceiver;
    unsafe fn unsafe_imp(&self) -> &imp::GPUableBuffer;
}

impl<Element: Send + Sync + 'static> SomeRenderSide for RenderSide<Element> {
    /**
    Safety:

    Must keep the returned guard active for the duration of GPU use.
*/
    unsafe fn acquire_gpu_buffer(&self, copy_info: &mut CopyInfo) -> Box<dyn SomeGPUAccess> {
        let underlying_guard = self.shared.multibuffer.access_gpu(copy_info);
        Box::new(GPUAccess {
            imp: underlying_guard,
            _phantom: PhantomData::<Element>,
        })
    }
    fn dirty_receiver(&self) -> DirtyReceiver {
        self.shared.multibuffer.gpu_dirty_receiver()
    }
    unsafe fn unsafe_imp(&self) -> &imp::GPUableBuffer {
        self.shared.multibuffer.access_gpu_unsafe()
    }
}

#[derive(Debug,Clone)]
pub struct ErasedRenderSide {
    pub(crate) element_size: usize,
    pub(crate) imp: Arc<dyn SomeRenderSide>,
    pub(crate) byte_size: usize,
}




#[derive(thiserror::Error, Debug)]
pub struct Error(#[from] imp::Error);
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}





impl<Element> Buffer<Element> {
    pub fn new(bound_device: Arc<BoundDevice>, size: usize, usage: GPUBufferUsage, debug_name: &str, initialize_with:impl Fn(usize) -> Element) -> Result<Self,Error> where Element: CRepr {
        let byte_size = size * std::mem::size_of::<Element>();
        assert_ne!(byte_size,0, "Zero-sized buffers are not allowed");

        let map_type = crate::bindings::buffer_access::MapType::Write; //todo: optimize for read vs write, etc.

        let buffer = imp::MappableBuffer::new(&bound_device, byte_size, map_type, debug_name, move |byte_array| {
          crate::bindings::forward::r#static::buffer::initialize_byte_array_with(size, byte_array, initialize_with)
        })?;

        let individual_buffer = IndividualBuffer {
            imp: buffer,
            _marker: PhantomData,
            count: size,
        };
        let gpu_buffer = imp::GPUableBuffer::new(bound_device,byte_size, usage,debug_name);

        Ok(Self {
            shared: Arc::new(Shared {
                multibuffer: Multibuffer::new(individual_buffer, gpu_buffer),
            }),
            count: size,
        })
    }
    /**
    Dequeues a texture.  Resumes when a texture is available.
     */
    pub async fn access_read(&self) -> CPUReadGuard<IndividualBuffer<Element>> {
        self.shared.multibuffer.access_read().await
    }
    pub async fn access_write(&self) -> CPUWriteGuard<IndividualBuffer<Element>, imp::GPUableBuffer> {
        self.shared.multibuffer.access_write().await
    }

    pub(crate) fn gpu_dirty_receiver(&self) -> DirtyReceiver {
        todo!()
    }

    /**An opaque type that can be bound into a [crate::bindings::bind_style::BindStyle]. */
    pub fn render_side(&self) -> RenderSide<Element> {
        RenderSide {
            shared: self.shared.clone(),
            count: self.count,
        }
    }


}

/**
Implementing this trait guarantees the type has C layout.
*/
pub unsafe trait CRepr {

}

unsafe impl CRepr for u64 {}
unsafe impl CRepr for u32 {}
unsafe impl CRepr for u16 {}
unsafe impl CRepr for u8 {}
unsafe impl CRepr for f32 {}
unsafe impl CRepr for f64 {}
unsafe impl CRepr for i32 {}
unsafe impl CRepr for i16 {}
unsafe impl CRepr for i8 {}

