# Type-checked Lisp: a non-compiled checker on the HM engine

Status: design / in progress. Part of the typed-JIT epic (#134). Builds directly
on the inference engine (#135) and the compileable type lattice (#136/#137/#138).

## 0. The thesis

The typed JIT already runs Hindley–Milner-lite inference over function bodies
(`src/jit/infer.rs`, `src/jit.rs`). Today it uses the result for exactly one
thing: **lowering to unboxed native code.** A type that does not land in the
*compileable* sub-lattice (`int64`/`float64`/`bool`/`char`/`(array T)`/struct) is
rejected.

But the inference is sound for far more than that. If we keep inferring past the
compileable boundary — assigning *checkable-but-not-compileable* types to conses,
symbols, strings, closures — we get a **type checker for ordinary Lisp for free**.
The split that makes this work:

> **Checkability ≠ compileability.** "Is this well-typed?" and "can this be
> lowered to unboxed native code?" are the same inference pass with two different
> acceptance criteria. Inference always runs and reports type errors;
> *additionally*, when the inferred type falls in the compileable sub-lattice, we
> emit native code.

This catches a large class of software type errors (passing a cons where a number
is expected, branch-type mismatches, arity errors) with no new engine — only a
wider type language and a second `resolve` target.

## 1. The boundary: Wand, fexpr, vau

We cannot type the operative layer. Wand's triviality result: an operative
(`$vau`, `defexpr`, anything that observes operand *syntax* or the caller
*environment*) admits no useful type. Also outside: `eval`, `current-environment`,
and **create-on-assign `setq`** (which mutates the lexical frame, defeating
lexical typing).

The checker handles this exactly as the compiler does — **gradually**. Inference
flows over the applicative island and degrades to a top type `Any` the moment a
value crosses an operative boundary. `Any` unifies with anything and propagates,
so the checker stays *sound on the typed core* and simply makes no claim across
the membrane. This is the same deopt boundary the codegen path already draws
(`docs/typed-jit-design.md` §2–§3), reused for checking.

Soundness contract: a value typed non-`Any` is guaranteed to have that type on
the applicative island; `Any` is the honest "we don't know" that quarantines the
operative world. (This is gradual typing's standard dynamic frontier; full
blame-tracking is out of scope for the first cut.)

## 2. The one real divergence: generalization vs monomorphization

This is the load-bearing new piece, *not* the extra type variants.

- The **compiler** wants **monomorphization**: `(defun id (x) x)` is rejected
  because no single unboxed representation can be chosen — it is ambiguous *for
  codegen*.
- The **checker** wants **generalization**: `(defun id (x) x)` is `∀α. α → α` —
  perfectly well-typed. To accept it, the checker must *generalize* a function's
  inferred type at its definition (close over the free type variables) and
  *instantiate* fresh copies at each call site (let-polymorphism / Algorithm W's
  `gen`/`inst`).

So the checker is strictly *more* HM than the compiler. The plan adds `gen`/`inst`
to the inference layer; the compiler path keeps requiring a fully-ground
monomorphic type (it just never generalizes, or instantiates-then-monomorphizes
per call site).

## 3. Type language

Extend `Ty` with checkable-but-not-compileable variants (the engine's
`unify`/`occurs`/`resolve` already recurse structurally, so they come along):

```
Compileable (today):  Int64 | Float64 | Bool | Char | Array(Ty) | Struct(def) | Var(u32)
Checkable (new):       List(Ty)        ; homogeneous proper list
                       Pair(Ty, Ty)    ; a cons cell (car, cdr)
                       Symbol
                       Str             ; the boxed LispVal::String (≠ (array char))
                       Fn(Vec<Ty>, Ty) ; arrow type, for higher-order checking
                       Any             ; the gradual top / operative frontier
```

Plus a type scheme `Scheme { vars: Vec<u32>, ty: Ty }` for generalized
definitions.

`is_compileable(&Ty) -> bool` partitions the lattice; codegen gates on it.

## 4. Engine changes (`src/jit/infer.rs`)

- `unify`: extend with the new structural cases (`List`/`Pair`/`Fn` recurse;
  `Symbol`/`Str` are nullary; **`Any` unifies with everything, no binding** — it
  is absorbing, the gradual rule).
- `occurs`: recurse into `List`/`Pair`/`Fn`.
- `resolve_checked(&Ty) -> Result<Ty>`: resolve to any concrete checkable type
  (an unconstrained variable generalizes rather than erroring).
- `resolve_compileable(&Ty) -> Result<Ty>`: today's strict resolve (the codegen
  gate); errors on any non-compileable or unresolved type.
- `generalize(&Ty) -> Scheme` and `instantiate(&Scheme) -> Ty`.

## 5. Surface & integration

- A pure **checking pass** that never compiles: `(check-type form)` /
  `(typecheck 'name)` — runs inference, returns the inferred type (or scheme) as
  a printable type, or a type error. No binding change, no codegen.
- The elaborator (`Cx::elab`) learns the new literal/op shapes only as needed:
  `cons`/`car`/`cdr`/`list`/`null`/symbol-literals map to `Pair`/`List`/`Symbol`;
  everything it cannot place becomes `Any` (gradual) rather than an error.
- `infer_untyped` already separates "typed" from "installed native"; it gains the
  checked path so a function that *type-checks but isn't compileable* is reported
  as checked (and left dynamic), while a compileable one additionally goes native.
- Calls into untyped/operative bindings yield `Any` for their result and do not
  constrain their arguments (the frontier).

## 6. Staging

1. **Type language + engine** — add the checkable variants, `Any` (absorbing),
   structural `unify`/`occurs`, and `resolve_checked`. Unit-test in isolation
   (the #135 discipline). *(load-bearing; first slice)*
2. **Generalization** — `Scheme`, `generalize`/`instantiate`, let/defun
   polymorphism. The piece that makes `id`, `compose`, etc. check.
3. **Checking surface** — `(check-type …)` reporting inferred types / errors,
   without compiling. Gradual `Any` at the fexpr/vau/`eval`/create-on-assign
   boundary.
4. **Wire to codegen gate** — `infer_untyped` reports checked-typed vs
   native-compiled; `is_compileable` decides. Type errors surface at definition
   time even when no native edition is produced.
5. **Polish** — cond/if branch typing across non-numeric types, list/string
   builtins' signatures, optional blame on `Any` coercions (later).

Throughout: the checker is *additive* and *sound-on-the-island*; it never rejects
a program the dynamic interpreter would have run (a checker error is a genuine
type clash, and the operative frontier is always escapable to `Any`). Native
codegen remains gated on `is_compileable`, unchanged in behavior.
