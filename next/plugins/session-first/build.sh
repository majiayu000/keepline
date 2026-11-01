#!/bin/sh
# Rebuilds session_first.wasm, the module sidebar.json points at.
set -eu
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/session_first.wasm session_first.wasm
