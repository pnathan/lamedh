# 9. The Typed JIT and Embedding

This chapter has two parts. §§9.1–9.6 cover how Lamedh runs your code
faster without you asking it to: the tiers between "tree-walked" and
"native machine code," how to read the checker's verdict on a function,
and the guarantees that hold across every tier. §§9.7–9.13 cover the other
side of "embeddable": linking `lamedh` into a Rust program, handing it
Lisp source, granting it capabilities, and getting typed values back out.

## 9.1 Three Tiers, One Semantics

Every `defun` in Lamedh ends up running in one of three ways:

1. **Tree-walked.** The evaluator recurses over the s-expression directly.
   The universal fallback — anything is tree-walkable, including macros,
   fexprs, `vau`, and code touching `eval` or `the-environment`.
2. **Typed, closure-compiled.** The body type-checks under Hindley-Milner
   inference to a monomorphic scalar/array signature and gets lowered to a
   tree of boxed Rust closures (`jit/runtime.rs`) that skip the tag
   dispatch and environment-chain walk of the evaluator but still run as
   ordinary Rust calls — no machine code emitted. What you get from a
   `--no-default-features` build.
3. **Typed, natively compiled.** Same type-checked core, lowered through
   Cranelift to actual machine code. What the default build (the `jit`
   Cargo feature) gives you when a function qualifies.

Tiers 2 and 3 are both reached through the same mechanism: a **one-door**
policy. You never ask for compilation explicitly for a plain `defun` — every
`defun` quietly *attempts* typed compilation the moment it's defined. If
inference succeeds and the result is a monomorphic scalar/array signature,
you silently get tier 2 or 3. If it doesn't, the function stays tier 1 and
nothing is printed — a function that can't be typed is not an error, it's
just dynamic. The project's shorthand for this is "types are weather, not
architecture": they inform how fast your code runs, not whether it runs.

## 9.2 Reading the Verdict: `see-type`

`(see-type 'name)` reports exactly what happened to a function, as data:

```lisp
(defun od-sq (x) (* x (* x 1)))
(see-type 'od-sq)
; => (TYPED (-> (INT64) INT64) COMPILED)
```

`COMPILED` means tier 3: this is running as native code. Under a
`--no-default-features` build, or for functions the native backend can't
yet lower, the same typed verdict instead reads `INTERPRETED` — tier 2, the
closure backend.

A function whose body genuinely can't be pinned to one numeric kind stays
tier 1, reported as `CHECKED`:

```lisp
(defun od-first (l) (car l))
(see-type 'od-first)
; => (CHECKED (FORALL (A) (-> ((LIST A)) A)))
```

`od-first` still type-checks — the checker proves a polymorphic scheme for
it — but a polymorphic `LIST A -> A` has no single unboxed representation
to compile to, so it runs through the ordinary evaluator and still
*works*, identically to any other function: `(od-first '(7 8))` => `7`.

The full verdict vocabulary is structured as data so you can pattern-match
on it programmatically (`lib/20-condensation.lisp` builds a classifier on
exactly this shape):

| Verdict | Meaning |
|---|---|
| `(TYPED sig COMPILED)` | Monomorphic, native machine code (tier 3). |
| `(TYPED sig INTERPRETED)` | Monomorphic, typed closure backend (tier 2). |
| `(CHECKED scheme)` | Type-checks, but polymorphic or otherwise not compile-eligible; tree-walked (tier 1). |
| `(DECLARED scheme)` | An axiom asserted via `declare-type!`, trusted at call sites but not derived from a body (row-typed concept accessors use this). |
| `(TYPE-ERROR msg)` | The body doesn't type-check at all; still runs (Lamedh doesn't reject ill-typed `defun`s), just never attempts compilation. |
| `(DYNAMIC reason)` | Not a plain lambda — variadic, a macro, a fexpr, etc. — outside the typed island entirely. |

`see-source` shows you the original, uncompiled definition regardless of
tier — compilation never destroys the source form used for introspection:

```lisp
(defun od-dbl (x) (* x 2))
(see-source 'od-dbl)
; => (LAMBDA (X) (* X 2))
```

## 9.3 What Compiles and What Doesn't

The typed island covers **monomorphic scalar and array code**: integers,
floats, chars, and arrays of those, with arithmetic, comparisons, `if`,
`let`, and self/mutual recursion. It does not cover:

