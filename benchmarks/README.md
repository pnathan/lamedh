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
- **Note**: Lamedh currently has limited string support, so this benchmark is aspirational

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
cargo run --release -- -i benchmarks/fibonacci/lisp/fibonacci.lisp
```

#### Loops

```bash
# Rust
cd loops/rust
cargo build --release
./target/release/loops_bench 5000 1000 10000

# Python
python3 loops/python/loops.py 5000 1000 10000

# Lamedh
cargo run --release -- -i benchmarks/loops/lisp/loops.lisp
```

#### Levenshtein

```bash
# Rust
cd levenshtein/rust
cargo build --release
./target/release/levenshtein_bench 5000 1000 ../words.txt

# Python
python3 levenshtein/python/levenshtein.py 5000 1000 levenshtein/words.txt
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

1. **Lamedh Integration**: The Lisp benchmarks are currently standalone files. Full integration with the benchmark harness requires:
   - Adding benchmarking library support to Lamedh
   - Implementing timing functions in Lisp
   - Adding command-line argument parsing

2. **String Support**: Lamedh has limited string handling capabilities, so the Levenshtein benchmark is aspirational and demonstrates what would be needed for full string support.

3. **Optimization Level**: These benchmarks test the evaluator without any optimizations like:
   - Tail call optimization
   - Constant folding
   - Inline caching
   - JIT compilation

## Future Improvements

- [ ] Add benchmark harness to Lamedh for proper timing integration
- [ ] Implement string primitives for Levenshtein benchmark
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
