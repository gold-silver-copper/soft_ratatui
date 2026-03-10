#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "$0")/.."

run() {
  echo
  echo "==> $*"
  "$@"
}

run cargo fmt --all -- --check

feature_sets=(
  "--no-default-features"
  ""
  "--no-default-features --features unicodefonts"
  "--no-default-features --features bdf-parser"
  "--no-default-features --features embedded-ttf"
  "--no-default-features --features cosmic-text"
  "--all-features"
)

for args in "${feature_sets[@]}"; do
  if [[ -n "$args" ]]; then
    # shellcheck disable=SC2206
    split_args=($args)
    run cargo test -p soft_ratatui --lib --tests "${split_args[@]}"
    run cargo test -p soft_ratatui --doc "${split_args[@]}"
  else
    run cargo test -p soft_ratatui --lib --tests
    run cargo test -p soft_ratatui --doc
  fi
done

run cargo doc -p soft_ratatui --all-features --no-deps
run cargo check --workspace --all-targets
