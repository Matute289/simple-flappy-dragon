#!/bin/bash
# Build Flappy Dragon for WASM and serve locally
# Requires: wasm-pack (cargo install wasm-pack)
set -e

echo "Building for WASM..."
wasm-pack build --target web --out-dir web/pkg

echo ""
echo "Build complete. Starting local server..."
echo "Open http://localhost:8080 in your browser."
python3 -m http.server 8080 --directory web
