/*! images_and_words is a GPU middleware and abstraction layer for high-performance
  graphics applications and games.

# The pitch

Suppose you want to write a game or graphics application.  You may consider:

* An off-the-shelf game engine, like Unity or Unreal.  But these might be much more engine than you need,
  be difficult to customize, have vendor lockin, make that one feature tough to optimize, etc.
* Writing directly to a low-level API like Vulkan, Metal, or DirectX.  But these are
  complex, verbose, and require you to solve many problems that have already been solved, let alone
  the hassle of multiplatform support.

Wouldn't it be nice to have a middle ground? And if you seriously look, those are out there.
Here is my chart:

| Strategy            | Examples             | API style   | API concepts                                                      | Synchronization concerns | Shaders                                 | Runtime size | Platform support                         | Development speed | Runtime speed                               |
|---------------------|----------------------|-------------|--------------------------------------------------------------------|--------------------------|------------------------------------------|--------------|-------------------------------------------|-------------------|-----------------------------------------------|
| Game engine         | Unity, Unreal, Godot | Scene-based | Scene, nodes, camera, materials                                     | Low                      | Mostly built-in; programmability varies  | Massive      | Excellent                                 | Very high         | Depends on how similar you are to optimized use cases |
| Low-level APIs      | DX, Vulkan, Metal    | Pass-based  | Passes, shaders, buffers, textures                                  | High                     | BYO, extremely customizable              | None         | Poor; write once, run once                | Very low          | Extreme                                        |
| Layered implementations | MoltenVK, Proton, wgpu | Pass-based  | Passes, shaders, buffers, textures                                  | High                     | BYO, customizable in theory; translation causes issues | Some         | Good in theory; varies in practice        | Very low          | Excellent on native platforms; varies on translated platforms |
| Constructed APIs    | WebGPU               | Pass-based  | Passes, shaders, buffers, textures                                  | Medium-high              | BYO, customizable, though many features stuck in committee | It’s complicated | Some browser support, some translation support | Medium-low      | Good                                          |
| GPU middleware      | images_and_words     | Pass-based  | Passes, shaders, camera, higher-order buffers and textures, multibuffering, common patterns | Medium-low              | BYO, inherit from backends               | Some         | Good in theory; varies in practice        | Medium            | Good                                          |

GPU middleware occupies a unique and overlooked niche in the ecosystem.  It provides a
cross-platform abstraction over GPU hardware, while also allowing you to bring your own
sound, physics, accessibility, and your entire existing codebase to the table.  These are the main
advantages of the middleware category as a whole.

Beyond the pros and cons of GPU middleware as a category, images_and_words is specifically the dream GPU API I
wanted in a career as a high-performance graphics application developer.  Often, the motivation for
GPU acceleration is we have some existing CPU code that we think is too slow, and we consider
some ways to improve it including GPU acceleration, but that might take a week to prototype on
one platform.  The major #1 goal of IW is to prototytpe GPU acceleration on various platforms
in a day or two at most.

A second major design goal is that eventually, you are likely to hit a second performance wall.
It should be easy to reason about the performance of IW applications, and to optimize
its primitives to meet your needs.  IW is designed to be a practical and performant target for my
own career of applications, and I hope it can be for yours as well.


# Higher-order memory types

The main innovation of IW is providing an obvious family of higher-order kinds of buffers and textures.

These types are layered atop traditional GPU buffers/textures, but are customized
for specific usecases, such as multibuffering or synchronization.  Because each type encodes
its usecase information, the behavior can be optimized in a usecase-specific way.

## The Three-Axis Type System

IW organizes GPU resources along three orthogonal axes, allowing you to select the precise
abstraction for your use case:

### 1. Resource Type Axis: Buffer vs Texture

**Buffers** provide:
- Arbitrary memory layouts with full programmer control
- Support for any type implementing `CRepr` trait
- Direct indexed access patterns
- Flexible size constraints
- Examples: vertex data, uniform blocks, compute storage

**Textures** provide:
- GPU-optimized storage for image data
- Hardware-accelerated sampling and filtering
- Fixed pixel formats (RGBA8, etc.)
- Spatial access patterns optimized for 2D/3D locality
- Examples: images, render targets, lookup tables

### 2. Mutability Axis: Static vs Dynamic

**Static** resources:
- Immutable after creation
- Optimized for many GPU reads per CPU upload
- Placed in GPU-only memory when possible
- Zero synchronization overhead
- Examples: mesh geometry, texture atlases

**Dynamic** resources:
- Mutable throughout lifetime
- Optimized for frequent CPU updates
- Automatic multibuffering to prevent stalls
- Transparent synchronization
- Examples: per-frame uniforms, streaming data

### 3. Direction Axis: Data Flow Patterns

| Direction | Flow | Use Cases | Status |
|-----------|------|-----------|---------|
| **Forward** | CPU→GPU | Rendering data, textures, uniforms | ✅ Implemented |
| **Reverse** | GPU→CPU | Screenshots, compute results, queries | ⏳ Planned |
| **Sideways** | GPU→GPU | Render-to-texture, compute chains | ⏳ Planned |
| **Omnidirectional** | CPU↔GPU | Interactive simulations, feedback | ⏳ Planned |

## Choosing the Right Type

To select the appropriate binding type:

1. **Identify data flow**: Where does data originate and where is it consumed?
2. **Determine update frequency**: Does it change every frame or remain constant?
3. **Consider access patterns**: Do you need shader sampling or structured access?

### Quick Decision Guide

| Your Use Case | Recommended Type |
|---------------|------------------|
| Mesh geometry that never changes | `bindings::forward::static::Buffer` |
| Textures loaded from disk | `bindings::forward::static::Texture` |
| Camera matrices updated per frame | `bindings::forward::dynamic::Buffer` |
| Render-to-texture targets | `bindings::forward::dynamic::FrameTexture` |
| Particle positions (CPU generated) | `bindings::forward::dynamic::Buffer` |
| Lookup tables for shaders | `bindings::forward::static::Buffer` or `Texture` |

## Implementation Status

Currently implemented:
- Forward Static Buffer ✅
- Forward Static Texture ✅
- Forward Dynamic Buffer ✅
- Forward Dynamic FrameTexture ✅

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

pub mod bindings;
mod bittricks;
mod entry_point;
pub mod images;
mod imp;
mod multibuffer;
pub mod pixel_formats;
mod send_phantom;
mod stable_address_vec;

pub use await_values::Observer;
pub use vectormatrix;

pub type Priority = some_executor::Priority;
pub type Strategy = vec_parallel::Strategy;
