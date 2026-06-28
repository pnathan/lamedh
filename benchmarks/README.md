# Lamedh Evaluator Benchmarks

This directory contains benchmarks for comparing the performance of the Lamedh (Lisp 1.5) evaluator against Rust and Python implementations.

## Overview

These benchmarks are based on the [languages benchmark suite](https://github.com/bddicken/languages) by @bddicken, which provides standardized microbenchmarks for comparing programming language performance.

## Benchmarks

### 1. Fibonacci
- **Description**: Calculates Fibonacci numbers using naive recursion
- **Test**: Sum of fibonacci(1) through fibonacci(n-1) where n=30
- **Purpose**: Tests recursive function call overhead and basic arithmetic

### 2. Loops
- **Description**: Nested loop performance test
- **Test**: 10,000 outer iterations × 100,000 inner iterations
- **Purpose**: Tests loop performance and array operations

### 3. Levenshtein Distance
- **Description**: Calculates edit distance between strings
- **Test**: Pairwise comparison of words from input file
- **Purpose**: Tests dynamic programming and string operations
- **Note**: A Lisp implementation exists, but it is a standalone workload and
  is not yet wired into the CSV benchmark runner.

## Structure

```
benchmarks/
├── fibonacci/
│   ├── rust/          # Rust implementation
│   ├── python/        # Python implementation
│   └── lisp/          # Lamedh (Lisp) implementation
├── loops/
│   ├── rust/
│   ├── python/
│   └── lisp/
├── levenshtein/
│   ├── rust/
│   ├── python/
│   ├── lisp/
│   └── words.txt      # Test data
├── run_benchmarks.sh  # Main benchmark runner
└── README.md          # This file
```

## Running Benchmarks

### Quick Start

```bash
cd benchmarks
./run_benchmarks.sh
```

### Custom Parameters

You can customize the benchmark duration:

```bash
# Run for 10 seconds with 2 second warmup
RUN_MS=10000 WARMUP_MS=2000 ./run_benchmarks.sh
```

### Individual Benchmarks

#### Fibonacci

```bash
# Rust
cd fibonacci/rust
cargo build --release
./target/release/fibonacci_bench 5000 1000 30

# Python
python3 fibonacci/python/fibonacci.py 5000 1000 30

# Lamedh
python3 fibonacci/lisp/benchmark.py 5000 1000 30

# Local comparison: C, SBCL, Ruby, Python, warm typed Lamedh JIT,
# and warm Lamedh after the Lisp/vau optimizer-to-compiler bridge.
RUN_MS=1000 WARMUP_MS=100 ./fibonacci/compare_local.sh 30
```

`Lamedh-JIT` embeds the library, defines typed functions once, warms them,
parses the call once, and times already-warm native typed calls. `Lamedh-OptJIT`
uses `deffun-typed-opt`, which runs Lisp/vau source optimization first and then
hands the optimized `deffun-typed` form to the same HM typed compiler.

#### Loops

```bash
# Rust
cd loops/rust
cargo build --release
./target/release/loops_bench 5000 1000 10000

# Python
python3 loops/python/loops.py 5000 1000 10000

# Lamedh
../target/release/lamedh -i loops/lisp/loops.lisp -s "(loops-benchmark 10000)"
```

#### Levenshtein

```bash
# Rust
cd levenshtein/rust
cargo build --release
./target/release/levenshtein_bench 5000 1000 ../words.txt

# Python
python3 levenshtein/python/levenshtein.py 5000 1000 levenshtein/words.txt

# Lamedh standalone workload
../target/release/lamedh -i levenshtein/lisp/levenshtein.lisp -s "(levenshtein-distance '(k i t t e n) '(s i t t i n g))"
```

## Output Format

All benchmarks output results in CSV format:

```
mean_ms,std_dev_ms,min_ms,max_ms,iterations,result
```

Where:
- `mean_ms`: Average execution time in milliseconds
- `std_dev_ms`: Standard deviation of execution times
- `min_ms`: Minimum execution time
- `max_ms`: Maximum execution time
- `iterations`: Number of benchmark iterations completed
- `result`: Verification value (to ensure correctness)

## Understanding Results

- **Lower mean_ms** = faster performance
- **Lower std_dev_ms** = more consistent performance
- **Higher iterations** (for same runtime) = faster performance

## Current Limitations

1. **Lamedh Integration**: Fibonacci has a Python harness that emits CSV for
   the Lisp implementation. Loops and Levenshtein are still standalone Lisp
   workloads rather than timed CSV harnesses.

2. **Levenshtein Representation**: The Lisp Levenshtein workload operates on
   list-shaped character data rather than the same string/array representation
   as the Rust and Python versions, so it is useful as a workload but not yet a
   strict apples-to-apples benchmark.

3. **Optimization Level**: Release builds include the current interpreter
   optimizations, TCO where implemented, and the default typed-JIT feature. Most
   benchmark files are still ordinary untyped Lisp and do not automatically use
   native typed kernels unless the workload explicitly opts into them.

## Future Improvements

- [ ] Add CSV harnesses for the remaining Lamedh workloads
- [ ] Port the Levenshtein workload to the current string/array APIs
- [ ] Add more benchmarks (hash tables, tree operations, etc.)
- [ ] Create performance regression tracking
- [ ] Add optimization levels comparison
- [ ] Generate comparative charts and graphs

## Contributing

When adding new benchmarks:

1. Implement in all three languages (Rust, Python, Lisp)
2. Use the same algorithm and approach across implementations
3. Follow the CSV output format for consistency
4. Update this README with the new benchmark description
5. Add the benchmark to `run_benchmarks.sh`

## License

These benchmarks are based on the [languages](https://github.com/bddicken/languages) project and follow the same approach for fair comparison.
