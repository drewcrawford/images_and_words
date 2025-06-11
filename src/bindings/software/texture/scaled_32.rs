// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*!
Utilities for working with texture coordinates that use floating point
sub-texel precision.

This module defines [`Scaled32`], a texture coordinate containing integer cell
positions with fractional offsets, and [`S32`], a lightweight type holding just
the fractional components.
*/

use crate::bindings::software::texture::Texel;
use crate::bindings::software::texture::scaled_row_cell::ScaledRowCell;

/// A texture coordinate with integer cell/row positions and floating-point sub-cell precision.
///
/// `Scaled32` represents a position within a texture using:
/// - Integer cell (x) and row (y) coordinates for the base texel position
/// - Floating-point offsets within that texel (cell_i, cell_j) in range [0.0, 1.0)
///
/// This type is similar to [`ScaledRowCell`] but uses 32-bit floating-point precision
/// for sub-texel coordinates instead of rational numbers. This makes it more suitable
/// for integration with floating-point based algorithms like Monte Carlo sampling.
///
/// # Coordinate System
///
/// The coordinate system follows standard texture conventions:
/// - Cell (x) increases from left to right
/// - Row (y) increases from top to bottom
/// - Sub-cell coordinates (cell_i, cell_j) represent fractional offsets within a texel
///
/// # Example
///
/// ```
/// use images_and_words::bindings::software::texture::scaled_32::Scaled32;
///
/// // Create a coordinate at texel (10, 20) with 0.5 offset in both dimensions
/// let coord = Scaled32::new(10, 20, 0.5, 0.5);
/// assert_eq!(coord.cell(), 10);
/// assert_eq!(coord.row(), 20);
/// assert_eq!(coord.x_evenly(), 10.5);
/// assert_eq!(coord.y_evenly(), 20.5);
/// ```
#[derive(Clone, PartialEq, Debug, Copy)]
pub struct Scaled32 {
    row: u16,
    cell: u16,
    cell_i: f32,
    cell_j: f32,
}

impl Scaled32 {
    /// Creates a new `Scaled32` coordinate from explicit components.
    ///
    /// # Arguments
    ///
    /// * `cell` - The x-coordinate of the base texel (0-based)
    /// * `row` - The y-coordinate of the base texel (0-based)
    /// * `cell_i` - The fractional x-offset within the texel, typically in [0.0, 1.0)
    /// * `cell_j` - The fractional y-offset within the texel, typically in [0.0, 1.0)
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::scaled_32::Scaled32;
    ///
    /// let coord = Scaled32::new(5, 10, 0.25, 0.75);
    /// assert_eq!(coord.cell(), 5);
    /// assert_eq!(coord.row(), 10);
    /// assert_eq!(coord.cell_i(), 0.25);
    /// assert_eq!(coord.cell_j(), 0.75);
    /// ```
    #[inline]
    pub const fn new(cell: u16, row: u16, cell_i: f32, cell_j: f32) -> Self {
        Self {
            row,
            cell,
            cell_i,
            cell_j,
        }
    }

    /// Creates a new coordinate from floating-point x/y values, clamping within texture bounds.
    ///
    /// This method takes continuous floating-point coordinates and converts them to the
    /// discrete cell/row representation with sub-cell precision. The coordinates are
    /// clamped to ensure they remain within the texture bounds minus one texel on each edge.
    ///
    /// # Arguments
    ///
    /// * `x` - The continuous x-coordinate
    /// * `y` - The continuous y-coordinate  
    /// * `clamp_width` - The texture width (coordinates clamped to [0, width-2])
    /// * `clamp_height` - The texture height (coordinates clamped to [0, height-2])
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::scaled_32::Scaled32;
    ///
    /// // Create a coordinate within a 100x100 texture
    /// let coord = Scaled32::new_clamping_inside_imprecise(50.7, 30.3, 100, 100);
    /// assert_eq!(coord.cell(), 50);
    /// assert_eq!(coord.row(), 30);
    /// assert!((coord.cell_i() - 0.7).abs() < 0.001);
    /// assert!((coord.cell_j() - 0.3).abs() < 0.001);
    ///
    /// // Coordinates are clamped to stay within bounds
    /// let clamped = Scaled32::new_clamping_inside_imprecise(150.0, -10.0, 100, 100);
    /// assert_eq!(clamped.cell(), 98); // Clamped to width-2
    /// assert_eq!(clamped.row(), 0);   // Clamped to 0
    /// ```
    #[inline]
    pub fn new_clamping_inside_imprecise(
        x: f32,
        y: f32,
        clamp_width: u16,
        clamp_height: u16,
    ) -> Scaled32 {
        let clamped_x = x.clamp(0.0, (clamp_width - 2) as f32);
        let clamped_y = y.clamp(0.0, (clamp_height - 2) as f32);
        let rowf = clamped_y.floor();
        let cellf = clamped_x.floor();
        let cell_i = clamped_x - cellf;
        let cell_j = clamped_y - rowf;
        Scaled32 {
            row: rowf as u16,
            cell: cellf as u16,
            cell_i,
            cell_j,
        }
    }
    /// Returns the row (y) coordinate of the base texel.
    #[inline]
    pub const fn row(&self) -> u16 {
        self.row
    }

