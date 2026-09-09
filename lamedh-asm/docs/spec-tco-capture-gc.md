# Spec: transitive closure capture, proper tail calls, and a deferred reference-counting collector for the data heap

Status: design only — no code in this document is meant to be pasted. Every
routine named `like_this` either already exists at the file/line given, or is a
proposed new routine whose name follows the existing `fold_binop_ast` /
`emit_check_callable` / `compile_*` conventions in `src/compiler.asm`.

Everything below was written against commit `2fbfe94` after reading
`README.md`, `src/tags.inc`, `src/heap.asm`, `src/boot.asm`, the whole of
`src/compiler.asm`'s special-form/call/lambda machinery, `src/native_errors.asm`,
`src/arrays.asm`, `src/symtab.asm`, `src/file_runner.asm`, `tests/run.sh`, and
`tests/cases/009_closures.asm` / `015_32args.asm` — and after running a handful
of probe programs through `build/lamedhc` to check the README's own claims
against the binary. Where a probe contradicted the README, the probe wins and
the discrepancy is called out.

## 0. Summary, order, and dependencies

| # | Feature | Real current state (measured) | Recommended landing order |
|---|---------|-------------------------------|---------------------------|
| 1 | Transitive free-variable capture | **Already correct** for every shape tried (2-, 3-, 4-deep nests; shadowing; `LET` in the middle; `&REST` middle; stack-passed outer params; macro-produced inner lambdas). What is broken is the *cost and fragility* of how it gets there: a macro call at lambda-nesting depth D is expanded **2D+1 times** at compile time, and the free-variable scan is shadowing-blind. | First or second |
| 2 | Proper tail calls | Not implemented. A 2-parameter tail-recursive loop segfaults between 200 000 and 300 000 iterations (8 MiB default stack, 32-byte frames ⇒ ~262 000 frames). Mutual recursion and 5-argument loops die the same way. | First or second |
| 3 | Data-heap GC | None. Two bump allocators; the 64 KB `PRINC-TO-STRING` capture buffer alone is a known per-call leak. | **Last** |

Features 1 and 2 touch disjoint parts of `compile_lambda`/`compile_call`
(1: the free-variable list that feeds the prologue and the closure-construction
copy loop; 2: the call-site emitter and a tail flag threaded through the
special forms) and can be built and landed in either order. Feature 3 must land
last: it adds bookkeeping at every site that writes a tagged pointer into a
heap object, so anything that adds new such sites — feature 1's cached capture
lists are compile-time data and are fine, but the separately-tracked
`&OPTIONAL`/`&KEY` work will add prologue code and possibly new runtime
allocations (a keyword→value list built with `cons`, or a new object kind) —
would otherwise have to be retrofitted. Section 3.7 lists exactly what a new
heap-object kind or new store site owes the collector, so that later work can
be reviewed against it.

---

## 1. Transitive free-variable capture through nested lambdas

### 1.1 Problem statement — what is actually wrong

The README ("v0 limits", and the Roadmap bullet "General (not single-level)
free-variable propagation") says a nested `LAMBDA` can only capture from its
*immediately* enclosing lambda. That is **not what the binary does**. Probes run
through `build/lamedhc` at `2fbfe94`, all printing the correct value:

```lisp
(DEFINE F (LAMBDA (A) (LAMBDA (B) (LAMBDA (C) (+ A (+ B C))))))
(PRINT (((F 1) 2) 3))                                   ; 6   — A reaches C through B
(DEFINE G (LAMBDA (A) (LET ((L 10)) (LAMBDA (B) (LAMBDA (C) (+ L (+ A (+ B C))))))))
(PRINT (((G 1) 2) 3))                                   ; 16  — LET-bound L too
(DEFINE H (LAMBDA (A) (LAMBDA (B) (LAMBDA (A) (+ A B)))))
(PRINT (((H 1) 2) 100))                                 ; 102 — inner A shadows correctly
(DEFINE M (LAMBDA (A) (LAMBDA (B) (LAMBDA (C) (LAMBDA (D) (+ A (+ B (+ C D))))))))
(PRINT ((((M 1) 2) 3) 4))                               ; 10  — four levels
(DEFUN OUTER4 (P0 P1 P2 P3 P4) (LAMBDA (B) (LAMBDA (C) (+ P4 (+ P3 (+ B C))))))
(PRINT (((OUTER4 0 0 0 10 20) 2) 3))                    ; 35  — stack-passed outer params
(DEFUN OUTER3 (A) (LAMBDA (&REST BS) (LAMBDA (C) (+ A (+ (CAR BS) C)))))
(PRINT (((OUTER3 1) 2 9) 3))                            ; 6   — &REST in the middle
(DEFMACRO MK (X) `(LAMBDA (Q) (+ ,X Q)))
(DEFUN OUTER6 (A) (LAMBDA (B) (MK (+ A B))))
(PRINT (((OUTER6 1) 2) 3))                              ; 6   — macro-produced inner lambda
```

Why it works: `scan_free_vars` (`compiler.asm:833`) never stops at a nested
`LAMBDA` — its `.list_case` treats `LAMBDA` as an ordinary head and walks the
parameter list and body like any other subforms. So when `compile_lambda`
scans B's body against A's `current_scope`, it sees every symbol C mentions,
B captures it into its own free frame, and B's `new_scope`
(`param_frame ++ rest-entry? ++ free_frame`, built at `compiler.asm:2297-2313`)
therefore contains it when C is compiled against `current_scope = scope_B`.
Transitivity falls out of "scan the whole subtree, resolve against the
enclosing scope". The README text at lines 815–818 and 2003–2004 is stale and
should be corrected as part of landing this feature (commit 1 below).

What *is* defective, and measurably so:

**D1 — every macro at nesting depth D is expanded 2D+1 times at compile time.**
Measured with a counting transformer
`(DEFMACRO M (X) (SETQ CNT (+ CNT 1)) X)` placed inside a 1/2/3/4-deep lambda
nest: `CNT` = 3, 5, 7, 9. Cause: `compile_lambda` calls `scan_free_vars` twice
per lambda — once at `:2253` to size the frame, and again at `:2645` ("re-derive
free_syms (same deterministic result)") to drive the closure-construction copy
loop — and each scan descends into every nested lambda, re-running
`invoke_macro` on every macro call it meets (`:906-917`). Then `compile_form`
expands it once more to actually compile it. The "same deterministic result"
comment is only true if every transformer is pure. The reference stdlib's own
`defun` (`../lib/00-core.lisp`) is not: it `putp`s/`remprop`s the name's
plist, pushes onto `$cg-pending`, and calls `gensym`. Today this shows up only
as counter drift, but it is the same hazard class as the macro-capture bug the
README already describes: the expansion that was *scanned* is not the
expansion that is *compiled*, and the scan's answer is only correct as long as
the two agree on their free variables.

**D2 — the scan is shadowing-blind, so closures over-capture.**
`(LAMBDA (X) (LAMBDA (X) X))` gives the inner closure an `nfree=1` slot
holding the outer `X`, copied at every closure creation and never read. Any
name bound *inside* the nested lambda (its params, its `LET`s, its `PROG`
vars, `HANDLER-CASE` clause vars, `PROG` labels, `&REST`) that happens to
also be bound in the enclosing scope is captured needlessly. Harmless to
results — `build_param_frame` entries precede `free_frame` entries in
`new_scope`, and a `LET` frame is prepended, so the inner binding always wins
`frame_lookup` — but it costs 8 bytes plus a load/store per closure creation
per name, and it makes every closure's captured array (which GC in §3 must
walk) larger than necessary.

**D3 — quadratic compile time.** A lambda nested D deep has its body walked
2D times by scans before it is compiled once. The stdlib conformance corpus
(`tests/run.sh`'s 40 files) is where this shows; it is not yet measured.

### 1.2 Design

One bottom-up analysis, run once per outermost `LAMBDA`, whose results are
cached per lambda form and consumed by `compile_lambda` in place of both
`scan_free_vars` calls; plus a memo for macro expansion so that scan and
compile see the same expansion object.

**New host-side data structures (`compiler.asm`, `.bss`):**

- `macroexpand_memo` — an open-addressing hash table keyed by the *address* of
  a macro call form's cons cell (forms are heap-allocated, immutable, and never
  relocate — the same fact `HASH-CODE` already relies on), value = the
  expansion (a tagged form). Fixed capacity (e.g. 65 536 entries × 16 bytes,
  1 MiB), cleared whenever `compile_thunk` is entered at nesting depth 0
  (a new `compile_nesting_depth` counter, incremented/decremented in
  `compile_thunk`, distinguishes a top-level form from an `EVAL`-during-
  macro-expansion nested one — the same nesting `compile_thunk` already
  saves/restores `current_scope` for). Overflow policy: if full, don't memoize
  (fall back to expanding again) — correctness does not depend on the memo,
  only D1's counts do.
- `capture_memo` — same shape, keyed by a `LAMBDA` form's cons address,
  value = its capture list (a proper list of symbols, in the order free-slot
  indices will be assigned).

**New routine `macroexpand_once(rdi=call form) -> rax=expansion`:** looks up
`macroexpand_memo`; on miss calls `invoke_macro` exactly as
`compile_form`'s macro dispatch and `scan_free_vars:906` do today, stores,
returns. Both of those call sites switch to it. This alone fixes D1 to
"expanded once" regardless of what else lands.

**New routine `analyze_lambda_captures(rdi=lambda form, rsi=enclosing scope)`**
— replaces the two `scan_free_vars` calls in `compile_lambda`. Semantics:

```
fv(form, bound) -> ordered set of symbols referenced in `form` that are not in `bound`
   symbol s          : {s} if s ∉ bound, else {}
   (QUOTE _)         : {}
   (LAMBDA ps . body), ($VAU ps . body), synthetic (LAMBDA () ...) :
                       inner = fv(body, bound ∪ names(ps))         ; names(ps) strips &REST
                       memo capture_memo[form] := inner ∩ RESOLVABLE   (see below)
                       result = inner
   (LET ((n init)...) . body)  : ⋃ fv(init_i, bound)  ∪  fv(body, bound ∪ {n_i})
   (LET* ((n init)...) . body) : fv(init_1, bound) ∪ fv(init_2, bound∪{n_1}) ∪ ... ∪ fv(body, bound∪{n_i})
   (PROG (v...) item...)       : items that are bare symbols are labels — skipped;
                                 other items: fv(item, bound ∪ {v...})
   (HANDLER-CASE p (h (v) . hb)): fv(p, bound) ∪ fv(hb, bound ∪ {v})
   (FUNCTION x)                : fv(x, bound)
   (m args...) where m is a global bound as a macro : fv(macroexpand_once(form), bound)
   (op args...) where op is a global bound to an HDR_OPERATIVE at compile time :
                                 see open question Q1
   (h args...) otherwise       : fv(h, bound) ∪ ⋃ fv(arg_i, bound)
   anything else (fixnum, string, immediate) : {}
