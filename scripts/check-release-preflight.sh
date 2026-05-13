#!/usr/bin/env bash
set -euo pipefail

export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

cargo package -p mkt-types --allow-dirty --no-verify
cargo publish -p mkt-types --dry-run --allow-dirty

cargo package -p mkt-core --allow-dirty --no-verify
cargo package -p mkt --allow-dirty --no-verify
