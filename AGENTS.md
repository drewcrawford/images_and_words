# Repository Guidelines

## Project Structure & Module Organization
- `src/`: Core library crate. `images/` handles rendering pipeline; `bindings/` hosts GPU buffer/texture abstractions; `pixel_formats/` defines typed pixel layouts; `imp/` contains backend glue; `lib.rs` wires the public API.
- `examples/`: Runnable demos (`simple_scene`, `animated_scene`). `build/wasm_example.sh` and `build/build_all_examples.sh` compile them to WebAssembly and write artifacts to `demo_site/<example>/build/`.
- `tests/`: Integration and performance-oriented suites (`buffer_performance`, `texture_alignment`, etc.). Doctests live alongside modules.
- `scripts/`: Automation entrypoints for linting, checks, docs, and tests across native and wasm32. `art/` stores logo/demo assets. `target/` is build output (ignore in PRs).

## Build, Test, and Development Commands
- Native build: `cargo build --features backend_wgpu` (default feature set). Add `,app_window` for windowed examples; add `,wgpu_webgl` to exercise WebGL.
- One-shot health check: `scripts/check_all` (fmt → native/wasm check → clippy → tests → docs) with `-D warnings`; append `--relaxed` to permit warnings.
- Focused steps: `scripts/check` (native + wasm32 checks), `scripts/tests` (native tests + `wasm-bindgen-test` via nightly), `scripts/clippy`, `scripts/fmt`, `scripts/docs`.
- Run demos: `cargo run --example simple_scene --features backend_wgpu,app_window` for native; `build/wasm_example.sh simple_scene` for browser builds (requires nightly toolchain + `wasm-bindgen` CLI).

## Coding Style & Naming Conventions
- Rust 2024 edition; rustfmt defaults (4-space indent). Keep code `-D warnings` clean; avoid blanket `allow` unless scoped and justified.
- Naming: modules/crates in `snake_case`; types/traits `UpperCamelCase`; functions/vars `snake_case`; constants `SCREAMING_SNAKE_CASE`. Prefer explicit feature gating and document required flags near APIs or examples that need them.
- Add concise comments only where GPU synchronization/backpressure or unsafe blocks are non-obvious.

## Testing Guidelines
- Run `scripts/tests` before opening PRs to cover native and wasm32 paths. On Linux CI, Wayland headless is handled by the script—no manual setup needed.
- Use `wasm-bindgen-test` friendly assertions; favor deterministic buffer/texture expectations over timing-based checks. Mirror test names after the function/type under test for searchability.
- When adding examples or platform-specific behavior, note required features (`backend_wgpu`, `app_window`, `wgpu_webgl`) and provide a minimal scene or fixture.

## Commit & Pull Request Guidelines
- Commit messages: short, imperative, scope-first (e.g., `add standard scripts`, `bump await_values`).
- PRs should describe behavior changes, affected targets (OS, GPU/driver, backend features), and linked issues. Include screenshots or short clips for rendering-visible changes and exact repro steps for regressions.
- Verify `scripts/check_all` is clean; if `--relaxed` was used, call it out in the PR text with rationale.