```

`RESOLVABLE` at a given lambda = "resolves by `frame_lookup` in the scope that
will be `current_scope` when that lambda is compiled". For the outermost
lambda that is the `rsi` passed in (today's `[current_scope]`). For a lambda
nested inside it, the enclosing lambda's future scope is
`params(enclosing) ∪ capture_list(enclosing) ∪ (any LET/LET*/PROG/HANDLER-CASE
names lexically between the two)` — which is exactly `bound` at the point the
nested lambda is visited, *plus* the enclosing lambda's own capture list. The
one subtlety: the enclosing lambda's capture list is not final until its whole
body has been walked, but a nested lambda's own free set is a subset of what
the enclosing body contributes, so the rule reduces to: **capture_list(inner)
= fv(inner body, params(inner)) minus names bound between inner and outer
(already handled by `bound`) — and every remaining name is either bound by
some enclosing lambda (then that enclosing lambda captures it too, by the same
recursion) or is a global (dropped by the `∩ RESOLVABLE` test at the outermost
level only)**. Implementation-wise the simplest correct form is a two-phase
pass: phase 1 computes `fv` bottom-up with `bound` sets exactly as above and
records per-lambda *raw* free sets; phase 2 walks top-down carrying the set of
names actually bound by enclosing lambdas + `current_scope`, intersecting each
raw set with it to produce the final capture list. Phase 2 is what turns
"global `PRINT` mentioned in the body" into "not captured".

Ordering: capture lists must be deterministic and identical between the
prologue (free-slot index assignment, `build_frame_from_list` at `:2291`) and
the copy loop (`:2651`). Reading the memoized list at both points guarantees
that by construction — today's guarantee rests on the two scans agreeing.

**Changes to `compile_lambda` (`:2216`):** replace `:2253-2257` with
`capture_memo` lookup; on miss (a lambda form the analysis never saw — the
synthetic forms `compile_unwind_protect:4232`, `compile_let`'s
`.dynamic_rewrite`, `compile_defmacro`, `compile_defexpr` build with `cons`
at compile time) call `analyze_lambda_captures(form, [current_scope])` right
there and proceed. Replace `:2645-2648` with the same lookup. Nothing else in
the prologue changes; `nregslots_scratch`, `lambda_frame_depth_scratch`,
`rest_sym_scratch`, and the `current_scope`/`current_frame_depth`/
`current_prog_ctx` save/restore are untouched.

**Changes to `scan_free_vars`:** keep it, unexported from the hot path, as the
debug oracle (§1.4). Its macro branch switches to `macroexpand_once`.

**Register/ABI implications:** none at runtime. Closure layout
(`HDR_CLOSURE`: `[8]=code [16]=nargs [24]=nfree [32..]`) is unchanged; only
`nfree` gets smaller for shadowed names.

### 1.3 Open questions for the implementer

- **Q1 — operative call operands.** Today's scanner descends into the raw
  operands of a `$VAU` operative call and captures any symbol that resolves,
  even though those operands are baked as a `(QUOTE ...)` literal and can only
  ever be `EVAL`ed in the global environment (README, "`$VAU`"). The
  principled v0 answer is "opaque, capture nothing" (matches the documented
  global-only `EVAL`); the conservative answer is "keep descending" (matches
  today, over-captures). Recommend opaque, but confirm no stdlib file relies on
  the over-capture (run `stdlib_conformance`).
- **Q2 — `DEFMACRO`/`DEFINE` forms nested inside a lambda body.** Rare in the
  corpus; treat `(DEFMACRO n ps . body)` like `(LAMBDA ps . body)` for `fv`
  purposes and `(DEFINE n v)` as `fv(v)`. Verify against `01-list.lisp`'s
  `defun`-inside-`let` idioms, if any.
- **Q3 — memo table key stability across `READ-FROM-STRING`/`EVAL` at runtime.**
  A form read at runtime and `EVAL`ed goes through `compile_thunk` at nesting
  depth 0 → memo cleared → fine. Confirm `compile_nesting_depth` is also
  decremented on the `native_throw` unwind path (a macro transformer that
  signals an error mid-expansion longjmps past `compile_thunk`'s epilogue).
  Recommend resetting the counter to 0 in `run_buffer` and every
  `tests/cases` driver's loop rather than trusting balanced decrements.

### 1.4 Risk areas

1. **Under-capture** (the analysis treats a name as bound when it isn't) is
   the dangerous direction: the reference silently compiles to a *global*
   load — the exact failure shape of the earlier macro-capture bug (garbage
   or `IMM_UNBOUND` read as a value, printing `3`). The `LET` vs `LET*` init
   rule is the classic mistake: `(LET ((X 1) (Y X)) ...)` inside a nested
   lambda — `Y`'s init refers to the *outer* `X` and must be captured;
   `(LET* ((X 1) (Y X)))` must not.
2. **Order drift** between prologue and copy loop if anyone ever calls
   `analyze_lambda_captures` twice with a different `bound`. Reading the memo
   at both points removes the possibility; keep it that way.
3. **Memo key collisions**: keying by cons address is only valid because forms
   never move and, at compile time, are never freed. §3 pins every object
   allocated while `rc_pin_depth > 0`, which covers this; but the memo must be
   cleared per top-level form so a freed-then-reused address (possible once
   §3 lands, for runtime-read forms) cannot alias.
4. **Behavior change observable through side-effecting macros:** transformers
   now run once instead of 2D+1 times. `GENSYM` numbering shifts. No
   `.expected` file currently prints a gensym produced inside a nested
   lambda, but check `045_gensym.expected` and the `stdlib_conformance`
   expected block before and after.

Oracle: in a `-DCAPTURE_CHECK` assembly (nasm `%ifdef`), after computing the
new list, also run the old `scan_free_vars` and assert `new ⊆ old` and that
every symbol in `old \ new` is bound somewhere inside the lambda (i.e. the
only thing the new pass removes is shadowed over-capture). Trap (`int3`) on
violation. Run the full suite once under it.

### 1.5 Testing

- `tests/cases/066_transitive_capture.asm` — kernel-only (no prelude: no
  `DEFUN`/`LIST`/`FUNCALL`), the first four probes above plus a shadowed
  `LET`-in-the-middle case, each printed. Passes today; pins the behavior.
- A new debug primitive `(CLOSURE-NFREE f)` — `compile_unary_hostcall` over a
  four-instruction `closure_nfree_tagged` reading `[raw+24]`. With it:
  `(CLOSURE-NFREE (LAMBDA (X) (LAMBDA (X) X)))` is `0` after this lands
  (inner closure, reached by calling the outer once) and `1` today. Put it in
  the same case file.
- **Expansion-count regression** in the same file: the `CNT` transformer above
  inside a 3-deep nest must leave `CNT = 1` (today: 7). This is the single
  most precise test of D1 and costs nothing.
- `tests/run.sh` `stdlib_conformance` must stay byte-identical; time it before
  and after (`time build/lamedhc build/stdlib_conformance.lisp`) and record
  the numbers in the commit message — this is the D3 measurement.

### 1.6 Incremental landing plan

1. README correction + `066_transitive_capture.asm` (probes only; no compiler
   change). One commit; makes the real baseline explicit.
2. `macroexpand_once` + `macroexpand_memo` + `compile_nesting_depth`; switch
   `compile_form`'s macro dispatch and `scan_free_vars:906` to it. Add the
   `CNT` test — it drops from 7 to 1 at this step already, with zero change to
   what gets captured. Full suite + conformance.
3. `analyze_lambda_captures` phase 1+2, `capture_memo`, consumed by
   `compile_lambda` at both sites with the miss fallback. Add `CLOSURE-NFREE`
   and the shadowing assertion. Run once under `CAPTURE_CHECK`.
4. Q1 (operative operands opaque), if conformance agrees.

---

## 2. Proper tail calls (frame reuse)

### 2.1 Problem statement

```lisp
(DEFINE COUNTDOWN (LAMBDA (N ACC) (IF (= N 0) ACC (COUNTDOWN (- N 1) (+ ACC 1)))))
(PRINT (COUNTDOWN 1000000 0))      ; today: exit 139 (SIGSEGV); N=200000 prints 200000
(DEFINE EVENP2 (LAMBDA (N) (IF (= N 0) T (ODDP2 (- N 1)))))
(DEFINE ODDP2  (LAMBDA (N) (IF (= N 0) NIL (EVENP2 (- N 1)))))
(PRINT (EVENP2 1000000))           ; today: exit 139
(DEFINE LOOP5 (LAMBDA (N A B C D) (IF (= N 0) (+ A (+ B (+ C D))) (LOOP5 (- N 1) B C D A))))
(PRINT (LOOP5 1000000 1 2 3 4))    ; today: exit 139 (two stack-passed args per frame)
```

Every call from compiled code is `call` (`emit_call32` for a named global
through the inline-cache trampoline, `emit_call_reg` for the indirect path —
`compile_call`, `compiler.asm:3346`), so each tail-recursive step costs one
native frame: return address + saved `rbp` + `8*(nregslots+nfree+LET slots)`.
`COUNTDOWN`'s frame is 32 bytes; the OS stack is 8 MiB (`ulimit -s 8192`
during `tests/run.sh`); 8 MiB / 32 B ≈ 262 000 frames, matching the observed
200k-pass / 300k-crash boundary. `_start` (`boot.asm`) runs on the plain
process stack — there is no `with_large_stack`-style trampoline as in the
Rust reference — and the README's stated goal is constant native stack for
Lisp-level tail loops (`FOR`/`DOTIMES` already rely on `WHILE`, but every
`defun` in the reference stdlib written as a tail-recursive walk does not).

### 2.2 What "tail position" means in this compiler's actual special-form set

A call site is in tail position iff its value is returned as the enclosing
*compiled function's* own return value with nothing executed afterward in
that function — and, specifically for this kernel, with **no catch-stack
frame installed by the enclosing function still live** at the moment of the
jump (the frame's saved `rbp`/`rsp` would point into the discarded frame).

Tail-transparent forms (a subform of these is tail iff the form itself is):

| Form | Emitter | Tail subform(s) | Explicitly **not** tail |
|------|---------|-----------------|-------------------------|
| `LAMBDA` body (also `$VAU`, `DEFEXPR`, `DEFMACRO` transformer bodies, the synthetic `(LAMBDA () cleanup...)`) | `compile_lambda:2566` → `compile_progn` | last body form | — |
| `PROGN` | `compile_progn:1352` | last form | earlier forms |
| `IF` | `compile_if:2042` | then-form, else-form (incl. the implicit `NIL` when `cadddr` is missing — a literal, moot) | test |
| `COND` | `compile_cond:1388` | last form of a clause body | every test; a body-less clause `(test)` — its value is inspected by the `cmp rax,NIL`/`je` so the test is a non-tail position |
| `AND` / `OR` | `compile_and:1470`, `compile_or:1531` | last operand | all earlier operands (compared after evaluation) |
| `LET` lexical fast path | `compile_let:1637` | last body form | every init |
| `LET*` | `compile_let_star:1939` | last body form | every init |
| macro call | `compile_form` macro dispatch | the expansion, compiled in place, inherits the caller's tail-ness — this is what makes `WHEN`/`UNLESS`/prelude `DEFUN` bodies/reference `cond`-style macros tail-capable | — |
| operative call `(op ...)` | rewritten to an ordinary `compile_call` with two baked `QUOTE` operands | the call itself is a tail call if the form is | — |

Never tail, and the reason each is a *correctness* exclusion, not a missed
optimization:

- `CATCH` body (`compile_catch:3696`), `HANDLER-CASE` protected form and
  handler body (`compile_handler_case`), `ERRORSET`, `BLOCK` body
  (`compile_block`), `UNWIND-PROTECT` body: a catch-stack frame recording
  *this* function's `rbp`/`rsp` is live and is popped after the body
  (`emit_pop_catch_frame`); a `THROW` from the callee would restore a
  discarded frame.
- `LET` with any `DEFDYNAMIC` binding: rewritten to `UNWIND-PROTECT`
  (`compile_let:1791`) — the restore must run after the body. Falls out of the
  previous rule automatically because the rewrite is compiled through
  `compile_form` and `UNWIND-PROTECT` never re-arms the flag.
- `WHILE` body (`compile_while:4815`; the loop continues), `PROG` items,
  `RETURN`'s value (jumps to the `PROG` exit), `GO`.
- `SETQ`/`DEFINE`/`DEFDYNAMIC`/`DEF` value forms (stored afterward), `THROW`
  tag/value, every argument position of every call, the operator position,
  every operand of `compile_binop`/`compile_*_hostcall` (pushed/popped around),
  `FUNCTION`'s operand (a name, by intent), `RECORD-NEW` fields.
- `APPLY` (hence prelude `FUNCALL`): a `compile_binary_hostcall` into
  `invoke_macro`, a host routine with its own frame — `(APPLY f args)` in
  tail position is **not** optimized in v0. Document it; the reference
  stdlib's `DEFPROTOCOL` dispatch goes through `apply`, so a protocol-
  dispatched tail-recursive loop still grows the stack. Making `invoke_macro`
  re-enterable as a tail jump is a possible follow-up, not part of this spec.
- The top-level thunk (`compile_thunk:7012`): the host invokes it with
  `push rax; call qword [rsp]; add rsp, 8` and every driver assumes it
  returns normally. Keep `compile_thunk` at tail=0 always.

### 2.3 Design

**Threading the flag.** Follow the codebase's existing pattern for
compile-time context (`current_scope`, `current_frame_depth`,
`current_prog_ctx`): one new compiler-global cell, `tail_ctx: resq 1`, with a
*consume-on-entry* discipline so that forgetting to re-arm it is always the
safe failure:

- `compile_form` (`:4846`) reads `[tail_ctx]` into a callee-saved register
  (it already pushes `r14`; use a fifth push or reuse `r14` after the atom
  check) **and immediately stores 0** — every subform it goes on to compile
  starts non-tail. It passes the snapshot in `rsi` to the tail-transparent
  helpers only: `compile_progn(rdi, rsi=tail)`, `compile_if`, `compile_cond`,
  `compile_and`, `compile_or`, `compile_let`, `compile_let_star`, and
  `compile_call(rdi=op, rsi=args, rdx=tail)`. Every other dispatch target is
  unchanged and never sees the flag.
- Those helpers, for each subform they classify as tail, set `[tail_ctx]`
  from their saved copy immediately before `call compile_form` (a two-line
  `compile_form_tail` = `mov [tail_ctx], rsi; jmp compile_form` keeps the
  sites uniform). `compile_progn` re-arms only for the last form;
  `compile_cond` only for the last form of a body that exists;
  `compile_let`/`compile_let_star` only for the body's last form, never an
  init (they call `compile_progn(body, tail)` — `compile_progn` does the
  rest).
- `compile_lambda:2567` calls `compile_progn(body, rsi=1)`. `compile_thunk`,
  `compile_while`, `compile_prog`, `compile_handler_case`, `compile_block`,
  `compile_errorset`, `compile_catch`, `compile_unwind_protect` pass `0`
  wherever they call `compile_progn`/`compile_form` (audit: every
  `call compile_progn` — there are ~9 — and every direct `call compile_if`
  etc. if any exist outside `compile_form`; today all special-form helpers
  are reached only through `compile_form`, so the audit is short).
- Defensive layer, because a stale `rsi` at an unaudited call site would be
  a silent stack-corruption bug: (a) each helper treats the flag as tail only
  if `rsi == 1` exactly, and in a `-DTAIL_CHECK` build traps on `rsi > 1`;
  (b) `compile_call` additionally requires `[current_lambda_depth] > 0` — a
  new counter incremented/decremented around `compile_lambda`'s body compile,
  saved/restored like `current_prog_ctx` — so no tail jump is ever emitted
  into a top-level thunk regardless of what the flag says.

**The tail call site (`compile_call`, two new sub-paths).** Preconditions
checked at compile time: `tail == 1`, `[current_lambda_depth] > 0`, and (v0)
`nargs <= 3`. Otherwise fall through to today's code unchanged.

Named-global (inline-cache) tail path — `.named_tail`:

1. `compile_call_args` (unchanged, right-to-left, args on the target stack).
2. Pop into `rsi`/`rdx`/`rcx` as today (`:3382-3396`).
3. `rax = nargs` (`emit_mov_reg_imm64`), as today.
4. **`emit_leave`** — target `mov rsp, rbp; pop rbp`. Every `LET`/`LET*`
   `sub rsp` reservation in this function is discarded by this; that is why
   `LET` bodies can be tail positions without emitting their `add rsp`
   first. `leave` does not touch `rax`/`rsi`/`rdx`/`rcx`.
5. `emit_jmp32` → the rel32 field address; patch it to the trampoline exactly
   as `:3493-3496` patches the `call`.
6. **No post-call `add rsp` cleanup** is emitted for this path (nothing
   returns here). With `nargs <= 3` there is nothing to clean anyway.

The trampoline needs one change, and it is the subtle one. Today's
trampoline (`:3409-3486`) discovers the call site it must patch by reading
the **return address** the `call` pushed: `emit_load_rsp_disp8(rax, 8)` then
`sub rax, 4` = the rel32 field. Entered by `jmp` after `leave`, `[rsp]` is
not this site's return address — it is the current function's *own* return
address into its caller H. `patch_rel32(that − 4, target)` would then rewrite
the 4 bytes preceding H's return point, which for a `call rel32` in H is
precisely H's own rel32 field: H's call to F would be silently redirected to
G, and the corruption would surface only the *next* time H called F, as a
wrong answer, nowhere near the tail call that caused it. Therefore the tail
trampoline bakes its patch address:

- Emit the trampoline with `mov rax, imm64` (placeholder 0) in place of the
  `[rsp+8]` load — `codegen_here + 2` is the imm64 operand, the same fixed
  fact `compile_catch:3682` and `emit_install_catch_frame:3884` already use.
- After step 5 above, `patch_imm64(placeholder, jmp_rel32_field_addr)` and
  `patch_rel32(jmp_rel32_field_addr, trampoline_entry)`.
- Everything else in the trampoline is identical (push nargs, resolve the
  symbol's cell, `emit_check_callable`, save/restore `rsi`/`rdi`, call
  `patch_rel32`, restore `rax`, `jmp rbx`). Its own pushes/pops balance, so
  on `jmp rbx` the target stack is `[return-address-of-F's-caller]` — exactly
  the state a `call`ed callee expects.

Factor the shared body into `emit_ic_trampoline(rdi=cell_addr, rsi=mode)` so
the two variants differ only in how `field_addr` is loaded; today's inline
copy at `:3409-3486` becomes `mode=0`.

Indirect tail path — `.indirect_tail`: as today through `:3571` (args,
operator, `emit_check_callable`, `rdi = closure`, pops, `rbx = code_ptr`,
`rax = nargs`), then `emit_leave`, then `emit_jmp_reg(REG_RBX)` instead of
`emit_call_reg`. `emit_check_callable` runs before `leave`; its failure path
calls `fail_wrong_type` → `native_throw`, which longjmps and never returns,
so frame state at that point is irrelevant.

**Why the callee's prologue is unaffected.** At a `call`ed entry `rsp` points
at the return address and `rsp ≡ 8 (mod 16)`. After `leave` in F, `rsp`
points at F's own return address — the same slot, same alignment, since F's
own entry had `rsp` there. The callee's `push rbp; mov rbp, rsp; sub rsp, N`
therefore produces `[rbp+8] = return address into F's caller` and
`[rbp+16..] = F's caller's stack-passed arguments to F`. Argument registers
`rsi`/`rdx`/`rcx` and the count in `rax` are set before `leave` and survive
it. The closure self-pointer `rdi` (indirect path) or the resolved closure
(trampoline restores it into `rdi`) is likewise in place. Nothing in
`compile_lambda`'s prologue reads anything the jump changed. `floats.asm`'s
16-byte alignment requirement holds for the same reason the alignment is
unchanged.

**v0 restriction `nargs <= 3`, and stage 2.** Stack-passed arguments are the
one thing that does not survive `leave`: `compile_call_args` leaves arg3+ on
the target stack *below* the current frame, and `leave` abandons them. The
only place a frame-reusing call can put them is in the current function's
own incoming-argument area `[rbp+16 ..]`, which exists only if this function
itself was called with that many stack arguments. Stage 2 rule: allow a tail
call with `nargs > 3` iff `nargs − 3 <= own_nstackargs`, where
`own_nstackargs = max(0, nfixed_current − 3)` for a function without `&REST`
(a `&REST` function's incoming stack area is a runtime quantity — exclude
it in stage 2, or compare against the stashed `nargs` at `[rbp-32]` at
runtime as a stage 3). Track `nfixed_current` in a new compiler-global
`current_lambda_nfixed` saved/restored around `compile_lambda`'s body like
`current_frame_depth`. Mechanics: after popping the three register args, the
remaining `nargs−3` values sit at `[rsp+0]`, `[rsp+8]`, … (arg3 first — the
order `compile_call_args` and `build_param_frame` already agree on); copy
`[rsp+8i]` → `[rbp+16+8i]` for `i < nargs−3` (`emit_load_rsp_disp8` +
`emit_store_local` with a positive disp — `emit_store_local` takes a disp32,
sign is irrelevant, the same fact `build_param_frame:594` relies on), then
`add rsp, 8*(nargs−3)` is unnecessary because `leave` follows. Source and
destination never overlap (one is below `rbp`, the other above). If
`nargs − 3 < own_nstackargs` the leftover slots hold F's dead incoming args,
which the callee never reads (it reads `nfixed_callee − 3` of them, and a
`&REST` callee reads exactly `nargs − 4 … 0`, which we set). `LOOP5` above
(5 params, 5-arg self tail call) is the stage-2 acceptance test.

**Interaction with the catch stack and unwinding.** A tail jump never
happens with a frame installed *by the jumping function* (§2.2). Frames
installed by callers hold *their* `rbp`/`rsp`; a `THROW` from the tail callee
restores those and skips F's frame, which is already gone — identical to what
would have happened had F returned normally first. `UNWIND-PROTECT` marker
frames (cleanup closure in `frame[8]`) are likewise only ever installed by a
function whose body is then non-tail. The `catch_stack_top` bookkeeping is
untouched by any of this.

**Interaction with §3.** The safe-point check §3 places at function entry
runs on tail entry exactly as on call entry; the conservative stack scan sees
one fewer frame, which is the point.

### 2.4 Risk areas

1. **Classifying a non-tail position as tail** silently corrupts the stack in
   ways that surface later (the `add rsp` cleanup that never runs, a catch
   frame pointing into freed stack). Most likely mistakes: `COND`'s body-less
   clause, `LET` inits, `AND`/`OR` non-last operands, `FUNCTION`'s operand,
   any special form added later that calls `compile_progn` without passing
   `rsi=0`. The consume-on-entry rule plus the `current_lambda_depth` guard
   bound the blast radius; the audit list in §2.3 is the checklist.
2. **The trampoline patch address** (described above) — silent, delayed,
   wrong-answer corruption in an unrelated function. The regression test in
   §2.5 is built specifically for it.
3. **`rax` clobbered between "set nargs" and the jump.** `leave` and
   `jmp` do not touch `rax`, but any emitter inserted between them that uses
   `rax` as scratch (`emit_cmp_rax_imm64` clobbers `rcx` too — an arg
   register!) would. Keep the sequence pop/pop/pop → (stage-2 copies) →
   `rax = nargs` → `leave` → `jmp`, with nothing in between.
4. **Stage 2 overwriting live incoming arguments.** Safe only because every
   argument has been fully evaluated before the first copy; an argument
   expression that reads a stack-passed parameter of F *after* the copies
   would read garbage — impossible with the sequence above, but true only
   because `compile_call_args` completes before any copy is emitted.
5. **`&REST` caller in stage 2** (excluded) and a `&REST` callee reached by
   a tail call with `nargs <= 3`: its stack-walk loop bound is
   `max(nfixed,3) − 3 = 0` and its start index `nargs − 4 < 0`, so the loop
   body never runs — same as a `call`ed entry. Verify with a test rather than
   by argument.
6. **Diagnostics get worse**: an `int3` trap deep in a tail-looping program
   has no chain of frames to inspect. Acceptable and expected; note in
   README.

### 2.5 Testing

`tests/cases/067_tail_calls.asm` (kernel-only; `DEFINE`/`LAMBDA`/`IF`/`COND`/
`AND`/`OR`/`LET`/`LET*`/`PROGN` are all special forms):

- `COUNTDOWN 1000000 0` → `1000000`. Fails today with exit 139; passes after
  commit 3 below. `tests/run.sh` should set `ulimit -s 8192` explicitly at the
  top so the depth is deterministic across machines (today it inherits
  whatever the shell had).
- Mutual recursion `EVENP2`/`ODDP2` at 1 000 000 through two named-global IC
  sites → `T`.
- Indirect tail call: `(DEFINE SELF (LAMBDA (F N) (IF (= N 0) 0 (F F (- N 1)))))`,
  `(SELF SELF 1000000)` → `0` (exercises `.indirect_tail`).
- One 1 000 000-deep loop for each tail-transparent form: last form of
  `PROGN`, both `IF` arms, a `COND` clause body, last operand of `AND` and of
  `OR`, `LET` and `LET*` bodies (the `LET` case is the one that proves the
  `sub rsp`/`leave` interaction).
- **Trampoline-patch regression**: `H` calls `F` twice and prints both
  results; `F` tail-calls `G` on its first invocation only (`IF` on a global
  flag) and returns a distinct value on its second. Today's trampoline
  entered by `jmp` would redirect H's second `(F)` to `G`. Expected output
  must show `F`'s own second value.
- **Correctness (not depth) of every non-tail exclusion**: a call inside
  `CATCH`, `HANDLER-CASE`, `UNWIND-PROTECT` (cleanup must still fire, and
  after the callee returns), `BLOCK`, a `LET` with a `DEFDYNAMIC` binding
  (restore must happen after the call), `WHILE` body, `COND` body-less
  clause, `AND` non-last operand — small depths, exact values.
- Stage 2: `LOOP5 1000000 1 2 3 4` → `10`; a 5-arg tail call *from* a
  2-param function must fall back to `call` and still return the right value;
  a `&REST` callee reached by a 2-arg tail call.
- `tests/run.sh`'s `file_runner_prelude` and `stdlib_conformance` outputs
  unchanged — `FOR`/`DOTIMES`/`REDUCE`/`MAPCAR` and the reference's own
  recursive list functions all exercise the new paths through macros.

### 2.6 Incremental landing plan

1. Add `tail_ctx` + `current_lambda_depth`, the consume-on-entry rule in
   `compile_form`, the `rsi` parameter on the eight helpers, and the audit of
   every `compile_progn`/helper call site — **with `compile_call` ignoring the
   flag**. Behaviorally a no-op; full suite green proves the plumbing didn't
   perturb anything.
2. Factor `emit_ic_trampoline(mode)` out of `compile_call`'s named path,
   `mode=0` only. No behavior change; suite green.
3. `.indirect_tail` (no trampoline involved) for `nargs <= 3`. Add
   `067_tail_calls.asm` with the `SELF` case and the exclusion-correctness
   cases. This is the first commit where a test flips from crashing to
   passing.
4. `.named_tail` + `mode=1` trampoline with the baked patch address. Add
   `COUNTDOWN`, `EVENP2`/`ODDP2`, the per-form loops, and the
   trampoline-patch regression.
5. Stage 2 (`current_lambda_nfixed`, stack-arg copy-up) + `LOOP5`.
6. README: remove the v0-limit bullet, document the `APPLY`/`FUNCALL` and
   `&REST`-caller exclusions.

---

## 3. A deferred reference-counting collector for the data heap

Scope: the data heap only (`heap.asm`, `data_alloc`). The RWX code heap and
everything in it (compiled functions, trampolines, patched call sites) is
never reclaimed and is out of scope. Symbols are never reclaimed either
(they are interned, address-baked into code, and hold the global value cells).

### 3.1 Problem statement

```lisp
(DEFINE SPIN (LAMBDA (N) (IF (= N 0) 0 (PROGN (CONS N NIL) (SPIN (- N 1))))))
(SPIN 1000000)          ; today: data_heap_cur advances by exactly 16 MiB and never retreats
```

Every `CONS`, closure creation, `F+`, `STRING-APPEND`, `PRINC-TO-STRING`
(64 KB per call, `print.asm:91`), `FD-READ`/`PORT-READ-*` scratch buffer,
record and condition is a permanent bump allocation. The README documents that
the 16 MiB arena was exhausted by loading the stdlib and was raised to 256 MiB;
`data_alloc` still has no bounds check, so exhaustion "corrupts subsequent
`data_alloc` calls into unmapped memory rather than erroring". A long-running
program that allocates in a loop has a hard, silent ceiling.

### 3.2 Where the count lives — and why not in the object

Constraints from `tags.inc`: a cons is a bare 16-byte `[car|cdr]` cell with
**no header word** (the tag is in the pointer); every heapobj's word 0 is a
full 64-bit `HDR_*` value; all objects are 16-byte aligned; addresses never
change and are baked as immediates into code, used as `HASH-CODE`, and as
`EQ` identity. Options:

- *Count in the header word's upper bits* — impossible for cons cells.
- *Widen cons to 32 bytes* — doubles list memory, and touches `cons`/`car`/
  `cdr` (`reader.asm`), the printer, `record_fields_tagged`, and the
  "16-byte cell" statement in `tags.inc` and the README. Rejected.
- **Side table indexed by granule (chosen).** The data heap is one contiguous
  `mmap` with 16-byte allocation granularity (`data_alloc` rounds to 16), so
  `granule = (raw_addr − data_heap_base) >> 4` is a dense index. One 8-byte
  entry per granule: `u32 count | u8 flags | u24 ngranules` (flags:
  `HEAD`, `PINNED`, `IN_ZCT`, `MARK`, `RAW`). Only an object's first granule
  has a meaningful entry. Size: 256 MiB / 16 × 8 = 128 MiB of *virtual*
  space, `mmap`ed in `heap_init_all` alongside the heap and committed lazily
  — physical cost ≈ half of live heap use, the same "reservation is free"
  argument `boot.asm` already makes. Nothing about any object's layout, tag,
  address, or identity changes; every existing `[raw+8]`-style access in
  every file stays valid.

`ngranules` in the entry (rather than deriving size from the header on free)
is what makes two later things cheap: a linear heap walk (§3.5 sweep and
`GC-VERIFY`) and freeing without a per-`HDR_*` size switch. Set it in
`data_alloc`.

### 3.3 Counting discipline: deferred reference counting

This compiler spills every local to a `[rbp+disp]` slot and keeps every
intermediate on the target stack (`compile_binop` pushes its lhs across the
rhs; `compile_call_args` pushes every argument). Classic immediate RC would
require an inc/dec pair around essentially every store the compiler emits,
plus decrementing every slot of every frame on function exit — including on
`native_throw`'s longjmp, which cannot visit the frames it skips. That is
both the wrong cost model and, given the catch stack, not implementable
soundly.

Instead: **deferred reference counting** (Deutsch & Bobrow 1976).

- Counts record **heap→heap references only**: a tagged pointer stored in a
  cons cell, a closure's captured array, an array/record/condition/port slot,
  or a symbol's value/macro/plist cell (symbols are pinned heap objects, so
  their cells are heap slots, not stack).
