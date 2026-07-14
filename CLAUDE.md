# AGENTS.md

This file provides guidance to Codex and other coding agents when working with this repository. `AGENTS.md` is Codex's preferred information file; `CLAUDE.md` is kept as a compatibility symlink target.

## Project Overview

**Lamedh** (ל, "Lamed") is an embeddable Lisp 1.5 interpreter written in Rust. It provides a reusable interpreter library, a command-line REPL/script runner, an embedded standard library, and modern extensions such as lexical closures, macros, fexprs, Kernel-style `vau` operatives, dynamic variables, sandbox capabilities, arrays, hash tables, structs, conditions, and an optional typed JIT.

## Workspace Layout

This repository is a Cargo workspace with two primary crates:

- **`lamedh`** (repo root, `src/`): the reusable interpreter library. It owns parsing, evaluation, environments, printing, optimization, the typed JIT support, sandbox capability checks, and stdlib embedding.
- **`lamedh-cli`** (`cli/`): the CLI/REPL driver binary, named `lamedh`. It owns argument parsing, terminal line editing, capability flags, script mode, `-i` loading, and `-s` expression execution.

`default-members = [".", "cli"]`, so plain `cargo build`, `cargo test`, and `cargo run` from the repo root operate on both crates, and `cargo run` launches the `lamedh` binary. Benchmark comparison crates under `benchmarks/*/rust` are excluded from the workspace and are built directly by `benchmarks/run_benchmarks.sh`.

## Build, Test, and Run Commands

- **Build**: `cargo build`
- **Build without the default Cranelift JIT backend**: `cargo build --no-default-features`
- **Run REPL**: `cargo run`
- **Run REPL with sandbox capabilities**: `cargo run -- --capability READ-FS --capability SHELL`
- **Load file(s) or directories before REPL/batch execution**: `cargo run -- -i <file-or-dir>` (repeatable)
- **Execute s-expression(s)**: `cargo run -- -s "<expression>"`
- **Run script with arguments**: `cargo run -- path/to/script.lisp arg1 arg2`
- **Run all tests**: `cargo test`
- **Run a specific test**: `cargo test <test_name>`
- **Faster local test runs**: `cargo nextest run` (optional; process-per-test with full cross-binary parallelism, faster than `cargo test`; `.config/nextest.toml` tunes it). It does **not** run doctests — add `cargo test --doc` when those matter.
- **Ship gate (authoritative)**: `scripts/gauntlet.sh [verdict-file]` — release, default + `--no-default-features` + `--features fuzz` + clippy; a ship needs `DEFAULT-GREEN`/`NDF-GREEN`/`FUZZ-GREEN`/`CLIPPY-GREEN` in the verdict file. This, not a bare `cargo test`, is the merge gate.
- **Lint**: `cargo clippy --workspace --all-targets`
- **Format**: `cargo fmt --all`
- **Benchmarks**: `cd benchmarks && ./run_benchmarks.sh`

Run `cargo fmt --all` and `cargo clippy --workspace --all-targets` before every commit; treat a clean clippy run as part of "done".

## Architecture

### Core Rust Modules (`src/`)

1. **`reader.rs`**: `nom`-based parser for s-expressions.
   - Parses atoms, strings, integers, floats, lists, dotted pairs, quote, quasiquote, unquote, characters, comments, radix literals, and shebang lines.
   - Symbols are interned and case-normalized to uppercase.
   - Parse errors include 1-based line/column positions; incremental reader helpers are used by the REPL and file loading.

2. **`evaluator/` and `evaluator.rs`**: evaluation engine.
   - Implements special forms, application, macros, fexprs, `vau`, quasiquote, dynamic variables, non-local control flow, conditions, builtins, tail-call mechanics, and optional compilation bridges.
   - Keep special forms and builtins small and prefer Lisp-layer definitions when practical.

