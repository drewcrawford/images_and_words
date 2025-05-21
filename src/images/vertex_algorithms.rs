/*!
Implements common vertex algorithms.
*/

use crate::images::index_algorithms::IndexGenerator;

/**
Generates a rectangular 2D grid of points.
*/
#[derive(Clone)]
pub struct GridGenerator {
    grid_width: usize,
    grid_height: usize,
}

impl GridGenerator {
    /**
    Creates a new grid generator for a rectangle of points.
    
    width and height are specified in terms of the width and height of the grid.
    */
    pub fn new_grid(grid_width: usize, grid_height: usize) -> Self {
        assert!(grid_width > 0 && grid_width > 0, "Invalid geometry");
        Self {
            grid_width,
            grid_height,
        }
    }
    
    /**
    Returns the total number of vertices in the x direction.

    This is the number of vertices in the grid width, plus one for the last vertex.
    */
    pub fn vertex_count_width(&self) -> usize {
        self.grid_width + 1
    }
    
    /**
    Returns the total number of vertices in the y direction.
    
    This is the number of vertices in the grid height, plus one for the last vertex.
    */
    pub fn vertex_count_height(&self) -> usize {
        self.grid_height + 1
    }
    
    /**
    Returns the number of vertices in the grid.
    */
    pub fn vertex_count(&self) -> usize {
        self.vertex_count_width() * self.vertex_count_height()
    }
    
    /**
    Returns the 2d coordinate for a given vertex index.
    */
    pub fn coordinates_for_vertex(&self, vertex: usize) -> (usize, usize) {
        let x = vertex % self.vertex_count_width();
        let y = vertex / self.vertex_count_width();
        assert!(x < self.vertex_count_width() && y < self.vertex_count_height(), "Index out of bounds");
        (x,y)
    }
    
    /**
    Returns an [IndexGenerator] for the grid.
    */
    pub fn index_generator(&self) -> IndexGenerator {
        IndexGenerator::new(self.vertex_count_width(), self.vertex_count_height())
    }
    
}

