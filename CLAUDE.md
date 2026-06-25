# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Lamedh** (ל, "Lamed") is a Lisp 1.5 implementation written in Rust. It provides a REPL and supports loading/evaluating Lisp files. The interpreter follows classic Lisp 1.5 semantics with modern extensions.

## Workspace Layout

The project is a Cargo workspace with two crates:

- **`lamedh`** (repo root, `src/`): the reusable interpreter **library**. Depends only on `nom`. This is what embedders depend on; it has no CLI/terminal dependencies.
- **`lamedh-cli`** (`cli/`): the **CLI/REPL driver** binary (named `lamedh`). Depends on `lamedh`, `clap`, and `rustyline`. This is the only crate that knows about argument parsing and the terminal.

`default-members` is set so plain `cargo run`/`cargo build`/`cargo test` from the repo root operate on both crates (and `cargo run` launches the `lamedh` binary). The benchmark comparison crates under `benchmarks/*/rust` are `exclude`d from the workspace.

## Build, Test, and Run Commands

- **Build**: `cargo build` (whole workspace)
- **Run REPL**: `cargo run` (launches the `lamedh` binary from `lamedh-cli`)
- **Load file(s)**: `cargo run -- -i <file.lisp>` (can be used multiple times, also accepts directories)
- **Execute s-expression**: `cargo run -- -s "<expression>"`
- **Run all tests**: `cargo test`
- **Run specific test**: `cargo test <test_name>`
- **Run benchmarks**: `cd benchmarks && ./run_benchmarks.sh`
- **Lint**: `cargo clippy --workspace --all-targets`
- **Format**: `cargo fmt --all`

> Run `cargo fmt --all` and `cargo clippy --workspace --all-targets` before every commit; treat a clean clippy run as part of "done".

## Architecture

### Core Modules (src/)

The codebase follows a classic interpreter architecture with four main modules:

1. **reader.rs**: Parser using nom combinators
   - Parses s-expressions, atoms, strings, numbers, floats
   - Handles reader macros: quote ('), quasiquote (`), unquote (,)
   - Supports dotted pairs and comments
   - All symbols are interned and case-normalized to uppercase

2. **evaluator.rs**: Evaluation engine
   - Special forms: QUOTE, QUASIQUOTE, IF, COND, AND, OR, DEF, LAMBDA, FUNCTION, LABEL, DEFINE, DEFEXPR, DEFMACRO, PROGN, SETQ, PROG, RETURN, GO, FOR, WHILE, LET, UNWIND-PROTECT, CATCH, THROW, BLOCK, RETURN-FROM
   - Non-local exit: `CATCH`/`THROW` (tag-based) and `BLOCK`/`RETURN-FROM` (name-based) use `LispError::Throw`/`LispError::ReturnFrom`; `UNWIND-PROTECT` runs cleanup forms regardless of how the body exits. `ERRORSET` (a function taking a quoted form) traps ordinary errors only and lets control-flow signals pass through.
   - `FOR`/`WHILE` are fast iterative loops: `(for (var start end [step]) body...)` (inclusive integer range, one reused frame, in-place counter mutation) and `(while cond body...)`
   - Applies built-in functions and user-defined lambdas/fexprs/macros
   - Supports fexprs (unevaluated argument functions) and macros with &REST
   - PROG provides labeled statements with GO/RETURN for non-local control flow
   - Quasiquotation with backtick (`) and unquote (,) for code generation

3. **environment.rs**: Environment and symbol table
   - Lexically scoped environments with parent chain
   - Global symbol table (SymbolTable) for interning symbols
   - Each symbol has a property list (plist) for metadata like docstrings
   - Builtins registered in `new_with_builtins()`

4. **printer.rs**: Output formatting
   - Pretty-prints LispVal types back to readable Lisp syntax

### Data Model (lib.rs)

**LispVal enum**: Core data type representing all Lisp values
- Symbol (with plist for properties like docstrings)
- Number (i64), Float (f64), String
- Cons cells (car/cdr pairs)
- Nil
- Builtin functions
- Lambda, Fexpr, Macro (closures with captured environments)
- HashTable (Rc<RefCell<HashMap>>)
- Error (first-class condition: a message `String` + a `data` cons/Nil) — built with `make-error`, signalled by `error`, bound by `handler-case`

**Environment**: Lexically scoped with parent chain. Symbols are globally interned via SymbolTable.

### Entry Points

- **cli/src/main.rs**: CLI with rustyline REPL (the `lamedh-cli` crate)
  - Automatically loads `prologue.lisp` and `lib/` directory at startup if present
  - `-i <path>`: Load file or directory (can be used multiple times)
  - `-s "<expr>"`: Execute single s-expression and exit

- **src/lib.rs**: The `lamedh` library — provides `eval_line()`, `load_file()`, `load_directory()`, `with_large_stack()` and the `LispValExtension` trait for embedders

### Standard Library

**lib/**: Standard library loaded at startup (numbered files loaded in order)
- **00-core.lisp**: `defun` macro with docstring support
- **01-list.lisp**: List utilities (`append`, `member`, `length`, `reverse`, `pairlis`, `null`)
- **02-cxr.lisp**: CXR functions (caar, cadr, caddr, etc.) generated via `defcxr` macro
- **03-meta.lisp**: Metaprogramming (`documentation`)
- **04-predicates.lisp**: Type predicates (`equal`, `consp`, `listp`)
- **05-math.lisp**: Math utilities (`<=`, `>=`, `/=`, `onep`, `minusp`, `add1`, `sub1`, `max`, `min`, `abs`)
- **12-control.lisp**: Control-flow macros (`when`, `unless`, `prog1`, `case`, `dolist`, `dotimes`) — non-mutating (epic #141)
- **13-functional.lisp**: Functional list toolkit (`reduce`, `filter`, `find`, `position`, `every`/`some`, `take`/`drop`, `iota`/`range`, `zip`, `flatten`, `group-by`, combinators) — function-first arg order (Common Lisp style), matching the `map*` family
- **14-strings.lisp**: String layer over the Rust primitives (`string-upcase`, `string-split`/`-join`, `string-trim`, `starts-with-p`, char predicates) — `foo-p` predicate naming
- **15-sets-hash.lisp**: Set/alist/hash helpers (`union`, `intersection`, `adjoin`, `alist-get`/`-put`, `maphash`, `hash->alist`)
- **16-conditions.lisp**: Condition macros over `errorset` (`ignore-errors`, `handler-case`); `catch`/`throw`, `block`/`return-from`, `unwind-protect` are kernel special forms
- **17-arrays.lisp**: Array helpers over the array primitives (`array->list`, `list->array`, `array-map`, `array-fill`, `array-copy`, `subarray`)
- **18-format.lisp**: `format` (CL-style subset: `~a ~s ~d ~% ~~`)

Files 06–11 and 97–99 cover builtin docs, shell helpers, vau forms, Lisp 1.5
appendix, the testing framework, the optimizer, and the help system.

**prologue.lisp**: Legacy prologue file (minimal, just sets `lisp` to `'lamedh`)

## Key Implementation Details

- **Symbol interning**: All symbols are stored once in the global SymbolTable and compared by pointer equality
- **Property lists**: Symbols have plists for storing metadata (e.g., docstrings via GETP/PUTP)
- **Macro expansion**: Macros support `&REST` for variadic arguments and expand before evaluation
- **Fexprs vs Macros**: Fexprs receive unevaluated arguments directly; macros expand to code that's then evaluated
- **PROG control flow**: PROG creates labels and uses LispError::Return/LispError::Go for non-local exits
- **Quasiquotation**: Implemented recursively in evaluator, unquote evaluates nested expressions

## Optimization Philosophy

**Prefer the Lisp layer; keep the Rust kernel small.** When an optimization can be
expressed as a Lisp-to-Lisp transform, implement it as an optimizer pass in
`lib/11-optimizer-vau.lisp` (e.g. constant folding, dead-binding removal, the
planned frame-collapse pass) rather than growing the Rust evaluator. The kernel
should stay a minimal set of primitives.

The exception is **hot-path evaluation mechanics that have no Lisp-layer
expression** — e.g. how arguments are collected, how environment frames are
allocated, or the in-memory size of `LispVal`. Those are intrinsically kernel
concerns and are optimized in Rust (see the boxing of large `LispVal` variants
and the single-allocation operand evaluation). When in doubt, ask whether the
change could be a Lisp pass; if yes, it belongs there.

## Testing

Tests are organized in `tests/` directory:
- Unit tests in individual modules (e.g., reader.rs)
- Integration tests for language features (arithmetic, control flow, functions, lists, prog)
- Lisp test files (e.g., docstring_test.lisp, prog_test.lisp)

Use `cargo test <test_name>` to run specific tests during development.

## Benchmarks

The `benchmarks/` directory contains performance benchmarks comparing Lamedh against Rust and Python:
- **fibonacci**: Recursive fibonacci calculation (tests function call overhead)
- **loops**: Nested loop performance (tests iteration speed)
- **levenshtein**: String edit distance (aspirational - tests DP and strings)

Run benchmarks: `cd benchmarks && ./run_benchmarks.sh`

See `benchmarks/README.md` for details on benchmark structure, running individual tests, and interpreting results.