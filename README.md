# Lamedh ל

**Lamedh** is an embeddable Lisp 1.5 dialect in Rust. Its center of gravity is
reflective Lisp: classical fexprs, Kernel-style `vau` operatives, Hindley-Milner
type checking for typed islands, and a typed compilation path backed by
Cranelift when the default `jit` feature is enabled.

```lisp
;; Fexpr: operands arrive unevaluated.
(defexpr quote-args (args) args)
(quote-args (+ 1 2) "hello")
; => ((+ 1 2) "hello")

;; VAU: operands plus the caller's environment.
(def inspect
  (vau (ops env)
    (list ops (eval (car ops) env))))

(let ((x 10))
  (inspect (+ x 5)))
; => (((+ X 5)) 15)

;; HM type checking for ordinary Lisp functions.
(defun inc (n) (+ n 1))
(check-type inc)
; => "INC : ((N int64)) -> int64 [compiled]"

;; Typed islands compile through the typed backend.
(defun-typed (sq int64) ((x int64))
  (* x x))

(sq 12)
; => 144
```

## What It Is

- **Fexpr-capable Lisp 1.5**: `DEFEXPR` creates user-defined special forms that
  receive raw operand syntax.
- **Kernel-style `vau`**: `VAU` / `$VAU` operatives receive raw operands and the
  caller environment explicitly.
- **HM typing**: `CHECK-TYPE` infers and reports types for ordinary functions,
  including polymorphic and list-shaped functions that are not compileable.
- **Typed compilation**: `DEFUN-TYPED` registers typed functions that run through
  the typed interpreter or native Cranelift backend, depending on features and
  compileability.
- **Embeddable runtime**: the `lamedh` crate exposes `Environment`,
  `eval_str()`, `eval_all()`, `load_file()`, and `LispValExtension`.
- **Sandboxed capabilities**: filesystem, shell, temp files, stdin, and
  networking (DNS/TCP/UDP) are on by default in the CLI; use `--sandbox`
  for a locked-down session, or grant individual capabilities with `-c`.
  The library API keeps them off by default; embedders call
  `env.enable_feature(...)`.  A Rust-only policy hook can scope a granted
  networking capability to specific hosts/ports.

## Quick Start

```bash
cargo build
cargo test --workspace

# Interactive REPL
cargo run

# Evaluate one or more forms and exit
cargo run -- -s "(mapcar (lambda (x) (* x x)) '(1 2 3 4 5))"
# => (1 4 9 16 25)

# Load a file before evaluating or entering the REPL
cargo run -- -i app.lisp
```

The default build enables the typed native backend:

```bash
cargo build
cargo test --workspace
```

The dependency-light build keeps the typed checker and closure backend but omits
Cranelift:

```bash
cargo build --no-default-features
cargo test -p lamedh --no-default-features
```

That build retains the library's `nom` parser and `smallvec` evaluator
dependencies. The default `jit` feature additionally pulls in Cranelift.

## Fexprs

`DEFEXPR` defines a function-like object whose operands are not evaluated before
the call. This is the Lisp 1.5 route to user-defined special forms.

```lisp
(defexpr first-form (args)
  (car args))

(first-form (+ 1 2) (/ 1 0))
; => (+ 1 2)
```

Because a fexpr can choose what to evaluate, it can implement control forms:

```lisp
(defexpr my-if (args)
  (if (eval (car args))
      (eval (car (cdr args)))
      (eval (car (cdr (cdr args))))))

(my-if (> 3 2) "yes" (/ 1 0))
; => "yes"
```

## VAU

`VAU` makes the caller environment explicit. That gives an operative both the
raw syntax and the exact environment in which selected forms should run.

```lisp
(def eval-first
  (vau (ops caller)
    (eval (car ops) caller)))

(let ((x 41))
  (eval-first (+ x 1)))
; => 42
```

The `$VAU` spelling is also accepted for Kernel-style code.

```lisp
(def $unless
  ($vau (ops env)
    (if (eval (car ops) env)
        nil
        (eval (car (cdr ops)) env))))

($unless nil "ran")
; => "ran"
```

## HM Typing

`CHECK-TYPE` runs the non-compiled Hindley-Milner checker over an ordinary
function. Compileable functions get concrete machine-friendly types; other
well-typed functions can still be checked.

```lisp
(defun id (x) x)
(check-type id)
; => "ID : (forall (a) (-> (a) a))"

(defun lsum (xs)
  (if (null xs)
      0
      (+ (car xs) (lsum (cdr xs)))))

(check-type lsum)
; => "LSUM : (-> ((list int64)) int64)"
```

Type errors are reported before the function has to fail at a later call site:

```lisp
(defun bad (x)
  (list 1 x (+ x x) nil))

(check-type bad)
; => "type error ..."
```

