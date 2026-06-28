# Lamedh Benchmark Results

Benchmark results comparing lamedh (Lisp 1.5 interpreter) against Rust (compiled) and Python 3.11 (interpreted).

These numbers are a historical snapshot. Re-run `benchmarks/run_benchmarks.sh`
on the target machine before using them for current performance claims.

## Local Fibonacci Comparison (n=30, warm typed JIT)

Command:

```bash
RUN_MS=1000 WARMUP_MS=100 ./benchmarks/fibonacci/compare_local.sh 30
```

Environment on this laptop:
- C compiler: GCC 13.3.0 via `cc -O3 -march=native`
- SBCL: 2.2.9
- Ruby: 3.0.1
- Python: 3.12.3
- Lamedh: embedded warm typed functions, native JIT enabled

| Language | Mean (ms) | Std Dev (ms) | Min (ms) | Max (ms) | Iterations | Result |
|----------|-----------|--------------|----------|----------|------------|--------|
| **C** | 2.682 | 0.133 | 2.553 | 3.612 | 373 | 1,346,268 |
| **SBCL** | 9.072 | 0.643 | 7.999 | 12.000 | 111 | 1,346,268 |
| **Lamedh-JIT** | 14.637 | 0.277 | 14.179 | 15.670 | 69 | 1,346,268 |
| **Lamedh-OptJIT** | 15.770 | 0.847 | 14.997 | 20.807 | 64 | 1,346,268 |
| **Ruby** | 154.654 | 5.081 | 150.328 | 162.527 | 7 | 1,346,268 |
| **Python** | 243.829 | 5.161 | 237.803 | 250.952 | 5 | 1,346,268 |

`Lamedh-JIT` defines `deffun-typed` functions once, warms them, parses the
call once, and times already-warm native typed calls. `Lamedh-OptJIT` uses
`deffun-typed-opt`, which runs the Lisp/vau source optimizer first and then
hands the optimized form to the same HM typed compiler. On this recursive
Fibonacci workload the optimizer has little source-level work to do, so the two
Lamedh rows should be read as effectively the same tier within run noise.

Ratios from this run: Lamedh-JIT is about 5.5× slower than C, 1.6× slower than
SBCL, 10.6× faster than Ruby, and 16.7× faster than Python.

**Test Environment:**
- Python: 3.11.14
- Rust: 1.x (release mode with optimizations)
- Lamedh: 0.1.0 historical release-mode snapshot. Current Lamedh is 0.2.x;
  release builds include the current evaluator optimizations and the default
  typed-JIT feature, although these untyped benchmark workloads generally do not
  opt into typed native kernels.

**Benchmark Parameters:**
- Runtime: 1000ms (1 second)
- Warmup: 100ms
- Each benchmark runs as many iterations as possible in the time window

---

## Fibonacci Benchmark (n=20)

Naive recursive Fibonacci calculation, summing fibonacci(1) through fibonacci(19).

| Language | Mean (ms) | Std Dev (ms) | Min (ms) | Max (ms) | Iterations | Result |
|----------|-----------|--------------|----------|----------|------------|--------|
| **Rust** | 0.043 | 0.004 | 0.041 | 0.146 | 23,100 | 10,945 |
| **Python** | 1.628 | 0.090 | 1.587 | 2.393 | 615 | 10,945 |
| **Lamedh** | 134.685 | 2.327 | 131.724 | 138.569 | 8 | 10,945 |

**Performance Ratios:**
- Lamedh vs Python: **82.7× slower**
- Lamedh vs Rust: **3,125× slower**
- Python vs Rust: **37.8× slower**

**Analysis:**
- Lamedh is significantly slower due to:
  - Interpreted evaluation without JIT compilation
  - Heavy use of recursive function calls
  - Symbol table lookups on every function call
  - Non-tail recursive calls in this workload
  - Environment chain traversal for variable lookup

---

## Loops Benchmark (10k × 100k iterations)

**Rust and Python results:**

| Language | Mean (ms) | Iterations | Result |
|----------|-----------|------------|--------|
| **Rust** | 1,786 | 1 | 499,956,027 |
| **Python** | 50,301 | 1 | 499,950,409 |

**Lamedh: Not tested at full scale**

The full benchmark is impractical for Lamedh. Based on a scaled-down version (100 × 1,000 iterations, 10,000× smaller):
- Scaled-down execution time: ~470ms
- **Extrapolated full benchmark time: ~83 minutes** (5,000 seconds)

This benchmark heavily exercises PROG-based loops which have significant overhead in the current interpreter implementation.

---

## Levenshtein Distance Benchmark

**Rust and Python results (10 words):**

| Language | Mean (ms) | Iterations | Result |
|----------|-----------|------------|--------|
| **Rust** | 0.006 | 156,067 | 351 |
| **Python** | 0.585 | 1,710 | 351 |

**Lamedh: standalone workload, not in CSV runner**

A Lisp implementation exists at `benchmarks/levenshtein/lisp/levenshtein.lisp`,
but it is not wired into the CSV runner and does not use the same string/array
representation as the Rust and Python implementations. Treat it as a standalone
workload until it is ported to the current string/array APIs and timed through a
comparable harness.

---

## Observations

### What This Tells Us

1. **Lamedh is a faithful interpreter**: It prioritizes correctness and Lisp 1.5 semantics over performance.

2. **Recursive overhead is significant**: The Fibonacci benchmark heavily exercises function call overhead, which is substantial in an interpreter.

3. **Room for optimization**: Potential improvements include:
   - Function inlining for small functions
   - Bytecode compilation instead of AST walking
   - JIT compilation for hot paths
   - Symbol caching to reduce lookup overhead

4. **Python's advantage**: Python has decades of optimization including:
   - Highly optimized C implementation
   - Inline caching for attribute/method lookups
   - Specialized opcodes for common operations
   - Still ~38× slower than compiled Rust

### Context Matters

These benchmarks test **computational performance**, not:
- Development speed
- Code expressiveness
- Interactive development workflow
- Metaprogramming capabilities
- Educational value

Lamedh is designed as a faithful Lisp 1.5 implementation for education and exploration, not for production computational workloads.

---

## Reproduction

To reproduce these results:

```bash
cd benchmarks

# Fibonacci (all three)
./fibonacci/rust/target/release/fibonacci_bench 1000 100 20
python3 ./fibonacci/python/fibonacci.py 1000 100 20
python3 ./fibonacci/lisp/benchmark.py 1000 100 20

# Or run all benchmarks
./run_benchmarks.sh
```

## Benchmark Completion Status

- ✅ **Fibonacci**: Complete for all 3 languages
- ⚠️ **Loops**: Complete for Rust/Python, impractical for Lamedh at full scale
- ⚠️ **Levenshtein**: Complete for Rust/Python; standalone Lisp workload exists,
  but is not directly comparable yet

## Future Work

- [ ] Optimize PROG loops to make full loops benchmark feasible
- [ ] Port Levenshtein to Lamedh's current string/array APIs and CSV harness
- [ ] Implement bytecode compiler for performance improvement
- [ ] Create performance regression tracking
- [ ] Compare against other Lisp implementations (SBCL, Racket, Clojure)
