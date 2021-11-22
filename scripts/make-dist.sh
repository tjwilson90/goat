#!/bin/bash

mkdir -p build/goat
cp target/x86_64-unknown-linux-musl/release/goat build/goat
cp -r assets build/goat
cd build
tar chzvf goat.tgz goat
