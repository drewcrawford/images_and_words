#!/bin/bash

# Build all WASM examples
EXAMPLES=("simple_scene" "animated_scene")

for example in "${EXAMPLES[@]}"; do
    echo "Building $example..."
    ./build/wasm_example.sh "$example"
    if [ $? -ne 0 ]; then
        echo "Failed to build $example"
        exit 1
    fi
done

echo "All examples built successfully!"