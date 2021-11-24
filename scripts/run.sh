#!/bin/bash

wasm-pack build crates/goat_wasm --target web --no-typescript
rm -rf assets/wasm
cp -r crates/goat_wasm/pkg assets/wasm
cargo run --release