    /// Returns the cell (x) coordinate of the base texel.
    #[inline]
    pub const fn cell(&self) -> u16 {
        self.cell
    }

    /// Returns the fractional x-offset within the texel.
    #[inline]
    pub const fn cell_i(&self) -> f32 {
        self.cell_i
    }

    /// Returns the fractional y-offset within the texel.
    #[inline]
    pub const fn cell_j(&self) -> f32 {
        self.cell_j
    }

    /// Returns the continuous x-coordinate by combining cell and fractional offset.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::scaled_32::Scaled32;
    ///
    /// let coord = Scaled32::new(10, 20, 0.25, 0.75);
    /// assert_eq!(coord.x_evenly(), 10.25);
    /// ```
    #[inline]
    pub fn x_evenly(&self) -> f32 {
        self.cell as f32 + self.cell_i
    }

    /// Returns the continuous y-coordinate by combining row and fractional offset.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::scaled_32::Scaled32;
    ///
    /// let coord = Scaled32::new(10, 20, 0.25, 0.75);
    /// assert_eq!(coord.y_evenly(), 20.75);
    /// ```
    #[inline]
    pub fn y_evenly(&self) -> f32 {
        self.row as f32 + self.cell_j
    }

    /// Converts this coordinate to a [`Texel`] representing the base texel position.
    ///
    /// This discards the fractional components and returns only the integer cell/row.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::scaled_32::Scaled32;
    /// use images_and_words::bindings::software::texture::Texel;
    ///
    /// let coord = Scaled32::new(10, 20, 0.25, 0.75);
    /// let texel = coord.reference_texel();
    /// assert_eq!(texel.x, 10);
    /// assert_eq!(texel.y, 20);
    /// ```
    #[inline]
    pub fn reference_texel(&self) -> Texel {
        Texel {
            x: self.cell,
            y: self.row,
        }
    }

    /// Extracts the fractional components as an [`S32`] mini-coordinate.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::scaled_32::Scaled32;
    ///
    /// let coord = Scaled32::new(10, 20, 0.25, 0.75);
    /// let s32 = coord.s32();
    /// assert_eq!(s32.cell_i, 0.25);
    /// assert_eq!(s32.cell_j, 0.75);
    /// ```
    #[inline]
    pub fn s32(&self) -> S32 {
        S32 {
            cell_i: self.cell_i,
            cell_j: self.cell_j,
        }
    }

    /// Creates a new coordinate by applying an offset and clamping to texture bounds.
    ///
    /// This method applies floating-point offsets to the current coordinate and ensures
    /// the result stays within the texture bounds [0, width-1] x [0, height-1].
    ///
    /// # Arguments
    ///
    /// * `dx` - The x-offset to apply
    /// * `dy` - The y-offset to apply
    /// * `clamp_width` - The texture width for clamping
    /// * `clamp_height` - The texture height for clamping
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::scaled_32::Scaled32;
    ///
    /// let coord = Scaled32::new(10, 10, 0.5, 0.5);
    ///
    /// // Normal offset
    /// let moved = coord.offset_clamped(2.7, 1.3, 100, 100);
    /// assert_eq!(moved.cell(), 13);
    /// assert_eq!(moved.row(), 11);
    /// assert!((moved.cell_i() - 0.2).abs() < 0.001);
    /// assert!((moved.cell_j() - 0.8).abs() < 0.001);
    ///
    /// // Clamping at boundaries
    /// let clamped = coord.offset_clamped(100.0, 100.0, 50, 50);
    /// assert_eq!(clamped.cell(), 49); // Clamped to width-1
    /// assert_eq!(clamped.row(), 49);  // Clamped to height-1
    /// ```
    #[inline]
    pub fn offset_clamped(mut self, dx: f32, dy: f32, clamp_width: u16, clamp_height: u16) -> Self {
        //applies the offset, storing the inner and outer result in-place
        fn apply(dn: f32, inner: &mut f32, outer: &mut u16, length: u16) {
            let new_inner = *inner + dn;
            //extend to a signed type that will be large enough for our purposes
            type SignedType = i32;
            let amount = if new_inner < 0.0 {
                new_inner.ceil() as SignedType // -0.1 => -1, -1.1 => -2, etc.
            } else {
                new_inner.floor() as SignedType
            };
            let proposed = *outer as SignedType + amount;
            if proposed >= (length - 1) as SignedType {
                //clamp right
                *inner = 0.0;
                *outer = length - 1;
            } else if proposed < 0 {
                //clamp left
                *inner = 0.0;
                *outer = 0;
            } else {
                *inner = new_inner.fract(); //fractional part only
                *outer = proposed.try_into().unwrap()
            }
        }
        apply(dx, &mut self.cell_i, &mut self.cell, clamp_width);
        apply(dy, &mut self.cell_j, &mut self.row, clamp_height);
        self
    }
}

