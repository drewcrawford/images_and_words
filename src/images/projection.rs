use vectormatrix::vector::Vector;

#[derive(Debug,Clone)]
pub struct Projection;

impl Projection {
    pub fn new(_camera_position: Vector<f32, 3>,_w: u16,_h:u16) -> Projection {
        Projection
    }
}