3. **`environment.rs`**: environment and symbol table.
   - Provides lexical parent chains, symbol interning, property lists, dynamic/special bindings, feature/capability flags, and builtin registration.
   - Use `Environment::with_stdlib()` for normal interpreter startup; use `new_with_builtins()` only when tests need a minimal kernel.
   - `with_stdlib()`/`with_prelude()` serve a deep-copy fork (`fork_world`) of a per-thread prototype — first call on a thread pays the real load, later calls cost milliseconds, and every returned environment is a fully isolated world. `with_stdlib_fresh()`/`with_prelude_fresh()` bypass the cache for one-environment processes.

4. **`printer.rs`**: readable output formatting for `LispVal` values.

5. **`optimizer.rs`** and **`lib/11-optimizer-vau.lisp`**: source-level optimization support.
   - Rust exposes kernel hooks; Lisp implements most Lisp-to-Lisp optimization passes.

6. **`jit/` and `jit.rs`**: typed JIT / native-code backend support.
   - The default workspace build enables the `jit` feature, which pulls in Cranelift. Use `--no-default-features` for the dependency-light typed checker / closure-interpreter path.

### Data Model (`src/lib.rs`)

`LispVal` is the central runtime value type. It includes symbols, numbers, floats, strings, chars, cons cells, nil, builtins, lambdas, fexprs, macros, hash tables, arrays, structs/extensions, first-class errors, and typed/JIT-related values. `Environment` values are shared handles around lexical/dynamic scopes and the global symbol table.

The tree-walking evaluator uses large Rust stack frames for non-tail calls. Entry points that run user Lisp should use `lamedh::with_large_stack`, which spawns a 512 MiB stack thread; the CLI and test harness already do this.

## CLI Behavior

`cli/src/main.rs` starts with `Environment::with_stdlib_fresh()` (one environment per process, so the per-thread prototype cache would only add overhead; identical result), grants any `--capability/-c` flags, binds script arguments as `*ARGV*`, loads each `-i` path, then chooses script mode, `-s` batch mode, or the REPL.

Important CLI semantics:

- `-i` accepts files or directories; directories load sorted `*.lisp` files.
- Batch modes (`script.lisp` or `-s`) exit non-zero on `-i` load failures.
- REPL mode reports `-i` load failures and continues.
- Script mode supports a leading shebang line and exposes remaining args as `*ARGV*`.
- `(exit n)` sets the process exit code.
- Incomplete REPL input gets a continuation prompt; Ctrl-C cancels the current input; Ctrl-D exits.
- Top-level integer overflow transitions print a warning and set the `OVERFLOW` flag.

## Sandboxing and Capabilities

Potentially dangerous host capabilities are disabled by default. Enable them explicitly in host code with `env.enable_feature(...)` or in the CLI with `--capability`:

- `READ-FS`: read-only filesystem operations.
- `CREATE-FS`: filesystem mutations.
- `TEMP-FS`: temporary file/directory creation.
- `SHELL`: shell helpers from `lib/07-shell.lisp` and related builtins.
- `IO`: stdin-consuming read operations.

Keep new host-facing side effects capability-gated.

## Standard Library (`lib/`)

