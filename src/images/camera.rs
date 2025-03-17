/*! Camera utilities. */

use std::sync::{Arc, Mutex};
use vectormatrix::vector::Vector;
use crate::bindings::dirty_tracking::{DirtyReceiver, DirtySender};
use crate::images::projection::{Projection, WorldCoord};

///Shared data between cameras instances
#[derive(Debug,Clone)]
struct Shared {
    /*
When updating these, we also need to update the matrix.
 */
    window_size: (u16,u16),
    camera_position: WorldCoord,
    projection: Projection,
    dirty_sender: DirtySender,
}

impl Shared {
    fn rematrix(&mut self) {
        self.projection = Projection::new(self.camera_position, self.window_size.0, self.window_size.1);
    }
}

#[derive(Debug,Clone)]
pub struct Camera {
    shared: Arc<Mutex<Shared>>,
}
impl Camera {

    pub fn new(window_size: (u16,u16), initial_position: WorldCoord) -> Camera {
        let initial_projection = Projection::new(initial_position, window_size.0, window_size.1);
        Self {
            shared: Arc::new(Mutex::new(Shared {
                dirty_sender: DirtySender::new(false),
                window_size,
                camera_position: initial_position,
                projection: initial_projection,
            }))
        }
    }
    pub(crate) fn copy_projection_and_clear_dirty_bit(&self) -> Projection {
        let mut guard = self.shared.lock().unwrap();
        let result = guard.projection.clone();
        guard.dirty_sender.mark_dirty(false);
        result
    }
    pub(crate) fn projection(&self) -> Projection {
        let guard = self.shared.lock().unwrap();
        guard.projection.clone()
    }
    pub(crate) fn dirty_receiver(&self) -> DirtyReceiver {
        DirtyReceiver::new(&self.shared.lock().unwrap().dirty_sender)
    }

    pub fn translate(&mut self, translation: WorldCoord) {
        let mut guard = self.shared.lock().unwrap();
        guard.camera_position.0 = guard.camera_position.0 + translation.0;
        guard.rematrix();
    }

    pub fn changed_size(&mut self, new_size: (u16,u16)) {
        let mut guard = self.shared.lock().unwrap();
        guard.window_size = new_size;
        guard.rematrix();
    }
}