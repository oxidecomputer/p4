#!/bin/bash
#:
#: name = "build-and-test"
#: variety = "basic"
#: target = "helios-2.0"
#: rust_toolchain = "stable"
#: output_rules = [
#:   "/work/debug/*",
#:   "/work/release/*",
#: ]
#:

set -o errexit
set -o pipefail
set -o xtrace

cargo --version
rustc --version

banner "check"
cargo fmt -- --check
cargo check
cargo clippy --all-targets -- --deny warnings

banner "build"
ptime -m cargo build
ptime -m cargo build --release

for x in debug release
do
    mkdir -p /work/$x
    cp target/$x/x4c /work/$x/
    cp target/$x/libsidecar_lite.so /work/$x/
done

banner "test"

cargo test

pushd test

banner "mac rewr"
RUST_BACKTRACE=1 cargo test mac_rewrite -- --nocapture

banner "dyn load"
RUST_BACKTRACE=1 cargo test dload -- --nocapture

banner "disag"
RUST_BACKTRACE=1 cargo test disag_router -- --nocapture

banner "dyn rtr"
RUST_BACKTRACE=1 cargo test dynamic_router -- --nocapture

banner "hub"
RUST_BACKTRACE=1 cargo test hub -- --nocapture

banner "router"
RUST_BACKTRACE=1 cargo test basic_router -- --nocapture

banner "headers"
RUST_BACKTRACE=1 cargo test headers -- --nocapture