- References from the target stack, target registers, host registers, host
  stack, catch-stack frames, and compile-time scratch are **not** counted.
- An object whose count is 0 is not freed; it is pushed onto the **ZCT**
  (zero-count table). Newly allocated objects have count 0 and are pushed
  immediately.
- Reclamation happens only at **safe points** (§3.5): scan the roots
  conservatively, mark what they reference, then free every ZCT member that
  is unmarked, unpinned, and still at count 0; freeing decrements children,
  which may push more ZCT entries, processed in the same pass.

What this buys, concretely for this codebase: the *only* places that need
new bookkeeping are the heap-store sites enumerated in §3.4 — a fixed list —
and `THROW` needs nothing at all (skipped frames held only uncounted
references).

**Lisp 1.5 and the assembly-Lisp lineage, honestly.** LISP 1.5's collector
was mark–sweep over the free-storage list with the push-down list as roots,
not reference counting. What this design borrows from it is (i) the
free-storage-list allocator — a freed cell's own first word threads the free
list, no separate free-block header — and (ii) "the stack *is* the root set,
scan it". Reference counting proper is Collins (1960) and Weizenbaum's SLIP
(1963), both of which simply did not collect cycles; the deferred/ZCT form is
Deutsch–Bobrow. That is the extent of the literature this spec leans on.