User-defined typed structs are nominal types in the HM unifier. A function that
takes `Foo` accepts values built by `make-foo`, not a plain integer or another
struct with the same fields. `let-typed` annotations use the same type names as
function signatures:

```lisp
(defrecord Foo (n int64))

(defun-typed (foo-n-plus-one int64) ((x Foo))
  (let-typed ((local Foo x))
    (+ (foo-n local) 1)))
```

Typed protocols (`defprotocol`/`definstance`, 0.3) give one name many typed
instances selected by inference; beyond that, type agreement is HM
unification with row-polymorphic records: same type, inferred type
variable, row subsumption, or error.

## Compilation

Typed definitions use explicit signatures and land in the typed registry. Under
the default `jit` feature, compileable typed functions get a native Cranelift
edition; without default features they still run through the typed closure path.

```lisp
(defun-typed (fib int64) ((n int64))
  (if (< n 2)
      n
      (+ (fib (- n 1)) (fib (- n 2)))))

(fib 10)
; => 55
```

When you want the Lisp/vau source optimizer to run before typed compilation,
use the explicit optimizer-to-compiler bridge:

```lisp
(defun-typed-opt (inc int64) ((x int64))
  (+ x 0))
; optimizer rewrites the body to X, then DEFUN-TYPED performs HM checking
; and installs the compiled typed edition.
```

Ordinary functions can also be analyzed and optimized opportunistically:

```lisp
(jit-optimize
  (defun dbl (n) (+ n n)))
; => "DBL : (forall (a) (-> (a) a))  [checked, dynamic]"

(dbl 21)
; => 42
```

Introspection is available from Lisp:

```lisp
(describe 'fib)
(disassemble 'fib)
```

## Files

Rust hosts can load a file directly with `load_file()`. Lisp source files can
include other files at top level with a C-style source directive:

```lisp
;; app.lisp
(include "lib/math.lisp")

(defun main () (sq 12))
```

Relative include paths resolve from the file containing the include, and include
cycles are reported as errors.

## Local Benchmark Note

The current Fibonacci comparison is best-of-5 on one machine, so treat
it as a machine-local snapshot rather than a portable claim.
For `n=30`, the warm typed native path measured about 18 ms for
`Lamedh-JIT` and 22 ms for `Lamedh-OptJIT`, compared with 2 ms C,
6 ms Rust, 12 ms SBCL, 80 ms Ruby, and 226 ms Python. The local
toolchain versions were GCC 13.3.0, Rust 1.96.0, SBCL 2.2.9,
Ruby 3.0.1, Python 3.10.10, and Lamedh 0.3.1. See
`benchmarks/benchmark-0.3.1.md` for the full multi-workload report.

`Lamedh-OptJIT` uses `defun-typed-opt`: Lisp/vau source optimization first,
then HM checking and native compilation. On this recursive Fibonacci workload,
the source optimizer has little to simplify, so the two Lamedh rows are best
read as the same performance tier within run noise.

## Embedding

`LispVal` holds an `Rc` internally, so it isn't `Send` — and
`with_large_stack` requires its closure's return type to be
`Send + 'static` because it runs on a spawned thread. Do the Lisp-side
work (creating the environment, evaluating, and reading back the result)
entirely inside the closure, and return a plain `Send` type such as
`String`:

```rust
use lamedh::{LispVal, environment::Environment, eval_str};

fn run_script(src: String) -> Result<String, String> {
    lamedh::with_large_stack(move || {
        let env = Environment::with_stdlib();

        env.register_fn("rust-add", |args, _env| {
            let a = args[0].as_number()?;
            let b = args[1].as_number()?;
            Ok(LispVal::from(a + b))
        });

        eval_str(&src, &env)
            .map(|v| lamedh::printer::print(&v))
            .map_err(|e| e.to_string())
    })
}
```

Grant capabilities explicitly when scripts need I/O:

```rust
env.enable_feature("READ-FS");
env.enable_feature("SHELL");
env.enable_feature("IO");
```

## Project Layout

```text
lamedh/
  src/          interpreter library, typed backend, reader, printer, optimizer
  cli/          CLI and REPL driver
  lib/          embedded Lisp standard library
  tests/        Rust and Lisp integration tests
  docs/         manual and topic documentation
  benchmarks/   benchmark harnesses and comparison programs
  examples/     embedding and Lisp examples
```

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p lamedh --no-default-features
cargo doc --workspace --no-deps
```

The generated reference docs come from `lib/99-help-data.lisp`:

```bash
./scripts/generate-docs.sh
```

## Documentation

- [Reference Manual](lamedh-manual.md)
- [Topic Docs](docs/index.md)
- [1.0 Roadmap](docs/roadmap_1_0.md)
- [Typed JIT Design](docs/typed-jit-design.md)

## License

AGPL-3.0. See [LICENSE](LICENSE).
