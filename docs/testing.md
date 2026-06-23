# Testing

Lamedh has two complementary test layers.

## 1. Rust tests (`cargo test`)

Unit and integration tests live in `src/*.rs` (`#[cfg(test)]`) and `tests/*.rs`.
The CI workflow runs `cargo test --verbose` on every push/PR.

**Coverage goal: 95% line coverage.** Measure it with:

```bash
ai/scripts/coverage.sh          # summary table (cargo-llvm-cov)
ai/scripts/coverage.sh --open   # browsable HTML report
```

## 2. Lisp tests (xUnit framework)

`lib/10-testing.lisp` provides a small xUnit-style framework, loaded as part of
the standard library.

```lisp
(deftest arithmetic
  (assert-equal (+ 1 2) 3)
  (assert-true  (member 'b '(a b c)))
  (assert-false (member 'z '(a b c))))

(run-tests)   ; prints a summary, returns T iff all assertions passed
```

### Assertions
- `(assert-true x)` / `(assert-false x)` / `(assert-nil x)`
- `(assert-equal actual expected)` — structural (`equal`)
- `(assert-eq actual expected)` — alias of `assert-equal`
- `(check ok msg)` — core primitive; pass when `ok` is non-nil

### Registry / runner
- `(deftest name body...)` registers a test (newest first in `*tests*`).
- `(run-tests)` resets counters, runs every registered test, prints
  `(assertions-passed N failed M)` and either `all-tests-passed` or a
  `(failures ...)` list, and returns `T` iff `M = 0`.
- `(clear-tests)` unregisters all tests; `(reset-tests)` only clears counters.

### The suite
Lisp test files live in `tests/lisp/*.lisp`. They are exercised under
`cargo test` by `tests/test_lisp_suite.rs`, which loads the stdlib + every
`tests/lisp` file and asserts `(run-tests)` returns `T`.

**Coverage goal: 100% of the Lisp standard library.**

Run the Lisp suite directly:

```bash
cargo run -- -i tests/lisp -s "(run-tests)"
# or see failure detail under cargo test:
cargo test lisp_test_suite_passes -- --nocapture
```

> Note: the Rust harness runs the suite on a 64 MB stack thread because the
> tree-walking interpreter recurses deeply with heavy frames and overflows the
> default ~2 MB `cargo test` thread stack. A recursion-depth guard / tail-call
> optimization (issues #61, #62) is the real fix.
