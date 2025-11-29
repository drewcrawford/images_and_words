# Changelog

All notable changes to images_and_words will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[Unreleased]: https://github.com/drewcrawford/images_and_words/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/drewcrawford/images_and_words/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/drewcrawford/images_and_words/releases/tag/v0.1.0
