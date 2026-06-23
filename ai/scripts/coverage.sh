#!/usr/bin/env bash
# Measure Rust test coverage. Goal: 95% line coverage.
#
#   ai/scripts/coverage.sh              # summary table
#   ai/scripts/coverage.sh --html       # browsable HTML report (target/llvm-cov)
#   ai/scripts/coverage.sh --open       # HTML + open in browser
#
# Prefers cargo-llvm-cov, falls back to cargo-tarpaulin.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

if cargo llvm-cov --version >/dev/null 2>&1; then
  case "${1:-}" in
    --html) exec cargo llvm-cov --html ;;
    --open) exec cargo llvm-cov --open ;;
    *)      exec cargo llvm-cov --summary-only ;;
  esac
elif cargo tarpaulin --version >/dev/null 2>&1; then
  exec cargo tarpaulin --skip-clean --out Stdout
else
  echo "No coverage tool found. Install one of:" >&2
  echo "  cargo install cargo-llvm-cov   (preferred)" >&2
  echo "  cargo install cargo-tarpaulin" >&2
  exit 1
fi