### 3.4 Site inventory — every place a tagged pointer is written into a heap object

Legend: **C** = creation site (fresh object; inc each heap-pointer child, then
push the new object to the ZCT), **M** = mutation site (dec old, inc new),
**R** = raw scratch buffer (not a Lisp object; free explicitly, no counts).

| File | Site | Kind | Change |
|------|------|------|--------|
| `reader.asm:36` `cons` | `[rax+0]=car`, `[rax+8]=cdr` | C | `rc_inc(car)`, `rc_inc(cdr)`. The hottest site; `rc_inc` must early-exit on tag `00`/`11` and on `PINNED` with ≤4 instructions. |
| `compiler.asm:2585-2682` `compile_lambda` emitted closure construction | copy loop stores `[rbx+32+8i]` | C | After the loop, emit `call rc_register(rax=tagged closure)` (host routine that incs `nfree` captured slots by reading `[raw+24]`). One emitted hostcall per closure creation. |
| `compiler.asm:3112` `compile_record_new` | `[rbx+8]=brand`, `[rbx+24+8i]=fields` | C | Same `rc_register` (dispatches on `HDR_RECORD`: brand is a pinned symbol, fields counted). |
| `conditions.asm:20` `make_error` | `[rax+8]`, `[rax+16]` | C | inc both. |
| `arrays.asm:81` `make_array` | slots `= IMM_NIL` | C | nothing to inc; push ZCT (done by `data_alloc`). |
| `arrays.asm:153` `make_typed_array` | raw slots | C | no children ever. |
| `arrays.asm:284` `array_set`, plain-array branch (`:293`) | `[rax+16+i*8]=rdx` | **M** | `rc_store_slot(&slot, new)`: dec old, inc new, store. Typed branches: no change. |
| `symtab.asm:406` `set_symbol_value` (`SET`), `:387` `set_symbol_plist` | `[sym+16]`, `[sym+40]` | **M** | `rc_store_slot`. |
| `compiler.asm` `compile_define:2167`, `compile_defdynamic:2201`, `compile_setq` `.global` `:2136`, `compile_defmacro`'s macro-slot install (`[sym+24]`), `bootstrap_globals` (`symtab.asm:250`, `T`) | emitted `emit_store_mem64` into a symbol cell | **M** | Emit a call to a new `rc_store_cell(rdi=cell, rsi=value)` hostcall instead of the bare store — the same "host routine address baked as imm64, `call rax`" shape every `compile_*_hostcall` uses. `SETQ` into a *local slot* (`:2128`) is untouched. |
| `strings.asm` `make_string:117`, `string_append:226`, `substring:295`; `chars.asm` `code_char_string`; `floats.asm:26` `make_float`; `modules.asm` (3× `make_string`) | leaf objects | C | no children; ZCT push by `data_alloc`. |
| `ports.asm:80` `port_alloc` + fills at `:190-204`, `:307-312`, `:405-416`, `:438-449` | `[16]` kind (pinned symbol), `[24]` name string, `[40]` mem_buf array | C | inc name, inc mem_buf. |
| `ports.asm:559` (close: `[40]`), `:954` (`[32]` flags — raw, no) | `[40]=mem_buf` rewrite | **M** | `rc_store_slot`. |
| `print.asm:91` `princ_to_string` (64 KB capture buffer), `fileio.asm:133` `file_read`, `ports.asm:588/660/777/857` scratch | raw buffers | **R** | allocate with a new `data_alloc_raw` (sets `RAW`, no ZCT push) and free at routine exit with `data_free`. These lifetimes are stack-shaped; no counts needed. This alone removes the README's named 64 KB-per-call leak. |
| `symtab.asm:201` `intern_symbol`, `:309` `gensym` | symbols | C | `PINNED`. Never freed. |
| `compiler.asm` `compile_vau` header overwrite; `catch_stack` frame words | not heap-slot references | — | none (catch frames are roots, §3.5). |

