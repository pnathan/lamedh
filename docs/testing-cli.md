# Test Runner (`lamedh --test`)

`lamedh --test` loads one or more `.lisp` files (or directories of them),
runs every test registered with `deftest` (see `lib/10-testing.lisp`), and
reports a machine-friendly pass/fail summary — the CLI-driven counterpart to
calling `(run-tests)` interactively.

## Usage

```text
lamedh --test file.lisp [file2.lisp ...]
lamedh --test tests/                      ; directory: sorted *.lisp files
lamedh --test --error-format=sexpr tests/
```

* Each path is a file or a directory. Directories load their sorted
  `*.lisp` files, exactly like `-i` (numeric prefixes such as `00-`, `01-`
  control order).
* `--test` with no paths is an error (exit `2`).
* Files are loaded into one shared environment — a full stdlib world with
  every sandbox capability enabled, the same "developer tool, not a
  sandbox" defaults batch/script mode uses (`--sandbox`/`-c` still apply if
  you need a locked-down run).

### Exit codes

| Code | Meaning                                            |
|------|-----------------------------------------------------|
| 0    | Every registered test passed (including zero tests).|
| 1    | At least one test failed.                            |
| 2    | A file/directory failed to load or parse.            |

### Output

`--error-format=human` (the default) prints one line per **failing** test:

```text
FAIL SUB-BROKEN: (EXPECTED 1 GOT 2)
```

followed by a final summary line, always in this exact form:

```text
test result: N passed; M failed
```

`--error-format=sexpr` prints the same failing tests as one readable
s-expression per line instead, followed by the identical summary line. The
schema mirrors [`lamedh --check`](check.md)'s finding sexprs:

```lisp
((test . "SUB-BROKEN") (status . fail) (message . "(EXPECTED 1 GOT 2)"))
```

| Key       | Type            | Notes                                          |
|-----------|-----------------|-------------------------------------------------|
| `test`    | string          | The test's name, as declared to `deftest`.       |
| `status`  | symbol          | Always `fail` — passing tests are not printed (mirroring `--check`'s "silence when clean"). |
| `message` | string          | The most recently recorded failure/error for that test. |

Passing tests produce no per-test output in either format — only the
summary line's `N passed` count reflects them. This mirrors `lamedh
--check`'s convention that clean input produces no findings.

## The Lisp-layer vocabulary

`lib/10-testing.lisp` is a small xUnit-style framework:

* `(deftest name body...)` registers a test (re-registering a name replaces
  the old test).
* `assert-true`, `assert-false`, `assert-nil`, `assert-equal`/`assert-eq`,
  and the underlying `check` primitive record pass/fail into
  `*test-pass*`/`*test-fail*`/`*test-failures*`.
* `(run-tests)` is the original, human-oriented entry point: it runs every
  registered test, prints a Lisp-native summary, and returns `T` iff
  everything passed.
* `(run-all-tests-detailed)` is the entry point `lamedh --test` uses: it
  reruns `run-one-test` per registered test (so the classic counters/bodies
  are unchanged) and additionally returns one `(name status message)`
  triple per test — `status` is the symbol `PASS` or `FAIL`, and `message`
  is `NIL` for a pass or the most recent failure/error description for a
  fail. The Rust CLI glue (`src/test_runner.rs`) only evaluates this one
  entry point and converts its result into the output above; all of the
  actual bookkeeping stays in Lisp per the project's Lisp-first philosophy.

A buggy test body that raises an error is trapped and recorded as a failure
(it does not abort the run), exactly as `run-tests` already behaved.
