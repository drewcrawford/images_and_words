/*!
This module implements a "dirty tracking" system that can
allow waiting on an aggregation of resources for a dirty signal.

This differs from a channel as in this programming model, each resource can freely mutate
its own signal between clean/dirty, whereas it would be challenging to yank values from a channel.

Another distinction is that the receivers can be 'lately-bound' - that is, they can be bound after the sender is created.
*/

#[derive(Debug,Clone)]
pub struct DirtySender {

}

impl DirtySender {
    pub fn new(dirty: bool) -> Self {
        DirtySender {

        }
    }
    pub fn mark_dirty(&self, dirty: bool) {
        todo!()
    }
}

pub struct DirtyReceiver {

}
impl DirtyReceiver {
    pub fn new(sender: &DirtySender) -> DirtyReceiver {
        DirtyReceiver {}
    }
}

pub struct DirtyAggregateReceiver {

}
impl DirtyAggregateReceiver {
    pub fn new(receivers: Vec<DirtyReceiver>) -> DirtyAggregateReceiver {
        DirtyAggregateReceiver {}
    }

    ///Waits for a dirty signal.
    pub async fn wait_for_dirty(&self) {
        todo!()
    }
}
