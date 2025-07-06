// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*!
This module implements a "dirty tracking" system that can
allow waiting on an aggregation of resources for a dirty signal.

This differs from a channel as in this programming model, each resource can freely mutate
its own signal between clean/dirty, whereas it would be challenging to yank values from a channel.

Another distinction is that the receivers can be 'lately-bound' - that is, they can be bound after the sender is created.
*/

use std::hash::Hash;
use std::sync::{Arc, Mutex};

struct OneShot {
    c: Option<r#continue::Sender<()>>,
}
impl Drop for OneShot {
    fn drop(&mut self) {
        // continue API requires us to send from all senders
        if let Some(sender) = self.c.take() {
            sender.send(()); //send a signal to avoid deadlocks
        }
    }
}
impl OneShot {
    fn new() -> Self {
        OneShot { c: None }
    }
    fn set(&mut self, sender: r#continue::Sender<()>) {
        self.c = Some(sender);
    }
}

struct SendState {
    dirty: bool,
    continuation: Arc<Mutex<OneShot>>,
}
struct SharedSendReceive {
    debug_label: String,
    send_state: Mutex<SendState>,
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
                send_state: Mutex::new(SendState {
                    dirty,
                    continuation: Arc::new(Mutex::new(OneShot::new())),
                }),
            }),
        }
    }
    pub fn mark_dirty(&self, dirty: bool) {
        let mut l = self.shared.send_state.lock().unwrap();
        l.dirty = dirty;
        if dirty {
            let continuation = l.continuation.lock().unwrap().c.take();
            drop(l); //allow lock to be retaken
            //send continuation if it exists
            continuation.map(|c| c.send(()));
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
        self.shared.send_state.lock().unwrap().dirty
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

    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        // Check if any of the receivers are dirty
        self.receivers.iter().any(|receiver| receiver.is_dirty())
    }

    ///Waits for a dirty signal.
    pub async fn wait_for_dirty(&self) {
        // it used to work like this (just a test implementation that correctly renders frames every 100ms)
        // portable_async_sleep::async_sleep(Duration::from_millis(100)).await;
        // return;
        //instead let's use real dirty tracking to wait for the next frame

        let (sender, receiver) = r#continue::continuation();
        let o = Arc::new(Mutex::new(OneShot::new()));
        o.lock().unwrap().set(sender);

        for receiver in &self.receivers {
            let shared = receiver.shared.clone();
            let mut send_state = shared.send_state.lock().unwrap();
            if send_state.dirty {
                // we can return immediately
                return;
            } else {
                // otherwise we need to set up a continuation
                send_state.continuation = o.clone();
            }
            //next receiver
        }
        receiver.await;
        // println!("Continuation received");
    }
}
