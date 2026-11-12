#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../rust"

echo "==> cargo fmt --check"
cargo fmt --all -- --check

echo "==> cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo test"
cargo test --workspace

echo "OK: rust verify passed"
