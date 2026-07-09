# build-rust.sh

#!/bin/bash

set -e

THISDIR=$(dirname $0)
cd $THISDIR

# Build the project for the desired platforms:
cargo build --release --no-default-features --features hardware_gpu_metal --target x86_64-apple-darwin
cargo build --release --no-default-features --features hardware_gpu_metal --target aarch64-apple-darwin
mkdir -p ./target/universal-macos/release

lipo \
    ./target/aarch64-apple-darwin/release/librsx_redux_macos.a \
    ./target/x86_64-apple-darwin/release/librsx_redux_macos.a -create -output \
    ./target/universal-macos/release/librsx_redux_macos.a

cargo build --release --no-default-features --features hardware_gpu_metal --target aarch64-apple-ios

cargo build --release --no-default-features --features hardware_gpu_metal --target x86_64-apple-ios
cargo build --release --no-default-features --features hardware_gpu_metal --target aarch64-apple-ios-sim
mkdir -p ./target/universal-ios/release

lipo \
    ./target/aarch64-apple-ios-sim/release/librsx_redux_macos.a \
    ./target/x86_64-apple-ios/release/librsx_redux_macos.a -create -output \
    ./target/universal-ios/release/librsx_redux_macos.a