// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use crate::bindings::forward::dynamic::buffer::CRepr;
use crate::images::render_pass::PassDescriptor;
use wgpu::TextureFormat;

#[repr(C)]
#[derive(Debug)]
pub struct CameraProjection {
    pub projection: [f32; 16],
}

unsafe impl CRepr for CameraProjection {}

#[derive(Debug)]
pub struct DebugCaptureData {
    pub dump_buf: wgpu::Buffer,
    pub dump_buff_bytes_per_row: u32,
    pub depth_dump_buf: wgpu::Buffer,
    pub depth_dump_buff_bytes_per_row: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PassConfig {
    pub pass_descriptors: Vec<PassDescriptor>,
    pub enable_depth: bool,
    pub surface_format: TextureFormat,
}

impl PassConfig {
    pub fn new(surface_format: TextureFormat) -> Self {
        PassConfig {
            pass_descriptors: Vec::new(),
            enable_depth: false,
            surface_format,
        }
    }

    pub fn add_pass(&mut self, descriptor: PassDescriptor) {
        if descriptor.depth {
            self.enable_depth = true;
        }
        self.pass_descriptors.push(descriptor);
    }
}

/**
Provides state tracking.

This is the recommended pattern for many renderloop usecases.

There are two values:
* requested - written simply via `update`
* submitted - copied from `requested` via `mark_submitted`.

The *requested* value contains the latest value that the external system (e.g. a game loop) has requested to be rendered.
The *submitted* value contains the value that has been submitted to the GPU for rendering.

The [`Self::is_dirty`] func compares the *requested* value against the *submitted* value.
*/
#[derive(Debug)]
pub struct RenderInput<T> {
    pub requested: T,
    pub submitted: Option<T>,
}

impl<T> RenderInput<T> {
    pub fn new(requested: T) -> Self {
        RenderInput {
            requested,
            submitted: None,
        }
    }
    pub fn update(&mut self, requested: T) {
        self.requested = requested;
    }
    pub fn is_dirty(&self) -> bool
    where
        T: PartialEq,
    {
        match &self.submitted {
            Some(submitted) => self.requested != *submitted,
            None => true, //if not submitted, it is dirty
        }
    }
    pub fn mark_submitted(&mut self)
    where
        T: Clone,
    {
        self.submitted = Some(self.requested.clone());
    }
}
