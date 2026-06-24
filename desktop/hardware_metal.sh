#!/bin/bash

FEATURES="hardware_gpu_metal"

if [[ "$2" == "--debug" ]]; then
    FEATURES="$FEATURES,debug"
fi

cargo run --release --no-default-features --features "$FEATURES" "$1"