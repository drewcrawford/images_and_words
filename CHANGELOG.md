# Changelog

All notable changes to images_and_words will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **WASM thread model migration** - Continued migration to `wasm_safe_thread` internals to tighten correctness around thread-bound GPU state on WebAssembly targets.
- **Chrome webdriver tuning for wasm-bindgen tests** - Updated `webdriver.json` flags for a setup that works in both local development and CI by using a SwiftShader Vulkan path.

### Fixed
- **Cross-environment WASM test stability** - Resolved a split-brain browser config where one flag set passed CI but failed locally (`./scripts/wasm32/tests`). Current flags now pass both local runs and Gitea CI.

## [0.3.0] - 2025-12-20

### Breaking Changes
- **CPUWriteAccess::write() is now async** - As part of the buffer optimization work, the `write()` method on `CPUWriteAccess` now returns a Future and must be awaited. This enables direct writes to GPU staging buffers via `write_buffer_with()`, eliminating an intermediate copy. Update your buffer writes from `write_access.write(data, offset)` to `write_access.write(data, offset).await`.
- **Removed Index trait from CPUWriteAccess** - You can no longer read from `CPUWriteAccess` using index notation (e.g., `write_access[0]`). The new design writes directly to GPU buffers without maintaining a CPU-side copy, making read access impossible. If you need to read buffer data, use a different buffer access pattern or maintain your own CPU-side copy.

### Added
- **Tracked thread mode for WASM** - WgpuCell now tracks which thread created it and verifies all accesses come from the same thread on wasm32 targets. This catches threading bugs early instead of letting them cause mysterious hangs in the browser. The default strategy automatically picks the right mode for your platform—Tracked on WASM, Relaxed everywhere else.
- **Broader await_values API export** - We now re-export the entire `await_values` crate instead of just the `Observer` type. This gives you access to all the value-watching goodness without manually adding the dependency to your Cargo.toml.
- **New benchmarks** - Added frame texture acquisition benchmarks to help us (and you) track rendering pipeline performance over time.

### Changed
- **Blazing fast buffer and texture uploads** - Completely reworked how we handle GPU data transfers. Buffer writes now happen in-place using `write_buffer_with`, texture copies skip unnecessary staging buffers, and we've optimized the write paths to minimize allocations. In benchmarks, these changes shaved significant time off upload-heavy workloads.
- **Port uses interior mutability** - The Port API now uses `&self` instead of `&mut self`, which fixes some gnarly hanging issues (particularly mt2-831) and makes the API more ergonomic. You can now share ports more freely without fighting the borrow checker.
- **Upgraded to wgpu 28** - Brought in the latest wgpu release with its performance improvements and API refinements. This includes updates to pipeline layout descriptors (goodbye `push_constant_ranges`, hello `immediate_size`) and multiview handling.
- **Dependency refresh** - Updated `test_executors` (0.4.0 → 0.4.1), `continue` (0.1.1 → 0.1.2), `app_window` (0.3.1 → 0.3.2), `send_cells` (0.2.0 → 0.2.1), and `logwise` (0.4.0 → 0.5.0). Staying fresh keeps the ecosystem happy.
- **Better logging domains** - Added proper logwise logging domains throughout the codebase, making it easier to filter and understand what's happening during rendering.

### Fixed
- **WASM threading reliability** - The new Tracked mode catches thread-safety violations on WASM that previously led to silent hangs or cryptic browser errors. If you're accessing GPU state from the wrong thread, you'll now get a clear assertion instead of mysterious timeouts.

## [0.2.0] - 2025-11-29

### Added
- **Debugging superpowers** - Integrated exfiltrate for capturing and analyzing GPU state. When things get weird, we can now peek under the hood and see exactly what's happening on the GPU side. This is a game-changer for tracking down those "works on my machine" rendering glitches.
- **Standard scripts** - Added the full suite of quality-of-life scripts (`scripts/check`, `scripts/tests`, `scripts/clippy`, etc.) so you can validate your code without memorizing incantations.
- **SPDX identifiers** - Proper license metadata throughout the codebase, because lawyers appreciate clarity even if the rest of us find it tedious.
- **Better documentation** - Expanded API docs and examples to help you get started faster. We want you building cool things, not scratching your head.

### Changed
- **Upgraded to wgpu 27** - Brought in the latest wgpu goodness with all its improvements and fixes. Your GPU will thank us.
- **Dependency refresh** - Updated `await_values` (0.1 → 0.2), `test_executors` (0.3 → 0.4), `logwise` (0.3 → 0.4), and `app_window` to their latest versions. Staying current keeps the bugs at bay.
- **Cleaned up internal threading** - Removed some unnecessary synchronization primitives and fixed a few spots where we were being overly cautious with mutexes. The result? Slightly snappier performance and cleaner code.
- **PortReporter is no longer UnwindSafe** - This struct now correctly advertises that it contains non-unwind-safe internals. If you were relying on panic recovery around it (and honestly, who was?), you'll need to adjust.

### Removed
- **Dropped tgar dependency** - Said goodbye to the `dump_tga()` convenience method and its pixel format conversions. This was mostly used for debugging anyway, and exfiltrate provides better tools for that now. If you were using this in production... we should talk.

### Fixed
- **CI reliability** - Squashed various issues that were making automated builds cranky. Builds should be smoother now.
- **WASM link arguments** - Added some mysterious-but-necessary linker flags that make WebAssembly builds actually work. Don't ask us to explain them; we just know they're needed.

## [0.1.0] - 2025-11-17

Initial release of images_and_words - GPU middleware that finds the sweet spot between "here's a pile of GPU primitives, good luck" and "here's a 50GB game engine that does everything."

### What's In The Box
- **Higher-order GPU resource types** - Buffers and textures that know how to multibuffer themselves and avoid pipeline stalls
- **Three-axis type system** - Pick your poison: Buffer/Texture × Static/Dynamic × Forward (CPU→GPU)
- **wgpu backend** - Broad platform support from Windows to macOS to WebAssembly
- **Examples and documentation** - Get up and running with simple_scene and animated_scene
- **Performance by default** - Automatic multibuffering, smart memory placement, and minimal synchronization overhead

[Unreleased]: https://github.com/drewcrawford/images_and_words/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/drewcrawford/images_and_words/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/drewcrawford/images_and_words/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/drewcrawford/images_and_words/releases/tag/v0.1.0
