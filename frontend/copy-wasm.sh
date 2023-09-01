#!/usr/bin/env bash
# Builds the Goat WASM file and adds it to static resources along with the wrapper JS code.

# Exits the shell script with a message.
function die() {
    echo "$1"
    exit 1
}

if ! command -v cargo &> /dev/null; then
    echo "Cargo (for Rust) is not installed; please install it so the WASM file can be built."
    exit
fi

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
cd $SCRIPT_DIR/../crates/goat_wasm || die "Could not find $SCRIPT_DIR/crates/goat_wasm to build WASM module"

cargo build --release --target wasm32-unknown-unknown

cd $SCRIPT_DIR
cp $SCRIPT_DIR/../crates/goat_wasm/pkg/goat_wasm_bg.wasm $SCRIPT_DIR/static
echo "Copied WASM file to static/wasm"

mkdir -p $SCRIPT_DIR/src/generated
cp $SCRIPT_DIR/../crates/goat_wasm/pkg/goat_wasm.js $SCRIPT_DIR/src/generated
echo "Copied WASM bindings to src/generated"