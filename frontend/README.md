# Goat

React-based frontend for Goat.

### Build Instructions

This depends on the Goat WASM module for keeping track of the actual full Game state and computing
valid game actions, so you will need to also have `Cargo` installed.

```bash
./copy-wasm.sh # Build and copy wasm + wasm bindings into the source tree.
yarn run build
```