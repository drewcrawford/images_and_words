use crate::bindings::software::texture::scaled_row_cell::ScaledRowCell;
use crate::bindings::software::texture::Texel;

/**
A scaled texture coordinate, that uses fp32 for the portion within a row/cell.

Compare with [ScaledRowCell].  The primary difference with this type is that no rational denominator
need be specified for the floating point portion.  This makes the type convenient for working with some
"other", floatingpoint based method.

At the moment, the primary usecase is monte carlo output.
*/
#[derive(Clone,PartialEq,Debug,Copy)]
pub struct Scaled32 {
    row: u16,
    cell: u16,
    cell_i: f32,
    cell_j: f32,
}

impl Scaled32 {
    #[inline] pub const fn new(cell: u16, row: u16, cell_i: f32, cell_j: f32) -> Self {
        Self { row, cell, cell_i, cell_j }
    }
    ///Create a new coordinate, from imprecise scaled x/y values, clamping 'inside' the given texture.
    ///
    /// The resulting coordinates has a similar length as the [ScaledRowCell] coordinates.  This is a length 'inside' the input texture.
    #[inline] pub fn new_clamping_inside_imprecise(x: f32, y: f32, clamp_width: u16, clamp_height: u16) -> Scaled32 {
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
            cell_j
        }
    }
    #[inline] pub const fn row(&self) -> u16 { self.row }
    #[inline] pub const fn cell(&self) -> u16 { self.cell }
    #[inline] pub const fn cell_i(&self) -> f32 { self.cell_i }
    #[inline] pub const fn cell_j(&self) -> f32 { self.cell_j }

    #[inline] pub fn x_evenly(&self) -> f32 { self.cell as f32 + self.cell_i}
    #[inline] pub fn y_evenly(&self) -> f32 { self.row as f32 + self.cell_j }

    ///Converts into into the reference texel.
    #[inline] pub fn reference_texel(&self) -> Texel {
        Texel{ x: self.cell, y: self.row }
    }

    #[inline] pub fn s32(&self) -> S32 {
        S32 {cell_i: self.cell_i, cell_j: self.cell_j }
    }

    ///Creates a new coordinate, by applying an offset and clamping between \[0-c) (e.g., \[0-(c-1)]!)
    #[inline] pub fn offset_clamped(mut self, dx: f32, dy: f32, clamp_width: u16, clamp_height: u16) -> Self {
        //applies the offset, storing the inner and outer result in-place
        fn apply(dn: f32,inner: &mut f32, outer: &mut u16, length: u16) {
            let new_inner = *inner + dn;
            //extend to a signed type that will be large enough for our purposes
            type SignedType = i32;
            let amount = if new_inner < 0.0 {
                new_inner.ceil() as SignedType // -0.1 => -1, -1.1 => -2, etc.
            }
            else {
                new_inner.floor() as SignedType
            };
            let proposed = *outer as SignedType + amount;
            if proposed >= (length - 1) as SignedType {
                //clamp right
                *inner = 0.0;
                *outer = length - 1;
            }
            else if proposed < 0 {
                //clamp left
                *inner = 0.0;
                *outer = 0;
            }
            else {
                *inner = new_inner.fract(); //fractional part only
                *outer = proposed.try_into().unwrap()
            }
        }
        apply(dx, &mut self.cell_i, &mut self.cell, clamp_width);
        apply(dy, &mut self.cell_j, &mut self.row, clamp_height);
        self
    }
}

/**The 'mini' form of [Scaled32].

This contains only the floating point within a cell.  It must be interpreted with
respect to the reference texel of the [Scaled32].
*/
pub struct S32 {
    pub cell_i: f32,
    pub cell_j: f32,
}

impl From<ScaledRowCell> for Scaled32 {
    fn from(f: ScaledRowCell) -> Self {
        Self {
            row: f.row,
            cell: f.cell,
            cell_i: f.x_evenly_within(),
            cell_j: f.y_evenly_within()
        }
    }
}

#[test] fn clamped() {
    let f = Scaled32::new(0, 0, 0.0, 0.0);
    assert_eq!(f.offset_clamped(0.5,0.5,100,100), Scaled32::new(0, 0, 0.5, 0.5));
    assert_eq!(f.offset_clamped(3.5,1.0,100,100), Scaled32::new(3, 1, 0.5, 0.0));
    assert_eq!(f.offset_clamped(3.5,1.0,2,1), Scaled32::new(1, 0, 0.0, 0.0));

    let f = Scaled32::new(1, 1, 0.5, 0.5);
    assert_eq!(f.offset_clamped(1.5, 1.5, 100,100), Scaled32::new(3,3,0.0,0.0));
    assert_eq!(f.offset_clamped(-1.5, -1.5, 100,100), Scaled32::new(0,0,0.0,0.0));

    assert_eq!(f.offset_clamped(-100.6, -100.9, 100,100), Scaled32::new(0,0,0.0,0.0));
    assert_eq!(f.offset_clamped(20.3, 22.3, 100,10), Scaled32::new(21,9,0.79999924,0.0));

}
