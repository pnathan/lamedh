#!/usr/bin/env bash
# Compare local Fibonacci implementations: C, SBCL, Ruby, Python, and Lamedh.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

RUN_MS="${RUN_MS:-1000}"
WARMUP_MS="${WARMUP_MS:-100}"
N="${1:-30}"

echo "Fibonacci local comparison"
echo "n=${N} run_ms=${RUN_MS} warmup_ms=${WARMUP_MS}"
echo "CSV: mean_ms,std_dev_ms,min_ms,max_ms,iterations,result"
echo

if command -v cc >/dev/null 2>&1; then
  cc -O3 -march=native "$SCRIPT_DIR/c/fibonacci.c" -lm -o "$SCRIPT_DIR/c/fibonacci_bench"
  printf "%-8s " "C"
  "$SCRIPT_DIR/c/fibonacci_bench" "$RUN_MS" "$WARMUP_MS" "$N"
fi

if command -v sbcl >/dev/null 2>&1; then
  printf "%-8s " "SBCL"
  sbcl --script "$SCRIPT_DIR/sbcl/fibonacci.lisp" "$RUN_MS" "$WARMUP_MS" "$N"
fi

if command -v ruby >/dev/null 2>&1; then
  printf "%-8s " "Ruby"
  ruby "$SCRIPT_DIR/ruby/fibonacci.rb" "$RUN_MS" "$WARMUP_MS" "$N"
fi

if command -v python3 >/dev/null 2>&1; then
  printf "%-8s " "Python"
  python3 "$SCRIPT_DIR/python/fibonacci.py" "$RUN_MS" "$WARMUP_MS" "$N"
fi

if [[ ! -x "$ROOT_DIR/target/release/lamedh" ]]; then
  (cd "$ROOT_DIR" && cargo build --release >/dev/null)
fi

cargo build --release --manifest-path "$SCRIPT_DIR/lamedh-warm/Cargo.toml" >/dev/null
printf "%-8s " "Lamedh-JIT"
"$SCRIPT_DIR/lamedh-warm/target/release/lamedh_fibonacci_warm" "$RUN_MS" "$WARMUP_MS" "$N"
printf "%-8s " "Lamedh-OptJIT"
"$SCRIPT_DIR/lamedh-warm/target/release/lamedh_fibonacci_warm" "$RUN_MS" "$WARMUP_MS" "$N" opt

echo
echo "Note: the Lamedh-JIT row embeds Lamedh, defines typed functions once,"
echo "warms them, parses the call once, and times already-warm native JIT calls."
echo "Lamedh-OptJIT uses deffun-typed-opt: Lisp/vau source optimization first,"
echo "then the same HM typed compiler and native JIT."
