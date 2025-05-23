use vectormatrix::matrix::Matrix;
use vectormatrix::vector::Vector;
const DRAW_DIST: f32 = 600.0;


/**
Consider the world coordinates.

We have an origin at top left but it's somewhat nonintuitive
what this means on an infinite canvas.  However

* +x -> right
* +y -> down
* +z -> up (from sea level; out of the screen)

Now we need to construct a view matrix.  The idea here is instead of moving the camera to its position,
we move the object such that the camera is at the origin.

* camera moves right (+x) -> world moves left (-x)
* camera moves down (+y) -> world moves up (-y)
* camera moves up (+z) -> world moves down (-z)


So far the matrix we need is

```ignore
| 1  0  0 -x|
| 0  1  0 -y|
| 0  0  1 -z|
| 0  0  0  1|
```

Now we want to do some axis flips.
x is ok
y needs inverse for WebGPU.
z needs inverse

```ignore
| 1  0  0 0|
| 0 -1  0 0|
| 0  0 -1 0|
| 0  0  0 1|
```


An orthographic projection can be defined as

```ignore
| 1.0 / s  0   0          0        |
| 0        r/s 0          0        |
| 0        0   1.0/(f-n) -n/(f-n)  |
| 0        0   0          1        |
```

Where s is the scale factor, r is the aspect ratio, n is the near plane and f is the far plane.
s can be given by the formula `base_scale * abs(camera_z / z_ref);`
with e.g. base_scale = 1.0 and z_ref = 1.0.

Alternatively the projection matrix can be defined as

```ignore
| fl   0 0                   0 |
| 0 fl*r 0                   0 |
| 0    0    f/(f-n) -f*n/(f-n) |
| 0    0          1          0 |

where fl is the focal length, r is the aspect ratio, n is the near plane and f is the far plane.
```
*/

fn m_view(camera_position: WorldCoord) -> Matrix<f32,4,4> {
    Matrix::new_rows([Vector::new([1.0, 0.0, 0.0, -camera_position.0.x()]),
                        Vector::new([0.0, 1.0, 0.0, -camera_position.0.y()]),
                        Vector::new([0.0, 0.0, 1.0, -camera_position.0.z()]),
                        Vector::new([0.0, 0.0, 0.0, 1.0])])
}

fn flip() -> Matrix<f32,4,4> {
    Matrix::new_rows([Vector::new([1.0, 0.0, 0.0, 0.0]),
                      Vector::new([0.0, -1.0, 0.0, 0.0]),
                      Vector::new([0.0, 0.0, -1.0, 0.0]),
                      Vector::new([0.0, 0.0, 0.0, 1.0])])
}

fn m_ortho(camera_position: WorldCoord, w: u16, h: u16) -> Matrix<f32,4,4> {
    let aspect_ratio = w as f32 / h as f32;
    let base_scale = 1.0; //higher numbers zoom out
    let z_ref = 2.0; //lower numbers zoom out
    let scale = base_scale * (camera_position.0.z() / z_ref).abs();
    let near = 0.1;
    let far = DRAW_DIST;
    Matrix::new_rows([Vector::new([1.0 / scale, 0.0, 0.0, 0.0]),
                        Vector::new([0.0, aspect_ratio / scale, 0.0, 0.0]),
                        Vector::new([0.0, 0.0, 1.0 / (far - near), -near / (far - near)]),
                        Vector::new([0.0, 0.0, 0.0, 1.0])])
}

fn m_proj(w: u16, h: u16) -> Matrix<f32,4,4> {
    let focal_length = 2.0; //lower numbers zoom out; I believe 2.0 is "natural"
    let near = 1.0;
    let far = DRAW_DIST;
    Matrix::new_rows([
        Vector::new([focal_length, 0.0, 0.0, 0.0]),
        Vector::new([0.0, focal_length * (w as f32 / h as f32), 0.0, 0.0]),
        Vector::new([0.0, 0.0, far / ( far - near), -far * near / (far - near)]),
        Vector::new([0.0, 0.0, 1.0, 0.0])
    ])
}



#[derive(Debug,Clone)]
pub struct Projection {
    matrix: Matrix<f32,4,4>,
    width: u16,
    height: u16,
}

impl Projection {
    pub fn new(camera_position: WorldCoord,w: u16,h:u16) -> Projection {
        let m_view = m_view(camera_position);

         // let proj = m_ortho(camera_position,w,h);
        let proj = m_proj(w,h);
        let r =  proj * flip() * m_view;
        Projection {
            matrix: r,
            width: w,
            height: h,
        }
    }

    pub fn project(self, world_coord: WorldCoord) -> ScreenCoord {
        // Convert WorldCoord to homogeneous coordinates (add w=1.0)
        let world_homogeneous = Vector::new([
            *world_coord.0.x(),
            *world_coord.0.y(),
            *world_coord.0.z(),
            1.0
        ]);
        
        // Apply projection matrix
        let projected = self.matrix * world_homogeneous;
        
        // Perform perspective divide (divide by w component)
        let w = *projected.columns()[0].w();
        if w == 0.0 {
            // Handle degenerate case
            return ScreenCoord { x: 0.0, y: 0.0 };
        }
        
        let ndc_x = projected.columns()[0].x() / w;
        let ndc_y = projected.columns()[0].y() / w;
        
        // Convert NDC [-1, 1] to screen coordinates [0, width] x [0, height]
        // Note: NDC y+ is up, screen y+ is down, so we flip y
        let screen_x = (ndc_x + 1.0) * (self.width as f32) / 2.0;
        let screen_y = (-ndc_y + 1.0) * (self.height as f32) / 2.0;
        
        ScreenCoord {
            x: screen_x,
            y: screen_y,
        }
    }

    pub fn matrix(&self) -> Matrix<f32,4,4> {
        self.matrix
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenCoord {
    pub x: f32,
    pub y: f32,
}
#[derive(Debug,Clone,Copy)]
pub struct WorldCoord(pub(crate) Vector<f32,3>);
impl WorldCoord {
    pub fn new(x: f32, y: f32, z: f32) -> WorldCoord {
        WorldCoord(Vector::new([x,y,z]))
    }
}