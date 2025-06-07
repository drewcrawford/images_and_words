# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# images_and_words

This program is a GPU middleware and abstraction layer for high-performance graphics applications and games.

## Architecture

The codebase uses a layered architecture where public APIs forward implementation to backend implementations:

* **nop backend** - Stub implementation for starting new backends  
* **wgpu backend** - Main production backend (enabled with `backend_wgpu` feature)

### Three-Axis Type System for GPU Resources

Resources are categorized along three dimensions:
1. **Resource Type**: Buffer vs Texture
2. **Mutability**: Static (immutable) vs Dynamic (mutable)
3. **Direction**: Forward (CPU→GPU), Reverse (GPU→CPU), Sideways (GPU→GPU)

### Key modules:
* `src/images/` - Core rendering APIs (Engine, render passes, shaders, views)
* `src/bindings/` - Buffer and texture binding types with higher-order memory management
* `src/imp/` - Backend implementations (nop, wgpu)
* `src/pixel_formats/` - Type-safe pixel format definitions
* `src/multibuffer.rs` - Automatic multibuffering system for dynamic resources

The main innovation is providing higher-order buffer/texture types optimized for specific use cases with built-in multibuffering and synchronization. This prevents common GPU pipeline stalls by allowing the CPU to write while the GPU reads.

## Development Commands

**Build:** `cargo build --features=backend_wgpu`

**Run tests:** `cargo test --features=testing,backend_wgpu`

**Run single test:** `cargo test --features=testing,backend_wgpu test_name`

**Build with app window support:** `cargo build --features=backend_wgpu,app_window`

**Check documentation:** `cargo doc --features=backend_wgpu --no-deps --open`

## Feature Flags

* `backend_wgpu` - Enables the wgpu GPU backend (required for most development)
* `app_window` - Enables window surface creation for applications
* `testing` - Enables testing APIs used by integration tests


# Preserve this generic section unedited.
v1
## Async

This project does not use tokio.

If you are writing test code, peruse the test_executors documentation at `target/doc/test_executors`.  This shows the API we use for tests.  Feel free to suggest new API to test_executors if that would help.

If you are writing production code, peruse the some_executor crate at `target/doc/some_executor`.  This shows the API we use to spawn tasks from production code.  Feel free to suggest new API to some_executor if that would help.

The executors themselves are abstracted behind other crates.  But they are probably one of:
* `some_global_executor`
* `some_local_executor`

## Crates

If you want to know about the behavior of dependent crates, look in the `target/doc/{crate_name}` directory.  Note this directory can be built with `cargo doc` if it is out of date, but that requires the crate to compile first.