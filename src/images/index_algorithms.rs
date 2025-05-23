/*!
Algorithms for index lists.
*/

/**

Generates an index buffer for a grid of points.


Suppose you have a 2D grid of points, and you want to draw it as a connected mesh.

One option is to use a triangle strip.  This may be more efficient but it involves reordering the points
into preferred order.

Another option is to use a triangle list, which is easier to debug.  That is what we will do here.

Assume that the points are in a 2D grid, with upper left origin.  Triangles generated are as follows.
```text
     0
      ─────────────X────────────▶
     │┌─────▲┌─────▲┌─────▲
     ││    ╱││    ╱││    ╱│
     ││ 0 ╱ ││ 2 ╱ ││ 4 ╱ │
     ││  ╱  ││  ╱  ││  ╱  │
    Y││ ╱ 1 ││ ╱  3││ ╱ 5 │
     │├─────▲├─────▲├─────▲
     ││    ╱││    ╱││    ╱│
     ││ 6 ╱ ││8  ╱ ││ 10╱ │
     ││  ╱  ││  ╱  ││  ╱  │
     ││ ╱ 7 ││ ╱ 9 ││ ╱ 11│
     ▼└□────┘└□────┘└□────┘
```

Considering a single cell of two triangles, for CCW frontface order,
vertices chosen as follows

```text
     0
      ─────X──────▶
     │ 0─────────▲2,3
     │ │        ╱│
     │ │       ╱ │
     │ │      ╱  │
    Y│ │     ╱   │
     │ │    ╱    │
     │ │   ╱     │
     │ │  ╱      │
     │ │ ╱       │
     │ │╱        │
     ▼ □─────────┘
      1,4          5
```

*/
const VERTEX_PER_TRIANGLE: usize = 3;
const TRIANGLES_PER_CELL: usize = 2;

const VERTEX_PER_CELL: usize = VERTEX_PER_TRIANGLE * TRIANGLES_PER_CELL;
/**
Generates indices for a grid of points.
*/
pub struct IndexGenerator {
    width: usize,
    height: usize,
}

impl IndexGenerator {
    /**
    Creates a new index generator for a grid of points.
    
    The grid is assumed to be a rectangle of points, with the width and height
    given as vertex counts.  The width and height must be greater than 1.
    */
    
    pub fn new(width: usize, height: usize) -> Self {
        assert!(width > 1 && height > 1, "Invalid geometry");
        Self {
            width,
            height,
        }
    }

    /**
    Returns the number of vertices in the mesh.
    */
    pub fn num_indices(&self) -> usize {
        self.num_triangles() * VERTEX_PER_TRIANGLE
    }



    /**
    Returns the number of triangles in the mesh.
*/
    pub fn num_triangles(&self) -> usize {
        (self.width - 1) * (self.height - 1) * TRIANGLES_PER_CELL
    }

    pub fn index_for(&self, buffer_pos: usize) -> usize {
        let cell_vertex = buffer_pos % VERTEX_PER_CELL;
        let cell = buffer_pos / VERTEX_PER_CELL;
        let cell_x = cell % (self.width - 1);
        let cell_y = cell / (self.width - 1);
        assert!(cell_y < self.height, "Index out of bounds");
        let (x,y) = match cell_vertex {
            0 => {
                (cell_x, cell_y)
            }
            1|4 => {
                (cell_x, cell_y+1)
            }
            2 | 3 => {
                (cell_x+1, cell_y)
            }
            5 => {
                (cell_x + 1, cell_y + 1)
            }
            _ => {
                unreachable!();
            }
        };
        assert!(x < self.width, "Index out of bounds");
        assert!(y < self.height, "Index out of bounds");
        y * self.width + x
    }
}

