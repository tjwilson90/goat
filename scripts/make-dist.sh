#!/bin/bash

mkdir -p build/goat
cp target/x86_64-unknown-linux-musl/release/goat_server build/goat/goat
cp -r assets build/goat
cd build
tar czvf goat.tgz goat
tar tvf goat.tgz
