# Lamedh on SBCL

A standalone Common Lisp (SBCL) implementation of the Lamedh Lisp 1.5
dialect, living alongside the reference Rust implementation (`../src`) as a
**parallel**, not a wrapper: this port does not embed or call into the Rust
`lamedh` crate. It has its own reader, evaluator, environment model, and
native primitives, all written in Common Lisp under `sbcl/src/`.

It reuses the reference implementation's own standard-library *source*
(`sbcl/lib/*.lisp` are byte-for-byte copies of files under `../lib/`) so
that the same Lamedh-level code runs unmodified on both interpreters. That
is the load-bearing conformance claim of this port: not "the two READMEs
agree about the syntax," but "the same `.lisp` files execute correctly on
both." `sbcl/src/bootstrap.lisp` loads every file in the same order as the
reference implementation's `with_stdlib()` (`src/lib.rs`'s
`STDLIB_SOURCES`) — the full Prelude plus every optional module: records
and the condensation change-plane, the module/namespace system, guard
fences, the structural pattern matcher and rulebook optimizer, sum types,
instrumentation, typed protocols, text/ports/base64/hex/url/json/mime,
shell, OS, TCP/UDP, and regex.

**A design principle this port follows throughout**: when a reference
`.lisp` file needed something host-specific, the fix was to add the
*minimal* native kernel hook that file actually calls (a record
constructor, a byte-level port primitive, a UTF-8 codec) and then load the
*unmodified* Lamedh source — never to reimplement that file's own logic in
Common Lisp. The Lamedh-level API stays the shared, portable surface;
only the handful of truly representation-specific primitives underneath it
differ per host, exactly as they do between the reference implementation's
own Rust kernel and its Lisp-layer stdlib.

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
implementation's `tests/lisp/*.lisp` language-level fixtures — and runs
them through the bootstrapped `(run-tests)` (from `lib/10-testing.lisp`).
At the time of writing this passes all **512 assertions** across
arithmetic, lists, predicates, list-processing, strings/symbols and string
completions, the TEXT UTF-8 boundary, every core special form, loops, hash
tables/plists, bitwise operations, and the broader stdlib-battery and
FORMAT/port suites (`95-stdlib-batteries.lisp`, `96-format-and-io.lisp`).

### Running the `examples/` programs