Anything allocated while **`rc_pin_depth > 0`** is `PINNED` at allocation:
`compile_thunk` increments it on entry and decrements on exit (reader output,
`current_scope` lists, `build_list2/3` synthetic forms, macro expansions,
`fold_binop_ast` results, the `&REST` param split — all compile-time
garbage this v0 accepts leaking exactly as today), and `run_buffer`/every
test driver resets it to 0 before *running* a thunk. `eval_form` therefore
compiles pinned and runs unpinned.

Independently, **anything baked into code is pinned at bake time**:
`rc_pin_deep(value)` — walk cons/closure/array/record/condition children
and set `PINNED` — is called from every emitter that takes a heap value as
an immediate: `compile_form`'s `.literal` and `QUOTE` cases (`:4882`,
`:4902`), `JIT-OPTIMIZE`, the operative call's two baked `QUOTE` operands,
`compile_error`'s baked default condition (`:3941`), `global_environment_sentinel`.
This is the rule that keeps a runtime `READ-FROM-STRING` result safe when it
is later passed to `EVAL`: the reader ran unpinned, `compile_thunk` bakes it,
it becomes immortal at that moment.

**Roots scanned at a safe point (not counted):** the target stack from the
current `rsp` up to `stack_base` (captured in `_start`, `boot.asm`, from the
initial `rsp` — one new `.bss` cell); `catch_stack[0 .. 32*catch_stack_top)`
(frame tags may be heap values; `UNWIND-PROTECT` markers hold the cleanup
closure in `frame[8]`); and all 16 GPRs as pushed by the safe-point stub.
Host-side `.bss`/`.data` cells were audited (`grep resq|dq` across `src/`):
`capture_buf` (raw), `reader_buf/pos/end` (raw), `rest_sym_scratch`,
`current_scope`, `lambda_frame_depth_scratch`, `nregslots_scratch`
(compile-time, pinned), `mask_stack` (fixnums), `rng_state`, `overflow_flag`,
`symtab_buckets` (symbols, pinned), `program_argv` (raw). `ports.asm` has no
cached stdin/stdout/stderr port — each `PORT-STD*` call allocates fresh
(`port_alloc` per call), so there is no hidden singleton root. **Any new
`.bss` cell that ever holds a tagged runtime value must be added to the
scanner's root list** — make it a review checklist item.

