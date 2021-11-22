#!/bin/bash

wasm-pack build crates/goat_wasm --target web --no-typescript
cargo run --release

