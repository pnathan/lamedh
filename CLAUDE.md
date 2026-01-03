# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Lamedh** (ל, "Lamed") is a Lisp 1.5 implementation written in Rust. It provides a REPL and supports loading/evaluating Lisp files. The interpreter follows classic Lisp 1.5 semantics with modern extensions.

## Build, Test, and Run Commands

- **Build**: `cargo build`
- **Run REPL**: `cargo run`
- **Load file(s)**: `cargo run -- -i <file.lisp>` (can be used multiple times, also accepts directories)
- **Execute s-expression**: `cargo run -- -s "<expression>"`
- **Run all tests**: `cargo test`
- **Run specific test**: `cargo test <test_name>`
- **Run benchmarks**: `cd benchmarks && ./run_benchmarks.sh`
- **Lint**: `cargo clippy`
- **Format**: `cargo fmt`

## Architecture

### Core Modules (src/)

The codebase follows a classic interpreter architecture with four main modules:

1. **reader.rs**: Parser using nom combinators
   - Parses s-expressions, atoms, strings, numbers, floats
   - Handles reader macros: quote ('), quasiquote (`), unquote (,)
   - Supports dotted pairs and comments
   - All symbols are interned and case-normalized to uppercase

2. **evaluator.rs**: Evaluation engine
   - Special forms: QUOTE, QUASIQUOTE, IF, COND, AND, OR, DEF, LAMBDA, FUNCTION, LABEL, DEFINE, DEFEXPR, DEFMACRO, PROGN, SETQ, PROG, RETURN, GO, LET
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

**Environment**: Lexically scoped with parent chain. Symbols are globally interned via SymbolTable.

### Entry Points

- **main.rs**: CLI with rustyline REPL
  - Automatically loads `prologue.lisp` and `lib/` directory at startup if present
  - `-i <path>`: Load file or directory (can be used multiple times)
  - `-s "<expr>"`: Execute single s-expression and exit

- **lib.rs**: Provides `eval_line()`, `load_file()`, `load_directory()` for library usage

### Standard Library

**lib/**: Standard library loaded at startup (numbered files loaded in order)
- **00-core.lisp**: `defun` macro with docstring support
- **01-list.lisp**: List utilities (`append`, `member`, `length`, `reverse`, `pairlis`, `null`)
- **02-cxr.lisp**: CXR functions (caar, cadr, caddr, etc.) generated via `defcxr` macro
- **03-meta.lisp**: Metaprogramming (`documentation`)
- **04-predicates.lisp**: Type predicates (`equal`, `consp`, `listp`)
- **05-math.lisp**: Math utilities (`onep`, `minusp`, `add1`, `sub1`, `max`, `min`, `abs`)

**prologue.lisp**: Legacy prologue file (minimal, just sets `lisp` to `'lamedh`)

## Key Implementation Details

- **Symbol interning**: All symbols are stored once in the global SymbolTable and compared by pointer equality
- **Property lists**: Symbols have plists for storing metadata (e.g., docstrings via GETP/PUTP)
- **Macro expansion**: Macros support `&REST` for variadic arguments and expand before evaluation
- **Fexprs vs Macros**: Fexprs receive unevaluated arguments directly; macros expand to code that's then evaluated
- **PROG control flow**: PROG creates labels and uses LispError::Return/LispError::Go for non-local exits
- **Quasiquotation**: Implemented recursively in evaluator, unquote evaluates nested expressions

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