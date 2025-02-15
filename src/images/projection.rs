use vectormatrix::vector::Vector;

#[derive(Debug,Clone)]
pub struct Projection;

impl Projection {
    pub fn new(_camera_position: WorldCoord,_w: u16,_h:u16) -> Projection {
        Projection
    }

    pub fn project(self, _world_coord: WorldCoord) -> ScreenCoord {
        todo!()
    }
}

pub struct ScreenCoord;
#[derive(Debug,Clone,Copy)]
pub struct WorldCoord;
impl WorldCoord {
    pub fn new(x: f32, y: f32, z: f32) -> WorldCoord {
        WorldCoord
    }
}