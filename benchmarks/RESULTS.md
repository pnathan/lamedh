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

## Loops Benchmark

*Note: This benchmark (10k × 100k nested loops) takes a very long time in Lamedh due to the PROG loop implementation. Results pending.*

---

## Levenshtein Distance Benchmark

*Note: Lamedh has limited string support, so this benchmark is aspirational. The Lisp implementation demonstrates what would be needed for full string operations.*

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

## Future Work

- [ ] Implement loops benchmark for Lamedh (requires optimization)
- [ ] Add string primitives for Levenshtein benchmark
- [ ] Implement bytecode compiler for performance improvement
- [ ] Add tail call optimization
- [ ] Create performance regression tracking
- [ ] Compare against other Lisp implementations (SBCL, Racket, Clojure)
