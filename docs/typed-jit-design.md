# Typed JIT: a pre-runtime type membrane that drives native codegen

Status: design / spike. Relates to #31 (x86-64 JIT), #68 (epic), #111 (tree-walker
constant factor), #108 (Arc/Send/Sync), #114 (RPLACA aliasing), #62 (TCO).

This document reframes the JIT in #31 around one idea: **the type checker runs
before runtime, in the Lisp/vau layer, and the types it proves are exactly what
let the backend emit unboxed native code.** It also pins down the part the REPL
makes hard — a `deffun-typed` must be callable as compiled code the instant it is
defined, may be *re*compiled later, and the old edition may be thrown away while a
call to it is still on the stack. So every call is a runtime dispatch.

## 0. The inversion of Coalton

Coalton types a static island inside Common Lisp and then hands it to **SBCL**,
which does the unboxing via type declarations. Lamedh inverts this: it types the
island in its own (vau/macro) layer *pre-runtime*, and then has to **be its own
optimizing backend**, because there is no SBCL underneath. The type work is the
easy half; the codegen is the lift, and it lives entirely in #31.

The type layer itself is the Turnstile-style elaborating membrane described in the
design thread: `deffun-typed`/`let-typed` run Algorithm W at macro-expansion time,
reject ill-typed definitions, and emit a typed core. We cannot type the fexpr/vau
layer itself (Wand's triviality result), so anything touching `defexpr`, `eval`,
`current-environment`, or create-on-assign `setq` is *outside* the typed island by
construction. That boundary is not an annoyance — it is the JIT's deopt boundary
(§3).

## 1. Why types are the speed, not a nicety

#31 wants to emit, for `(* y y 2)`:

```asm
mov rax, [rdi]      ; load y
imul rax, rax       ; y * y
imul rax, 2
```

That is only valid if `y` is an **unboxed i64**. In lamedh `y` is a `LispVal` —
a tagged enum (`Number(i64)`, `Float(f64)`, `Cons{car:Rc,cdr:Rc}`,
`Array(Rc<RefCell<…>>)`, …). With no type proof, the honest codegen for `(* y y)`
is *not* `imul`; it is:

> match the tag → branch int vs float → `checked_mul` → re-wrap into a
> `LispVal::Number` → (for some result types) an `Rc` allocation,

preceded by a walk up the `Environment` parent chain to resolve `y`. That is the
constant factor #111 is trying to shave, and a JIT without types just bakes the
same boxed dispatch into machine code.

HM types license the fast path. In order of payoff:

1. **Unboxing.** `int64 → i64` register, `float64 → xmm`. The `imul` becomes
   legal; no tag, no re-box, no `Rc`. This is most of the win and it retires #111
   and #114 (unboxed scalars are not `Rc`-shared, so the aliasing hazard is gone).
2. **Devirtualization.** HM proves `(foo x)` targets a known function of known
   arity and unboxed convention, so we emit a direct call instead of routing every
   call through a runtime `match` on a `LispVal`.
3. **Dead-checking.** Monomorphic typed code statically kills the `numberp`
   guards, the int/float `cond`, and the symbol-table lookups — proven
   unreachable, not merely shrunk.

Ceiling: a typed unboxed numeric loop can run 50–100× a tree-walker, because it
removes interpretation overhead *and* boxing. The existing fast `FOR`/`WHILE`
(one reused frame, in-place counter mutation) is the interpreter hand-approximating
this for one special form.

## 2. The type *is* the ABI — the membrane is the JIT boundary

The gradual-typing membrane and #31's `universal_call` trampoline are the same
object. Gradual typing already inserts coercions at the typed/untyped edge:
re-establish a tag on the way in (`assert numberp`, extract the `i64`), wrap on the
way out (raw `i64 → LispVal::Number`). **Those coercions are exactly the box/unbox
marshalling a native↔interpreter ABI needs.** One mechanism, not two.

This dissolves #31's worst hand-waving. Its `execute_interpreted_from_compiled`
has a literal `FIXME: Need real parameter names` doing `format!("arg_{}", i)`, plus
a `sync_compiled_cache` dance. With the typed boundary all of that is **derived
from the function's HM type**: the type gives the arity, the register/stack layout,
and the per-edge coercions. The only values that cross are the ones the type says
cross, in the representation the type dictates. No name guessing, no cache to keep
coherent.

Wand's result even tells you *where* the boundary must be: any point where an
operative could observe operand syntax or the environment provably cannot be inside
typed native code, so it is necessarily a membrane crossing / deopt.

## 3. The hard part: REPL redefinition makes every call a dispatch

A `deffun-typed` must be runnable as compiled code the moment it is defined; it may
be recompiled later (better optimization, or a redefinition); and a previous
compiled edition may need to be discarded **while a call into it is still on the
stack**. The discipline that makes this safe is an **indirection cell** per
function — classic Lisp `fdefn`, SBCL-style — plus "code is reclaimed, never
eagerly freed."

### 3.1 The function cell

Symbols do not bind code directly; they bind a stable cell. Callers always go
through the cell, so swapping an edition is one atomic store.

```text
struct FnType { params: Vec<Ty>, ret: Ty }          // the HM signature = the ABI

struct CompiledCode {                                // an executable edition
    entry:      NativeEntry,                          // fn(*const LispVal, usize) -> LispVal  (boxed ABI)
    fast_entry: Option<UnboxedEntry>,                 // island-internal unboxed convention
    pages:      ExecutableMemory,                     // mmap'd; Drop = munmap
    ty:         FnType,
    generation: u64,
}

struct FunctionCell {
    name:       Symbol,
    ty:         FnType,                               // current signature
    interp:     TypedCore,                            // source of truth; ALWAYS runnable
    compiled:   ArcSwap<Option<Arc<CompiledCode>>>,   // lock-free hot-swap
    generation: AtomicU64,                            // bumped on every (re)definition
}
```

`ArcSwap` (or `arc-swap` crate / a hand-rolled epoch scheme) is the key: a reader
loads the current `Arc<CompiledCode>` and holds it for the duration of the call; a
writer `store`s a new edition; the old `Arc` is dropped only when the last in-flight
caller finishes, at which point `ExecutableMemory::drop` munmaps the pages. That is
"code is GC'd, not freed" and it is what makes *throwing away the previous edition
while it is on the stack* safe. This is also the concrete reason to move the cell
(and the code pages) from `Rc` to `Arc` — i.e. #108.

### 3.2 A call is: load cell → decide interpret/compiled

```text
fn dispatch(cell: &FunctionCell, args: &[LispVal]) -> LispVal {
    let cur = cell.compiled.load();                  // cheap atomic load
    match &*cur {
        Some(code) => (code.entry)(args.as_ptr(), args.len()),   // native, boxed ABI at the edge
        None        => interp_typed_core(&cell.interp, args),    // fallback while compiling / when deopted
    }
}
```

Two editions of the *same* function never run from one cell at once for *new*
calls; but an *old* edition already executing keeps its own `Arc` and runs to
completion against valid pages. No on-stack replacement is required, because we do
**not** speculate on the typed island — its types are proven, not guessed. The only
thing that can deopt is a membrane crossing (§2), and that is a normal call edge,
not a mid-frame patch.

### 3.3 Define-time and recompile-time behavior

- **On `deffun-typed`:** run HM (reject if ill-typed), install the cell with
  `interp` set and `compiled = None`, then kick compilation. Because the types are
  proven we compile **eagerly/AOT** (no call-count warmup like #31's thresholds).
  Until the edition is ready the cell serves the interpreter, so the function is
  callable *immediately* and gets faster *transparently* when the store lands. If
  Cranelift is fast enough we can even compile synchronously for small bodies.
- **On redefinition:** bump `generation`, replace `interp`, `compiled.store(None)`
  (or store the new edition when ready). In-flight old-edition frames are
  unaffected; their `Arc` keeps their pages alive until they return.

### 3.4 Inter-function calls and stale direct calls

If compiled `A` bakes a direct `call B_native`, redefining `B` leaves `A` pointing
at a dead edition. Two policies:

- **(a) call-through-cell (default).** `A` emits `call [B.cell.entry]` — one
  indirect load per cross-function call. Always correct; redefinition is just the
  `ArcSwap` store. Costs the last few percent of call speed.
- **(b) direct + backpatch.** Bake the address and keep a per-cell list of call
  sites to rewrite on redefinition. Faster steady-state, much more machinery.

For a REPL-first Lisp, default to **(a)**; correctness and trivial invalidation beat
a few percent. Self-recursion *within* one body may bake direct, since redefining
replaces the whole body atomically anyway.

### 3.5 Signature changes invalidate typed callers

If `(deffun-typed (foo str) …)` is redefined with a different signature, callers
that were type-checked and compiled against the *old* `FnType` are now potentially
ill-typed. Sound REPL policy: a `generation` bump on `foo` marks dependent compiled
editions **stale**; on their next call the cell sees the mismatch and falls back to
`interp` (which re-checks against the current type at the membrane) and schedules
recompile. A precise version keeps a reverse dependency edge (callers-of-`foo`) and
re-runs HM on them; the lazy generation check is the cheap correct floor. Either
way, the contract at the membrane is the backstop: a value shaped for the old type
fails the coercion rather than corrupting.

## 4. Backend: lower to Cranelift, do not hand-emit x86

#31 sketches hand-assembled opcode bytes (`0x48, 0x8B, …`) with manual relocations
— a long, fragile, x86-only road, and an entire "register allocation" phase to
write. Once HM hands us typed IR with `i64`/`f64`, lower to **Cranelift**: we write
typed IR, it does instruction selection and register allocation and gives us
AArch64/portability for free. HM is what makes the lowering trivial — Cranelift IR
is typed, so we translate `int64 → i64`, never inventing types at the metal. LLVM
is the heavier alternative if we later want max throughput; Cranelift is the right
first backend (fast compiles suit a REPL).

## 4a. Prototype status (implemented in `src/jit.rs`)

Stage 1 is built and tested (`src/jit.rs`, `src/jit/tests.rs`, `examples/typed_jit.rs`):

- The membrane (`Jit::define`) elaborates `deffun-typed` with a monomorphic
  bidirectional checker and rejects ill-typed defs *before runtime*.
- `int64`/`float64`/`bool`; `+ - * / mod`, `< > <= >= = /=`, `and`/`or`/`not`,
  `if`, `let-typed`, and **calls** — self-recursion, cross-function, and (via
  `Jit::declare`) mutual recursion. Runtime values are unboxed `u64` words; the
  static type says how each op reads its word, so there is no tag in the hot path.
- The cell dispatch and redefinition model from §3 are real: calls route through
  the registry by id, redefining a callee is an edition swap, and a call pins
  (`Rc`-clones) the edition it runs.
- 49 tests; the `agree` helper runs every example through both editions as a
  differential check.

The backend is closures, not native code yet, so it ties the JIT's *own* unboxed
interpreter. The measured win is against lamedh's **boxed** evaluator: the example
shows ~16× on `fib(28)`. The decisive win over an unboxed tree-walk is what native
codegen (§4) buys next.

**It lands in the language.** `DEFFUN-TYPED` is a real special form: the registry
lives in `SharedState` (shared across the whole environment chain), and a
successful definition installs a `LispVal::Native` entry under the function name.
So a typed function is callable from ordinary untyped Lisp through the membrane —
`(+ (fib 15) 1)` works at the REPL, an ill-typed definition is rejected before it
binds anything, and redefinition updates behavior live. Typed→typed calls (self,
cross-function) route through the registry by id, not back through the evaluator.
The one boundary not yet exposed in surface syntax is a *forward declaration* (for
REPL-level mutual recursion); it exists in the Rust API as `Jit::declare`.

## 5. The spike

Smallest thing that proves the whole thesis end to end. See
`docs/spike/typed_jit_spike.rs` (illustrative, not wired into the crate).

1. Pick one monomorphic typed function:
   `(deffun-typed (sq int64) ((x int64)) (* x x))`.
2. `infer`: confirm `sq : int64 -> int64`, emit typed core.
3. Lower the typed core to Cranelift IR (`iadd`/`imul` on `i64`), JIT to an entry.
4. Install a `FunctionCell` with `interp` = the AST and `compiled` = the JIT
   edition; route the symbol's binding through `dispatch`.
5. One contract trampoline as the membrane: boxed `LispVal::Number ↔ i64` at the
   entry, so the interpreter and any untyped caller can still call `sq`.
6. Redefinition test: redefine `sq`, assert the old `Arc<CompiledCode>` is dropped
   only after in-flight calls return, and that new calls hit the new edition.

Success = identical result to the tree-walker, ~50× on a tight `(sq …)` loop, and a
clean redefinition with no use-after-free. Then widen: `+ - < if let-typed`, then
direct typed→typed calls (policy (a)), then `float64`, then self-recursive loops
(tie into #62 TCO so a typed tail self-call becomes a backedge, not a frame).

## 6. Staging

1. **Type membrane** (`lib`-level `deffun-typed`/`let-typed`, Algorithm W) — no
   backend yet; rejects ill-typed defs, emits typed core. Independently useful.
2. **FunctionCell + dispatch** in the interpreter (still interpreting) — proves the
   redefinition/identity model with zero codegen risk. Pure refactor of how the
   evaluator resolves a call to a typed function.
3. **Cranelift island** for monomorphic int functions (the spike) behind a
   `jit` cargo feature, default off, so `cargo build`/`clippy` stay green.
4. **Widen** types and call forms; AArch64 falls out of Cranelift.
5. **Membrane polish:** contract coercions as the universal trampoline; deopt at
   every fexpr/`eval` edge.

Throughout: the typed core stays the reference model (the #68 mandate), the
interpreter is always a correct fallback in every cell, and nothing in `src/`
depends on the backend unless the `jit` feature is on.
