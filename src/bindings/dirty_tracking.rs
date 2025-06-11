// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*!
This module implements a "dirty tracking" system that can
allow waiting on an aggregation of resources for a dirty signal.

This differs from a channel as in this programming model, each resource can freely mutate
its own signal between clean/dirty, whereas it would be challenging to yank values from a channel.

Another distinction is that the receivers can be 'lately-bound' - that is, they can be bound after the sender is created.
*/

use std::hash::Hash;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// This represents the shared state between different LateBoundSenders
#[derive(Debug)]
struct SharedSend(Mutex<Option<r#continue::Sender<()>>>);

impl SharedSend {
    fn new() -> Self {
        SharedSend(Mutex::new(None))
    }

    fn set_sender(&self, sender: r#continue::Sender<()>) {
        *self.0.lock().unwrap() = Some(sender);
    }

    fn r#continue(&self) {
        if let Some(sender) = self.0.lock().unwrap().take() {
            sender.send(());
        }
    }
}

#[derive(Debug)]
struct LateBoundSender(Arc<SharedSend>);

impl LateBoundSender {
    fn new() -> Self {
        LateBoundSender(Arc::new(SharedSend::new()))
    }

    fn set_sender(&self, sender: r#continue::Sender<()>) {
        self.0.set_sender(sender);
    }

    fn clone_shared(&self) -> Self {
        LateBoundSender(Arc::clone(&self.0))
    }

    fn r#continue(&self) {
        self.0.r#continue();
    }
}

#[derive(Debug)]
struct SharedSendReceive {
    //each dirty can be independently set and unset
    dirty: AtomicBool,
    continuation: Mutex<LateBoundSender>,
}

#[derive(Debug, Clone)]
pub struct DirtySender {
    shared: Arc<SharedSendReceive>,
}

impl DirtySender {
    pub fn new(dirty: bool) -> Self {
        let s = LateBoundSender::new();
        DirtySender {
            shared: Arc::new(SharedSendReceive {
                dirty: AtomicBool::new(dirty),
                continuation: Mutex::new(s),
            }),
        }
    }
    pub fn mark_dirty(&self, dirty: bool) {
        self.shared.dirty.store(dirty, Ordering::Relaxed);
        if dirty {
            if let Ok(continuation) = self.shared.continuation.lock() {
                continuation.r#continue();
            }
        }
    }
}

#[derive(Debug)]
pub struct DirtyReceiver {
    shared: Arc<SharedSendReceive>,
}
impl DirtyReceiver {
    pub fn new(sender: &DirtySender) -> DirtyReceiver {
        DirtyReceiver {
            shared: sender.shared.clone(),
        }
    }
    fn attach_continuation(&self, continuation: &LateBoundSender) {
        if let Ok(mut cont) = self.shared.continuation.lock() {
            *cont = continuation.clone_shared();
        }
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
impl DirtyAggregateReceiver {
    pub fn new(receivers: Vec<DirtyReceiver>) -> DirtyAggregateReceiver {
        DirtyAggregateReceiver { receivers }
    }

    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        // Check if any of the receivers are dirty
        self.receivers
            .iter()
            .any(|receiver| receiver.shared.dirty.load(Ordering::Relaxed))
    }

    ///Waits for a dirty signal.
    pub async fn wait_for_dirty(&self) {
        // it used to work like this (just a test implementation that correctly renders frames every 100ms)
        // portable_async_sleep::async_sleep(Duration::from_millis(100)).await;
        // return;
        //instead let's use real dirty tracking to wait for the next frame

        let (sender, receiver) = r#continue::continuation();
        let late_bound_sender = LateBoundSender::new();
        late_bound_sender.set_sender(sender);

        //set continuation up first
        for dirty_receiver in &self.receivers {
            dirty_receiver.attach_continuation(&late_bound_sender);
            // println!("attached continuation {late_bound_sender:?} to receiver {dirty_receiver:?}");
        }
        // println!("Attached all continuations");
        //now check dirty value
        for receiver in &self.receivers {
            if receiver.shared.dirty.load(Ordering::Relaxed) {
                //mark future as ready to go immediately; this ensures we don't miss any messages
                late_bound_sender.r#continue();
                break; // Only need one to trigger
            }
        }
        // println!("Waiting for continuation");
        //wait for next receiver!
        receiver.await;
        // println!("Continuation received");
    }
}
