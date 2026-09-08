# Lamedh on SBCL

A standalone Common Lisp (SBCL) implementation of the Lamedh Lisp 1.5
dialect, living alongside the reference Rust implementation (`../src`) as a
**parallel**, not a wrapper: this port does not embed or call into the Rust
`lamedh` crate. It has its own reader, evaluator, environment model, and
native primitives, all written in Common Lisp under `sbcl/src/`.

It reuses the reference implementation's own standard-library *source*
(`sbcl/lib/*.lisp` are byte-for-byte copies of files under `../lib/`) so
that the same Lamedh-level code — `defun`, `let`, macros, fexprs, `vau`,
`prog`/`go` loops, and so on — runs unmodified on both interpreters. That is
the load-bearing conformance claim of this port: not "the two READMEs agree
about the syntax," but "the same `.lisp` files execute correctly on both."

## Running it

```sh
cd sbcl
sbcl --non-interactive \
  --eval '(require :asdf)' \
  --eval '(asdf:load-asd (truename "lamedh.asd"))' \
  --eval '(asdf:load-system :lamedh)' \
  --eval '(lamedh-rt:toplevel)'
```

or, for a REPL:

```sh
sbcl --non-interactive --eval '(require :asdf)' \
  --eval '(asdf:load-asd (truename "lamedh.asd"))' \
  --eval '(asdf:load-system :lamedh)' \
  --eval '(lamedh-rt:run-repl)'
```

`(lamedh-rt:run-file "path/to/script.lisp")` and `(lamedh-rt:run-string
"(+ 1 2)")` are the embedding entry points — analogous to the reference
implementation's `eval_str`/`load_file`.

## Running the ported test suite

```sh
cd sbcl
sbcl --non-interactive --load tests/run-tests.lisp
```

This loads `sbcl/tests/*.lisp` — byte-for-byte copies of the reference
implementation's `tests/lisp/*.lisp` language-level fixtures — and runs them
through the bootstrapped `(run-tests)` (from `lib/10-testing.lisp`). At the
time of writing this passes all 188 assertions across arithmetic, lists,
predicates, list-processing (`mapcar`/`assoc`/`subst`/`sublis`/...),
strings/symbols, every core special form (`prog`/`let`/`label`/`defexpr`/
`defmacro`/quasiquote), `for`/`while` loops, hash tables and property
lists, and bitwise operations.

## What is a genuine parallel implementation vs. what is explicitly out of scope

**Implemented from scratch in Common Lisp** (`sbcl/src/`):

- `reader.lisp` -- a hand-written recursive-descent reader matching the
  reference grammar: Lisp-1.5 octal `177Q` and assembly-style hex `0FFh`
  literals, CL-style `#x`/`#b`/`#o` radix literals, the `'c'` character
  literal (disambiguated from the quote reader macro), earmuff (`*name*`)
  and keyword (`:name`) symbol classes, `` ` ``/`,`/`,@`/`#'` reader macros,
  line and nesting `#| |#` block comments, and shebang stripping.
- `runtime.lisp` -- the environment model (lexical frame chain over a
  single global/root frame; dynamic/special variables reuse SBCL's own
  `PROGV`/`SYMBOL-VALUE` machinery rather than reimplementing shallow
  binding) and the evaluator: a trampolined `LEVAL` giving proper tail-call
  elimination through `PROGN`/`IF`/`COND`/`LET`/`LET*`/lambda application
  (verified to 200,000 stack frames deep in one call in ad hoc testing),
  every special form in the reference implementation's core set (`QUOTE`,
  `QUASIQUOTE`/`UNQUOTE`/`UNQUOTE-SPLICING`, `IF`, `COND`, `AND`, `OR`,
  `PROGN`, `SETQ`, `DEF`, `DEFDYNAMIC`/`DEFVAR`, `LAMBDA`, `FUNCTION`,
  `LABEL`, `DEFINE`, `DEFEXPR`, `DEFMACRO`, anonymous `MACRO`/`FEXPR`/`VAU`
  constructors, `PROG`/`RETURN`/`GO`, `WHILE`, `FOR`, `LET`/`LET*`, `BLOCK`/
  `RETURN-FROM`, `CATCH`/`THROW`, `UNWIND-PROTECT`, `HANDLER-CASE`), and
  first-class lexical closures, macros, fexprs, and Kernel-style `vau`
  operatives.
- `printer.lisp`, `builtins.lisp` -- readable-value printing and the
  ~120 native primitives the bootstrap library is built on (cons-cell
  kernel, arithmetic, strings, hash tables, arrays, property lists,
  `eval`/`apply`/`funcall`).

