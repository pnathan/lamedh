# Lamedh conformance corpus

One implementation-neutral set of cases for the observable behavior in
`KERNEL.md`. Any host (the Rust interpreter, `sbcl/`, `lamedh-asm/`, a WASM
build) runs the same files through a thin runner and compares printed output.

## Layout

- `cases/*.lisp`: case files, one per `KERNEL.md` part group. Run in sorted order.
- `cases/99-*.lisp`: disputed cases. A runner parses them and skips them. Each
  carries a `;; DISPUTED:` comment stating what `KERNEL.md` claims and what the
  reference prints.
- `PENDING.md`: topics left out of the corpus, and why.

## Case format

Each case file is readable by the Lamedh reader. It holds a sequence of forms,
one per case, of exactly this shape:

    (case "reader/uppercase-fold" "KERNEL.md Part I: Symbols" "'foo" "FOO")

- `ID`: a string, unique across all case files (including `99-*`).
- `SPEC-REF`: the `KERNEL.md` part and section the case checks.
- `SOURCE`: Lisp source text, one or more top-level forms. Evaluated in order in
  a fresh standard-library environment (`Environment::with_stdlib()` plus
  `prologue.lisp` in the Rust runner). Capabilities are not granted.
- `EXPECT`: the exact printed text (PRIN1 style, Part III) of the value of the
  last form. Printed `NIL` is `()`.
- `EXPECT` = `ERROR`: asserts that evaluation signals an error. Message text is
  not compared, because it is not portable (Part VIII). The same literal also
  matches a reader failure in `SOURCE`, so check the case by hand when adding it.
  It also matches an unmatched `THROW`, `RETURN-FROM`, `RETURN`, or `GO` that
  reaches the top level, since Part VIII says those are not conditions but still
  halt the unit of execution. A runner must treat that escape as an error result,
  not abort the process.

Strings in a case file use the reader's string escapes. A Lisp string inside
`SOURCE` is written with `\"`, and a backslash is written `\\`.

## Adding a case

1. Pick the case file for the `KERNEL.md` part. Add the case as one line.
2. Give it a new unique ID under that file's prefix (`eq/`, `num/`, `fuel/`, ...).
3. Run the Rust runner: `cargo test --release --test test_conformance`.
4. If the reference disagrees with `KERNEL.md`, do not change `EXPECT` to match.
   Move the case to `cases/99-disputed.lisp` with a `;; DISPUTED:` comment that
   gives the `KERNEL.md` claim and the observed output.

Keep cases to one assertion. Do not use time, random numbers, gensym names,
hash-table key order, the filesystem, the network, a shell, or JIT tiers.

Cases must be fast: every case finishes well under 100 ms on the release build,
and the whole suite runs in a few seconds. Use no deep or unbounded recursion,
no large loops or allocations (keep counts in the low hundreds), no sleeps, no
wall-clock assertions, and no recursion-limit or stack-depth tests. The Rust
runner fails any single case that takes over 1 second and prints the total
suite wall time.

## Writing a runner for another implementation

1. Read each `cases/*.lisp` file in sorted order and parse it with the host's
   reader. Each top-level form is `(case ID SPEC-REF SOURCE EXPECT)`.
2. Skip files named `99-*` and report their count.
3. For each case, start a fresh standard-library environment. Evaluate
   `SOURCE` form by form. Take the printed value of the last form.
4. If evaluation signals an error, the result is `ERROR`. Compare the result
   with `EXPECT`, and collect every mismatch.
5. Fail the run if any case mismatches, if an ID repeats, or if fewer cases
   ran than the runner expects.

Use the same printed form as `KERNEL.md` Part III. Where the host's printer
differs for an opaque value, the case should not exist (see `PENDING.md`).

## Status

- Rust runner: `tests/test_conformance.rs`. Passes against the Rust reference.
  Run with `cargo test --release --test test_conformance`.
- Cases: 143 in `cases/`, 0 disputed. Files `10` through `60`.
- `sbcl/` and `lamedh-asm/` runners: not written.
- WASM: not built.