### 3.5 Mechanism

**`src/gc.asm` (new file; add to `CORE_SRCS` in `tests/run.sh` and the
`Makefile`):**

- `rc_table_base` (side table), `zct` (fixed array, e.g. 1 M entries ×
  8 bytes = 8 MiB `.bss`), `zct_count`, `zct_overflowed`, `free_lists[1..8]`
  (exact-fit granule classes) + `free_list_large`, `stack_base`,
  `rc_pin_depth`, `rc_bytes_live`, `rc_bytes_since_collect`.
- `rc_inc(rdi=tagged)` / `rc_dec(rdi=tagged)`: tag test → pointer? →
  `granule = (raw − base) >> 4` → `PINNED`? → count±1; on dec to 0 push ZCT
  (dedupe via `IN_ZCT`); saturate at 2³¹ (treat as pinned).
- `rc_store_slot(rdi=&slot, rsi=new)`: `old=[slot]; [slot]=new;
  rc_inc(new); rc_dec(old)` — inc before dec so `(STORE a i (FETCH a i))`
  can't transiently free.
- `rc_register(rdi=tagged obj)`: dispatch on header, inc each child.
- `rc_free(granule)`: dispatch on tag/header to enumerate children — cons:
  2 words; `HDR_CLOSURE`/`HDR_OPERATIVE`: `[32 .. 32+8*[24])`; `HDR_ARRAY`:
  `[16 .. 16+8*[8])`; `HDR_RECORD`: `[24 .. 24+8*[16])`; `HDR_CONDITION`:
  `[8]`,`[16]`; `HDR_PORT`: `[16]`,`[24]`,`[40]`; `HDR_STRING`/`HDR_FLOAT`/
  `HDR_TYPED_ARRAY`: none; `HDR_SYMBOL`: never reached (pinned) — `rc_dec`
  each, then thread the granule run onto its free list (first word of the
  run = next link; class by `ngranules`), clear `HEAD`, subtract from
  `rc_bytes_live`. **Iterative**: children that hit 0 go to the ZCT and are
  processed by the drain loop below, never by recursion — freeing a
  million-cell list must not recurse a million deep on the host stack (this
  is exactly the failure §2 is fixing on the target side; don't reintroduce
  it on the host side).