- **Lists.** `car`/`cdr`/`cons` and anything built on them stay `CHECKED`
  and tree-walked — `od-first` above is the example. Lists have no fixed
  unboxed layout the way an `(array int64)` does.
- **Closures over row types and `any`.** Row-polymorphic code (the kind
  `defconcept`/`definterface` produce) and anything bottoming out at the
  gradual-typing top type `any` stays dynamic.
- **Fexprs, `vau`, `eval`, `the-environment`, and create-on-assign `setq`.**
  These touch the interpreter's own machinery and can't be typed at all —
  Wand's triviality result says you can't statically type an operative that
  inspects its own unevaluated operands and calling environment. This is a
  hard boundary, not a missing feature.

For the explicit form, `defun-typed` requires every parameter and the
return type to be given (or inferred) as a monomorphic type up front and
errors if the body doesn't check:

```lisp
(defun-typed (dtsq int64) ((x int64)) (* x x))
(see-type 'dtsq)
; => (TYPED (-> (INT64) INT64) COMPILED)
```

`defun*` sits between plain `defun` and `defun-typed`: like `defun`, it
falls back silently to an ordinary lambda when inference is ambiguous, but
like `defun-typed` it accepts explicit type annotations and prints a
compilation note on success:

```lisp
(defun* add ((x int64) (y int64)) (+ x y))
; stderr: ; defun* ADD : (X int64) (Y int64) -> int64  [compiled]
(see-type 'add)
; => (TYPED (-> (INT64 INT64) INT64) COMPILED)
```

Note that `(defun* sq (x) (* x x))` with no annotations stays `CHECKED`:
`(* x x)` alone doesn't pin `x` to a numeric kind (it's consistent with
both `int64` and `float64`), so inference correctly refuses to guess. Adding
a literal, as in `od-sq` above (`(* x (* x 1))`), or an explicit type
annotation, resolves the ambiguity.

You can trigger a compilation attempt on an already-defined function
explicitly with `jit-optimize`, which is the mechanism `defun` uses
internally and is safe to call redundantly — it never errors, it just
reports what happened:

```lisp
(defun od-first (l) (car l))
(jit-optimize od-first)
; => "OD-FIRST : (forall (a) (-> ((list a)) a))  [checked, dynamic]"
```

## 9.4 Semantics Are Identical Across Tiers

Compilation is a performance decision, never a semantic one: a compiled
function must produce exactly the results, errors, and condition-flag side
effects the tree-walking evaluator would have produced for the same call.
Two places this is easy to get wrong, and where Lamedh has differential
tests pinning the behavior (`tests/test_jit_flag_parity.rs`):

**`mod` is Euclidean, not truncated,** in every tier:

```lisp
(mod -7 3)
; => 2
```

**Integer overflow wraps and sets the `OVERFLOW` flag**, whether the
arithmetic happened in the evaluator or in compiled code:

```lisp
(clear-all-flags)
(defun tadd (x) (+ x 1))
(tadd 9223372036854775807)
; => -9223372036854775808
(flag-set-p 'OVERFLOW)
; => T
```

The one case that looks like it should overflow but doesn't:
`(mod MIN-INT64 -1)` is exactly `0` under Euclidean semantics (unlike
truncated `rem`, which would overflow), and neither tier sets `OVERFLOW`
for it. Division by zero errors identically in both tiers rather than
returning a flag — `(/ 1 0)` and a compiled equivalent both signal
`Division by zero` rather than silently producing `0`. If a compiled
function's result, flags, or error type ever diverge from what
`(declare (no-compile))` on the same body produces, that is a compiler
bug, not an acceptable optimization artifact.

## 9.5 Opting Out of Compilation

Sometimes you want a function pinned to the tree-walker — for debugging,
for benchmarking the interpreter itself, or because a fence around it
needs every call instrumented (§9.6). Two ways to declare that:

`(declare (no-compile))` as the first form in a function body pins that
one function:

```lisp
(defun od-pin (x) (declare (no-compile)) (* x 2))
(od-pin 21)
; => 42
(see-type 'od-pin)
; => (CHECKED (-> (INT64) INT64))
```

The `declare` form is stripped from the body before the function runs, and
`see-type` shows `CHECKED` — it still type-checks fine, it's just never
compiled. An explicit `jit-optimize` on a pinned function is refused rather
than silently compiling anyway:

```lisp
(jit-optimize od-pin)
; => "OD-PIN: compile disabled by declaration"
```

