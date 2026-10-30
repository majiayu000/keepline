#!/bin/sh
# Rebuilds sidebar_status.wasm, the module sidebar.json points at.
set -eu
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/sidebar_status.wasm sidebar_status.wasm