The embedded standard library is loaded by `Environment::with_stdlib()` from the compile-time `STDLIB` list in `src/lib.rs`. Since the rows port (#297 step 0), `20-condensation.lisp` is embedded like the rest (`21-interfaces.lisp` was removed in 0.3 — protocols are the one dispatch system).

Notable modules:

- `00-core.lisp`: `defun`, `defun*`, `prog2`, `cset`, `csetq`.
- `01-list.lisp`, `02-cxr.lisp`, `04-predicates.lisp`, `05-math.lisp`: core list, CXR, predicate, and math helpers.
- `07-shell.lisp`: shell convenience layer; requires `SHELL` capability.
- `08-vau.lisp`: Kernel-style derived forms.
- `09-lisp15.lisp`: Lisp 1.5 appendix compatibility.
- `10-testing.lisp`: Lisp xUnit helpers.
- `11-optimizer-vau.lisp`: source optimizer passes.
- `12-control.lisp` through `18-format.lisp`: control, functional, strings, sets/hash, conditions, arrays, and format helpers.
- `19-call-graph.lisp`: call graph analysis.
- `20-condensation.lisp`: condensation layer — **`defrecord`**, THE record definition form (0.3): `(defrecord Name (field type)... [(:invariant ...)] [(:derive equality|printer|lens ...)])` defines a branded, checker-denotable, nominal, row-subsumable type over one runtime representation (StructObj), tier-dispatched (all-native fields compile via the internal `defstruct-typed` machinery; anything else gets dynamic constructor/accessors with lockstep-declared branded schemes). Generates `make-Name`, `Name-p`, `Name-field`, `validate-Name`; values are read generically with `record-ref` and updated with `record-with`; every record flows through row-polymorphic functions naming a subset of its fields. Also the `derive` form and the sexpr change plane (`edit!`, `condense-trace`). `defconcept` and untyped `defstruct` were removed in 0.3 (see CHANGELOG.md); `defstruct-typed` survives only as internal machinery.
- `21-cl-compat.lisp`: Common Lisp compatibility forms such as `setf`, `push`, `pop`, `incf`, `decf`, `subseq`, and `elt`.
- `22-guard.lisp`: guard fences — `with-fuel`, `with-capabilities`, `sandboxed`, capability manifests (`capabilities-needed`).
- `23-match.lisp`: structural pattern language — `pat-match`, `match`, `destructuring-bind`, `sgrep`/`sgrep-file`, `rewrite`.
- `24-rules.lisp`: the rulebook optimizer — `defrule`/`list-rules`/`apply-rules` feeding `optimize-form`.
- `25-variants.lisp` through `28-types.lisp`: sums (`defvariant`/`variant-case`, Option/Result), instrumentation (`trace`/`time`/`step-count`), modules (`defmodule`), and the declared type table.
- `29-protocols.lisp`: THE dispatch system — typed protocols (`defprotocol`/`definstance`, dispatch-position aware, fn-first HOFs like `map`) plus conformance (`implements!`/`implements-p`).
- `97-doc-renderer.lisp`, `98-help-system.lisp`, `99-help-data.lisp`: REPL help/documentation.

`defun*` is the recommended default function definition form when HM-style type inference should be attempted automatically. It falls back silently to a plain lambda when types are ambiguous. Use `defun` when type inference should not run.

## Optimization Philosophy

Prefer the Lisp layer; keep the Rust kernel small. If an optimization can be expressed as a Lisp-to-Lisp transform, implement it as an optimizer pass in `lib/11-optimizer-vau.lisp` rather than growing the Rust evaluator.

Kernel/Rust changes are appropriate for hot-path evaluation mechanics that have no Lisp-layer expression, such as argument collection, environment-frame allocation, value representation, stack behavior, or native-code backend integration.

## Testing Guidance

Tests live in `tests/` and module-level `#[cfg(test)]` blocks. Integration coverage includes arithmetic, lists, control flow, functions, fexprs/vau, dynamic variables, conditions, sandboxing, optimizer/compile bridge behavior, typed JIT behavior, and Lisp stdlib suites.

When changing behavior:

- Add focused Rust integration tests under `tests/` for host-visible semantics.
- Add Lisp tests when the behavior belongs to the Lisp layer or stdlib.
- Use `cargo test <name>` for tight iteration, then run `cargo test` before finishing.
- Run `cargo fmt --all` before committing.
- Run `cargo clippy --workspace --all-targets` when practical; note any environment-only failures.

## Coding Conventions

- Rust edition is 2024.
- Do not wrap imports in `try`/`catch`-style blocks.
- Keep CLI concerns in `cli/`; keep interpreter logic in the library crate.
- Keep host side effects sandboxed behind explicit capability checks.
- Preserve Lisp 1.5 compatibility unless a modern extension is clearly documented.
- Prefer adding stdlib functionality in Lisp over adding Rust builtins unless performance, host integration, or representation access requires Rust.
- Keep benchmark comparison crates excluded from the workspace.
