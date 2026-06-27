# Lamedh ל

**Lamedh** (Hebrew: ל, "Lamed") is a Lisp 1.5 interpreter written in Rust — embeddable, extensible, and faithful to the classic semantics with modern conveniences added on top.

```
(ל)> (defun fib (n)
       (if (< n 2) n
           (+ (fib (- n 1)) (fib (- n 2)))))
FIB
(ל)> (fib 10)
55
```

## Features

- **Lisp 1.5 core** — `LAMBDA`, `COND`, `QUOTE`, `PROG`/`GO`/`RETURN`, property lists, association lists
- **Modern extensions** — `DEFUN`, `LET`, `DEFMACRO`, quasiquote/unquote, hash tables, arrays, structs
- **Lexical scoping** with interned symbols and `EQ` by pointer equality
- **Fexprs and vau operatives** — functions that receive unevaluated arguments
- **Iterative loops** — `FOR` (integer range) and `WHILE` with a single reused frame
- **Capability-gated sandbox** — filesystem, shell, and stdin access all off by default
- **Lisp-layer optimizer** — constant folding and dead-binding elimination as Lisp passes, keeping the Rust kernel small
- **Typed checker/JIT path** — HM-style checking for typed islands, with Cranelift native code under the default `jit` feature
- **Interactive REPL** with line editing, history, and an in-REPL help system (`(help 'car)`)
- **Embeddable library** — the `lamedh` crate exposes `eval_line()`, `load_file()`, and the `LispValExtension` trait

## Quick start

```bash
# Build
cargo build

# Interactive REPL
cargo run

# Load a file, then drop into REPL
cargo run -- -i myfile.lisp

# Evaluate an expression and exit
cargo run -- -s "(mapcar '(1 2 3 4 5) (lambda (x) (* x x)))"
# => (1 4 9 16 25)
```

## Examples

```lisp
;; Recursive functions
(defun factorial (n)
  "Compute N!"
  (if (= n 0) 1
      (* n (factorial (- n 1)))))
(factorial 10)  ; => 3628800

;; Higher-order functions
(mapcar '(1 2 3 4 5) (lambda (x) (* x x)))
; => (1 4 9 16 25)

;; Iterative loops (no stack growth)
(let ((sum 0))
  (for (i 1 100)
    (setq sum (+ sum i)))
  sum)   ; => 5050

;; Quasiquote / macros
(defmacro when (test &rest body)
  `(if ,test (progn ,@body) nil))

;; Hash tables
(let ((h (make-hash-table)))
  (set-bang h 'answer 42)
  (get h 'answer))   ; => 42

;; In-REPL help
(help 'mapcar)
(help 'categories)
```

## Project layout

```
lamedh/
  src/           interpreter library (reader, evaluator, environment, printer, optimizer)
  cli/           CLI/REPL binary (clap + rustyline)
  lib/           Lisp standard library loaded at startup
  tests/         integration tests
  benchmarks/    performance benchmarks (excluded from workspace)
  docs/          reference documentation
  examples/      example Lisp programs
```

The workspace has two crates:

| Crate | Type | Purpose |
|-------|------|---------|
| `lamedh` | library | Reusable interpreter. Default features include the typed JIT backend; `--no-default-features` keeps the dependency-light checker path. |
| `lamedh-cli` | binary | CLI/REPL driver. Depends on `lamedh`, `clap`, `rustyline`. |

## Development

```bash
cargo test                                  # run all tests
cargo clippy --workspace --all-targets      # lint
cargo fmt --all                             # format
cargo doc --no-deps --open                  # browse API docs
cd benchmarks && ./run_benchmarks.sh        # performance benchmarks
```

## Documentation

The full **[Reference Manual](lamedh-manual.md)** covers every special form, built-in function, the standard library, the optimizer, embedding guide, and more.

Shorter topic docs live in [`docs/`](docs/index.md).
The 1.0 release gates are tracked in [`docs/roadmap_1_0.md`](docs/roadmap_1_0.md);
the crate version remains on `0.2.x` during that ramp.

## License

AGPL-3.0 — see [LICENSE](LICENSE).
