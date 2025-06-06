//! GPU rendering engine and graphics pipeline components.
//! 
//! This module provides the core rendering infrastructure for GPU-accelerated graphics
//! applications. It includes the rendering engine, shader management, render passes,
//! and various utilities for vertex processing and drawing operations.
//! 
//! # Architecture
//! 
//! The module is organized around several key concepts:
//! 
//! - **[`Engine`]**: The main entry point for rendering operations, managing the GPU device
//!   and rendering context
//! - **[`render_pass`]**: Configuration for GPU draw operations including shaders and draw commands
//! - **[`shader`]**: Vertex and fragment shader types for GPU programming
//! - **[`view`]**: Display surface abstraction for rendering targets
//! - **[`port`]**: Viewport and camera management for 3D rendering
//! - **[`projection`]**: Coordinate systems and projection matrices
//! 
//! # Getting Started
//! 
//! To create a basic rendering setup:
//! 
//! ```
//! # #[cfg(feature = "testing")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use images_and_words::images::{Engine, projection::WorldCoord};
//! 
//! // Initialize the rendering engine for testing
//! let engine = Engine::for_testing().await?;
//! 
//! // Access the main rendering port for drawing
//! let mut port = engine.main_port_mut();
//! // Port is now ready for rendering operations
//! # Ok(())
//! # }
//! ```
//! 
//! # Vertex Processing
//! 
//! The module provides utilities for working with vertex data:
//! 
//! - [`vertex_layout`]: Define vertex attribute layouts for shaders
//! - [`vertex_algorithms`]: Common vertex generation algorithms
//! - [`index_algorithms`]: Index buffer generation for primitive assembly
//! 
//! These components work together to prepare geometry data for GPU consumption.

pub use engine::Engine;


pub mod render_pass;

pub(crate) mod device;
pub(crate) mod engine;

pub mod port;


pub(crate) mod camera;
pub mod shader;
pub mod view;
pub mod projection;
mod frame;
pub mod index_algorithms;
pub mod vertex_layout;
pub mod vertex_algorithms;

pub use device::BoundDevice;



