# Execution tiers, the type gate, and typed regions

Status: design. Relates to #200 (closure/compile-to-IR tier), #133 (typed-JIT
TCO), #126 (clean-region certifier + effect summaries), #169 (shared abstractions
between typed values), #108 (Arc/Send/Sync), and the typed-JIT epic (#134). Builds
on `docs/typed-jit-design.md` and `docs/typed-checker-design.md`; read those first.

This document has two layers. The first (§1–§2) is a **theory**: Lamedh is
converging on a totally-ordered stack of execution tiers, and the HM type system
is the *gate* between the interpreted tier and the compiled tier — not merely a
checker, but the operational trigger that admits a form into fast code. The
second (§3 onward) is a **concrete instance** of that theory: `deftypedregion`, a
programmer-declared *compilation unit* whose members are promoted across the gate
as a group, unlocking the last optimizations standing between the typed island
and C-level speed. Read the region as one worked example of the tier model, not
as a standalone feature.

## 1. The theory: execution tiers and the type gate

### 1.1 Three tiers

For any given form, Lamedh has (or is converging on) three implementation tiers
with identical observable semantics and strictly increasing speed:

- **Tier 0 — naive tree-walking interpretation.** The original `eval`/`eval_step`
  path (`src/evaluator/*`). Every node is re-dispatched and re-resolved on every
  visit; values are boxed `LispVal` throughout; operators are resolved by walking
  the environment chain each call. This is the historical floor and the reference
  semantics.
