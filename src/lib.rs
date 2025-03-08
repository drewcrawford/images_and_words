/*! images_and_words is a GPU middleware and abstraction layer for high-performance
  graphics applications and games.

Here is a quick chart to compare IW against many other solutions:

|| Strategy                | Examples               | API style   | API concepts                                                                                | Synchronization concerns | Shaders                                               | Runtime size     | Platform support                               | Development speed | Runtime speed                                                 |
|-------------------------|------------------------|-------------|---------------------------------------------------------------------------------------------|--------------------------|-------------------------------------------------------|------------------|------------------------------------------------|-------------------|---------------------------------------------------------------|
| Game engine             | Unity, Unreal, Godot   | Scene-based | Scene, nodes, camera, materials                                                             | Low                      | Mostly builtin; programmability varies                | Massive          | Excellent                                      | Very high         | Depends on how similar you are to optimized usecases          |
| Low-level APIs          | DX, Vulkan, Metal      | Pass-based  | Passes, shaders, buffers, textures                                                          | High                     | BYO, Extremely customizable                                | None             | Poor; write once, run once                     | Very low          | Extreme                                                       |
| Layered implementations | MoltenVK, Proton, wgpu | Pass-based  | Passes, shaders, buffers, textures                                                          | High                     | BYO, Customizable in theory, translation causes issues     | Some             | Good in theory, varies in practice             | Very low          | Excellent on native platforms, varies on translated platforms |
| Constructed APIs        | WebGPU                 | Pass-based  | Passes, shaders, buffers, textures                                                          | Medium-high              | BYO, Customizable, though many features stuck in committee | It's complicated | Some browser support, some translation support | Medium-low        | Good                                                          |
| GPU middleware          | images_and_words       | Pass-based  | Passes, shaders, camera, higher-order buffers and textures, multibuffering, common patterns | Medium-low               | BYO, Inherit from backends                                 | Some             | Good in theory, varies in practice             | Medium            | Good                                                          |

# Higher-order memory types

The main innovation of IW is providing an obvious family of higher-order kinds of buffers and textures.

These types are layered atop traditional GPU buffers/textures, but are customized
for specific usecases, such as multibuffering or synchronization.  Because each type encodes
its usecase information, the behavior can be optimized in a usecase-specific way.

Examples include:

| Class    | Use case       | Potential optimizations                 | Multibuffering | Synchronization      |
|----------|----------------|-----------------------------------------|----------------|----------------------|
| Static   | Sprites, etc   | Convert to a private, GPU-native format | Not needed     | Not needed           |
| Forward  | Write CPU->GPU | Unified vs discrete memory              | Builtin        | Builtin              |
| Reverse  | Write GPU->CPU | Unified vs discrete memory              | Builtin        | Builtin              |
| Sideways | Write GPU->GPU | private, GPU-native format              | Builtin        | TBD                  |


# Backends

In the interests of getting going, current development targets [wgpu](https://wgpu.rs)
as backend, so we inherit its broad support for DX12, Vulkan, Metal, WebGPU, Angle, WebGL, etc.

On the other hand, a career of more casual GPU programming has left me quite skeptical.  I am skeptical
of achieving native performance expectations atop a web-designed API, I am skeptical that wgpu's
actual contribution guidelines are too conservative for me to solve my problems via PRs, and I am skeptical
of any single graphics API, most of which have come and gone while I'm trying to support applications.

Part of my motivation for developing IW is an insurance policy against fickle graphics APIs, so I can
deal with all that in once place.  Accordingly, IW is inherently designed to be portable to other backends.
There aren't any published, although I have prototypes for Vulkan and Metal.

If you are similarly-situated, please contact me such as to join forces, but in any case,
patches to bring up new backends are welcome.

*/



mod entry_point;
pub mod images;
pub mod bindings;
pub mod pixel_formats;
mod imp;
mod multibuffer;
mod bittricks;
mod stable_address_vec;

pub use vectormatrix;

pub type Priority = some_executor::Priority;

