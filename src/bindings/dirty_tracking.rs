/*!
This module implements a "dirty tracking" system that can
allow waiting on an aggregation of resources for a dirty signal.

This differs from a channel as in this programming model, each resource can freely mutate
its own signal between clean/dirty, whereas it would be challenging to yank values from a channel.

Another distinction is that the receivers can be 'lately-bound' - that is, they can be bound after the sender is created.
*/

use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

// This represents the shared state between different LateBoundSenders
#[derive(Debug)]
struct SharedSend(Mutex<Option<r#continue::Sender<()>>>);

impl SharedSend {
    fn new(sender: Option<r#continue::Sender<()>>) -> Self {
        SharedSend(Mutex::new(sender))
    }
    fn r#continue(&self) {
        self.0.lock().unwrap().take().map(|t| t.send(()));
    }
    fn is_bound(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
}


#[derive(Debug)]
struct LateBoundSender(Mutex<Arc<SharedSend>>);

impl LateBoundSender {
    fn new() -> Self {
        LateBoundSender(Mutex::new(Arc::new(SharedSend::new(None))))
    }

    fn with_sender(sender: r#continue::Sender<()>) -> Self {
        LateBoundSender(Mutex::new(Arc::new(SharedSend::new(Some(sender)))))

    }

    fn bind(&self, other: &LateBoundSender) {
        // First check if the other sender is bound
        assert!(other.0.lock().unwrap().is_bound());
        // Replace our Arc with a clone of the other's Arc
        *self.0.lock().unwrap() = Arc::clone(&other.0.lock().unwrap());
    }

    fn r#continue(&self) {
        if let Ok(guard) = self.0.lock() {
            guard.r#continue();
        }
    }
}




#[derive(Debug)]
struct SharedSendReceive {
    //each dirty can be independently set and unset
    dirty: AtomicBool,
    continuation: LateBoundSender,
}

#[derive(Debug,Clone)]
pub struct DirtySender {
    shared: Arc<SharedSendReceive>,
}

impl DirtySender {
    pub fn new(dirty: bool) -> Self {
        let s = LateBoundSender::new();
        DirtySender {
            shared: Arc::new(SharedSendReceive {
                dirty: AtomicBool::new(dirty),
                continuation: s,
                }
            )
        }
    }
    pub fn mark_dirty(&self, dirty: bool) {
        self.shared.dirty.store(dirty, Ordering::Relaxed);
        if dirty {
            self.shared.continuation.r#continue();
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
        self.shared.continuation.bind(continuation);
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
        DirtyAggregateReceiver {
            receivers,
        }
    }

    ///Waits for a dirty signal.
    pub async fn wait_for_dirty(&self) {
        //todo!
        
        portable_async_sleep::async_sleep(Duration::from_millis(100)).await;
        return; //todo!
        /*let (sender, receiver) = r#continue::continuation();
        let late_bound_sender = LateBoundSender::with_sender(sender);

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
            }
        }
        //wait for next receiver!
        receiver.await;*/

    }
}
