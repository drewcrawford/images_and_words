// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*!
This module implements a "dirty tracking" system that can
allow waiting on an aggregation of resources for a dirty signal.

This differs from a channel as in this programming model, each resource can freely mutate
its own signal between clean/dirty, whereas it would be challenging to yank values from a channel.

Another distinction is that the receivers can be 'lately-bound' - that is, they can be bound after the sender is created.
*/

use r#continue::Sender;
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct OneShot {
    c: Arc<AtomicPtr<r#continue::Sender<()>>>,
}
impl Drop for OneShot {
    fn drop(&mut self) {
        self.send_if_needed();
    }
}
impl OneShot {
    fn new(sender: r#continue::Sender<()>) -> Self {
        OneShot {
            c: Arc::new(AtomicPtr::new(Box::into_raw(Box::new(sender)))),
        }
    }

    fn send_if_needed(&mut self) {
        let swap = self.c.swap(std::ptr::null_mut(), Ordering::Relaxed);
        if !swap.is_null() {
            //we have a continuation, so we need to send it
            let boxed_sender = unsafe { Box::from_raw(swap) };
            boxed_sender.send(()); // send the signal
        }
    }
}

const CLEAN_PTR: *mut OneShot = std::ptr::null_mut();
const DIRTY_PTR: *mut OneShot = 1 as *mut OneShot;

struct SendState {
    //semantics:
    //if CLEAN_PTR, then the state is clean
    //if DIRTY_PTR, then the state is dirty
    //if some other pointer, then the state is clean and the pointer is a continuation to
    // be called when the state becomes dirty.
    dirty: AtomicPtr<OneShot>,
}

impl SendState {
    fn dirty_or_continue(&self, one_shot: &OneShot) -> Result<(), ()> {
        let mut load_continuations: *mut OneShot = std::ptr::null_mut();
        //we don't particularly need lhs here
        let mut dirty_detected = false;
        _ = self
            .dirty
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |current| {
                if current == DIRTY_PTR {
                    dirty_detected = true;
                    None
                } else if current == CLEAN_PTR {
                    let c = one_shot.clone();
                    Some(Box::into_raw(Box::new(c))) // mark as dirty and store continuation
                } else {
                    load_continuations = current;
                    //swap the current pointer with a new one
                    let c = one_shot.clone();
                    Some(Box::into_raw(Box::new(c))) // mark as dirty and store continuation
                }
            });

        if load_continuations != std::ptr::null_mut() {
            //we had a continuation, so we need to send it
            let mut boxed_continuation = unsafe { Box::from_raw(load_continuations) };
            boxed_continuation.send_if_needed(); //why not
        }
        if dirty_detected {
            Ok(()) // dirty
        } else {
            Err(()) // clean, but we had a continuation
        }
    }
    fn new(dirty: bool) -> Self {
        SendState {
            dirty: AtomicPtr::new(if dirty { DIRTY_PTR } else { CLEAN_PTR }),
        }
    }
    fn mark_dirty(&self) {
        let mut load_continuations: *mut OneShot = std::ptr::null_mut();
        //we don't particularly need lhs here
        _ = self
            .dirty
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |current| {
                if current == CLEAN_PTR {
                    Some(DIRTY_PTR) // mark as dirty
                } else if current == DIRTY_PTR {
                    None // already dirty, do nothing
                } else {
                    load_continuations = current;
                    Some(DIRTY_PTR)
                }
            });
        if load_continuations != std::ptr::null_mut() {
            //we had a continuation, so we need to send it
            let mut boxed_continuation = unsafe { Box::from_raw(load_continuations) };
            boxed_continuation.send_if_needed();
        }
    }

    fn mark_clean(&self) {
        _ = self
            .dirty
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |current| {
                if current == DIRTY_PTR {
                    Some(CLEAN_PTR) // mark as clean
                } else if current == CLEAN_PTR {
                    None // already clean, do nothing
                } else {
                    //leave the continuation as is
                    None
                }
            });
    }
    fn is_dirty(&self) -> bool {
        let current = self.dirty.load(Ordering::Relaxed);
        if current == DIRTY_PTR {
            true // dirty
        } else if current == CLEAN_PTR {
            false // clean
        } else {
            // if it's some other pointer, it means we have a continuation that is not yet called
            // so we consider it clean until the continuation is called
            false
        }
    }
}

struct SharedSendReceive {
    debug_label: String,
    send_state: SendState,
}
#[derive(Clone)]
pub struct DirtySender {
    shared: Arc<SharedSendReceive>,
}

impl std::fmt::Debug for DirtySender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirtySender")
            .field("label", &self.shared.debug_label)
            .finish()
    }
}

impl DirtySender {
    pub fn new(dirty: bool, label: impl Into<String>) -> Self {
        DirtySender {
            shared: Arc::new(SharedSendReceive {
                debug_label: label.into(),
                send_state: SendState::new(dirty),
            }),
        }
    }
    pub fn mark_dirty(&self, dirty: bool) {
        if dirty {
            self.shared.send_state.mark_dirty();
        } else {
            self.shared.send_state.mark_clean();
        }
    }
}

pub struct DirtyReceiver {
    shared: Arc<SharedSendReceive>,
}

impl std::fmt::Debug for DirtyReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirtyReceiver")
            .field("label", &self.shared.debug_label)
            .finish()
    }
}
impl DirtyReceiver {
    pub fn new(sender: &DirtySender) -> DirtyReceiver {
        DirtyReceiver {
            shared: sender.shared.clone(),
        }
    }

    pub fn debug_label(&self) -> &str {
        &self.shared.debug_label
    }
    pub fn is_dirty(&self) -> bool {
        self.shared.send_state.is_dirty()
    }
}

impl PartialEq for DirtyReceiver {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
}
impl Eq for DirtyReceiver {}

impl Hash for DirtyReceiver {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.shared).hash(state);
    }
}

pub struct DirtyAggregateReceiver {
    receivers: Vec<DirtyReceiver>,
}

impl std::fmt::Debug for DirtyAggregateReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let labels: Vec<&str> = self.receivers.iter().map(|r| r.debug_label()).collect();
        f.debug_struct("DirtyAggregateReceiver")
            .field("receivers", &labels)
            .finish()
    }
}
impl DirtyAggregateReceiver {
    pub fn new(receivers: Vec<DirtyReceiver>) -> DirtyAggregateReceiver {
        DirtyAggregateReceiver { receivers }
    }

    pub fn who_is_dirty(&self) -> Vec<&str> {
        self.receivers
            .iter()
            .filter(|receiver| receiver.is_dirty())
            .map(|receiver| receiver.debug_label())
            .collect()
    }

    ///Waits for a dirty signal.
    pub async fn wait_for_dirty(&self) {
        // it used to work like this (just a test implementation that correctly renders frames every 100ms)
        // portable_async_sleep::async_sleep(Duration::from_millis(100)).await;
        // return;
        //instead let's use real dirty tracking to wait for the next frame

        let (sender, receiver) = r#continue::continuation();
        let o = OneShot::new(sender);

        for receiver in &self.receivers {
            let shared = &receiver.shared;
            match shared.send_state.dirty_or_continue(&o) {
                Ok(()) => {
                    //we're dirty
                    return;
                }
                Err(continuation) => {
                    // we have a continuation, so we need to wait for it
                }
            }
            //next receiver
        }
        receiver.await;
        // println!("Continuation received");
    }
}