The repository root's `examples/<name>/main.lisp` programs (the reference
implementation's own showcase/self-check suite, run against it by
`tests/test_examples.rs`) are unmodified Lamedh source, so they can be run
against this port exactly like any other file: grant `READ-FS` (most of
them read a file), bind `*ARGV*`, and `run-file` each one — for example,
from a REPL started with `(enable-all-features)` already in effect (the
CLI's own default):

```lisp
(run-string "(def *ARGV* ())")
(run-file "../examples/fibonacci/main.lisp")
```

At the time of writing, **51 of the 53** examples run to completion
unmodified. The two that do not — `lamed-nebula` and `typed-numerics` —
both call `DEFUN-TYPED`, the reference implementation's HM-checker/typed-
JIT entry point; this port signals a clear, named error for it (see "The
type checker" below) rather than running it, since there is no checker
here to elaborate a typed signature against. Every other example,
including several that specifically exercise this port's own recent
work — `sandbox-fuel` (real `SPAWN` threads, `WITH-CAPABILITIES` denial),
`text-stats`/`shapes` (capability-gated file I/O, protocol dispatch),
`quicksort`/`mergesort`/`priority-queue`/`monte-carlo-pi` (`RANDOM`/
`RANDOM-SEED!`) — passes its own self-check.

## What's implemented from scratch in Common Lisp (`sbcl/src/`)

- `reader.lisp` — a hand-written recursive-descent reader matching the
  reference grammar: Lisp-1.5 octal `177Q` and assembly-style hex `0FFh`
  literals, CL-style `#x`/`#b`/`#o` radix literals, the `'c'` character
  literal (disambiguated from the quote reader macro), earmuff (`*name*`)
  and keyword (`:name`) symbol classes, `` ` ``/`,`/`,@`/`#'` reader macros,
  the `#S(brand v1 v2 ...)` record literal, line and nesting `#| |#` block
  comments, and shebang stripping.
- `runtime.lisp` — the environment model (lexical frame chain over a
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
  `RETURN-FROM`, `CATCH`/`THROW`, `UNWIND-PROTECT`, `HANDLER-CASE`,
  `DEFSTRUCT-TYPED`, `WITH-FUEL`, `WITH-CAPABILITIES`), first-class lexical
  closures, macros, fexprs, and Kernel-style `vau` operatives, plus the
  record system (one runtime representation, `LAMEDH-STRUCT`, shared by
  every `DEFRECORD`/`DEFSTRUCT-TYPED` — see "Records" below) and a kernel
  step-fuel counter charged once per trampoline iteration
  (`WITH-FUEL`/`STEP-COUNT`'s shared unit).
- `printer.lisp` — readable-value printing, including `#S(...)` records.
- `builtins.lisp` — the cons-cell kernel, arithmetic (with Lisp-1.5 char
  promotion: a Char value numifies to its code point in `+`/`-`/`=`/`<`/...,
  matching `(+ 'a' 1)` => `98`), strings, hash tables, arrays,
  property lists, `eval`/`apply`/`funcall`/`gensym`/`intern`.
- `compile.lisp` — the ahead-of-time Lamedh → SBCL compiler every `LAMBDA`/
  `DEFUN` attempts unconditionally (see "Ahead-of-time compilation" below).
- `extra.lisp` — the native hooks the condensation/records, module, guard-
  fence, and instrumentation *Lamedh* files call into: `RECORD-NEW`/
  `RECORD-REF`/`RECORD-WITH`/`RECORD-BRAND`/`RECORD-FIELDS`, the
  `DECLARE-TYPE!`/`SEE-TYPE` axiom surface (see "The type checker" below),
  `ERRORSET`/`SEE-SOURCE`, `SEXPR-RENAME` and the `$MODULE-SOURCE-LOOKUP`
  module registry, `KERNEL-FUEL-REMAINING`/`KERNEL-FUEL-SET!`, and `SPAWN`'s
  real-thread implementation (see "Deliberate deviations").
- `io.lisp` — host I/O, every capability-gated primitive checking
  `REQUIRE-FEATURE!`: files, byte-level ports (file/memory/stdio), the
  explicit UTF-8↔String boundary, `(shell ...)` (`uiop:run-program`), OS
  process/environment/time/randomness/spawn primitives (`sb-posix`), TCP/
  UDP (`sb-bsd-sockets`), a Thompson-NFA/Pike's-VM regex engine, and
  honest "unavailable" TLS stubs (see below).

## Reused unmodified as data (`sbcl/lib/`)

Every file `sbcl/src/bootstrap.lisp` loads is a byte-for-byte copy of the
corresponding `../lib/*.lisp` file: `00` through `06`, `08`, `09`, `10`,
`12` through `44`, and `97` through `99` — i.e. everything in the
reference implementation's `with_stdlib()` load list. This is the entire
standard library: `defun` (including `&optional`/`&key` extended lambda
lists), `when`/`unless`/`case`/`typecase`/`dolist`/`dotimes`, `flet`/
`macrolet`/`fexprlet`/`vaulet`, the functional toolkit, the full string
library, set/alist/hash-table helpers, arrays, `setf`/`push`/`pop`/`incf`/
`decf`, `REQUIRE`/`PROVIDE` and `DEFMODULE`/`WITH-MODULE`/`IMPORT`,
`DEFRECORD`/`DERIVE`/the sexpr change plane, guard fences and real
threaded capability processes, `MATCH`/
`DESTRUCTURING-BIND`/`SGREP`/`REWRITE`, the rulebook optimizer,
`DEFVARIANT`/`VARIANT-CASE`/Option/Result, `TRACE`/`TIME`/`STEP-COUNT`,
typed protocols (`DEFPROTOCOL`/`DEFINSTANCE`) and conformance
(`IMPLEMENTS!`), `TEXT`/`PORTS`/`BASE64`/`HEX`/`URL`/`JSON`/`MIME`, `SHELL`,
`NET`/`TCP`/`UDP`/`HTTP`, `OS`/`OS-LINUX`, `TLS`, `REGEX`, and the REPL
help/documentation system.

## The type checker

The reference implementation's HM type checker (`src/check.rs`) is not
ported: this is the one genuinely large subsystem left out, since a real
port would be its own substantial project. What *is* preserved, honestly:
`DECLARE-TYPE!`/`SEE-TYPE` (the declared-scheme axiom surface every
`DEFRECORD`/`DEFVARIANT`/protocol instance calls into) records every
declared scheme and reports it back as `(DECLARED scheme)` — which is
exactly what a `DECLARE-TYPE!` axiom *means* even in the reference
implementation (trusted at call sites, not derived from the body) — while
anything never declared reports `(DYNAMIC "not statically checked in this
port")` rather than being reported as verified. `CHECK-TYPE` similarly
cannot statically infer a type without a checker; instead of leaving it
unbound, it actually *runs* the given expression and reports what
happened (an honest dynamic approximation — see its docstring in
`extra.lisp` for the one deliberate exception: a missing-protocol-
instance error is reformatted to match the checker's own backtick-quoted
message convention, since that specific fact — "no instance for this
type" — is equally true whether discovered statically or dynamically).
`defun*`/HM inference and the typed JIT are consequently also not
ported; `JIT-OPTIMIZE` is a no-op special form so `defun`'s expansion
(which calls it on every definition) still loads unmodified, and
`DEFUN-TYPED` itself — the reference implementation's typed-definition
entry point — signals a clear, named "not supported in this port" error
rather than being left unbound (an opaque "Unbound variable" crash); the
only two `examples/` programs that do not run under this port
(`lamed-nebula`, `typed-numerics`) are the ones that call it (see
"Running the `examples/` programs" above). `RECORD-COMPILED-P` always
reports `NIL`: this port has no separate compiled tier (every record —
whichever tier `DEFRECORD` would have chosen in the reference
implementation — uses the same `LAMEDH-STRUCT` representation), so
reporting `NIL` is accurate, not a missed optimization to fix.

## Ahead-of-time compilation (`compile.lisp`)

The reference implementation's typed JIT (Cranelift) is a *separate*
code-generation backend bolted onto its interpreter, gated by its type
checker's verdict. This port needed no equivalent second backend, and no
type checker to gate it: **every** `LAMBDA`/`DEFUN` is handed directly to
**SBCL's own native compiler** (`compile`) at the moment it is created
(`src/compile.lisp`'s `TRY-COMPILE-LAMBDA`, hooked into the `LAMBDA`
special form) — SBCL is already a mature, optimizing compiler for exactly
the value representations this port uses (fixnums, bignums, double-floats,
conses, characters, strings). The compiler decides *whether* it recognizes
the body well enough to emit native code, not *whether it is safe to try*:
recognizing a body is form-by-form structural, not type-directed, so
`defun*`/`check-type` inference plays no role here (see "The type checker"
above for why that piece is not ported).

The supported subset is intentionally narrow: literals, `QUOTE`, `IF`/
`PROGN`/`AND`/`OR`, parameter references (native CL lexicals) and free
variable references (`ENV-RESOLVE` against the lambda's closed-over
environment, so a later redefinition is still seen), and calls to any
name that is **already** bound, at the moment this lambda is compiled, to
an ordinary function or another compiled/interpreted lambda — never a
macro, fexpr, or vau, and never unbound. That last rule is what makes a
self- or mutually-recursive `DEFUN` safe without a dedicated recursion
check: `DEF`/`LABEL` bind a function's own name only *after* its `LAMBDA`
form (and hence its compile attempt) has already run, so such a function
always finds its own name unbound at compile time and falls back to the
tree-walking interpreter — which is exactly where it needs to run anyway,
since only `LEVAL`'s trampoline gives self-tail-calls this port's O(1)-
Lamedh-stack guarantee. `LET`/`SETQ`/loops/dynamic parameters/nested
closures/condition handling, and any call to a not-yet-bound or
macro-like name, fall back the same way: tree-walked, fully correct,
simply not (yet) native-compiled. Falling back is never a degraded mode —
an uncompiled lambda behaves identically to how every lambda behaved
before this compiler existed. On the reference embedded standard library,
this compiles roughly 45% of all top-level functions (442 of 952) to
native SBCL code on a cold bootstrap, dominated by leaf arithmetic/
predicate/string helpers and functions built on already-loaded
infrastructure; recursive control constructs (loops expressed as
self-recursion, which is idiomatic throughout this stdlib) are the main
class that remains interpreted. Widening the subset — `LET`, or a
self-tail-call loop rewrite that would let compiled-to-compiled recursion
keep an O(1) Lamedh stack — is future work, not a correctness gap in what
exists today.

## Deliberate deviations

- **Integers are CL bignums**, not wrapping 64-bit integers; the reference
  implementation's `OVERFLOW` flag and float-promotion-on-overflow behavior
  is not reproduced.
- **`EQ`** is identity for symbols and callables, but *value* equality for
  the immutable atomic types (numbers, characters, strings) and *deep
  structural* equality for records/structs — recursing into every field,
  including any cons cells a field happens to hold — matching the
  reference implementation's derived `LispVal` `PartialEq` exactly,
  asymmetry and all: a cons cell is never `EQ` to anything, not even
  itself by identity (Lisp 1.5: `EQ` is defined only for atoms), while
  that same cons *nested inside a struct field* is compared structurally
  as part of the struct's own equality (this is what makes `EQUAL` on two
  `DEFVARIANT` values like `(ok 14)` work at all, since `EQUAL`'s own
  definition, `lib/04-predicates.lisp`, bottoms out at `EQ` for atoms).
  `RANDOM`/`RANDOM-SEED!` are ported too: deterministic given the same
  seed, but not bit-for-bit identical to the reference implementation's
  RNG — every `examples/` program using them relies only on statistical
  or structural properties (a shuffled list stays a permutation, a Monte
  Carlo estimate converges), never the exact sequence.
- **Integer `/`** truncates (C/Rust-style), unlike CL's exact-rational
  result for non-dividing integers; **`ROUND`** rounds half away from zero
  (`f64::round`'s convention), not CL's round-half-to-even.
- **A Char value numifies** to its code point in every arithmetic/
  comparison builtin (`(+ 'a' 1)` => `98`, `(= 'A' 65)` => true) — the
  reference implementation's "char promotes to integer like C" rule.
  `CODE-CHAR` returns a one-character *string* and `MAKE-CHAR` returns a
  genuine Char value — two different return types, both intentional, per
  the reference stdlib's own documented convention.
- **`PROG`'s `GO`/`RETURN`** use CL `CATCH`/`THROW` under fixed tags scoped
  to the nearest enclosing `PROG` (an unmatched `GO` label errors rather
  than searching an outer `PROG`), matching the reference implementation's
  own non-lexical, nearest-enclosing-loop semantics.
- **`HANDLER-CASE`** supports the `(error (var) ...)` clause shape used
  throughout the reference stdlib, plus a catch-all for any other signaled
  CL condition (so a builtin's internal error — division by zero, a
  wrong-type argument — is still catchable).
- **The capability/sandbox model is enforced.** Every host-facing
  primitive (files, ports, shell, OS, network) calls `REQUIRE-FEATURE!`
  against a granted-capability set (the CLI's `--sandbox`/`--capability`
  flags, or `ENABLE-FEATURE`/`DISABLE-FEATURE` from host code) intersected
  with the dynamic-extent `WITH-CAPABILITIES` mask, matching the reference
  implementation's attenuation-only semantics: a fence can only narrow the
  active set, never widen it. Once a resource handle is open (a port, a
  spawned OS process), subsequent operations on that same handle are not
  re-gated — "continue authority" — mirroring the reference implementation.
  The mask itself is `*CAPABILITY-MASK*`, either the keyword `:ALL` (no
  fence: every host-granted capability is effective) or an explicit list
  of capability-name strings — deliberately not `NIL` for "no fence",
  since `NIL` and `'()` are the same object in Lisp and a `NIL`-means-
  unmasked convention could not distinguish that from `(WITH-CAPABILITIES
  () ...)`'s explicit, everything-denied empty fence.
- **`MAKE-ENVIRONMENT`** (no arguments) returns a genuinely independent root
  environment: its own global table, seeded from a snapshot of the calling
  environment's current bindings at the moment of the call, with no shared
  mutable state afterward. `(the-environment)`/`(current-environment)`
  capture the calling lexical scope. One narrower-than-the-reference-
  implementation scope decision, made deliberately given this port's
  reliance on process-wide CL symbol identity: dynamic-variable defaults,
  property lists, record/variant schemas, and declared type schemes stay
  process-wide, not per-environment.
- **`SPAWN`/`AWAIT`** (capability processes, `lib/22-guard.lisp`) run on
  genuine `SB-THREAD` threads. Each spawned child gets a fresh root
  environment (a snapshot of the parent's globals at fork time, via
  `MAKE-FRESH-ROOT-ENV`), so later global definitions in either thread are
  invisible to the other — real share-nothing for the Lamedh environment
  model. The same process-wide-cache caveat as `MAKE-ENVIRONMENT` above
  applies here too: plists, record/type schemas, and the dynamic-symbol
  registry are still shared CL hash tables across every thread.
- **Regex** (`lib/44-regex.lisp`) is backed by a Thompson-NFA byte-code
  compiler and a Pike's-VM simulator: the AST compiles to a small program
  (`:CHAR`/`:ANY`/`:CLASS`/`:BOL`/`:EOL`/`:SAVE`/`:JMP`/`:SPLIT`/`:MATCH`),
  and matching simulates every live thread in lockstep with per-step,
  per-program-counter deduplication — the same mechanism that gives RE2 its
  guarantee. Total work is bounded by O(length(s) × length(program)):
  linear in the input for a fixed pattern, with no possibility of the
  catastrophic exponential blowup a backtracking matcher suffers on
  patterns such as `(a+)+b`. Leftmost-first (greedy, Perl/PCRE) priority
  among ambiguous alternatives is preserved by processing threads in
  priority order; unanchored search injects a fresh lowest-priority start
  thread at every position not yet matched, in one linear left-to-right
  pass. Named capture groups and full Unicode-aware classes remain
  unsupported, as they were under the prior backtracking matcher.
- **TLS is honestly unavailable**, not silently degraded: every
  `lib/43-tls.lisp` primitive exists (so the file loads without error) but
  signals a clear error when actually invoked. This is a genuine external-
  dependency gap — SBCL has no built-in TLS, and this port was built
  without network access to fetch an OpenSSL binding (`cl+ssl`) — not a
  design choice; wiring one in is a mechanical follow-up.
- **`OS:NOW`** has one-second resolution (`get-universal-time`), not
  nanosecond; **`OS:HOSTNAME`**/**`OS:PPID`** use `(machine-instance)`/
  `sb-posix:getppid` rather than reading `/proc` directly, and **file
  permission predicates** (`FILE-WRITABLE-P`, ...) are approximations, not
  a real `stat`-based check.
