#!/bin/sh
if [ -z "$1" ]
then
    echo "Usage: rebuild-nixon.sh TARGET"
    exit 1
fi
cargo build \
    --release \
    --manifest-path nixon/Cargo.toml \
    --target "$1"
cp nixon/target/$1/release/nixon src
