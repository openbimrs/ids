#!/usr/bin/env bash
# Complete standalone verification gate for openbimrs/ids.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo fmt --all -- --check
cargo build --workspace --all-targets
cargo test --workspace --all-features
# `audit-schema` alone must build and pass without the template catalog.
cargo test --workspace --features audit-schema
cargo clippy --workspace --all-targets --features audit-schema -- -D warnings
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo package -p openbim-ids
