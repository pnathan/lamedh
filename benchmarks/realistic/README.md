# Realistic mixed workload benchmark

A "real-world-ish" benchmark that exercises the evaluator the way ordinary
Lisp programs do — lists, an associative key/value store, plenty of function
calls and recursion, and a deliberately messy stateful function — rather than
a single micro-loop. Used to measure end-to-end interpreter performance and to
compare it across git revisions.

## Files

- **`realistic.lisp`** — *portable* workload. Uses only primitives common to a
  wide span of lamedh history (no `for`/`while`, no hash tables, no
  `list`/`mod`/`sub1`), so the **same file runs on old and new builds** and the
  two can be compared fairly. Drive with `(bench REPS)`; returns an integer
  checksum so the work can't be optimized away and both builds must agree.

  Four workloads:
  1. **List processing** — build `1..n`, `map` (square), `filter` (evens),
     `foldl` (sum), plus `append`/`reverse`/`length` churn.
  2. **Association-list key/value store** — build an alist, then many `assoc`
     lookups.
  3. **Function-call heavy** — `fib`, tail-recursive `tsum`, and `ackermann`.
  4. **A messy large function** — walk a list of record alists, categorise,
     accumulate per-category totals, track count and max, via a `prog` go-loop
     with many locals and branches.

- **`realistic-hashtable.lisp`** — *modern* workload using current features
  (real hash tables, `for`/`while`, `list`, multi-form `let`): a hash
  histogram, a memoised iterative `fib`, a nested-`for` grid sum, and a
  `while`-driven list drain. Not portable to old revisions; for benchmarking
  the current build. Drive with `(bench-ht REPS)`.

## Running

On the current build:

```sh
cargo build --release
./target/release/lamedh -i benchmarks/realistic/realistic.lisp -s '(bench 30)'
./target/release/lamedh -i benchmarks/realistic/realistic-hashtable.lisp -s '(bench-ht 60)'
```

Time it with your shell, e.g. `time ./target/release/lamedh -i … -s '(bench 30)'`.

## Comparing two revisions

`benchmarks/compare-revisions.sh` builds two revisions in throwaway git
worktrees, runs the **portable** workload on both, verifies they produce the
same checksum (identical work), and reports the speedup:

```sh
benchmarks/compare-revisions.sh <rev_a> <rev_b> [reps]
# e.g. how far we've come since the original benchmark suite (#42):
benchmarks/compare-revisions.sh 83c2891 main 15
```

## Result

Measured on this workload between `83c2891` (the original benchmark suite,
2026-01-03) and current `main` (release builds, best-of-2, identical
checksum):

| revision | time (reps=15) |
|----------|----------------|
| `83c2891` (baseline) | ~18.8s |
| `main` (current)     | ~1.6s  |
| **speedup**          | **~11.7×** |

The gains come from a sequence of evaluator changes — boxing the large
`LispVal` variants (72→24 bytes), a faster binding hasher, allocation-free
integer arithmetic and `IF`, single-allocation operand evaluation, and a
per-symbol value-cell global namespace — each validated by profiling
(valgrind callgrind + DHAT).