- `data_alloc` (`heap.asm`): consult the exact-fit free list for the
  requested granule count first (then `free_list_large` first-fit for >8),
  else bump as today; set `HEAD`, `ngranules`, `PINNED` if
  `rc_pin_depth > 0`; push ZCT unless `RAW`/`PINNED`; add to counters.
  **`data_alloc` never triggers collection** — see the invariant below.
- `data_alloc_raw` / `data_free` for §3.4's **R** sites.
- `rc_safepoint()` — the routine compiled code calls: `cmp
  [zct_count], THRESH; jb ret; jmp rc_collect` on the fast path (3 instructions
  plus the call), so a function entry costs one predictable branch.
- `rc_collect()`: push all 16 GPRs; (1) walk `[rsp, stack_base)` and the
  catch stack: for each 8-byte word `w` with tag `01` or `10` whose raw
  address lies in `[data_heap_base, data_heap_cur)` and whose granule has
  `HEAD` set, set `MARK`; (2) drain the ZCT: pop entries; skip (and clear
  `IN_ZCT`) any with count > 0 or `PINNED`; re-queue into a fresh ZCT any
  with `MARK`; `rc_free` the rest; loop until the ZCT is empty; swap in the
  re-queued list; (3) walk the same root ranges again clearing `MARK`;
  (4) if `zct_overflowed`, run the linear sweep: walk granules from
  `data_heap_base` by `ngranules`, freeing every `HEAD` with count 0,
  unmarked, unpinned (this converts a ZCT overflow from a leak into a slow
  collection); pop GPRs.

**The safe-point invariant, and why collection cannot run from `data_alloc`.**
At the moment `rc_collect` runs, every live reference to an unpinned object
must be visible to the scanner as a *tagged* word in a scanned range or a
pushed register. Two things violate that in the middle of ordinary code:
(a) an object under construction whose only reference is a raw untagged
address — `compile_lambda`'s emitted `rbx = raw closure addr` between
`data_alloc` and the final `or rax, TAG_HEAPOBJ` (`:2606-2682`), the same in
`compile_record_new`, `make_string`'s `r13`, `string_append`'s `r15`;
(b) interior pointers — `string_bytes` results handed to syscalls. Both occur
only *inside* host routines and emitted construction sequences, never at a
function boundary. Hence: safe points are emitted by the compiler only at
(i) every compiled function's entry, immediately after the prologue's
free-variable extraction and `&REST` list build (all incoming values are
now tagged words in slots or in `rdi`/`rsi`/`rdx`/`rcx`), and (ii) the
back-edge of `WHILE` (`compile_while:4817`) and of a backward `GO`
(`compile_go`) so allocation-heavy loops without calls still collect.
`data_alloc` itself only ever allocates. Because the stub pushes every GPR,
a host routine's callee-saved-register-held tagged pointer (e.g.
`invoke_macro`'s `r12` closure while the callee's entry safe point runs) is
covered without any host routine changing.

**Trigger policy:** `THRESH` on ZCT occupancy (e.g. 65 536) or
`rc_bytes_since_collect >= 16 MiB`, checked in `rc_safepoint`. `(GC-COLLECT)`
forces one.

**Cycles.** Plain RC never reclaims a cycle. In this kernel the only ways to
build one are `STORE` into an array/record slot (a hash table bucket that
contains the table; a closure stored into an array it captured) and
`SET-SYMBOL-PLIST!` (rooted at a symbol anyway, so not a leak in any
meaningful sense). `RPLACA`/`RPLACD` are non-destructive `CONS`es here
(README), so cons cycles cannot be formed at all, and closures capture by
value, so a closure cannot reference itself except through a global. **v0
stance: cycles leak, documented**, matching every early RC Lisp. The known
cheap upgrade — Bacon–Rajan synchronous trial deletion over "candidates"
(objects whose count was decremented to a nonzero value) — needs only the
child-enumeration `rc_free` already has, and can be added as a separate
feature without touching any store site.

