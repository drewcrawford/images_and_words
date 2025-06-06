use crate::bindings::software::texture::scaled_row_cell::ScaledRowCell;

/**An iterator over the [ScaledRowCell] coordinates.

# Spacing

See the section on spacing [ScaledRowCell].

# Order

Note that this iterator goes in *TEXTURE ORDER*, not *CELL ORDER*.

# Length

The length of this array is 1 less than the width or height of the input texture.  This is chosen so that each output texel
has an input texel at each corner, instead of having a dangling output texel with no corresponding source texel.

If you have no input texture, and are trying to create a new texture of given size, use the constructor [Self::new_no_input].
*/
#[derive(Debug, Copy, Clone)]
pub struct ScaledIterator {
    next: ScaledRowCell,
    tx_width: u16,
    tx_height: u16,
}

impl ScaledIterator {
    /**
    Create a new iterator
    * `tx_width`: The width of some texture.  We generate coordinates that are texel-aligned.
    * `tx_height`: The height of some texture.  We generate coordinates that are texel-aligned.
    * `scale`: The number of elements between each texel.  `scale=1` means texel-aligned, `scale=2` means the 0,2,4,... elements are texel-aligned and the 1,3,5... elements are half-aligned, etc.
    */
    pub fn new(tx_width: u16, tx_height: u16, scale: u8) -> Self {
        Self {
            next: ScaledRowCell {
                cell: 0,
                row: 0,
                scale,
                cell_i: 0,
                cell_j: 0,
            },
            tx_height,
            tx_width,
        }
    }

    /**
    Creates a new iterator with no input texture.

    In this case we return an iterator that will iterate the entire output size requested.
    The `new` constructor generally iterates to one less than that size.
    */
    pub fn new_no_input(out_width: u16, out_height: u16, scale: u8) -> Self {
        Self::new(out_width + 1, out_height + 1, scale)
    }
    /**
    Creates a new [ScaledRowCell] based on an output coordinate.

    This is primarily used for tests and similar.

    * `tx_width`: The width of the input texture
    * `tx_height`: The height of the input texture
    * `scale`: Scale for iteration, see [Self::new]
    * `output_x`: `x` coordinate in the output texture
    * `output_y`: `y` coordinate in the output texture
     */
    pub const fn new_output_coordinate(
        tx_width: u16,
        tx_height: u16,
        scale: u8,
        output_x: u16,
        output_y: u16,
    ) -> ScaledRowCell {
        assert!(output_x < (tx_width - 1) * scale as u16);
        assert!(output_y < (tx_height - 1) * scale as u16);
        let cell = output_x / scale as u16;
        let row = output_y / scale as u16;
        let cell_i = (output_x % scale as u16) as u8;
        let cell_j = (output_y % scale as u16) as u8;
        ScaledRowCell {
            cell,
            row,
            scale,
            cell_i,
            cell_j,
        }
    }
}

impl Iterator for ScaledIterator {
    type Item = ScaledRowCell;

