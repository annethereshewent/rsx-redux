#!/bin/bash

FEATURES="hardware_gpu"

if [[ "$2" == "--debug" ]]; then
    FEATURES="$FEATURES,debug"
fi

cargo run --release --no-default-features --features "$FEATURES" "$1"