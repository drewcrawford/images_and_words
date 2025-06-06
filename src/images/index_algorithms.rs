/*!
Algorithms for generating index lists for rendering grids as triangle meshes.

This module provides utilities for converting a 2D grid of vertices into a list of indices
suitable for rendering as triangles. The generated indices create two triangles per grid cell,
forming a connected mesh.

# Overview

When rendering a grid of points as a mesh, you need to specify which vertices form each triangle.
This module generates indices in a triangle list format, where every three consecutive indices
define one triangle.

# Example

```
use images_and_words::images::index_algorithms::IndexGenerator;

// Create a 3x3 grid (9 vertices total)
let generator = IndexGenerator::new(3, 3);

// The grid will have 2x2 = 4 cells, each with 2 triangles = 8 triangles total
assert_eq!(generator.num_triangles(), 8);

// Each triangle has 3 vertices, so we need 24 indices total
assert_eq!(generator.num_indices(), 24);

// Get the vertex index for the first few positions
assert_eq!(generator.index_for(0), 0); // First triangle, first vertex
assert_eq!(generator.index_for(1), 3); // First triangle, second vertex
assert_eq!(generator.index_for(2), 1); // First triangle, third vertex
```

# Populating an Index Buffer

```
use images_and_words::images::index_algorithms::IndexGenerator;

// Create a 4x3 grid
let generator = IndexGenerator::new(4, 3);

// Populate an index buffer
let mut indices = Vec::with_capacity(generator.num_indices());
for i in 0..generator.num_indices() {
    indices.push(generator.index_for(i) as u16);
}

// The indices are now ready to be uploaded to the GPU
assert_eq!(indices.len(), generator.num_indices());

// First triangle should be: 0, 4, 1
assert_eq!(indices[0], 0);
assert_eq!(indices[1], 4);
assert_eq!(indices[2], 1);
```
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
/// Generates indices for a grid of vertices to form a triangle mesh.
///
/// This struct creates index patterns for rendering a 2D grid of vertices as triangles.
/// Each cell in the grid (formed by 4 vertices) is split into 2 triangles using a
/// consistent winding order for proper face culling.
///
/// # Grid Layout
///
/// The grid uses an upper-left origin with X increasing to the right and Y increasing downward:
///
/// ```text
///      0 ─── 1 ─── 2
///      │  ╱  │  ╱  │
///      │ ╱   │ ╱   │
///      3 ─── 4 ─── 5
///      │  ╱  │  ╱  │
///      │ ╱   │ ╱   │
///      6 ─── 7 ─── 8
/// ```
///
/// # Triangle Generation
///
/// Each cell generates two triangles with counter-clockwise winding order:
/// - First triangle: top-left, bottom-left, top-right
/// - Second triangle: bottom-left, bottom-right, top-right
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexGenerator {
    width: usize,
    height: usize,
}

impl IndexGenerator {
    /// Creates a new index generator for a grid of vertices.
    ///
    /// # Arguments
    ///
    /// * `width` - The number of vertices in the X direction (must be > 1)
    /// * `height` - The number of vertices in the Y direction (must be > 1)
    ///
    /// # Panics
    ///
    /// Panics if width or height is less than or equal to 1, as at least a 2x2 grid
    /// is required to form triangles.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::index_algorithms::IndexGenerator;
    ///
    /// // Create a generator for a 4x3 grid (12 vertices total)
    /// let generator = IndexGenerator::new(4, 3);
    ///
    /// // This will create (4-1) * (3-1) = 6 cells
    /// // Each cell has 2 triangles, so 12 triangles total
    /// assert_eq!(generator.num_triangles(), 12);
    /// ```
    pub fn new(width: usize, height: usize) -> Self {
        assert!(width > 1 && height > 1, "Invalid geometry");
        Self { width, height }
    }

    /// Returns the total number of indices needed to render the grid.
    ///
    /// This is the number of triangles multiplied by 3 (vertices per triangle).
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::index_algorithms::IndexGenerator;
    ///
    /// let generator = IndexGenerator::new(3, 2);
    /// // 2x1 cells = 2 cells, 2 triangles per cell = 4 triangles
    /// // 4 triangles * 3 vertices per triangle = 12 indices
    /// assert_eq!(generator.num_indices(), 12);
    /// ```
    pub fn num_indices(&self) -> usize {
        self.num_triangles() * VERTEX_PER_TRIANGLE
    }

    /// Returns the number of triangles in the mesh.
    ///
    /// Each cell in the grid generates 2 triangles, and the number of cells
    /// is `(width - 1) * (height - 1)`.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::index_algorithms::IndexGenerator;
    ///
    /// let generator = IndexGenerator::new(5, 4);
    /// // (5-1) * (4-1) = 4 * 3 = 12 cells
    /// // 12 cells * 2 triangles per cell = 24 triangles
    /// assert_eq!(generator.num_triangles(), 24);
    /// ```
    pub fn num_triangles(&self) -> usize {
        (self.width - 1) * (self.height - 1) * TRIANGLES_PER_CELL
    }

    /// Returns the vertex index for a given position in the index buffer.
    ///
    /// This method maps from a position in the linear index buffer to the actual
    /// vertex index in the grid. The index buffer is organized as a sequence of
    /// triangles, with each triangle consisting of 3 vertex indices.
    ///
    /// # Arguments
    ///
    /// * `buffer_pos` - The position in the index buffer (0-based)
    ///
    /// # Returns
    ///
    /// The vertex index in the original grid (0-based, row-major order)
    ///
    /// # Panics
    ///
    /// Panics if `buffer_pos` is out of bounds for the generated indices.
    ///
    /// # Example
    ///
    /// ```
    /// use images_and_words::images::index_algorithms::IndexGenerator;
    ///
    /// // Create a 3x3 grid:
    /// // 0 - 1 - 2
    /// // |   |   |
    /// // 3 - 4 - 5
    /// // |   |   |
    /// // 6 - 7 - 8
    /// let generator = IndexGenerator::new(3, 3);
    ///
    /// // First triangle of first cell: vertices 0, 3, 1
    /// assert_eq!(generator.index_for(0), 0);
    /// assert_eq!(generator.index_for(1), 3);
    /// assert_eq!(generator.index_for(2), 1);
    ///
    /// // Second triangle of first cell: vertices 1, 3, 4
    /// assert_eq!(generator.index_for(3), 1);
    /// assert_eq!(generator.index_for(4), 3);
    /// assert_eq!(generator.index_for(5), 4);
    /// ```
    ///
    /// # Triangle Order
    ///
    /// For each cell, the triangles are generated in the following pattern:
    ///
    /// ```text
    /// top_left ────── top_right
    ///     │  ╲    2  ╱ │
    ///     │ 1 ╲    ╱   │
    ///     │    ╲  ╱    │
    ///     │     ╲╱     │
    /// bottom_left ── bottom_right
    /// ```
    ///
    /// - Triangle 1: (top_left, bottom_left, top_right)
    /// - Triangle 2: (bottom_left, bottom_right, top_right)
    pub fn index_for(&self, buffer_pos: usize) -> usize {
        let cell_vertex = buffer_pos % VERTEX_PER_CELL;
        let cell = buffer_pos / VERTEX_PER_CELL;
        let cell_x = cell % (self.width - 1);
        let cell_y = cell / (self.width - 1);
        assert!(cell_y < self.height, "Index out of bounds");
        let (x, y) = match cell_vertex {
            0 => (cell_x, cell_y),
            1 | 4 => (cell_x, cell_y + 1),
            2 | 3 => (cell_x + 1, cell_y),
            5 => (cell_x + 1, cell_y + 1),
            _ => {
                unreachable!();
            }
        };
        assert!(x < self.width, "Index out of bounds");
        assert!(y < self.height, "Index out of bounds");
        y * self.width + x
    }
}
