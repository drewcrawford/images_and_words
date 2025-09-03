// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::bindings::software::texture::scaled_32::Scaled32;
use crate::bindings::software::texture::{Normalized, Texel};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;

/**
A scaled texture coordinate based on rows and cells.

The problem being solved here involves some source texture.  And we are scaling it to some new size, creating an abstract output
texture, that may or may not be materialized into memory.

This type uses integer math, and will *exactly* identify some position in the output texture.  This type is appropriate
for cases where fp error may be significant.

In order to do this, we call positions between the source texture texels, 'cells'.  We refer to each cell
by the texel of its upper-left coordinate.
 * For texels themselves, we say they belong to to the the cell so referenced by that texel,
 * For positions where only x or only y is exactly a texel coordinate ("between the source texture texels" in a subset of dimensions)
   we choose the cell referenced by the texel coordinate in dimension of exact match, and the upper or left texel in the
   other dimension.

By this method, any position in an output scheme can be assigned to some cell associated with a source texel.

# Spacing

The interior spacing of the [ScaledRowCell] is not defined.  Stated alternately, the spacing must be interpreted by some user.
Options include:

* [ScaledRowCell::x_evenly]: Evenly spaced within a cell, not across cells
* [ScaledRowCell::x_evenly_within]: Similar but in the range 0-1
* [ScaledRowCell::x_evenly_on_first]: 'On' the left edge, not necessarily the right
*
*/
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ScaledRowCell {
    /* You might sort of naively imagine we have, say, two f32s in here.
    in fact, that was the original implementation.  The difficulty is that the precision is different for different base values,
    in different areas of the grid.  This turns out to be an issue.

    Instead, let's do everything in integer math.
     */
    ///The base x value.  Range from [0,(width-2)]
    pub(crate) cell: u16,
    ///The base y value.  Range from [0,(height-2)]
    pub(crate) row: u16,
    ///The scale of the output texture.
    pub(crate) scale: u8,
    ///Ranges from [0,scale).
    pub(crate) cell_i: u8,
    ///Ranges from [0,scale).
    pub(crate) cell_j: u8,
}

impl ScaledRowCell {
    #[inline]
    pub const fn new(cell: u16, row: u16, scale: u8, cell_i: u8, cell_j: u8) -> Self {
        assert!(cell_i < scale);
        assert!(cell_j < scale);
        Self {
            cell,
            row,
            scale,
            cell_i,
            cell_j,
        }
    }

    #[inline]
    pub const fn cell(&self) -> u16 {
        self.cell
    }
    #[inline]
    pub const fn row(&self) -> u16 {
        self.row
    }
    ///The position in the x dimension interior to the cell.
    ///
    /// This ranges from [0,scale)
    #[inline]
    pub const fn cell_i(&self) -> u8 {
        self.cell_i
    }

    ///The position in the y dimension interior to the cell.
    ///
    /// This ranges from [0,scale)
    #[inline]
    pub const fn cell_j(&self) -> u8 {
        self.cell_j
    }

    ///The internal scale of the coordinate.  This is both x and y dimensioned.
    #[inline]
    pub const fn scale(&self) -> u8 {
        self.scale
    }

    ///Returns an interpretation of the x coordinate as a floating point value.
    ///
    /// This interpretation is evenly spaced within a given cell.  It is NOT evenly spaced across cells necessarily.
    #[inline]
    pub fn x_evenly(&self) -> f32 {
        self.cell as f32 + self.x_evenly_within()
    }

    ///Returns an interpretation of the y coordinate as a floating point value.
    ///
    /// This interpretation is evenly spaced within a given cell.  It is NOT evenly spaced across cells necessarily.
    #[inline]
    pub fn y_evenly(&self) -> f32 {
        self.row as f32 + self.y_evenly_within()
    }

