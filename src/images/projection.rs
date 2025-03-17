use vectormatrix::matrix::Matrix;
use vectormatrix::vector::Vector;
const HI: f32 = 500.0;
const LO: f32 = -500.0;
/*

Moreover in many practical cases we want to push this question into GPU code, which means potentially profilerating it
or embedding it in the definitions of various shaders.

Instead what I think will happen is I'll just define the camera matrix in such a way that, when we multiply it by a coordinate in meters, we
get a sensible result.
 */

/*
Projection coordinates, a brief guide.

First, operations happen in reverse.  For exapmle if we have
M3 * M2 * M1 * M0 ...

it means M0 happens first, and then M1 is applied, and then M2, and then M3.  We call the rightward direction (towards M0) "pre" and
the leftward direction (towards M3) post.

Let's start with our world coordinates w_x,w_y,w_z.  This is a meter space, in which

z
 \
  x---------->
 |
 |
 y
 v

where z=0 is sea level, and z is between LO and HI.

Now we're going to define Mcamera.  This is a matrix which translates our world coordinates based on a camera position (also given in world coordiantes).

Suppose our camera moves down, right, and "into".  Then our world will move up, left, and "out of" e.g.

MCamera.x = world.x - camera.x
MCamera.y = world.y - camera.y
MCamera.z = world.z - camera.z

In matrix form, MCamera works as shown

|1 0 0 -camera.x| * |w_x|
|0 1 0 -camera.y| * |w_y|
|0 0 1 -camera.z| * |w_z|
|0 0 0         1| * |w_w|
*/

fn m_camera(camera_position: WorldCoord) -> Matrix<f32,4,4> {
    Matrix::new_rows([
        Vector::new([1.0,0.0,0.0,-1.0*camera_position.0.x()]),
        Vector::new([0.0,1.0,0.0,-1.0*camera_position.0.y()]),
        Vector::new([0.0,0.0,1.0,-1.0*camera_position.0.z()]),
        Vector::new([0.0,0.0,0.0,1.0])
        ]
    )
}

/*
To derive our fovs, we need a design rule.  Let's start with "the long side is fov_preferred."

This means we need to find the short side.  To do this, let's imagine a plane some distance d from the eye.
Then length of the part within the field of view, along the longside, is

length_longside = d * Tan[fov_preferred]

To find the short side, we multiply by the aspect ratio px_short/px_long

length_shortside = px_short/px_long * d * Tan[fov_preferred]

we also know that

Tan[fov_shortside] = Length_shortsize/d
fov_shortside = atan(length_shortside/d)

so

fov_shortside = Atan[px_short/px_long * Tan[preferred_pov]
 */

fn find_fov(w: u16, h: u16) -> (f32,f32) {
    const PREFERRED_FOV: f32 = 0.78;
    fn solve(short: u16,long: u16) -> f32 {
        ((short as f32) / (long as f32) * PREFERRED_FOV.tan()).atan().abs()
    }
    if w <= h {
        (solve(w,h),PREFERRED_FOV)
    }
    else {
        (PREFERRED_FOV, solve(h,w))
    }
}

/*

Some remarks about this.  First, this is wrong for either Metal or Vulkan, but we will fix that in post.

Secondly, although it purports to specify a near and far plane, the camera is at some position, distinct from either of them,
that seems to be at the origin.  This is the purpose of the MCamera matrix we discussed previously.

Finally we need to parameterize n,f.  In world coordinates they might be given as

n_w = HI
f_w = LO

Keep in mind though, we need to adjust all these to camera coordinates.  So actually

n = -HI + camera.z
f = -LO + camera.z

Putting all this together, we can chug a matrix MProj:
*/
fn m_proj(camera_z: f32, fov_x: f32, fov_y: f32) -> Matrix<f32,4,4> {
    //weird things happen if the near plane goes behind the camera.
    let n = (-HI+camera_z).max(0.0);
    let f = -LO + camera_z;
    opengl_projection_matrix(n,f,fov_x,fov_y)
}

