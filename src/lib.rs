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

On the other hand, I have intentionally designed IW to support multiple backends, and have prototyped
Vulkan and Metal-based approaches myself.  I intend to stand up other backends as I need them.  If
you need them before I do, get in touch.

Longer-term I am skeptical of wgpu as a backend.  I am skeptical I can meet native performance expectations
with a web-based API, I am skeptical of wgpu's guidance on accepting contributions to solve these issues,
and I am skeptical of any single graphics API as I've seen them come and go while I'm supporting an
application.

A substantial motivation for creating IW is to design an API that can solve these problems and
become a practical and performant target for my own applications.  In the short term, I need
features/optimizations that don't happen in design-by-committee APIs.  In the long term, I need to
maintain my applications after APIs have been deprecated.  IW is the middleware to bridge
this gap in one place.

# Contributions

If you are motivated enough to consider writing your own solution, I would love to have your help
here instead.



*/



mod entry_point;
pub mod images;
pub mod bindings;
pub mod pixel_formats;
mod imp;
mod multibuffer;
mod bittricks;
mod stable_address_vec;
mod send_phantom;

pub use vectormatrix;

pub type Priority = some_executor::Priority;