- **Tier 1 — optimized but interpreted.** The compile→execute IR (the closure /
  `Code`-tree work, #200): each definition is lowered *once*, at definition time,
  into an IR walked by a unified trampoline, so per-node dispatch and symbol
  resolution are compiled away — but values are still boxed `LispVal` and there is
  still no native code. Crucially, this tier carries an always-correct fallback
  node (an `Interp`-style leaf) for any form not yet lowered, so Tier 1 is *total*:
  it can represent every program Tier 0 can, and is never slower than Tier 0.
- **Tier 2 — fast compiled land.** The HM-typed island (`defun-typed`,
  `src/jit/*`): unboxed, monomorphic, Cranelift-native (or the unboxed-closure
  fallback without the `jit` feature). No `LispVal` tag, no dispatch, no boxing —
  the machine works on raw `u64` words whose interpretation is fixed statically by
  the type (`src/jit/native.rs`, `runtime.rs`).

All three tiers are implemented today: Tier 0 (`src/evaluator/*`), Tier 1
(`src/evaluator/compile.rs`, the `Code` enum in `src/lib.rs`, `run_trampoline` in
`src/evaluator/functions.rs` — landed via #200's M1'/M2, merged to `main`), and
Tier 2 (`src/jit/*`). Everything below builds on a verified-present Tier 1, not a
speculative one.

### 1.2 The tier ladder and its gates

The tiers form a chain `Tier 0 ⊑ Tier 1 ⊑ Tier 2` ordered by speed, with an
explicit gate on each promotion:

```
  speed
    ^
    |   +----------------------------------------------------------+
 T2 |   | TIER 2  unboxed . monomorphic . Cranelift-native         |
    |   |         no tag, no dispatch, no box                      |
    |   +----------------------------------------------------------+
    |        ^  GATE up: HM elaboration              |  DROP down: hit an operative,
    |        |  succeeds AND the type lands          |  a non-compileable type, or
    |        |  in the compileable sublattice        |  an untyped/foreign boundary
    |        |  (int64/float64/bool/char/            v
    |        |   (array T)/struct)
    |   +----------------------------------------------------------+
 T1 |   | TIER 1  compiled Code IR . boxed LispVal . one trampoline|
    |   |         `Interp` leaf = unconditional correct fallback   |
    |   +----------------------------------------------------------+
    |        ^  subsumes (the Interp leaf can host any Tier-0 form)
    |   +  -  -  -  -  -  -  -  -  -  -  -  -  -  -  -  -  -  -  -  +
 T0 |     TIER 0  naive tree-walk (eval / eval_step) -- reference floor
    |   +  -  -  -  -  -  -  -  -  -  -  -  -  -  -  -  -  -  -  -  +
    +-------------------------------------------------------------------->
```

The Tier-1→Tier-2 gate is the load-bearing claim of this document:

> **The gate between Tier 1 and Tier 2 is the HM type system itself.** Passing a
> form through Hindley–Milner elaboration successfully *and* reducing it to the
> compileable sublattice (`is_compileable()`: `int64`/`float64`/`bool`/`char`/
> `(array T)`/struct — `src/jit/types.rs`) is not "type-checking" in the usual
> incidental sense. In this architecture, **type-checking success is the
> operational trigger that admits code into Tier 2.** Failure to type (an
> operative, a non-compileable type, an untyped call) is not an error — it simply
> leaves the form at Tier 1.

The type system does double duty: (a) a **soundness proof** — Wand's triviality
result means an operative admits no useful type, so the gate structurally excludes
`$vau`/fexpr/`eval`/`current-environment`/create-on-assign `setq` from Tier 2
(`docs/typed-jit-design.md` §0, `docs/typed-checker-design.md` §1); and (b) a
**staging discriminator** — it decides, per form, which tier that form may execute
in. `docs/typed-checker-design.md` already splits these two roles cleanly:
*checkability* (well-typed on the applicative island, gradual `Any` at the
operative frontier) versus *compileability* (`is_compileable`) — the second is
exactly the Tier-2 admission predicate.

### 1.3 The architectural invariant: nothing drops below Tier 1

Because Tier 1's `Interp` fallback can host any form Tier 0 can, and is never
slower than Tier 0, the whole design enjoys one clean guarantee worth stating as
an invariant:

> **Invariant (no regression).** Nothing in this design ever drops a form below
> Tier 1. The worst case for *any* Lisp form — a `deftypedregion` member that
> never type-checks, ordinary untyped Lisp, a form that hits an operative — is
> *optimized interpretation* (Tier 1), never *naive tree-walking* (Tier 0),
> because Tier 1 subsumes Tier 0 via the `Interp` leaf's unconditional
> correctness.

So opting into typing is strictly upside: success promotes you to Tier 2; failure
leaves you exactly where ordinary Lisp already sits (Tier 1), itself never worse
than the pre-#200 baseline. A region that fails to compile does not punish the
programmer with a slow path — it declines a *bonus*. This is the property that
lets typing be *opt-in and speculative* without risk, and it is the same
discipline the typed JIT already follows per function (the typed core is always a
correct fallback in every cell — `docs/typed-jit-design.md` §3).

### 1.4 The type system as a staging discriminator: multi-stage programming

The framing "a type decides not just *what value* but *at what stage/phase* a form
runs" is the central idea of **multi-stage programming**. Taha & Sheard's MetaML
(TCS 2000; Taha's thesis, "Multi-Stage Programming: Its Theory and Applications,"
1999) is a statically-typed language with explicit staging annotations
(brackets/escape/run) in which **type-checking is performed once, for all stages,
before the first stage executes**, and the type of an expression records which
stage it belongs to and what may cross between stages (cross-stage persistence /
cross-stage safety). That "type-checked once, ahead of execution, with staging
encoded in the type" is precisely the shape of Lamedh's gate: HM runs at
define-time and its verdict *is* the staging decision.

Be honest about how tight the analogy is. MetaML's stages are **generative**: a
value of type `<int>` is a *piece of code* that will produce an `int` when run —
the stages construct and splice program fragments. Lamedh's tiers are an
**operational refinement hierarchy**: the same value, the same semantics,
implemented faster. So the *principle* MetaML formalized — a type system as a
static, once-and-for-all discriminator of staging/phase — is the real, citable
grounding for "the type system is the gate between execution tiers." The specific
bracket/escape calculus and cross-stage-persistence machinery are *not* what
Lamedh implements, and claiming otherwise would overreach. What Lamedh does share
concretely: Tier-2 promotion is itself a staged compilation — HM elaboration +
Cranelift codegen run at an earlier stage (define time) and emit code that runs at
a later stage, compilation-as-a-stage gated by a type, which is the multi-stage
thesis in operational rather than generative form.

## 2. Precedents: the gate is an old idea

The tier/gate architecture is not novel in kind; it is the standard shape of every
system that lets code opt into speed while staying callable the same way. Naming
the precedents precisely also sharpens where Lamedh differs.

### 2.1 Lisp 1.5 `COMPILE` (EXPR → SUBR) — the original gate, on brand

Lamedh's namesake heritage contains the first instance. In the *LISP 1.5
Programmer's Manual* (McCarthy et al., MIT, 1962), a function definition lived on
the symbol's property list under `EXPR`/`FEXPR` (interpreted). `COMPILE` looked up
that S-expression, translated it through LISP Assembly Program (LAP) into machine
code, and replaced the `EXPR`/`FEXPR` with a `SUBR`/`FSUBR` (compiled). Two details
are the direct ancestors of this design: **not every function need be compiled** —
compiled and interpreted definitions coexist in one image; and **compiled
functions calling interpreted ones call the interpreter at run time**. That is
exactly Lamedh's picture — a per-definition gate (`EXPR`→`SUBR` ↔ Tier-1→Tier-2),
mixed tiers in one running image, and a membrane where compiled code re-enters the
interpreter (`docs/typed-jit-design.md` §2's `universal_call` trampoline). Lamedh's
contribution is *what the gate is*: in 1962 an explicit `COMPILE` call; here,
HM-typability.

### 2.2 Common Lisp `compile`/`declare`/`optimize` — and the honest SBCL contrast

Common Lisp standardized "types as a compiler hint that opens a fast path":
`(declare (optimize (speed 3) (safety 0)))` plus `(the fixnum x)` / declared
types. SBCL specifically uses declared *and inferred* types to emit unboxed
fixnum/float arithmetic, falling back to generic dispatch when it cannot prove a
representation. The precedent is real; the **difference is worth stating plainly
rather than glossing**:

> SBCL's *ungated* path is still **compiled-generic** — boxed, dynamically
> dispatched, but native machine code, never tree-walked. Lamedh's ungated path
> (Tier 1) is an **optimized interpreter**, not compiled-generic. So Lamedh's two
> tiers are interpreter-vs-native; SBCL's are native-fast-vs-native-generic.

This is not a defect to hide — it is the deliberate consequence of keeping the
Rust kernel small and being its own backend (`docs/typed-jit-design.md` §0, "the
inversion of Coalton"): Lamedh has no SBCL underneath to provide a
compiled-generic floor, so its floor is an optimized interpreter (Tier 1). The
upside is that Lamedh's fast tier is genuinely unboxed-native, matching SBCL's fast
tier in kind.

### 2.3 Typed Racket — the closest living relative of the membrane

Typed Racket (Tobin-Hochstadt & Felleisen, POPL 2008; later work through OOPSLA
2012) is the nearest analogue to "opt into typed code inside a Lisp-family
language, get checked/optimized code, with a guarded boundary to untyped code." Its
soundness story is **contracts at the boundary**: static types are compiled to
higher-order contracts attached at the *lexical boundary* between a typed module
and untyped Racket, so a value flowing across is dynamically checked to honor the
type it was promised. This is structurally Lamedh's **membrane** (`docs/typed-jit-design.md`
§2): the gradual-typing coercions Lamedh inserts at the typed/untyped edge (assert
`numberp` and extract the `i64` on the way in, re-box on the way out) *are* the
box/unbox marshalling of the native↔interpreter ABI, and they play the same role
Typed Racket's boundary contracts play — the enforcement point that lets sound
typed code coexist with unchecked dynamic code. Two honest differences: Typed
Racket is "macro" gradual typing (whole modules typed or not — Tobin-Hochstadt &
Felleisen — versus Siek & Taha's per-expression "micro" approach), matching
Lamedh's per-definition / per-region granularity; and Typed Racket's contract
overhead at boundaries is a known cost ("Is Sound Gradual Typing Dead?", POPL
2016), the same tax Lamedh's membrane coercions levy, and the reason to make the
*unit* big enough that boundary crossings are rare (§3).

### 2.4 RPython — the separate-dialect alternative Lamedh deliberately rejects

PyPy's RPython is a statically-analyzable *restricted subset* of Python (no
`eval`/`exec`, no runtime type-changing, whole-program type inference) used to
write the PyPy interpreter and translate it to C. "Stay inside RPython → C-like
speed; step outside → back to slow dynamic Python" is the tier-gate idea. But
RPython is a **whole separate restricted dialect** — the restriction applies to a
full program, not to single functions, and you cannot freely mix RPython and full
Python at a fine grain within one running computation. Lamedh takes the *opposite*
strategy: a **gradual, embedded overlay on the same language**, where any single
definition (or region) may cross the gate independently and coexist with untyped
Lisp in one image. RPython-style (separate restricted dialect) and
Typed-Racket/Lamedh-style (gradual embedded opt-in) are two routes to the same
goal; Lamedh chooses the gradual route for Lispness — the programmer never leaves
Lisp, they just annotate.

### 2.5 Julia — evidence that the ceiling is achievable, not hopeful

Julia is the strongest real-world evidence that a type system driving
specialization/unboxing, with a *graceful dynamic fallback*, reaches near-C in
practice. Julia specializes each method per concrete argument-type tuple; when a
function is **type-stable** (output type determined by input types), the JIT
unboxes, devirtualizes, and inlines, and Julia routinely benchmarks within ~1–2× of
C on numeric code. When a function is type-*unstable*, it falls back to boxed
dynamic dispatch and heap allocation — **slower, not incorrect** (Pelenitsyn et
al., "Type Stability in Julia," OOPSLA 2021). That is exactly Lamedh's tier-drop
discipline (type-stable ⇒ Tier 2; unstable ⇒ Tier 1, still correct), and the
empirical backing for the 2–3× ceiling in §9 — an architecture-family result, not a
Lamedh aspiration.

## 3. `deftypedregion`: a compilation unit

### 3.1 Why a unit — and why this granularity

The gate can be applied at three granularities, and the middle one is right:

- **Whole program** (MLton: the entire program is one compilation unit,
  monomorphized and optimized together — Weeks, "Whole-Program Compilation in
  MLton," 2006). Maximal optimization, but incompatible with a REPL: you cannot
  recompile the world on every definition.
- **Per function** (today's `defun-typed`). Fine-grained and REPL-friendly, but
  every cross-function call pays cell-indirection (`native.rs::emit_call`: load
  entry cell, branch, `call_indirect`), because any single function may be
  redefined independently at any time, so no callee address can be baked.
- **Per programmer-declared region** (`deftypedregion`). Big enough to internally
  inline / direct-call / stack-allocate, small enough to reason about and redefine
  atomically. This is MLton's whole-program idea shrunk to a *local,
  programmer-chosen* scope — the same term ("compilation unit") deliberately, and
  the natural granularity for a REPL-first Lisp.

Multiple compilation units coexist in one running image, each independently
frozen/typed/compiled, each with its own generation counter and atomic redefinition
story (§7). The region is the unit at which Tier-2 promotion becomes *collective*
rather than per-function, and that collectivity is exactly what §6's optimizations
need.

### 3.2 Surface syntax

```lisp
(deftypedregion mandelbrot
  (:export escape-count)          ; the public boundary — everything else is internal

  (declare-typed (escape-count int64) ((cr float64) (ci float64) (max int64)))

  (defun-typed (sq float64) ((x float64))          ; internal: not exported
    (* x x))

  (defun-typed (escape-count int64)
      ((cr float64) (ci float64) (max int64))
    (let-typed ((zr 0.0) (zi 0.0) (n 0))
      (iterate zr zi cr ci n max)))

  (defun-typed (iterate int64)                     ; internal, mutually recursive
      ((zr float64) (zi float64) (cr float64) (ci float64) (n int64) (max int64))
    (if (or (>= n max) (> (+ (sq zr) (sq zi)) 4.0))
        n
        (iterate (+ (- (sq zr) (sq zi)) cr)
                 (* 2.0 (* zr zi))
                 cr ci (+ n 1) max))))
```

- Every body is an ordinary `defun-typed`/`declare-typed` (or `defun*`), HM-checked
  and rejected before binding exactly as today. The region adds no typing rule.
- Members may be *authored* with macros, `$vau`, and fexprs; those are expanded
  away at the freeze step (§4) before typing runs.
- `:export` lists the **public** functions — they install membrane entries
  (`LispVal::Native`) callable from untyped Lisp. Non-exported names are
  **internal**: visible to region peers, not installed as global entries.
- `declare-typed` gives intra-region forward declaration for mutual recursion
  (reusing `Jit::declare`, `registry.rs`).
- The whole form compiles as a unit under one generation counter (§7).
- The degenerate region (one exported function, no helpers) must stay bit-for-bit
  equivalent to a standalone `defun-typed`.

### 3.3 The speed ceiling this targets

Measured baseline (`benchmarks/RESULTS.md`,
`RUN_MS=1000 WARMUP_MS=100 ./benchmarks/fibonacci/compare_local.sh 30`, warm native
typed `fib(30)`): Lamedh-JIT is **5.5× slower than C** (gcc -O3 -march=native),
1.6× slower than SBCL 2.2.9, ~10.6× faster than Ruby. Real and current. Between
Tier 2 today and C stand three distinct gaps, each against a *different* workload —
do not conflate them:

1. **Call overhead / partial devirtualization.** Every typed→typed call —
   *including self-recursion* — is emitted (`native.rs::emit_call`) as: marshal args
   through a stack-slot buffer, load the callee entry cell, branch on `is_native`,
   `call_indirect` (or fall to the trampoline). There is no truly-direct,
   baked-address call anywhere yet. Dominates call-bound tree recursion (`fib`).
   **Closed by** §6.1 (direct internal calls) + §6.2 (inlining).
2. **Stack growth on tail recursion.** #133 Tier 1 (self-tail-call → loop) is
   unimplemented. Dominates accumulator loops. `fib` is *not* tail-recursive, so
   #133 does nothing for the number above. **Closed by** #133 — orthogonal but
   composing with regions.
3. **Escape analysis — entirely missing.** Every array/struct allocates into the
   per-call `Ctx` arena (`runtime.rs::alloc_buffer`), living until the top-level
   membrane call returns; nothing is stack-allocated, because any typed function is
   reachable from anywhere. Dominates array/struct numeric code. **Closed by** §6.3
   (region-scoped Tofte–Talpin allocation).

The region attacks gaps 1 and 3 directly and composes with #133 for gap 2.

## 4. Freezing: making the Tier-1→Tier-2 promotion sound for macro-authored code

The Tier-2 gate *requires* the Wand boundary — no operative in code that HM-types.
Region authoring *wants* full Lisp — `defmacro`, `$vau`, fexprs for convenience.
These conflict only if operative machinery is still present when HM runs. **Phase
separation dissolves the conflict: run the operatives first, then type the
residue.**

### 4.1 The freeze pipeline

`deftypedregion` compiles in four phases:

1. **Author.** Members written with arbitrarily-expressive Lisp — full
   metaprogramming, unrestricted.
2. **Freeze (crystallize).** At a defined point after load, the region's forms are
   **recursively macro/operative-expanded to a fixpoint, once**. The result, by
   construction, contains no remaining macro/`$vau`/fexpr *calls* — every operative
   has run and been replaced by the applicative code it produced. This becomes the
   region's **source of truth**.
3. **Verify closedness on the residue.** Check the frozen form is now fully
   applicative (HM-typeable per `docs/typed-checker-design.md`). If any form *still*
   contains an irreducible operative (a residual `eval`, an unexpandable `$vau`, a
   `current-environment`, a create-on-assign `setq`), **reject**, naming the member
   and the construct that survived.
4. **Type and compile.** Hand the frozen applicative forms to the unchanged
   `defun-typed` pipeline (`Jit::define`) — HM, membrane, native codegen. Nothing
   downstream knows macros were ever involved.

### 4.2 Why the freeze removes the time dimension from the hazard

The precise hazard this closes is the one that sank the untyped tree-walker's
`body_is_opaque` scanner: a symbol that is an ordinary function (or unbound) *when
code is compiled* can **become a macro later** — after compilation, before first
call — and that macro's expansion can write into a frame the optimizer already
assumed safe. Every "is this currently a macro?" check is doomed because it
snapshots a *mutable binding at a point in time*, and the binding can change after
the snapshot.

The freeze removes the time dimension instead of snapshotting it correctly:

> Expansion happens **once**, at a defined freeze point. The compiled artifact
> depends on the **expanded code**, not on the live, mutable macro bindings that
> produced it. There is no remaining window in which a macro defined or redefined
> afterward can retroactively invalidate anything — after the freeze the region
> contains no macro references at all, so there is nothing left for a later macro
> definition to attach to.

This is strictly stronger than any "is it a macro right now" test: it does not need
to be correct *about time*, because it *eliminates the future dependency*.

### 4.3 `macrolet`/`vaulet` are a different, simpler case — do not conflate them

A **local, lexically-scoped** operator binding — Common-Lisp-style `MACROLET`, and
Lamedh's `VAULET`/`FEXPRLET`/`FLET` (defined in `lib/12-control.lisp` as `defmacro`s
expanding to `LET` over the kernel's anonymous `MACRO`/`VAU`/`FEXPR`/`LAMBDA`
constructors) — used *inside* a member body is **not** the global-macro hazard and
needs **none** of the freeze machinery:

- It is lexically scoped to one body (or sub-form); it never escapes and never
  persists as a mutable global registry entry.
- It is resolved entirely during *that body's own elaboration* — operator dispatch
  in this Lisp-1 resolves the head symbol through the ordinary lexical environment
  chain (`lib/12-control.lisp` header), so a name locally bound to a macro value is
  expanded at its call sites right there, during elaboration.
- It therefore *cannot* be "redefined later": there is no "later" for it — it lives
  and dies within one compilation. No fixpoint, no global dependency tracking, no
  generation invalidation.

State this plainly for future implementers: **the global-`defmacro` freeze (§4.1)
and local `macrolet`/`vaulet` resolution are two different mechanisms at two
different scopes.** The freeze is for global operative bindings reachable from a
region (mutable, redefinable, needing the once-and-fixpoint discipline); local
operator bindings are just ordinary lexical macro-expansion during elaboration of
the enclosing form, exactly as any Lisp compiler already handles `MACROLET`.
Applying the heavyweight global-freeze discipline to the lexical case (or vice
versa) would be a category error.

### 4.4 Precedent: Lisp-family phase separation

- **Racket / Scheme phase separation** (Flatt et al., "Macros that work together,"
  JFP 22(2), 2012): phases communicate only through the expansion protocol; the
  phase-1 expander's output is the phase-0 residual, which carries *no trace* of the
  macro machinery. A frozen region is exactly a phase-0 residual.
- **Common Lisp `compile-file`**: macros expand fully at compile time; the FASL
  contains no expansion machinery; redefining a macro does not retroactively change
  already-compiled call sites. The freeze is `compile-file`'s expand-then-compile
  discipline applied to a region.

This makes "Lispness preserved" *true* rather than aspirational: full macro power
during authoring, provably operative-free residue at the point HM runs.

### 4.5 The rejection is a sound failure mode, not a heuristic

"Reject if an operative survives full expansion" is Wand's boundary applied to the
*post-expansion* form — sounder and more permissive than scanning source:

- **Sounder**: the check runs on the exact code that will be typed, with no
  operative machinery left to reason about; closedness is decided by the same
  criterion the typed checker already uses.
- **More permissive**: much syntax that *looks* operative-shaped is a macro
  invocation expanding into ordinary applicative code; a pre-expansion scanner would
  reject it, the post-expansion check admits it. The programmer hears "no" only when
  a construct genuinely survives all expansion — a true, explainable boundary.

## 5. The closedness/soundness argument

### 5.1 What each frozen member already guarantees

A frozen member that HM-checks has, by construction (Wand; §4;
`docs/typed-jit-design.md` §0, `docs/typed-checker-design.md` §1):

- no `defexpr`/`$vau`/operative (expanded away, or rejected at freeze §4.5);
- no `eval`, no `current-environment`, no create-on-assign `setq`;
- every call resolved at elaboration to a concrete typed-function **id**
  (`Core::Call(id, …)`), never a runtime symbol lookup;
- every value a concrete monomorphic type in the compileable lattice
  (`types.rs::is_compileable`); no `Var` survives to a signature.

That is a stronger closedness statement than an untyped effect analysis could
*prove* — here it holds *by the elaborator's acceptance criterion on the frozen
residue*, not via a separate pass over mutable source.

### 5.2 What the group boundary adds

Exactly one new invariant, a scheduling fact rather than a semantic one:

> **Group atomicity.** A region is (re)compiled as a single unit. No member's
> compiled edition is ever installed while another member of the same generation is
> missing or stale.

This generalizes `docs/typed-jit-design.md` §3.4's single-function argument
("self-recursion may bake direct, since redefining replaces the whole body
atomically") from one function to a closed group. Redefining any member re-freezes
and recompiles the *whole* region under one new generation; an internal call site
can therefore bake a **direct address** to an internal callee, because the only
event that could invalidate it — redefinition — replaces the entire region
atomically, re-emitting every internal call site against the new addresses in the
same pass. Nothing internal goes stale relative to another internal member. The
exported boundary is what makes escape analysis sound: a value that does not flow
out through an exported return (or into caller-visible state) cannot outlive the
region's top-level call (§6.3).

### 5.3 Why this is safer ground than a reflection scanner

`body_is_opaque` was a syntactic pre-scan trying to prove "no reflection in this
body" for the untyped tree-walker; three subtle bugs, the last (§4.2's
symbol-became-a-macro) fundamentally unfixable. The region inherits none of that,
for two compounding reasons: the **freeze removes the time dimension** (§4.2), so no
later macro can reach into compiled code; and **there is nothing to scan for** —
HM-typability of the frozen residue *is* the closedness proof, and a member that
would reflect either expanded to something applicative or was rejected at freeze. A
later incompatible redefinition of a called function is caught by the existing
generation-bump + dependency-invalidation backstop (`docs/typed-jit-design.md`
§3.5). The lesson: **prove closedness by construction (freeze + types), never by
scanning code that can change underneath you** — the same reason
`docs/typed-checker-design.md` uses gradual `Any` for the operative frontier rather
than a heuristic.

## 6. The three unlocked optimizations

Backend caveat: the native tier lowers to **Cranelift** (chosen for REPL compile
speed, not LLVM throughput); the analyses are backend-independent, but the
realizable fraction of each win is bounded by Cranelift's instruction selection
(§9).

### 6.1 Direct, devirtualized internal calls

**Analysis.** Partition the region call graph into internal→internal edges (both
endpoints region members) and edges crossing the exported boundary.

**Mechanism.** For an internal→internal edge, emit a direct Cranelift `call` to the
callee within the same `JITModule` (one module per region → internal callees are
`Linkage::Local` `FuncRef`s), replacing today's entry-cell load + `is_native`
branch + `call_indirect` with a plain `call`: no cell load, no branch, no
indirection. Boundary-crossing edges keep call-through-cell (`docs/typed-jit-design.md`
§3.4 policy (a)), since foreign functions redefine independently and their
addresses can go stale. This is the concrete advance over today: the current
backend has *no* baked-direct call at all ("direct" in the prototype note means
`call_indirect` through the cell, versus the trampoline). §5.2's group atomicity is
what makes the baked address sound.

### 6.2 Cross-function inlining across the region

**Analysis.** With the region as one compilation unit and the internal call graph
in hand, inline internal callees under a size/growth budget — GHC bounds inlining by
unfolding size; OCaml Flambda inlines after closure conversion within its
optimization pipeline. Recursive members inline to bounded depth (peel one or two
levels), never unbounded; direct-recursive tail calls hand off to #133's
self-tail-call → loop lowering instead.

**Mechanism.** Pure compile-time transform on the region IR — no runtime mechanism.
Because members are monomorphic and unboxed, an inlined body needs no coercion at
the splice point (the same property that lets the membrane derive its ABI from the
HM type, `docs/typed-jit-design.md` §2): caller and callee already agree on unboxed
representations, so inlining is straight substitution, unlike inlining across a
gradual boundary. For `fib`, §6.1 + §6.2 move the number; #133 does not (`fib` is
not tail-recursive) — be honest about which lever moves which workload.

### 6.3 Region-scoped allocation (Tofte–Talpin)

The biggest missing piece versus C, and the region boundary is the first place
Lamedh can bound an allocation's lifetime.

**Precedent.** Tofte & Talpin region inference (POPL'94; *Information and
Computation* 1997) manages memory with a **stack of regions** and two constructs:
`e at ρ` (allocate `e`'s result in region `ρ`) and `letregion ρ in e` (create `ρ`,
evaluate `e`, deallocate `ρ`). Regions nest LIFO (a later region frees earlier) and
are always deallocated in the function that created them. The central theorem:
region inference is *safe* — when it says a region can be freed, no live value
points into it. Cyclone (Grossman et al., PLDI'02) exposed the same discipline with
explicit annotations and an outlives relation. The known failure mode is a **region
leak**: a value forced to outlive its scope is pushed to an outer/global region, so
a region can grow unbounded — safe, but not always tight.

**Mapping.** Today's `Ctx` arena *is* a single coarse region: one per membrane
call, freed only at the outermost return. A typed region refines that into a
**nested stack of regions** — one per region frame, and where escape analysis is
tight, per function activation — exactly Tofte–Talpin's `letregion` stack.

**Analysis.** Interprocedural escape analysis *within the region*: for each
`ArrayNew`/`StructNew`, does its pointer flow to (a) the return of an **exported**
member, or (b) a store into caller-visible state? If neither, it does not escape the
region and can live in a region-scoped arena freed at the region's top-level return.
If it further does not escape the *allocating function's* activation, it can be
**stack-allocated** in that function's Cranelift frame — a true `letregion`, freed
on return with zero bookkeeping. This is decidable by following typed dataflow
*because* the region is closed: freeze (§4) + HM exclude reflection and operative
aliasing, so there is no hidden path by which a pointer escapes unseen (contrast the
untyped tree-walker, where `eval`/`setq`/a macro could smuggle a reference out — the
exact hole that sank `body_is_opaque`).

**Mechanism.** A LIFO region stack replacing the flat `Vec<Box<[u64]>>` arena for
non-escaping allocations: push on region-frame entry, allocate into the top, pop
(bulk-free) on exit. Stack-promotable values become Cranelift `StackSlot`s. Escaping
values fall back to the per-call arena (the safe default), so the analysis only ever
*tightens* lifetimes and can never free something live — mirroring Tofte–Talpin
safety. Mis-inference in the conservative direction yields a region leak (arena
grows), never a use-after-free; that asymmetry is the safety margin.

## 7. REPL redefinition semantics for a region

Mirror `docs/typed-jit-design.md` §3, scaled from a function to a group. The
current implementation is single-threaded `RefCell`/`Rc` (not the doc's
aspirational `ArcSwap`/`Arc`, #108) — the group needs no atomics for the same reason
a single `TypedFn` does not: no concurrent access to race, and an in-flight call
pins (`Rc`-clones) the edition it runs.

- **One generation for the group.** The region owns a single `generation`;
  redefining any member re-freezes (§4) and recompiles the whole region and bumps it
  once. Internal direct calls and inlining are only valid against one consistent
  snapshot, so partial recompilation is forbidden.
- **All-or-nothing native tier.** Either every member compiles natively (with
  internal direct calls + inlining) or the region falls back to per-member
  closure/interpreter editions, each independently correct (`runtime.rs`),
  preserving the "typed core is always a correct fallback" invariant. This is the
  §1.3 no-regression invariant at region scope: the fallback is Tier 1, never Tier 0.
- **In-flight calls.** An executing edition holds its own `Rc` clone of the region
  module; old pages unmap only when the last in-flight caller returns
  (`NativeEdition` owns its `JITModule`, `Drop` frees the code). A redefinition swaps
  the region for *new* calls; old frames run to completion on valid pages. No
  on-stack replacement — the typed island is proven, not speculated; the only
  membrane crossings are ordinary call edges. Foreign typed functions the region
  calls (or that call an exported member) are invalidated via the existing per-cell
  generation check (`docs/typed-jit-design.md` §3.5); the region bakes no direct
  addresses across its exported boundary.
- **Redefining a macro a frozen region depended on.** This is the one place live
  redefinition is intentionally *not* transparent — by design, matching
  `compile-file`. The frozen residue no longer references the macro (§4.1), so
  redefining that macro *after* freeze has **no effect** on the compiled region; the
  programmer must **explicitly re-freeze/recompile** to pick up the new definition (a
  visible action, bumping the generation). State this plainly: it is not a
  regression from Lisp so much as the same "redefinition is an explicit recompile"
  model `defun-typed` already imposes, extended to cover a region's macro
  dependencies.

## 8. Relation to #169 and #126

### 8.1 #169 — region-scoped monomorphization toward generics

Nominal structs are strict-equality-typed today; no traits/subtyping/generics
(`docs/typed-jit-design.md` §4a). A region is a natural monomorphization unit in the
MLton sense — MLton eliminates polymorphism by *duplicating each polymorphic
function at every type it is instantiated at*, then optimizing a monomorphic IR
(Weeks 2006). A generic region

```lisp
(deftypedregion (stack T) (:export push pop) …)
```

would be instantiated per concrete `T` (`(stack int64)`, `(stack float64)`), each a
fresh fully-monomorphic compiled region. **Poor-man's polymorphism**: no dictionary
passing, no row polymorphism, no runtime type representation — exactly MLton's
strategy (and GHC's `SPECIALIZE`) scoped to a region rather than a whole program,
which suits an incremental/REPL compiler. It cannot express first-class polymorphism
or existentials, and code size grows with instantiation count (MLton's tradeoff). It
slots in *above* unification exactly where `docs/typed-jit-design.md` §4a says a
trait layer belongs — an explicit instantiation layer, not a weakening of nominal
identity — and composes with the freeze: a generic region is frozen once *per
instantiation*, each an operative-free monomorphic residue.

### 8.2 #126 — one certificate interface, two provers

#126 proposes a **clean-region certifier**: an abstract interpreter over ordinary
*untyped* Lisp computing a per-lambda effect summary (reads/writes dynamic vars?
reflects? calls `eval`? uses an operative?), so a lambda proven reflection-free gets
lexical addressing / slot frames in the **tree-walker** (a Tier-0→Tier-1 promotion
for untyped code). Structurally, a region's closedness claim and an effect system's
"no unhandled effects" claim are the same kind of proof: Koka's effect types
(Leijen, MSFP 2014) certify semantically (no `exn` effect ⇒ never throws unhandled);
Talpin–Jouvelot's type-and-effect discipline (LICS'92) tracks `init`/`read`/`write`
per region.

**Position: unify the certificate *interface*, not the *implementation*.** They are
legitimately different tools because their inputs differ:

- The region gets its certificate **by construction, for free**: freeze (§4) + HM
  already exclude reflection (Wand); asking it to run an effect analysis would
  reintroduce the after-the-fact scanning §5.3 warns against. Total and cheap, but
  only for typed (frozen) code.
- #126 must **discharge the proof the hard way**: untyped Lisp has no types and no
  freeze, so it needs a genuine abstract interpretation, with all the difficulty
  `body_is_opaque` demonstrated.

Forcing one mechanism would either cripple the region (re-prove what types give) or
over-claim for #126 (pretend an untyped analysis is as strong as a typing rule).
What they *should* share is the **output**: a common effect/closedness lattice
("pure-applicative", "reflection-free", "region-escaping") so one downstream
consumer — the optimizer deciding direct calls, inlining, stack promotion — treats
"frozen + HM-typed ⇒ pure-applicative" and "#126-certified ⇒ reflection-free" as two
*producers* of the same certificate feeding one *decision procedure*. Same
interface, two sound provers, chosen by whether the code carries types. That is the
honest unification: the certificate format, not the prover. Note the symmetry with
§1's tiers: #126 is the gate for a *lower* promotion, Tier 0 → Tier 1, proving the
reflection-freedom that lets the tree-walker use slot frames; the region is the gate
for the *higher* promotion, Tier 1 → Tier 2. Same shape of proof, two different
gates.

## 9. Revised ceiling estimate

The working estimate was "2–3× slower than C" for numeric/array-heavy typed code
once TCO + region inlining + region-scoped allocation exist. Against the precedents:

- **MLton** (whole-program, monomorphized, unboxed native ints/reals/arrays)
  typically lands within ~1.5–3× of C on numeric/array benchmarks.
- **GHC** with strictness/demand analysis + worker-wrapper unboxing (Peyton Jones &
  Launchbury, FPCA'91; Gill & Hutton, JFP 2009) reaches ~2–3× C on tight unboxed
  loops, occasionally matching.
- **Julia** type-stable numeric code benchmarks within ~1–2× C in practice (§2.5) —
  the strongest *empirical* backing, and the closest architectural cousin (JIT
  specialization + graceful dynamic fallback).
- **OCaml Flambda** cross-module inlining + `[@@unboxed]`/`[@@inline]` buys the
  interprocedural fraction of the gap.

So **2–3× C is well-calibrated for numeric/array-heavy typed code** — confirmed,
with two caveats to carry explicitly:

1. **Backend, not analysis, is the residual bound.** MLton/GHC/Julia ship strong
   backends (native / LLVM); Lamedh uses Cranelift for REPL compile speed, whose
   instruction selection is lighter than LLVM's, so even a perfect region pipeline
   sits at the *high* end of the band. An optional LLVM tier (`docs/typed-jit-design.md`
   §4) is the lever if the last factor matters.
2. **Pure-call microbenchmarks stay higher.** `fib(30)` at 5.5× C is call overhead
   with almost no work to amortize the residual. Direct calls (§6.1) + bounded
   inlining (§6.2) should bring it toward ~3–4× C, not 2×: no arithmetic body to hide
   the last dispatch/frame cost, and gcc -O3 partially inlines the recursion too. The
   2–3× figure is for code with real per-call *work* (loops, array kernels) where
   unboxing + stack allocation dominate — precisely the workload §6.3 targets.

Net: **2–3× C for numeric/array-heavy typed code, ~3–4× for pure-call-bound
microbenchmarks, backend-bounded at the high end of each** — a real improvement on
the current 5.5× that does not over-promise parity with an LLVM-backed whole-program
compiler.

## 10. Staging

Low-risk and independently valuable first, bigger lifts later — the discipline of
`docs/typed-jit-design.md` §6 and the checker doc's staged rollout.

1. **[low risk, independently useful] Surface + grouping, no new codegen.**
   `deftypedregion` parses to a set of `defun-typed`/`declare-typed`, installs
   exported members as today, threads one shared generation, emits identical code.
   Pure front-end + registry; proves the redefinition/atomicity model (§7) with zero
   backend risk (the analogue of the JIT doc's stage 2).
2. **[low risk, high leverage] Freeze/crystallize (§4).** Recursive macro/operative
   expansion to a fixpoint, once; the post-expansion reject-if-still-operative check
   (§4.5); hand the residue to unchanged `Jit::define`. Reuses the existing expander
   and typed pipeline; new code is the driver plus the residual-operative check. The
   soundness keystone (§4.2) — land it early, before region-specific codegen, since it
   also makes stages 3–5 provably safe. Keep the local `macrolet`/`vaulet` path (§4.3)
   firmly separate.
3. **[low risk] One Cranelift module per region.** Compile frozen members into a
   single `JITModule`; internal callees become `Linkage::Local` `FuncRef`s. Keep
   call-through-cell for now (correctness unchanged) — the members now share a module,
   the precondition for §6.1/§6.2.
4. **[medium] Direct internal calls (§6.1).** Replace internal→internal `emit_call`
   sequences with plain `call`. Differential-test every region call against the
   interpreter (`agree` harness) and against the pre-region cell edition. Retires most
   of gap 1.
5. **[medium] Bounded cross-region inlining (§6.2).** Size/growth-budgeted inlining;
   hand tail-recursive members to #133. Compose with #133, don't duplicate it.
6. **[bigger lift] Region-scoped allocation (§6.3).** Interprocedural escape analysis
   + LIFO region stack + Cranelift `StackSlot` promotion. The largest piece (a real
   analysis with a correctness obligation) and the biggest payoff for array/struct
   code; ship last, behind the conservative fallback so a mis-inference is a leak,
   never a use-after-free.
7. **[research] Region monomorphization (§8.1, #169).** Generic regions instantiated
   and frozen per concrete type set. Depends only on stages 1–2; can prototype in
   parallel, but an open design question, not a scheduled deliverable.

Throughout: the typed core stays the reference model (#134); every member's
interpreter/closure edition remains a correct fallback in every cell (the §1.3
invariant — worst case Tier 1, never Tier 0); the region tier is all-or-nothing
native (§7); and nothing in `src/` depends on the backend unless the `jit` feature
is on.

## References

- J. McCarthy et al. *LISP 1.5 Programmer's Manual.* MIT Press, 1962. (`COMPILE`:
  `EXPR`/`FEXPR` → `SUBR`/`FSUBR`; interpreter links with compiled code; not all
  functions need be compiled.)
- W. Taha and T. Sheard. *MetaML and multi-stage programming with explicit
  annotations.* Theoretical Computer Science 248(1–2), 2000. See also W. Taha,
  *Multi-Stage Programming: Its Theory and Applications*, PhD thesis, OGI, 1999.
- S. Tobin-Hochstadt and M. Felleisen. *The Design and Implementation of Typed
  Scheme.* POPL 2008. (Typed Racket: static types compiled to higher-order contracts
  at the typed/untyped boundary.) See also *Is Sound Gradual Typing Dead?*, POPL 2016
  (boundary-contract cost).
- The PyPy team. *RPython* — a statically-analyzable restricted subset of Python,
  whole-program type-inferred and translated to C (`rpython.readthedocs.io`).
- J. Bezanson, S. Karpinski, V. Shah, A. Edelman. *Julia: A Fresh Approach to
  Numerical Computing.* SIAM Review 59(1), 2017. A. Pelenitsyn et al., *Type
  Stability in Julia: Avoiding Performance Pathologies in JIT Compilation.* OOPSLA
  2021.
- M. Tofte and J.-P. Talpin. *Implementation of the Typed Call-by-Value λ-calculus
  using a Stack of Regions.* POPL 1994. *Region-Based Memory Management.* Information
  and Computation 132(2), 1997.
- D. Grossman et al. *Region-Based Memory Management in Cyclone.* PLDI 2002.
- S. Weeks. *Whole-Program Compilation in MLton.* ML Workshop, 2006. See also
  `mlton.org/WholeProgramOptimization`.
- S. Peyton Jones and J. Launchbury. *Unboxed Values as First Class Citizens in a
  Non-Strict Functional Language.* FPCA 1991. A. Gill and G. Hutton. *The
  worker/wrapper transformation.* JFP 19(2), 2009. S. Peyton Jones and S. Marlow.
  *Secrets of the Glasgow Haskell Compiler inliner.* JFP 12(4–5), 2002.
- *Optimisation with Flambda*, OCaml manual (cross-module inlining via `.cmx`,
  `[@@inline]`, `[@@unboxed]`).
- M. Flatt, R. Culpepper, D. Darais, R. B. Findler. *Macros that work together.* JFP
  22(2), 2012. (Racket phase separation: phase-1 expansion → phase-0 residual.)
- D. Leijen. *Koka: Programming with Row-Polymorphic Effect Types.* MSFP 2014
  (arXiv:1406.2061). J.-P. Talpin and P. Jouvelot. *The Type and Effect Discipline.*
  LICS 1992; Information and Computation 111(2), 1994.
- M. Wand. *The Theory of Fexprs is Trivial.* Lisp and Symbolic Computation 10(3),
  1998. (The operative layer admits no useful type — the gate's closedness proof by
  construction.)