`(declaim (no-compile name1 name2 ...))` pins one or more names *before*
they're defined, which is the form to reach for at the top of a file:

```lisp
(declaim (no-compile od-later))
(defun od-later (x) (+ x 1))
(see-type 'od-later)
; => (CHECKED (-> (INT64) INT64))
```

## 9.6 Fuel Fences Disable Compilation Automatically

`lib/22-guard.lisp`'s `with-fuel` meters execution by code-walking the
fenced body and inserting a tick at every function entry and loop
back-edge. Compiled code — closure or native — has no ticks in it, so
running compiled code inside a fuel fence would silently bypass the
budget. Rather than let that happen, entering a `with-fuel` fence
downgrades compilation for code defined inside it: `jit-optimize` becomes
a no-op, `defun-typed` signals an error, and `defun*` is downgraded to a
plain `defun` with type annotations stripped.

```lisp
(with-fuel 100 (+ 1 2))
; => 3
```

This is one of the few places tier selection is *forced* rather than
inferred — see chapter 7 for the rest of the fuel/capability guard story.

## 9.7 Building Without the Native Backend

The default `cargo build` enables the `jit` Cargo feature, pulling in
`cranelift-jit`/`cranelift-module`/`cranelift-codegen`/`cranelift-frontend`
and giving you tier 3 (`COMPILED`). For a smaller build, no JIT'd machine
code in your process, or a platform Cranelift doesn't target, build
without default features:

```bash
cargo build --no-default-features
```

You keep the full typed checker and tier 2 (the closure interpreter):
every `see-type` verdict that would have read `COMPILED` now reads
`INTERPRETED`, and the function still runs, just without native codegen.
Nothing about the Lisp-level API changes — `defun`, `defun*`,
`defun-typed`, `see-type`, `jit-optimize` all behave the same; only the
tag on a `TYPED` verdict shifts.

## 9.8 Embedding Lamedh in a Rust Host

The `lamedh` crate is designed to be linked into another Rust binary as a
scripting or extension layer. The minimal `Cargo.toml`:

```toml
[dependencies]
lamedh = "0.3"
```

That pulls in the default features (`jit` — Cranelift — and `concurrency` —
channels, `spawn`). To skip Cranelift in your host, same as §9.7:

```toml
[dependencies]
lamedh = { version = "0.3", default-features = false }
```

There's also an `arc-val` feature that swaps every `Rc`/`RefCell` in
`LispVal` and `Environment` for `Arc`/`RwLock`-backed equivalents — a
stepping stone toward thread safety, not a complete one (`SharedState`
still holds a plain `Cell<bool>`, so `LispVal` isn't automatically
`Send + Sync` even with it on).

## 9.9 A Minimal Host

Every embedding follows the same shape: spawn the large stack, build an
environment, evaluate. This is `lamedh-cli`'s own startup sequence, reduced
to the essentials (`cli/src/main.rs` does exactly this, plus argument
parsing):

```rust,ignore
use lamedh::{eval_str, LispError, LispVal};
use lamedh::environment::Environment;

fn run_script(src: &str) -> Result<LispVal, LispError> {
    lamedh::with_large_stack(move || {
        let env = Environment::with_stdlib();
        eval_str(src, &env)
    })
}
```

`Environment::with_stdlib()` gives you a root environment with all
built-in primitives *and* the embedded Lisp standard library (`defun`,
`append`, `defrecord`, `match`, everything under `lib/`) — no `.lisp` files
need to exist on disk at runtime, the stdlib is compiled into the binary.

`with_large_stack` is not optional in practice: the tree-walking evaluator
uses large Rust stack frames for non-tail calls, and the default system
thread stack (2–8 MiB) overflows before the interpreter's own
recursion-depth guard fires. It spawns a dedicated 512 MiB thread
(`INTERPRETER_STACK_SIZE`) and runs your closure there. Because `LispVal`
and `Environment` are `!Send`, construct the environment *inside* the
closure — not outside it and captured — which is why `run_script` builds
`env` inside the closure rather than taking it as a parameter.

Two siblings of `eval_str` round out the entry points: `eval_all(src, &env)
-> Result<Vec<LispVal>, LispError>` evaluates every top-level form in
`src` in order (`eval_str` errors if given more than one form), and
`eval_line(line, &env) -> String` — what the REPL and `-s` batch mode
actually call — evaluates one line and formats the result *or* the error
as a printable string, never returning `Err`:

```rust,ignore
let results = lamedh::eval_all("(defun sq (x) (* x x)) (sq 9)", &env)?;
let text: String = lamedh::eval_line("(+ 1 2 3)", &env);
assert_eq!(text, "6");
```

## 9.10 Granting Capabilities and Registering Host Functions

Filesystem access, shell execution, and stdin reads are off by default in
every environment, embedded or not — see chapter 7 for the full capability
list. From host code, grant one with `enable_feature` (this is exactly
what `--capability` does on the CLI side, per `cli/src/main.rs`):

```rust,ignore
let env = Environment::with_stdlib();
env.enable_feature("SHELL");    // (sh "ls"), (shell-exit-code), ...
env.enable_feature("READ-FS");  // (read-file ...), (file-exists-p ...), ...
```

Names are case-normalized to uppercase, so `"shell"` and `"SHELL"` are
equivalent; `disable_feature`/`feature_enabled` are the matching
remove/query calls.

To expose a Rust function as a Lisp callable, use `register_fn`. The
closure receives already-evaluated arguments and the calling environment
and returns a `Result<LispVal, LispError>`; the name is uppercased on
registration, same as every other binding, and a subsequent
`(defun greet ...)` in Lisp would shadow it like any redefinition:

```rust,ignore
env.register_fn("greet", |args, _env| {
    let name = args[0].as_str_val()?;
    Ok(LispVal::from(format!("Hello, {name}!")))
});
```

```lisp
(greet "world")
; => "Hello, world!"
```

## 9.11 Reading Results Back

`eval_str`/`eval_all` hand you a typed `LispVal`, not a string — match on
the variant you expect:

```rust,ignore
match eval_str("(+ 1 2 3)", &env)? {
    LispVal::Number(n) => println!("got {n}"),
    other => println!("unexpected: {}", lamedh::printer::print(&other)),
}
```

The variants you'll match on most: `Symbol` (interned, case-normalized),
`Number(i64)`, `Float(f64)`, `String(String)`, `Char(u8)`,
`Cons { car, cdr }` and `Nil` for lists, `Array` and `HashTable` for the
mutable collection types, `Struct` for `defrecord`/`defstruct-typed`
values, and `Error` for first-class condition objects
(`ErrorObj { message, data }`). Convenience extractors do the tag check
for you: `as_number() -> Result<i64, _>`, `as_float()`, `as_str_val()`,
`as_list_vec() -> Result<Vec<LispVal>, _>`, `is_truthy()`.

To turn any `LispVal` back into readable Lisp syntax (what the REPL prints,
what `eval_line` returns as text), use `lamedh::printer::print`:

```rust,ignore
let val = eval_str("(list 1 2.5 \"str\")", &env)?;
assert_eq!(lamedh::printer::print(&val), "(1 2.5 \"str\")");
```

## 9.12 Minimal Kernels for Tests

`Environment::with_stdlib()` panics if the embedded stdlib source fails to
parse or evaluate — right for a compile-time invariant, overkill for a
test needing only a couple of builtins. `Environment::new_with_builtins()`
gives a root environment with all 100+ Rust-level primitives but *no*
Lisp standard library — no `defun`, no `append`, none of `lib/`. Lamedh's
own suite uses exactly this (`tests/test_helpers.rs`) to `load_file` and
`load_directory` a `lib/` snapshot from disk instead of the embedded copy,
so tests exercise source edits without a rebuild. Ordinary host code
should reach for `with_stdlib()`; reach for `new_with_builtins()` plus
explicit loading only when you need that level of control over what's on
disk versus embedded.

## 9.13 Where to Go From Here

That closes the manual. Between the nine chapters you have the full
surface: syntax and control flow, data structures, records and the
row-polymorphic checker, closures/macros/fexprs/`vau`, conditions and
restarts, the sandbox and concurrency primitives, the pattern/rewrite
layer, and now the execution tiers underneath all of it plus how to host
the interpreter yourself. For the internals below the Lisp-visible API —
how the evaluator, environment, and JIT are actually implemented in Rust —
see `docs/architecture.md` and `docs/typed-jit-design.md` in the repo. For
what's in flight, the issue tracker and `CHANGELOG.md` are the sources of
truth; behavior documented here was verified against the interpreter it
describes, at version 0.3.0.