    fn next(&mut self) -> Option<Self::Item> {
        let mut current = self.next;
        //we need to regularize from low iteration order to high iteration order, because several of these can cascade
        if current.cell_i == self.next.scale {
            //next cell!  Texture order!
            current.cell += 1;
            current.cell_i = 0;
        }
        if current.cell == self.tx_width - 1 {
            //next jrow!  Texture order!
            current.cell_j += 1;
            current.cell = 0; //note cell_i was cleared above
        }
        if current.cell_j == self.next.scale {
            //next row!
            current.cell_j = 0;
            current.row += 1;
            //cell_i, cell were cleared above
        }
        if current.row == self.tx_height - 1 {
            return None;
        }

        self.next = current;
        //advance cell_i "irregularly".
        //design note: the idea here is we don't pay the price of regularlizing next unless there is a next value,
        //this slightly reduces the cost of iteration
        self.next.cell_i += 1;
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use crate::bindings::software::texture::scaled_iterator::ScaledIterator;
    use crate::bindings::software::texture::scaled_row_cell::ScaledRowCell;

    #[test]
    fn test() {
        let mut iter = ScaledIterator::new(3, 3, 2);
        //note we go in TEXTURE ORDER.  First, go hard X direction:
        assert_eq!(iter.next(), Some(ScaledRowCell::new(0, 0, 2, 0, 0)));
        assert_eq!(iter.next(), Some(ScaledRowCell::new(0, 0, 2, 1, 0)));
        //X direction next cell!
        assert_eq!(iter.next(), Some(ScaledRowCell::new(1, 0, 2, 0, 0)));
        assert_eq!(iter.next(), Some(ScaledRowCell::new(1, 0, 2, 1, 0)));

        //now we step down to next "jrow", j=1
        assert_eq!(iter.next(), Some(ScaledRowCell::new(0, 0, 2, 0, 1)));
        assert_eq!(iter.next(), Some(ScaledRowCell::new(0, 0, 2, 1, 1)));
        assert_eq!(iter.next(), Some(ScaledRowCell::new(1, 0, 2, 0, 1)));
        assert_eq!(iter.next(), Some(ScaledRowCell::new(1, 0, 2, 1, 1)));

        //we repeat all this for row=1
        assert_eq!(iter.next(), Some(ScaledRowCell::new(0, 1, 2, 0, 0)));
        assert_eq!(iter.next(), Some(ScaledRowCell::new(0, 1, 2, 1, 0)));
        //X direction next cell!
        assert_eq!(iter.next(), Some(ScaledRowCell::new(1, 1, 2, 0, 0)));
        assert_eq!(iter.next(), Some(ScaledRowCell::new(1, 1, 2, 1, 0)));

        //next "jrow", j=1
        assert_eq!(iter.next(), Some(ScaledRowCell::new(0, 1, 2, 0, 1)));
        assert_eq!(iter.next(), Some(ScaledRowCell::new(0, 1, 2, 1, 1)));
        assert_eq!(iter.next(), Some(ScaledRowCell::new(1, 1, 2, 0, 1)));
        assert_eq!(iter.next(), Some(ScaledRowCell::new(1, 1, 2, 1, 1)));

        assert_eq!(iter.next(), None);
    }

    #[test]
    fn output_coordinates() {
        //Tests new_output_coordinate
        //uses same iteration as main test
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 0, 0),
            ScaledRowCell::new(0, 0, 2, 0, 0)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 1, 0),
            ScaledRowCell::new(0, 0, 2, 1, 0)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 2, 0),
            ScaledRowCell::new(1, 0, 2, 0, 0)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 3, 0),
            ScaledRowCell::new(1, 0, 2, 1, 0)
        );

        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 0, 1),
            ScaledRowCell::new(0, 0, 2, 0, 1)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 1, 1),
            ScaledRowCell::new(0, 0, 2, 1, 1)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 2, 1),
            ScaledRowCell::new(1, 0, 2, 0, 1)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 3, 1),
            ScaledRowCell::new(1, 0, 2, 1, 1)
        );

        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 0, 2),
            ScaledRowCell::new(0, 1, 2, 0, 0)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 1, 2),
            ScaledRowCell::new(0, 1, 2, 1, 0)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 2, 2),
            ScaledRowCell::new(1, 1, 2, 0, 0)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 3, 2),
            ScaledRowCell::new(1, 1, 2, 1, 0)
        );

        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 0, 3),
            ScaledRowCell::new(0, 1, 2, 0, 1)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 1, 3),
            ScaledRowCell::new(0, 1, 2, 1, 1)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 2, 3),
            ScaledRowCell::new(1, 1, 2, 0, 1)
        );
        assert_eq!(
            ScaledIterator::new_output_coordinate(3, 3, 2, 3, 3),
            ScaledRowCell::new(1, 1, 2, 1, 1)
        );
    }
}
