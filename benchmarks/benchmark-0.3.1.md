# Benchmark Report — Lamedh 0.3.1

**Date:** 2026-07-15  
**CPU:** Intel Core i7-9750H @ 2.60 GHz  
**OS:** Linux 6.16.3 (Ubuntu)  
**Toolchain:** Rust 1.96.0, GCC 13.3.0, SBCL 2.2.9, Ruby 3.0.1, Python 3.10.10  

## Method

Every result is **best of 3–5 runs** to reduce noise.  All Lamedh runs
use the release binary (`cargo build --release`).  C is compiled with
`gcc -O2`.  SBCL uses `(declare (optimize (speed 3)))`.  Rust benchmarks
use `opt-level = 3`.

Three Lamedh execution tiers are tested:

- **Tree-walker** — plain `defun`, the general-purpose evaluator.
- **JIT** — `defun-typed`, Cranelift native code via HM type checking.
- **OptJIT** — `defun-typed-opt`, source optimizer then Cranelift.

---

## 1. Fibonacci (n=30)

Naive recursive fibonacci.  Pure function-call overhead and integer
arithmetic.

| Implementation         | Best (ms) |  vs C |
|------------------------|----------:|------:|
| C (gcc -O2)            |       2.0 |  1.0x |
| Rust                   |       5.7 |  2.9x |
| SBCL                   |      12.0 |  6.0x |
| **Lamedh-JIT**         |      17.9 |  9.0x |
| **Lamedh-OptJIT**      |      22.4 | 11.2x |
| Ruby                   |      80.4 | 40.2x |
| Python                 |     225.6 |  113x |
| **Lamedh tree-walker** |    2568.0 | 1284x |

The Cranelift tier is within 1.5x of SBCL, 4.5x faster than Ruby, and
12.6x faster than Python.  The tree-walker is ~143x slower than the JIT
— the cost of full Lisp 1.5 generality (fexprs, vau, boxed dispatch).

---

## 2. Realistic Mixed Workload (10 repetitions)

Six sub-workloads per repetition: array sum-of-squares (n=300),
hash-table key-value lookup (250 keys, 1500 lookups), recursive
fibonacci (n=23), tail-recursive sum (n=300), Ackermann (2,50), and
array-based record processing (300 records).

| Implementation                | Best (ms) |   vs C |
|-------------------------------|----------:|-------:|
| C (gcc -O2)                   |       0.8 |   1.0x |
| SBCL                          |       2.0 |   2.5x |
| Rust                          |       2.4 |   3.0x |
| Ruby                          |      27.6 |  34.5x |
| **Lamedh OptJIT + arrays**    |      49.8 |  62.3x |
| **Lamedh JIT + arrays**       |      47.6 |  59.5x |
| Python                        |     100.0 |   125x |
| **Lamedh array + for**        |    1097.7 |  1372x |
| **Lamedh alist + prog**       |    1131.4 |  1414x |

The JIT tier compiles fib, tsum, and ackermann natively; the array and
hash-table operations remain in the tree-walker.  This hybrid still
achieves ~2x faster than Python.  The pure tree-walker variants (array
and alist) are ~23x slower than the JIT hybrid — dominated by the
recursive fib(23) in the tree-walker.

The gap between Lamedh-JIT (48 ms) and SBCL (2 ms) comes from the
array/hash-table operations staying in the tree-walker.  Compiling
`for` loops with `aref`/`aset` through Cranelift would close most of
this gap.

---

## 3. Levenshtein Distance (kitten → sitting, 10,000 iterations)

Two-row dynamic programming edit distance.  Tests loop + random-access
array performance.

| Implementation              | Best (ms) |   vs C |
|-----------------------------|----------:|-------:|
| C (gcc -O2)                 |       1.6 |   1.0x |
| Rust                        |       1.9 |   1.2x |
| SBCL                        |      15.0 |   9.4x |
| Python (array)              |     243.3 |   152x |
| Ruby                        |     287.1 |   179x |
| **Lamedh array-based**      |    6673.4 |  4171x |
| **Lamedh list-based**       |   34930.1 | 21831x |

No JIT or OptJIT comparison: the JIT tier cannot compile array access
or string indexing.  The array-based version is 5.2x faster than the
list-based version (O(1) vs O(n) element access), but both are slow
because every `aref`/`aset` call dispatches through the tree-walker's
full evaluation path.

This benchmark is the strongest argument for extending the Cranelift
backend to compile typed array loops.

---

## Conclusions

1. **The Cranelift JIT tier is competitive on scalar code.**  Fibonacci
   runs within 1.5x of SBCL and 9x of C.  For typed integer functions,
   the JIT delivers real performance.

2. **Array/hash-table operations are the bottleneck.**  On the
   realistic workload, the JIT hybrid is 48 ms vs SBCL's 2 ms — a 24x
   gap almost entirely due to array/hash operations staying in the
   tree-walker.  On Levenshtein, the tree-walker is 4171x slower than C.

3. **Extending Cranelift to compile `for`/`while` loops with
   `aref`/`aset` is the highest-leverage optimization available.**
   The realistic workload would go from 48 ms to approximately 5–10 ms
   (within 3–5x of C), and Levenshtein would go from seconds to
   milliseconds.  This is the single change that would most improve
   Lamedh's benchmark standing.

4. **The source optimizer (OptJIT) shows marginal value on these
   workloads.**  The optimizer's value emerges on code with redundant
   operations, constant folding, or dead branches — not already-minimal
   recursive kernels.

5. **The tree-walker is 100–1000x slower than compiled languages.**
   This is expected and acceptable for the general-purpose evaluator
   that handles fexprs, vau, macros, and dynamic dispatch.  The JIT
   tier exists to escape this cost for hot paths.

---

## Future work

- **Typed arrays in Cranelift**: compile `for`/`while` with
  `aref`/`aset` to native code (the SIMD bulk ops already exist;
  per-element access is the gap).
- **String/byte operations in Cranelift**: `index`, `length`, character
  comparison — enable JIT-compiled string algorithms.
- **Rewrite Levenshtein benchmark** to use the JIT-compiled array
  path once typed arrays are available.
