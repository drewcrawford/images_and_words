// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! 3D to 2D projection transformations for rendering.
//!
//! This module provides the mathematical foundations for converting 3D world coordinates
//! into 2D screen coordinates using perspective projection. It handles camera positioning,
//! view transformations, and projection matrices.
//!
//! # Coordinate System
//!
//! The world coordinate system follows these conventions:
//! - **+X**: Right
//! - **+Y**: Down
//! - **+Z**: Up (out of the screen)
//!
//! The origin is at the top-left of the infinite canvas.
//!
//! # Example
//!
//! ```
//! use images_and_words::images::projection::{Projection, WorldCoord};
//!
//! // Set up camera position
//! let camera_position = WorldCoord::new(0.0, 0.0, 100.0);
//!
//! // Create projection for 800x600 viewport
//! let projection = Projection::new(camera_position, 800, 600, 1.0);
//!
//! // Project a 3D point to screen coordinates
//! let world_point = WorldCoord::new(50.0, 30.0, 10.0);
//! let screen_coord = projection.project(world_point);
//! ```
//!
//! # Complete Example
//!
//! ```
//! use images_and_words::images::projection::{Projection, WorldCoord, ScreenCoord};
//!
//! // Camera looking down from above at an angle
//! let camera = WorldCoord::new(0.0, -50.0, 200.0);
//! let projection = Projection::new(camera, 1920, 1080, 1.0);
//!
//! // Project multiple points forming a square on the ground (z=0)
//! let corners = [
//!     WorldCoord::new(-50.0, -50.0, 0.0),  // top-left
//!     WorldCoord::new(50.0, -50.0, 0.0),   // top-right
//!     WorldCoord::new(50.0, 50.0, 0.0),    // bottom-right
//!     WorldCoord::new(-50.0, 50.0, 0.0),   // bottom-left
//! ];
//!
//! for corner in corners {
//!     let screen_pos = projection.clone().project(corner);
//!     // Each corner is projected to screen coordinates
//! }
//! ```

use vectormatrix::matrix::Matrix;
use vectormatrix::vector::Vector;
const DRAW_DIST: f32 = 600.0;

// Internal implementation notes:
//
// View Matrix Construction:
// The view matrix transforms world coordinates so that the camera is at the origin.
// - Camera moves right (+x) -> world moves left (-x)
// - Camera moves down (+y) -> world moves up (-y)
// - Camera moves up (+z) -> world moves down (-z)
//
// Base view matrix:
// | 1  0  0 -x|
// | 0  1  0 -y|
// | 0  0  1 -z|
// | 0  0  0  1|
//
// Axis flip matrix for WebGPU compatibility:
// | 1  0  0 0|
// | 0 -1  0 0|  (y inverted for WebGPU)
// | 0  0 -1 0|  (z inverted)
// | 0  0  0 1|
//
// Orthographic projection matrix:
// | 1.0/s  0   0          0        |
// | 0      r/s 0          0        |
// | 0      0   1.0/(f-n) -n/(f-n)  |
// | 0      0   0          1        |
//
// Where s = scale factor (base_scale * abs(camera_z / z_ref))
//       r = aspect ratio, n = near plane, f = far plane
//
// Perspective projection matrix:
// | fl   0 0                   0 |
// | 0 fl*r 0                   0 |
// | 0    0    f/(f-n) -f*n/(f-n) |
// | 0    0          1          0 |
//
// where fl = focal length, r = aspect ratio, n = near plane, f = far plane

/// Constructs a view matrix that transforms world coordinates relative to the camera position.
///
/// The view matrix effectively moves the world so that the camera is at the origin,
/// which is the inverse of the camera's position transformation.
fn m_view(camera_position: WorldCoord) -> Matrix<f32, 4, 4> {
    Matrix::new_rows([
        Vector::new([1.0, 0.0, 0.0, -camera_position.0.x()]),
        Vector::new([0.0, 1.0, 0.0, -camera_position.0.y()]),
        Vector::new([0.0, 0.0, 1.0, -camera_position.0.z()]),
        Vector::new([0.0, 0.0, 0.0, 1.0]),
    ])
}

/// Creates a matrix that flips the Y and Z axes for WebGPU compatibility.
///
/// WebGPU uses a different coordinate system than our world coordinates,
/// requiring Y and Z axes to be inverted.
fn flip() -> Matrix<f32, 4, 4> {
    Matrix::new_rows([
        Vector::new([1.0, 0.0, 0.0, 0.0]),
        Vector::new([0.0, -1.0, 0.0, 0.0]),
        Vector::new([0.0, 0.0, -1.0, 0.0]),
        Vector::new([0.0, 0.0, 0.0, 1.0]),
    ])
}

// Orthographic projection matrix (unused, kept for debugging/reference)
// fn m_ortho(camera_position: WorldCoord, w: u16, h: u16) -> Matrix<f32,4,4> {
//     let aspect_ratio = w as f32 / h as f32;
//     let base_scale = 1.0; // higher numbers zoom out
//     let z_ref = 2.0; // lower numbers zoom out
//     let scale = base_scale * (camera_position.0.z() / z_ref).abs();
//     let near = 0.1;
//     let far = DRAW_DIST;
//     Matrix::new_rows([Vector::new([1.0 / scale, 0.0, 0.0, 0.0]),
//                         Vector::new([0.0, aspect_ratio / scale, 0.0, 0.0]),
//                         Vector::new([0.0, 0.0, 1.0 / (far - near), -near / (far - near)]),
//                         Vector::new([0.0, 0.0, 0.0, 1.0])])
// }
/// Creates a perspective projection matrix.
///
/// # Parameters
/// - `w`: Viewport width
/// - `h`: Viewport height
///
/// Uses a focal length of 2.0 which provides a natural field of view.
/// Lower focal length values result in wider field of view (more zoom out).
fn m_proj(w: u16, h: u16) -> Matrix<f32, 4, 4> {
    let focal_length = 2.0; // lower numbers zoom out; 2.0 is "natural"
    let near = 1.0;
    let far = DRAW_DIST;
    Matrix::new_rows([
        Vector::new([focal_length, 0.0, 0.0, 0.0]),
        Vector::new([0.0, focal_length * (w as f32 / h as f32), 0.0, 0.0]),
        Vector::new([0.0, 0.0, far / (far - near), -far * near / (far - near)]),
        Vector::new([0.0, 0.0, 1.0, 0.0]),
    ])
}

