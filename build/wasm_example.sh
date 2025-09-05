#!/bin/bash

# Variables
EXAMPLE_NAME="${1:-simple_scene}"
TARGET="wasm32-unknown-unknown"
PROFILE="wasm_release"
FEATURES="backend_wgpu,app_window,wgpu_webgl"
RUSTFLAGS="-Dwarnings -C target-feature=+atomics"

# Build command
RUSTFLAGS="$RUSTFLAGS" cargo +nightly build \
    --example "$EXAMPLE_NAME" \
    --target="$TARGET" \
    --profile="$PROFILE" \
    --features="$FEATURES"

# wasm-bindgen command
wasm-bindgen --target web \
    "target/$TARGET/$PROFILE/examples/$EXAMPLE_NAME.wasm" \
    --out-dir "demo_site/$EXAMPLE_NAME/build"