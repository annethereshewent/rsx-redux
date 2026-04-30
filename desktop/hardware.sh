echo "running game $1"

cargo run --release --no-default-features --features hardware_gpu,old_spu "$1"