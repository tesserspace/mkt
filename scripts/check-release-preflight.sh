#!/usr/bin/env bash
set -euo pipefail

export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

for package in mkt-types mkt-core mkt; do
  cargo package -p "$package" --allow-dirty --no-verify
  cargo publish -p "$package" --dry-run --allow-dirty
done