/// Represents a 3D to 2D projection transformation.
///
/// Combines view and projection matrices to transform world coordinates
/// into screen coordinates. The projection uses perspective transformation
/// with configurable camera position and viewport dimensions.
///
/// # Example
///
/// ```
/// use images_and_words::images::projection::{Projection, WorldCoord};
///
/// let camera = WorldCoord::new(0.0, 0.0, 50.0);
/// let projection = Projection::new(camera, 1920, 1080, 1.0);
///
/// // Get the combined transformation matrix
/// let matrix = projection.matrix();
/// assert_eq!(projection.width(), 1920);
/// assert_eq!(projection.height(), 1080);
/// ```
#[derive(Debug, Clone)]
pub struct Projection {
    matrix: Matrix<f32, 4, 4>,
    width: u16,
    height: u16,
    //scale: f64,
}

impl Projection {
    /// Creates a new projection with the specified camera position and viewport dimensions.
    ///
    /// # Arguments
    /// * `camera_position` - The position of the camera in world coordinates
    /// * `w` - Viewport width in pixels
    /// * `h` - Viewport height in pixels
    /// * `_scale` - Currently unused scale parameter. Note that the projection mathematics
    ///   primarily depend on the aspect ratio (w/h) rather than absolute pixel dimensions,
    ///   making the projection scale-independent across different screen resolutions.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::projection::{Projection, WorldCoord};
    ///
    /// let camera = WorldCoord::new(10.0, 20.0, 100.0);
    /// let projection = Projection::new(camera, 800, 600, 1.0);
    /// ```
    pub fn new(camera_position: WorldCoord, w: u16, h: u16, _scale: f64) -> Projection {
        let m_view = m_view(camera_position);

        // let proj = m_ortho(camera_position,w,h);
        let proj = m_proj(w, h);
        let r = proj * flip() * m_view;
        Projection {
            matrix: r,
            width: w,
            height: h,
            //scale,
        }
    }

    /// Projects a 3D world coordinate to 2D screen coordinates.
    ///
    /// Applies the combined view-projection transformation and performs
    /// perspective division to convert from 3D world space to 2D screen space.
    ///
    /// # Arguments
    /// * `world_coord` - The 3D point to project
    ///
    /// # Returns
    /// A `ScreenCoord` with x,y coordinates in screen space where:
    /// - (0,0) is the top-left corner
    /// - (width,height) is the bottom-right corner
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::projection::{Projection, WorldCoord};
    ///
    /// let camera = WorldCoord::new(0.0, 0.0, 100.0);
    /// let projection = Projection::new(camera, 800, 600, 1.0);
    ///
    /// // Project a point in front of the camera
    /// let world_point = WorldCoord::new(0.0, 0.0, 50.0);
    /// let screen = projection.project(world_point);
    /// // Point should be near center of screen
    /// ```
    pub fn project(self, world_coord: WorldCoord) -> ScreenCoord {
        // Convert WorldCoord to homogeneous coordinates (add w=1.0)
        let world_homogeneous = Vector::new([
            *world_coord.0.x(),
            *world_coord.0.y(),
            *world_coord.0.z(),
            1.0,
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

    /// Returns the combined view-projection transformation matrix.
    ///
    /// This matrix can be used directly in shaders or for batch transformations.
    pub fn matrix(&self) -> Matrix<f32, 4, 4> {
        self.matrix
    }

    /// Returns the viewport width in pixels.
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Returns the viewport height in pixels.
    pub fn height(&self) -> u16 {
        self.height
    }
}

/// Represents a 2D coordinate on the screen.
///
/// Screen coordinates use the standard 2D graphics convention where:
/// - (0, 0) is the top-left corner
/// - x increases to the right
/// - y increases downward
///
/// # Example
///
/// ```
/// use images_and_words::images::projection::ScreenCoord;
///
/// let point = ScreenCoord { x: 100.5, y: 200.3 };
/// println!("Screen position: ({}, {})", point.x, point.y);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ScreenCoord {
    /// Horizontal position in pixels from the left edge
    pub x: f32,
    /// Vertical position in pixels from the top edge
    pub y: f32,
}

/// Represents a 3D coordinate in world space.
///
/// World coordinates follow these conventions:
/// - **+X**: Right
/// - **+Y**: Down
/// - **+Z**: Up (out of the screen)
///
/// # Example
///
/// ```
/// use images_and_words::images::projection::WorldCoord;
///
/// // Create a point 10 units right, 5 units down, 20 units up
/// let position = WorldCoord::new(10.0, 5.0, 20.0);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct WorldCoord(pub(crate) Vector<f32, 3>);

impl WorldCoord {
    /// Creates a new world coordinate from x, y, and z components.
    ///
    /// # Arguments
    /// * `x` - Horizontal position (positive = right)
    /// * `y` - Vertical position (positive = down)
    /// * `z` - Depth position (positive = up/out of screen)
    pub fn new(x: f32, y: f32, z: f32) -> WorldCoord {
        WorldCoord(Vector::new([x, y, z]))
    }
}
