# Lamedh Benchmark Results

Benchmark results comparing lamedh (Lisp 1.5 interpreter) against Rust (compiled) and Python 3.11 (interpreted).

**Test Environment:**
- Python: 3.11.14
- Rust: 1.x (release mode with optimizations)
- Lamedh: 0.1.0 (release mode, no JIT/optimizations)

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
  - No tail call optimization
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

**Lamedh: Not implemented**

While Lamedh has basic string primitives (`concat`, `index`, `stringp`), the Levenshtein benchmark has not been adapted to use Lamedh's string API. This benchmark requires:
- String length calculation
- Character-by-character comparison
- Multi-dimensional array operations

The Lisp implementation in this repository is aspirational and demonstrates what would be needed for full string operation support.

---

## Observations

### What This Tells Us

1. **Lamedh is a faithful interpreter**: It prioritizes correctness and Lisp 1.5 semantics over performance.

2. **Recursive overhead is significant**: The Fibonacci benchmark heavily exercises function call overhead, which is substantial in an interpreter.

3. **Room for optimization**: Potential improvements include:
   - Tail call optimization for PROG-based loops
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
- ❌ **Levenshtein**: Complete for Rust/Python, not implemented for Lamedh

## Future Work

- [ ] Optimize PROG loops to make full loops benchmark feasible
- [ ] Implement Levenshtein for Lamedh's string API
- [ ] Implement bytecode compiler for performance improvement
- [ ] Add tail call optimization
- [ ] Create performance regression tracking
- [ ] Compare against other Lisp implementations (SBCL, Racket, Clojure)
