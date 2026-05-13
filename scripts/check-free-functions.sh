#!/usr/bin/env bash
set -euo pipefail

while IFS= read -r file; do
  allow_rationale=false
  if sed -n '1,12p' "$file" | rg -q 'RATIONALE:'; then
    allow_rationale=true
  elif [ -n "${file%/*}" ] && [ -f "${file%/*}/mod.rs" ] && sed -n '1,12p' "${file%/*}/mod.rs" | rg -q 'RATIONALE:'; then
    allow_rationale=true
  fi

  ast-outline map "$file" | awk -v file="$file" -v allow_rationale="$allow_rationale" '
  function fail(symbol) {
    printf "free function outside helper: %s %s\n", symbol, file > "/dev/stderr";
    bad = 1;
  }

  BEGIN {
    is_test_file = (file ~ /\/tests?\.rs$/ || file ~ /\/.*\/tests\.rs$/);
    is_tool_file = (file ~ /\/internal\.rs$/ || file ~ /\/internal\/.*\.rs$/ || file ~ /\/helper\.rs$/ || file ~ /\/helper\/.*\.rs$/);
  }

  /^[^[:space:]]+[[:space:]]+(pub(\([^)]+\))?[[:space:]]+)?fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\(/ {
    if (is_test_file || is_tool_file || allow_rationale == "true") next;

    symbol = $0;
    sub(/[[:space:]]+L[0-9].*$/, "", symbol);
    fail(symbol);
  }

  END {
    exit bad;
  }
  '
done < <(rg --files crates -g '*.rs')
