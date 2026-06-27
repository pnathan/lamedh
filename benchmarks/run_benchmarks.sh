#!/bin/bash
# Benchmark runner script for lamedh evaluator
# Compares performance of Rust, Python, and Lamedh (Lisp)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Benchmark parameters
RUN_MS=${RUN_MS:-5000}      # 5 seconds by default
WARMUP_MS=${WARMUP_MS:-1000} # 1 second warmup

echo "======================================"
echo "Lamedh Evaluator Benchmark Suite"
echo "======================================"
echo "Based on https://github.com/bddicken/languages"
echo ""
echo "Runtime: ${RUN_MS}ms (warmup: ${WARMUP_MS}ms)"
echo ""

# Build Rust benchmarks
echo -e "${BLUE}Building Rust benchmarks...${NC}"
(cd fibonacci/rust && cargo build --release) > /dev/null 2>&1
(cd loops/rust && cargo build --release) > /dev/null 2>&1
(cd levenshtein/rust && cargo build --release) > /dev/null 2>&1
echo -e "${GREEN}✓ Rust benchmarks built${NC}"
echo ""

# Build lamedh (if not already built)
echo -e "${BLUE}Building lamedh...${NC}"
(cd .. && cargo build --release) > /dev/null 2>&1
echo -e "${GREEN}✓ Lamedh built${NC}"
echo ""

# Fibonacci Benchmark
echo "======================================"
echo "FIBONACCI BENCHMARK (n=30)"
echo "======================================"
FIB_N=30

echo -e "${YELLOW}Rust:${NC}"
./fibonacci/rust/target/release/fibonacci_bench $RUN_MS $WARMUP_MS $FIB_N

echo -e "${YELLOW}Python:${NC}"
python3 ./fibonacci/python/fibonacci.py $RUN_MS $WARMUP_MS $FIB_N

echo -e "${YELLOW}Lamedh (Lisp):${NC}"
python3 ./fibonacci/lisp/benchmark.py $RUN_MS $WARMUP_MS $FIB_N
echo ""

# Loops Benchmark
echo "======================================"
echo "LOOPS BENCHMARK (divisor=10000)"
echo "======================================"
LOOPS_DIVISOR=10000

echo -e "${YELLOW}Rust:${NC}"
./loops/rust/target/release/loops_bench $RUN_MS $WARMUP_MS $LOOPS_DIVISOR

echo -e "${YELLOW}Python:${NC}"
python3 ./loops/python/loops.py $RUN_MS $WARMUP_MS $LOOPS_DIVISOR

echo -e "${YELLOW}Lamedh (Lisp):${NC}"
echo "(Standalone workload; no CSV timing harness yet)"
echo "Run: ../target/release/lamedh -i loops/lisp/loops.lisp -s '(loops-benchmark ${LOOPS_DIVISOR})'"
echo ""

# Levenshtein Benchmark
echo "======================================"
echo "LEVENSHTEIN BENCHMARK"
echo "======================================"
WORDS_FILE="levenshtein/words.txt"

echo -e "${YELLOW}Rust:${NC}"
./levenshtein/rust/target/release/levenshtein_bench $RUN_MS $WARMUP_MS $WORDS_FILE

echo -e "${YELLOW}Python:${NC}"
python3 ./levenshtein/python/levenshtein.py $RUN_MS $WARMUP_MS $WORDS_FILE

echo -e "${YELLOW}Lamedh (Lisp):${NC}"
echo "(Standalone workload; not directly comparable to Rust/Python string implementations yet)"
echo "Run: ../target/release/lamedh -i levenshtein/lisp/levenshtein.lisp -s \"(levenshtein-distance '(k i t t e n) '(s i t t i n g))\""
echo ""

echo "======================================"
echo "Benchmark Complete!"
echo "======================================"
echo ""
echo "CSV Output Format: mean_ms,std_dev_ms,min_ms,max_ms,iterations,result"
echo ""
echo "Lower mean_ms = faster performance"
echo "Lower std_dev_ms = more consistent performance"
