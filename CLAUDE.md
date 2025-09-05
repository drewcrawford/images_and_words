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

### macOS-specific requirements
On macOS, set the deployment target: `export MACOSX_DEPLOYMENT_TARGET=15`

### Building
**Build:** `cargo build --features=backend_wgpu`

**Build with app window support:** `cargo build --features=backend_wgpu,app_window`

**Build for WASM:** `./build/wasm_example.sh simple_scene`

### Testing
**Run all tests:** `cargo test --features=backend_wgpu,testing`

**Run single test:** `cargo test --features=backend_wgpu,testing test_name`

**Run specific test file:**
* `cargo test --features=backend_wgpu,testing --test buffer_performance`
* `cargo test --features=backend_wgpu,testing --test sendable_futures`
* `cargo test --features=backend_wgpu,testing --test texture_alignment`
* `cargo test --features=backend_wgpu,testing --test wgpu_cell_threading_error`

### Linting and Validation
**Run clippy:** `cargo clippy --features=backend_wgpu`

**Format check:** `cargo fmt --check`

**Quick check script (runs all validations):** `./quickcheck.sh`

### Documentation
**Build and open docs:** `cargo doc --features=backend_wgpu --no-deps --open`

### Examples
**Run simple scene:** `cargo run --example simple_scene --features=backend_wgpu,app_window`

**Run animated scene:** `cargo run --example animated_scene --features=backend_wgpu,app_window`

## Feature Flags

* `backend_wgpu` - Enables the wgpu GPU backend (required for most development)
* `app_window` - Enables window surface creation for applications
* `testing` - Enables testing utilities
* `wgpu_webgl` - Enables WebGL backend for wgpu (for web targets)
* `logwise_internal` - Internal logging features

## WASM/WebAssembly Support

The project supports WebAssembly targets with special configuration:
* Uses `wasm32-unknown-unknown` target
* Requires nightly Rust for atomics support
* Build with: `./build/wasm_example.sh [example_name]`
* Cargo config enables atomics: `-C target-feature=+atomics`

## Logging

Uses `logwise` for logging. Example syntax:
```rust
logwise::info_sync!("Here is foo: {foo}", foo=3);
```

Complex types require coercion through `logwise::privacy`:
```rust
logwise::warn_sync!("Here is foo: {foo}", foo=logwise::privacy::LogIt(example));
```


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
# important-instruction-reminders
Do what has been asked; nothing more, nothing less.
NEVER create files unless they're absolutely necessary for achieving your goal.
ALWAYS prefer editing an existing file to creating a new one.
NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.