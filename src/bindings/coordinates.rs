//! Coordinate system types for the images_and_words graphics middleware.
//!
//! This module defines coordinate types used throughout the graphics pipeline
//! for specifying positions in 2D raster/texture space.

/// 2D coordinate in raster/texture space using the images_and_words standard coordinate system.
///
/// # Coordinate System
///
/// The IW (images_and_words) standard coordinate system is a cross-platform coordinate
/// system where:
/// - Origin (0,0) is at the top-left corner
/// - X-axis increases towards the right
/// - Y-axis increases downwards
///
/// ```text
///            x
///       0 ────────▶
///       │ ┌───────┐
///     y │ │       │
///       │ │       │
///       │ │       │
///       ▼ └───────┘
/// ```
///
/// This coordinate system matches common raster graphics conventions used in
/// most image formats and GPU texture coordinates.
///
/// # Usage
///
/// `RasterCoord2D` is primarily used for:
/// - Specifying pixel positions in textures and framebuffers
/// - Defining regions of interest for rendering operations
/// - Texture sampling coordinates (when normalized)
///
/// # Example
///
/// ```
/// use images_and_words::bindings::coordinates::RasterCoord2D;
///
/// let top_left = RasterCoord2D { x: 0, y: 0 };
/// let bottom_right = RasterCoord2D { x: 1920, y: 1080 };
///
/// // Access individual components
/// assert_eq!(top_left.x, 0);
/// assert_eq!(bottom_right.y, 1080);
/// ```
///
/// # Limitations
///
/// The use of `u16` limits coordinates to a maximum of 65,535 in each dimension.
/// This is sufficient for most texture and framebuffer sizes but may need to be
/// considered for very large render targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RasterCoord2D {
    /// The horizontal position, increasing from left to right.
    pub x: u16,
    /// The vertical position, increasing from top to bottom.
    pub y: u16
}

impl RasterCoord2D {
    /// Creates a new `RasterCoord2D` with the specified x and y coordinates.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::bindings::coordinates::RasterCoord2D;
    ///
    /// let coord = RasterCoord2D::new(100, 200);
    /// assert_eq!(coord.x, 100);
    /// assert_eq!(coord.y, 200);
    /// ```
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    /// Returns the origin coordinate (0, 0).
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::bindings::coordinates::RasterCoord2D;
    ///
    /// let origin = RasterCoord2D::origin();
    /// assert_eq!(origin.x, 0);
    /// assert_eq!(origin.y, 0);
    /// ```
    pub fn origin() -> Self {
        Self { x: 0, y: 0 }
    }
}