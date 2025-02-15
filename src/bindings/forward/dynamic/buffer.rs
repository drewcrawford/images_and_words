/*! Dynamic buffer implementation.

A dynamic buffer is data we expect to change dynamically.
It is not necessarily any frame, the exact optimizations are passed by argument.
*/
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use crate::bindings::visible_to::CPUStrategy;
use crate::images::BoundDevice;
use crate::imp;
use crate::multibuffer::{multibuffer, Producer, ProducerWriteGuard, Receiver, ReceiverReadGuard};

pub enum WriteFrequency {
    ///Significantly less than once per frame.
    Infrequent,
}
pub struct Buffer<Element> {
    producer: Producer<imp::Product<Element>>,
    render_side: Option<RenderSide>,
}
#[derive(Debug)]
pub struct RenderSide {
    receiver: Receiver<imp::Delivery>
}
impl RenderSide {
    pub(crate) fn dequeue(&mut self) -> GPUBorrow {
        GPUBorrow(self.receiver.receive())
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
pub struct CPUAccess<Element>(ProducerWriteGuard<imp::Product<Element>>);
impl<Element> Deref for CPUAccess<Element> {
    type Target = imp::Product<Element>;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
impl<Element> DerefMut for CPUAccess<Element> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}

impl<Element> Buffer<Element> {
    pub fn new<I: Fn(usize) -> Element>(bound_device: &Arc<BoundDevice>, size: usize, write_frequency: WriteFrequency, cpu_strategy: CPUStrategy, debug_name: &str, initialize_with:I) -> Self {
        let products = imp::Product::new(bound_device, size, write_frequency, cpu_strategy, debug_name, initialize_with);
        let (producer,receiver) = multibuffer(products);
        Self {
            producer,
            render_side: Some(RenderSide {
                receiver,
            })
        }
    }
    /**
    Dequeues a texture.  Resumes when a texture is available.
     */
    pub fn dequeue<'s>(&'s mut self) -> impl Future<Output=CPUAccess<Element>> + 's where Element: Send {
        async {
            let guard = self.producer.borrow_write().await;
            CPUAccess(guard)
        }
    }
    /**An opaque type that can be bound into a [crate::bindings::bind_style::BindStyle]. */
    pub fn render_side(&mut self) -> RenderSide {
        self.render_side.take().unwrap()
    }

    /**
    Returns the CPUAccess and marks the contents as ready for GPU submission.

    The renderloop will generally re-use each buffer until the next buffer is submitted.
    In this way failing to keep up will not drop the framerate (although it may block your subsystem).

    There is currently no support for atomically submitting two different textures together, mt2-471.
     */
    pub fn submit<'s>(&'s mut self, cpu_access: CPUAccess<Element>) -> impl Future<Output=()> + 's {
        self.producer.submit(cpu_access.0)
    }
}