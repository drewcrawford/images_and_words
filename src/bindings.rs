// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*! GPU resource binding types organized along three conceptual axes.

# Overview

The bindings module provides types for managing data transfer between CPU and GPU memory.
These types are organized along three orthogonal axes that help you choose the right
abstraction for your use case:

## The Three Axes

### 1. Resource Type Axis: Buffer vs Texture

**Buffers** are best when you need:
- Arbitrary data layouts with programmer control
- Any type implementing `CRepr` (C-compatible memory layout)
- Flexible size constraints
- Direct memory access patterns

**Textures** are best when you need:
- GPU shader sampling capabilities
- Hardware-accelerated filtering and interpolation
- Fixed pixel formats (RGBA8, etc.)
- Optimized 2D/3D spatial access patterns

Note: Textures may use private hardware formats internally, offering better performance
at the cost of conversion overhead.

### 2. Mutability Axis: Static vs Dynamic

**Static** resources are:
- Immutable after creation
- Optimized for many GPU reads per CPU upload
- Best for data that rarely or never changes (meshes, lookup tables)
- More efficient for GPU access

**Dynamic** resources are:
- Mutable after creation
- Optimized for frequent CPU updates
- Best for per-frame data (uniforms, streaming vertices)
- May use multibuffering for performance

### 3. Direction Axis: Forward, Reverse, Sideways, Omnidirectional

**Forward** (CPU→GPU):
- Send data from CPU to GPU
- Most common pattern for rendering
- Currently implemented

**Reverse** (GPU→CPU) - *planned*:
- Read data back from GPU
- For compute results, screenshots

**Sideways** (GPU→GPU) - *planned*:
- Transfer between GPU resources
- For render-to-texture, compute pipelines

**Omnidirectional** (CPU↔GPU) - *planned*:
- Bidirectional data flow
- For interactive compute, feedback systems

## Choosing the Right Type

To select the appropriate binding type, consider:

1. **Data flow direction**: Are you sending data to GPU (forward), reading back (reverse), or both?
2. **Update frequency**: Does the data change every frame (dynamic) or rarely (static)?
3. **Access pattern**: Do you need shader sampling (texture) or structured data (buffer)?

### Common Patterns

- **Vertex/Index data**: `forward::static::Buffer` (meshes don't change)
- **Per-frame uniforms**: `forward::dynamic::Buffer` (updates each frame)
- **Texture assets**: `forward::static::Texture` (images loaded once)
- **Render targets**: `forward::dynamic::Texture` (rendered each frame)

## Module Organization

This module is organized hierarchically by direction, then by mutability:
- `forward/` - CPU to GPU transfers
  - `static/` - Immutable resources
  - `dynamic/` - Mutable resources
- `software/` - CPU-side texture operations
- Additional utilities for binding, visibility, and resource tracking

*/

pub mod bind_style;
pub mod forward;

pub use bind_style::BindStyle;
pub(crate) mod buffer_access;
pub mod coordinates;
pub(crate) mod dirty_tracking;
pub mod resource_tracking;
pub mod sampler;
pub mod software;
pub mod visible_to;