**Reused unmodified as data** (`sbcl/lib/`): `00-core.lisp` through
`05-math.lisp`, `08-vau.lisp`, `09-lisp15.lisp`, `10-testing.lisp`,
`12-control.lisp` through `15-sets-hash.lisp`, `17-arrays.lisp`, and
`21-cl-compat.lisp` — copied verbatim from `../lib/`. This is the Prelude
plus the control-flow/functional/string/set/array/CL-compat layer: `defun`
(including `&optional`/`&key` extended lambda lists), `when`/`unless`/
`case`/`typecase`/`dolist`/`dotimes`, `flet`/`macrolet`/`fexprlet`/
`vaulet`, the functional toolkit (`filter`/`reduce`/`fold`/`zip`/`group-by`/
...), the full string library, set/alist/hash-table helpers, arrays, and
`setf`/`push`/`pop`/`incf`/`decf`. `JIT-OPTIMIZE` (which `defun`'s expansion
calls on every definition) is a no-op special form here rather than an
error, so these files load byte-for-byte unmodified.

**Explicitly out of scope for this port** (documented, not silently
dropped): the typed JIT / Cranelift backend and `defun*`/HM type inference
(`src/check.rs`, `src/jit*`); `defrecord`/protocols/condensation
(`20-condensation.lisp`, `29-protocols.lisp`); the module/namespace system
(`06-require.lisp`, `27-modules.lisp` — `REQUIRE`/`PROVIDE`/`DEFMODULE`
exist here only as harmless no-ops so files that call them still load);
guard fences and capability processes (`22-guard.lisp`); structural pattern
matching and the rule-based optimizer (`23-match.lisp`, `24-rules.lisp`);
sum types and instrumentation (`25-variants.lisp`, `26-instrument.lisp`);
`format` (`18-format.lisp`); and every host-integration module gated behind
a capability in the reference implementation (`07-shell.lisp`, networking,
TLS, OS, regex, base64/hex/URL/JSON/MIME). None of these are conceptually
irreconcilable with this architecture — they were simply out of scope for
the time available. The sandboxing capability model itself is not
reproduced; this port is meant to run trusted Lamedh source, matching how
it is invoked here (its own process, not embedded as a library the way
`lamedh-cli` embeds the Rust crate).

## Deliberate semantic deviations

- **Integers are CL bignums**, not wrapping 64-bit integers. The reference
  implementation's `OVERFLOW` flag and float-promotion-on-overflow behavior
  is not reproduced.
- **`EQ`** is identity for conses, symbols, and callables, but *value*
  equality for the immutable atomic types (numbers, characters, strings) --
  matching the reference implementation's derived structural equality on
  `LispVal`, not a naive CL `EQ` (which would make two `read`-produced
  strings with the same content compare unequal).
- **`PROG`'s `GO`/`RETURN`** are implemented with CL `CATCH`/`THROW` under
  fixed tags scoped to the nearest enclosing `PROG` (an unmatched `GO`
  label signals an error rather than searching an outer `PROG`), matching
  the reference implementation's own non-lexical, nearest-enclosing-loop
  semantics.
- **`HANDLER-CASE`** supports exactly the `(error (var) ...)` clause shape
  used throughout the reference stdlib (there is no user-facing condition
  type hierarchy to discriminate on here).
- **Backtraces, kernel fuel budgets (`WITH-FUEL`), and the capability mask
  (`WITH-CAPABILITIES`)** are not implemented; forms that use them are
  simply not loaded (see "explicitly out of scope" above).

## Architecture note: native compilation

The reference implementation ships a typed JIT (Cranelift) as a *separate*
compiled tier alongside its tree-walking interpreter. This port does not
duplicate that machinery. Instead, `JIT-OPTIMIZE` is a no-op here (see
`runtime.lisp`), and the intended path to native speed is architecturally
simpler on this host: once a Lamedh-level type check (a port of
`src/check.rs`'s HM inference, or the `defun*`/`declare-typed`/`check-type`
surface) accepts a function as fully typed, that function's body can be
handed directly to **SBCL's own native compiler** (`compile`) instead of
being interpreted by `LEVAL` -- SBCL is already a mature, optimizing
native-code compiler for exactly the value representations this port uses
(fixnums, double-floats, conses, `SIMPLE-VECTOR`). This eliminates the need
for a second code-generation backend entirely: the type checker's job
becomes deciding *when* it is safe to compile, not *how* to emit machine
code. This is a natural next step, not yet implemented.
