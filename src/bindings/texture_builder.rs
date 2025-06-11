// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Builder pattern for texture creation to reduce parameter count.

use std::sync::Arc;
use crate::images::BoundDevice;
use crate::bindings::visible_to::{TextureUsage, CPUStrategy};
use crate::Priority;

/// Builder for creating textures with a cleaner API than long parameter lists.
/// 
/// This builder addresses clippy warnings about functions having too many arguments
/// by providing a fluent interface for texture creation parameters.
pub struct TextureBuilder<'a, I> {
    device: &'a Arc<BoundDevice>,
    width: u16,
    height: u16,
    visible_to: TextureUsage,
    debug_name: &'a str,
    priority: Priority,
    initializer: I,
    cpu_strategy: Option<CPUStrategy>,
    mipmaps: Option<bool>,
}

impl<'a, I> TextureBuilder<'a, I> {
    /// Create a new texture builder with required parameters.
    pub fn new(
        device: &'a Arc<BoundDevice>,
        width: u16,
        height: u16,
        visible_to: TextureUsage,
        debug_name: &'a str,
        priority: Priority,
        initializer: I,
    ) -> Self {
        Self {
            device,
            width,
            height,
            visible_to,
            debug_name,
            priority,
            initializer,
            cpu_strategy: None,
            mipmaps: None,
        }
    }

    /// Set the CPU strategy for dynamic textures.
    pub fn with_cpu_strategy(mut self, cpu_strategy: CPUStrategy) -> Self {
        self.cpu_strategy = Some(cpu_strategy);
        self
    }

    /// Set whether to generate mipmaps for static textures.
    pub fn with_mipmaps(mut self, mipmaps: bool) -> Self {
        self.mipmaps = Some(mipmaps);
        self
    }

    /// Get the device reference.
    pub fn device(&self) -> &'a Arc<BoundDevice> {
        self.device
    }

    /// Get the width.
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Get the height.
    pub fn height(&self) -> u16 {
        self.height
    }

    /// Get the texture usage.
    pub fn visible_to(&self) -> TextureUsage {
        self.visible_to
    }

    /// Get the debug name.
    pub fn debug_name(&self) -> &'a str {
        self.debug_name
    }

    /// Get the priority.
    pub fn priority(&self) -> Priority {
        self.priority
    }

    /// Get the initializer.
    pub fn initializer(&self) -> &I {
        &self.initializer
    }

    /// Get the CPU strategy if set.
    pub fn cpu_strategy(&self) -> Option<CPUStrategy> {
        self.cpu_strategy
    }

    /// Get the mipmaps setting if set.
    pub fn mipmaps(&self) -> Option<bool> {
        self.mipmaps
    }
}