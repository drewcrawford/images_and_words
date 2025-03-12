/*!
This module implements a "dirty tracking" system that can
allow waiting on an aggregation of resources for a dirty signal.

This differs from a channel as in this programming model, each resource can freely mutate
its own signal between clean/dirty, whereas it would be challenging to yank values from a channel.

Another distinction is that the receivers can be 'lately-bound' - that is, they can be bound after the sender is created.
*/

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};



///Like [ContinueOnce], but shared and cloneable.
#[derive(Debug,Clone)]
struct SharedContinueOnce {
    /*
    We need:
    1.  Arc, for clone
    2.  Mutex, for interior mutability
    3.  Option, for late binding
    4.  Arc, for clone across related types
    5.  Option, for take
     */
    imp: Arc<Mutex<Option<Arc<Option<r#continue::Sender<()>>>>>>
}

impl SharedContinueOnce {
    fn r#continue(&self) {
        todo!()
    }
    fn blank() -> Self {
        SharedContinueOnce {
            imp: Arc::new(Mutex::new(None))
        }
    }
    fn new(sender: r#continue::Sender<()>) -> Self {
        SharedContinueOnce {
            imp: Arc::new(Mutex::new(Some(Arc::new(Some(sender)))))
        }
    }

    fn set_continuation(&self, other: &Self) {
        self.imp //arc
            .lock().unwrap() //mutex
            .replace( //late bind
                other.imp //OTHER: arc
                    .lock().unwrap() //OTHER: mutex
                    .clone() //clone
                    .expect("No continuation") //replace accepts T, not option
            );

    }
}


#[derive(Debug)]
struct SharedSendReceive {
    //each dirty can be independently set and unset
    dirty: AtomicBool,
    continuation: SharedContinueOnce,
}

#[derive(Debug,Clone)]
pub struct DirtySender {
    shared: Arc<SharedSendReceive>,
}

impl DirtySender {
    pub fn new(dirty: bool) -> Self {
        let (sender, receiver) = r#continue::continuation();
        let s = SharedContinueOnce::new(sender);
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
        self.shared.continuation.r#continue();
    }
}

pub struct DirtyReceiver {
    shared: Arc<SharedSendReceive>,
}
impl DirtyReceiver {
    pub fn new(sender: &DirtySender) -> DirtyReceiver {
        DirtyReceiver {
            shared: sender.shared.clone(),
        }
    }
    fn attach_continuation(&self, continuation: &SharedContinueOnce) {
        self.shared.continuation.set_continuation(continuation)
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
        let (sender, receiver) = r#continue::continuation();
        let continuation = SharedContinueOnce::new(sender);

        //set continuation up first
        for receiver in &self.receivers {
            receiver.attach_continuation(&continuation);
        }
        //now check dirty value
        for receiver in &self.receivers {
            if receiver.shared.dirty.load(Ordering::Relaxed) {
                return;
            }
        }
        //wait for next receiver!
        receiver.await;

    }
}
