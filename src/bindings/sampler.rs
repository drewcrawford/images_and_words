// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Texture sampler configuration types.
//!
//! This module provides types for configuring how textures are sampled in shaders.
//! Samplers control texture filtering, coordinate normalization, and other aspects
//! of texture access.
//!
//! # Overview
//!
//! When binding a texture to a shader, you can optionally specify a sampler that
//! controls how the texture data is accessed. Different sampler types provide
//! different filtering and interpolation behaviors.
//!
//! # Example
//!
//! ```
//! # if cfg!(not(feature="backend_wgpu")) { return; }
//! # #[cfg(feature = "testing")]
//! # {
//! use images_and_words::bindings::sampler::SamplerType;
//! use images_and_words::bindings::bind_style::{BindStyle, BindSlot, Stage, SamplerInfo};
//! # test_executors::sleep_on(async {
//! # use images_and_words::images::view::View;
//! # use images_and_words::images::projection::WorldCoord;
//! # use images_and_words::bindings::forward::r#static::texture::Texture;
//! # use images_and_words::pixel_formats::{BGRA8UNormSRGB, BGRA8UnormPixelSRGB};
//! # use images_and_words::bindings::visible_to::TextureUsage;
//! # use images_and_words::Priority;
//! # let engine = images_and_words::images::Engine::rendering_to(View::for_testing(), WorldCoord::new(0.0, 0.0, 0.0)).await.expect("can't get engine");
//! # let device = engine.bound_device();
//! # let config = images_and_words::bindings::visible_to::TextureConfig {
//! #     width: 256,
//! #     height: 256,
//! #     visible_to: TextureUsage::FragmentShaderSample,
//! #     debug_name: "sample_texture",
//! #     priority: Priority::UserInitiated,
//! #     cpu_strategy: images_and_words::bindings::visible_to::CPUStrategy::WontRead,
//! #     mipmaps: true,
//! # };
//! # let texture: Texture<BGRA8UNormSRGB> = Texture::new(
//! #     &device,
//! #     config,
//! #     |_texel| BGRA8UnormPixelSRGB { b: 128, g: 128, r: 128, a: 255 }
//! # ).await.expect("Failed to create texture");
//!
//! let mut bind_style = BindStyle::new();
//!
//! // Bind a texture with mipmapped sampling
//! let sampler = SamplerInfo {
//!     pass_index: 1,  // Sampler will be bound to slot 1
//!     sampler_type: SamplerType::Mipmapped,
//! };
//! bind_style.bind_static_texture(
//!     BindSlot::new(0),
//!     Stage::Fragment,
//!     &texture,
//!     Some(sampler)
//! );
//! # });
//! # }
//! ```

/// Specifies the type of texture sampling to use.
///
/// Sampler types control how texture data is accessed and filtered when
/// sampled in shaders. Different types provide different quality/performance
/// tradeoffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SamplerType {
    /// Enables mipmapped texture sampling with linear filtering.
    ///
    /// This sampler type:
    /// - Uses normalized texture coordinates (0.0 to 1.0 range)
    /// - Enables linear filtering for smooth interpolation between texels
    /// - Supports mipmapping for improved quality when textures are minified
    /// - Uses linear filtering between mipmap levels
    ///
    /// Mipmapped sampling is ideal for textures that will be viewed at varying
    /// distances, as it reduces aliasing artifacts and improves performance
    /// by using pre-computed lower resolution versions of the texture.
    ///
    /// # GPU Configuration
    ///
    /// When this sampler type is used, the GPU sampler is configured with:
    /// - Address modes: Clamp to edge for all axes
    /// - Magnification filter: Linear
    /// - Minification filter: Linear
    /// - Mipmap filter: Linear
    Mipmapped,
}
