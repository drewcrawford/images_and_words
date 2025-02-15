/*! Camera utilities. */

use std::sync::{Arc, Mutex};
use vectormatrix::vector::Vector;
use crate::images::projection::Projection;

#[derive(Debug)]
pub struct Camera {
    /*
    When updating these, we also need to update the matrix.
     */
    window_size: (u16,u16),
    camera_position: Vector<f32, 3>,
    projection: Arc<Mutex<Projection>>,
}
impl Camera {
    // /**Poll for new movement. */
    // pub fn poll_movement(&mut self, poller: &Poller, release_pool: &ReleasePool) {
    //     let mut camera_translation = Float3::ZERO;
    //     if poller.is_pressed(Command::CameraUp) {
    //         *camera_translation.y_mut() += -1.0;
    //     }
    //     if poller.is_pressed(Command::CameraDown) {
    //         *camera_translation.y_mut() += 1.0;
    //     }
    //     if poller.is_pressed(Command::CameraLeft) {
    //         *camera_translation.x_mut() += -1.0;
    //     }
    //     if poller.is_pressed(Command::CameraRight) {
    //         *camera_translation.x_mut() += 1.0;
    //     }
    //     if poller.is_pressed(Command::CameraIn) {
    //         *camera_translation.z_mut() += -1.0;
    //     }
    //     if poller.is_pressed(Command::CameraOut) {
    //         *camera_translation.z_mut() += 1.0;
    //     }
    //     if camera_translation != Float3::ZERO {
    //         self.camera_position = self.camera_position.elementwise_add(camera_translation);
    //         self.rematrix()
    //     }
    // }
    fn rematrix(&mut self) {
        self.projection = Arc::new(Mutex::new(Projection::new(self.camera_position, self.window_size.0, self.window_size.1)));
    }
    pub fn new(window_size: (u16,u16), initial_position: Vector<f32, 3>) -> Camera {
        let initial_projection = Projection::new(initial_position, window_size.0, window_size.1);
        Self {

            window_size: window_size,
            camera_position: initial_position,
            projection: Arc::new(Mutex::new(initial_projection)),
        }
    }
    pub const fn projection(&self) -> &Arc<Mutex<Projection>> {
        &self.projection
    }
    pub fn changed_size(&mut self, new_size: (u16,u16)) {
        self.window_size = new_size;
        self.rematrix();
    }
}