**Observability primitives** (`compile_nullary_hostcall`/`unary` shapes,
dispatched in `compile_form`): `(HEAP-BYTES-USED)` = `data_heap_cur −
data_heap_base` (bump high-water), `(HEAP-BYTES-LIVE)` = `rc_bytes_live`,
`(GC-COLLECT)`, `(REFCOUNT x)`, and **`(GC-VERIFY)`** — walk the whole heap
linearly, recompute every unpinned object's count from scratch by
enumerating every heap slot in every `HEAD` object plus every symbol cell,
compare with the side table, print the first mismatch (address, header,
expected, actual) and return `NIL`, else `T`. `GC-VERIFY` is the tool that
turns "did I instrument every site?" from a code-reading exercise into a
test, and it is what §3.6's plan gates every step on.

### 3.6 Testing

`tests/cases/068_rc_reclaim.asm` (kernel-only):

1. **Reclamation is observable**: record `(HEAP-BYTES-USED)`, run
   `(SPIN 1000000)` (16 MiB of dropped conses today), record again; assert
   the delta is under 4 MiB (after the first collection the bump pointer
   stops advancing — freed 16-byte granules are reused). Print `T`/`NIL`.
2. **Live data survives**: build a 100 000-element list into a global, run
   the garbage loop, walk the list and print its length and a checksum.
3. **Closure captures are retained while the closure is live and released
   after**: capture a fresh 64 KB string in a closure held in a global; run
   garbage; call the closure and print the string's length; `SETQ` the global
   to `NIL`, `(GC-COLLECT)`, assert `(HEAP-BYTES-LIVE)` dropped by ≥ 64 KB.
4. **`STORE` releases the overwritten value** — same shape with an array
   slot; and **`SET-SYMBOL-PLIST!`/`DEFINE` rebinding** releases the old
   value.
5. **`THROW` across frames holding fresh garbage**: a loop that `CATCH`es a
   `THROW` from three calls deep, each level allocating — no crash, memory
   bounded, and `(GC-VERIFY)` is `T` afterward.
6. **`PRINC-TO-STRING` no longer leaks**: 10 000 calls, heap delta under
   1 MiB (today: 640 MiB — which would actually exceed the arena; use 2 000
   calls = 128 MiB today vs < 1 MiB after).
7. **Literal pinning**: `(EVAL (READ-FROM-STRING "(QUOTE (1 2 3))"))` in a
   loop of 1 000 with garbage in between; results correct; `(GC-VERIFY)` `T`.

`tests/run.sh`: append `(PRINT (GC-VERIFY))` and `(GC-COLLECT) (PRINT
(GC-VERIFY))` to the end of the `stdlib_conformance` program (expected `T`
twice). Loading 40 stdlib files exercises every creation/mutation site the
reference library actually uses; a missed `rc_inc` anywhere shows up as a
count mismatch here rather than as a corrupted list three programs later.
Also add the same two lines to `file_runner_prelude`.

### 3.7 Risk areas

1. **A missed `rc_inc` at a creation or mutation site** → an object freed
   while still referenced from the heap → its granules reused by the next
   `cons` → `(CAR x)` silently returns another list's car. This is the GC
   analogue of the `MAX`/`MIN` third-argument bug: no crash, wrong values.
   `GC-VERIFY` at the end of the conformance run is the systematic detector;
   run it after every commit in §3.8.
2. **A raw or interior pointer live across a safe point.** Only possible if
   a safe point is emitted somewhere other than function entry / loop
   back-edge, or if a future emitted construction sequence calls a compiled
   function mid-construction. Rule for reviewers: an emitted sequence that
   holds a raw address in a register must not contain a `call` into compiled
   code between `data_alloc` and the tagging `or`.
3. **Pinning gaps**: a heap value reaching code as an immediate through a
   path not listed in §3.4 (grep every `emit_mov_reg_imm64` whose `rsi` came
   from a tagged value). A missed one frees a literal: `(QUOTE (1 2))`
   returns garbage on the second evaluation.
4. **ZCT dedupe (`IN_ZCT`) desync** → double free → free-list corruption.
   `rc_free` must assert `HEAD` is set and clear it; `data_alloc` must assert
   a free-list pop has `HEAD` clear. Keep both asserts in non-debug builds;
   they are one `test` each.
5. **`&OPTIONAL`/`&KEY` and any later feature** adding (a) a new heapobj
   kind — owes `rc_register`, `rc_free`'s child enumeration, `rc_pin_deep`,
   and `GC-VERIFY`'s walker a case; (b) a new heap store — owes an
   `rc_store_slot`; (c) a new host `.bss` cell holding a tagged runtime value
   — owes the root scanner an entry; (d) a new emitted construction sequence
   — owes rule 2. This is the concrete reason GC lands last.
6. **Side-table index range**: valid only while the data heap is one arena.
   If `DATA_HEAP_BYTES` ever becomes growable, the table must grow in step.
7. **Compile-time allocations are still a leak** (pinned). Long sessions
   that `EVAL` in a loop grow the heap by their expansions. Documented v0;
   the fix (freeing pinned compile-time garbage after `compile_thunk`
   returns, except what was baked) is a separate feature.

### 3.8 Incremental landing plan

1. Side table + `HEAD`/`ngranules`/`PINNED`/`RAW` flags, free lists,
   `data_alloc_raw`/`data_free`, `(HEAP-BYTES-USED)`; convert the **R**
   sites (`princ_to_string`, `file_read`, four `ports.asm` scratch buffers).
   No counting yet. Test 6 passes; every existing test unchanged. A real,
   user-visible win on its own.
2. `rc_inc`/`rc_dec`/ZCT/`IN_ZCT`, `rc_register`, all **C** sites,
   `rc_pin_depth` + `rc_pin_deep` at bake sites, `(REFCOUNT)`, `(GC-VERIFY)`.
   Collection still off. Gate: `GC-VERIFY` = `T` at the end of
   `stdlib_conformance` and `file_runner_prelude`.
3. All **M** sites (`array_set`, symbol cells via `rc_store_cell` from
   `compile_define`/`compile_setq`/`compile_defdynamic`/`compile_defmacro`/
   `bootstrap_globals`, `set_symbol_value`, `set_symbol_plist`, port slots).
   Gate: same `GC-VERIFY` checks, plus test 4.
4. `stack_base` capture in `boot.asm`, `rc_collect` (scan / drain / unmark),
   `rc_free`, `(GC-COLLECT)`, `(HEAP-BYTES-LIVE)`; safe-point stub emitted at
   function entry but **calling `rc_collect` only when `GC-COLLECT` set a
   force flag**. Tests 1–5, 7 with explicit `(GC-COLLECT)` calls.
5. Automatic triggers in `rc_safepoint`, `WHILE`/`GO` back-edge safe points,
   the overflow sweep. Tests 1–7 without explicit collects; full suite;
   conformance with `GC-VERIFY`.
6. README: replace "no garbage collector" with the actual model, the cycle
   stance, the compile-time-pinned leak, and the reviewer checklist from
   §3.7 item 5.

---

## 4. Cross-cutting notes

- **Ordering recap**: 1 and 2 in either order (they do not overlap:
  1 changes what list `compile_lambda:2253`/`:2645` consumes; 2 changes
  `compile_call` and adds an `rsi` to eight helpers); 3 last. Within 3,
  steps 1–3 are pure bookkeeping with the collector off and can be reviewed
  purely against `GC-VERIFY`.
- **Every commit in every plan above is gated on `make test` green** —
  `tests/run.sh` runs the 63 kernel cases (numbered through `065`), `file_runner_prelude`, and
  `stdlib_conformance` (40 reference files as one buffer). None of the three
  features may change the conformance program's output.
- **README hygiene**: the README is this project's design record. Each
  feature's final commit should update the "v0 limits", "Roadmap", and (for
  1) the stale capture claims, in the same dense, self-justifying register
  the rest of the file uses — including the two measured numbers from this
  spec (2D+1 expansions; ~262 000-frame stack ceiling) so the next reader can
  re-verify them.