/*

Now let's examine the OpenGL projection matrix (TM).  It is.


| 2n/(r-l)   0       (r+l)/(r-l)           0 |
| 0         2n/(t-b) (t+b)/(t-b)           0 |
| 0         0        -(f+n)/(f-n)  -2fn/(f-n)
| 0         0        -1                    0 |

r,l,t,b here are given on the near plane.

When we define this in terms of field of view theta_x,theta_y, we have
r=2*n*Tan[theta_x]
l=-r
t=2*n*Tan[theta_y]
b=-t

producing the simpler form


|Cot[thetax]/2             0            0            0|
|            0 Cot[thetay]/2            0            0|
|            0             0 (-f-n)/(f-n) -2*f*n/(f-n)|
|            0             0           -1            0|


*/

#[inline] fn opengl_projection_matrix(n: f32, f: f32, fov_x: f32, fov_y: f32 ) -> Matrix<f32,4,4> {
    let cot_thetax = 1.0 / fov_x.tan();
    let cot_thetay = 1.0 / fov_y.tan();

    Matrix::new_rows([
        Vector::new([cot_thetax/2.0, 0.0,0.0,0.0]),
        Vector::new([0.0,cot_thetay/2.0,0.0,0.0]),
        Vector::new([0.0,0.0,(-f-n)/(f-n), -2.0*f*n/(f-n)]),
        Vector::new([0.0,0.0,-1.0,0.0])
    ])
}


/*

For post, we simply want to get proj output into the 2x2x1 volume, MPost=

|1 0   0   0|
|0 1   0   0|
|0 0 1/2 1/2|
|0 0   0   1|

*/
const M_POST: Matrix<f32,4,4> = Matrix::new_columns([
    Vector::new([1.0,0.0,0.0,0.0]),
    Vector::new([0.0, 1.0, 0.0, 0.0]),
    Vector::new([0.0, 0.0, 0.5, 0.0]),
    Vector::new([0.0, 0.0, 0.5, 1.0]),
]
);

/*
Most helpful resources online are written for the OpenGL environment, so the next thing we we want to do is get into something
resembling the OpenGL space.

Which is
  ^
  |
  y
  x--------->
 /
z

to do this, let's define the opengl coordintes x_o, y_o, z_o.  we'll let the x/y origin be the same, so

x_o = x_w
y_o = -y_w
z_o = z_w

In matrix form, MToGL =

|1  0 0 0|
|0 -1 0 0|
|0  0 1 0|
|0  0 0 1|

*/
const M_TO_GL: Matrix<f32, 4,4> = Matrix::new_columns([
    Vector::new([1.0, 0.0, 0.0, 0.0]),
    Vector::new([0.0, -1.0, 0.0, 0.0]),
    Vector::new([0.0, 0.0, 1.0, 0.0]),
    Vector::new([0.0, 0.0, 0.0, 1.0])
    ]);


#[derive(Debug,Clone)]
pub struct Projection(pub(crate) Matrix<f32,4,4>);

impl Projection {
    pub fn new(camera_position: WorldCoord,w: u16,h:u16) -> Projection {
        let mcamera = m_camera(camera_position);
        let (fov_x,fov_y) = find_fov(w,h);

        let mproj = m_proj(*camera_position.0.z(), fov_x,fov_y);
        let r = M_POST.mul_matrix(mproj).mul_matrix(M_TO_GL).mul_matrix(mcamera);
        Projection(r)

    }

    pub fn project(self, _world_coord: WorldCoord) -> ScreenCoord {
        todo!()
    }
}

pub struct ScreenCoord;
#[derive(Debug,Clone,Copy)]
pub struct WorldCoord(pub(crate) Vector<f32,3>);
impl WorldCoord {
    pub fn new(x: f32, y: f32, z: f32) -> WorldCoord {
        WorldCoord(Vector::new([x,y,z]))
    }
}