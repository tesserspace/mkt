#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings -D clippy::too_many_arguments
"$(dirname "$0")"/check-free-functions.sh