/// The fractional component of a [`Scaled32`] coordinate.
///
/// `S32` represents only the sub-texel portion of a texture coordinate,
/// containing the floating-point offsets within a single texel. This type
/// must be interpreted in the context of a reference texel position to
/// have meaning.
///
/// # Fields
///
/// * `cell_i` - The fractional x-offset within a texel, typically in [0.0, 1.0)
/// * `cell_j` - The fractional y-offset within a texel, typically in [0.0, 1.0)
///
/// # Example
///
/// ```
/// use images_and_words::bindings::software::texture::scaled_32::{Scaled32, S32};
///
/// let coord = Scaled32::new(10, 20, 0.25, 0.75);
/// let fractional = coord.s32();
///
/// // S32 contains only the fractional parts
/// assert_eq!(fractional.cell_i, 0.25);
/// assert_eq!(fractional.cell_j, 0.75);
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct S32 {
    pub cell_i: f32,
    pub cell_j: f32,
}

// --- boilerplate for S32 ---

impl From<Scaled32> for S32 {
    #[inline]
    fn from(value: Scaled32) -> Self {
        value.s32()
    }
}

impl From<ScaledRowCell> for Scaled32 {
    /// Converts a [`ScaledRowCell`] coordinate to `Scaled32`.
    ///
    /// This conversion transforms the rational sub-texel coordinates of `ScaledRowCell`
    /// into floating-point representation.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::bindings::software::texture::scaled_32::Scaled32;
    /// use images_and_words::bindings::software::texture::scaled_row_cell::ScaledRowCell;
    ///
    /// let row_cell = ScaledRowCell::new(10, 20, 4, 1, 3); // scale=4, i=(1+1)/(4+1), j=(3+1)/(4+1)
    /// let scaled32: Scaled32 = row_cell.into();
    ///
    /// assert_eq!(scaled32.cell(), 10);
    /// assert_eq!(scaled32.row(), 20);
    /// assert_eq!(scaled32.cell_i(), 0.4); // 2/5
    /// assert_eq!(scaled32.cell_j(), 0.8); // 4/5
    /// ```
    fn from(f: ScaledRowCell) -> Self {
        Self {
            row: f.row,
            cell: f.cell,
            cell_i: f.x_evenly_within(),
            cell_j: f.y_evenly_within(),
        }
    }
}

#[test]
fn clamped() {
    let f = Scaled32::new(0, 0, 0.0, 0.0);
    assert_eq!(
        f.offset_clamped(0.5, 0.5, 100, 100),
        Scaled32::new(0, 0, 0.5, 0.5)
    );
    assert_eq!(
        f.offset_clamped(3.5, 1.0, 100, 100),
        Scaled32::new(3, 1, 0.5, 0.0)
    );
    assert_eq!(
        f.offset_clamped(3.5, 1.0, 2, 1),
        Scaled32::new(1, 0, 0.0, 0.0)
    );

    let f = Scaled32::new(1, 1, 0.5, 0.5);
    assert_eq!(
        f.offset_clamped(1.5, 1.5, 100, 100),
        Scaled32::new(3, 3, 0.0, 0.0)
    );
    assert_eq!(
        f.offset_clamped(-1.5, -1.5, 100, 100),
        Scaled32::new(0, 0, 0.0, 0.0)
    );

    assert_eq!(
        f.offset_clamped(-100.6, -100.9, 100, 100),
        Scaled32::new(0, 0, 0.0, 0.0)
    );
    assert_eq!(
        f.offset_clamped(20.3, 22.3, 100, 10),
        Scaled32::new(21, 9, 0.79999924, 0.0)
    );
}