    ///Returns an interpretation of the x coordinate as a floating point value within the cell (e.g. in the range 0-1)
    ///
    /// This interpretation is evenly spaced within a given cell.  It is NOT evenly spaced across cells necessarily.
    #[inline]
    pub fn x_evenly_within(&self) -> f32 {
        (self.cell_i + 1) as f32 / (self.scale + 1) as f32
    }
    ///Returns an interpretation of the x coordinate as a floating point value within the cell (e.g. in the range 0-1)
    ///
    /// This interpretation is evenly spaced within a given cell.  It is NOT evenly spaced across cells necessarily.
    #[inline]
    pub fn y_evenly_within(&self) -> f32 {
        (self.cell_j + 1) as f32 / (self.scale + 1) as f32
    }

    ///Returns an interpolation of the x coordinate as a floating point value.  The first value in a cell is 'on' the cell border,
    /// the last value is near the far edge.
    #[inline]
    pub fn x_evenly_on_first(&self) -> f32 {
        (self.cell_i) as f32 / (self.scale) as f32
    }

    ///Returns an interpolation of the y coordinate as a floating point value.  The first value in a row is 'on' the row border,
    /// the last value is near the far edge.
    #[inline]
    pub fn y_evenly_on_first(&self) -> f32 {
        self.cell_j as f32 / (self.scale) as f32
    }

    ///Converts into a normalized coordinate.
    ///
    /// Pass in the width and height of the *source* texture.
    #[inline]
    pub fn into_normalized(self, width: u16, height: u16) -> Normalized {
        //is this the right "within" style?
        let scaled_x = self.x_evenly(); //self.cell as f32 + self.x_evenly()
        let scaled_y = self.y_evenly(); //self.row as f32 + self.cell_j as f32 / self.scale as f32;
        let norm_x = scaled_x / width as f32;
        let norm_y = scaled_y / height as f32;
        Normalized::new(norm_x, norm_y)
    }

    #[inline]
    pub fn reference_texel(&self) -> Texel {
        Texel {
            x: self.cell,
            y: self.row,
        }
    }

    /**
        Converts to a coordinate in a different texture.

         Suppose you have a [ScaledRowCell] in some source texture.  Now you want to get a 'matching' coordinate for
         a texture of distinct size.

         This will get that corresponding coordinate, making some assumptions:
         * left, right, bottom, top edges are aligned
         * In particular, cell=0,row=0,cell_i=0,cell_j=0 refers to the upper left corner of the destination texture, etc.
         * The remaining values are spread 'evenly' across the destination texture, that is we have the same distance between samples.

         # Limtiations
         The second requirement introduces some limitations:
         * Resulting destination cells  will not be sampled in the same location for each cell
         * Resulting destination cells may not even be sampled the same number of times across a texture
    */
    #[inline]
    pub fn rescale_evenly(
        self,
        src_width: u16,
        src_height: u16,
        dst_width: u16,
        dst_height: u16,
    ) -> Scaled32 {
        //in order to keep fp issues out, we're going to go to some length
        //to get a lot of this into integer math.
        type ExtendedInt = u32;

        let xnum = (dst_width - 1) as ExtendedInt
            * (self.cell * self.scale as u16 + self.cell_i as u16) as ExtendedInt;
        let xdenom = (src_width - 1) * self.scale as u16;
        let ynum = (dst_height - 1) as ExtendedInt
            * (self.row * self.scale as u16 + self.cell_j as u16) as ExtendedInt;
        let ydenom = (src_height - 1) * self.scale as u16;

        //integer in self, and will be the integer part in the return
        let xi = xnum / (xdenom as ExtendedInt);
        let yi = ynum / (ydenom as ExtendedInt);

        //fractional part.
        let xnumf = xnum % (xdenom as ExtendedInt);
        let ynumf = ynum % (ydenom as ExtendedInt);

        let xf = (xnumf as f32) / (xdenom as f32);
        let yf = (ynumf as f32) / (ydenom as f32);

        Scaled32::new(xi.try_into().unwrap(), yi.try_into().unwrap(), xf, yf)
    }
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
fn test_rescale() {
    let f = ScaledRowCell::new(32, 0, 1, 0, 0);
    let g = f.rescale_evenly(128, 128, 64, 64);
    assert!(g.cell() < 64);
}
