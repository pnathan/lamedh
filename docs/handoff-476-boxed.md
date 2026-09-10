# Handoff: #476 `Ty::Boxed` — state, decisions, and what's left

Branch `claude/adoring-davinci-ag4p3a`, PR #480, 8 commits on top of `4e37e4c`.
Written for whoever picks this up next. Facts here were verified in-session; where
something is unverified it says so.

## What is done

`Ty::Boxed` — a compileable opaque handle to an arbitrary `LispVal` — exists across the
whole typed stack: the type and lattice, the membrane, elaboration guards, five intrinsics
on all tiers, docs, CHANGELOG, and randomized fuzz coverage.

| Commit | What |
|---|---|
| `d27030d` | Thread `&Ctx` through `Value::from_word`. Mechanical prerequisite. |
| `d500920` | The type, lattice, `Ctx` root table, membrane, three escape refusals. |
| `4e85738` | Elaboration guards: arithmetic/comparison refused on boxed operands. |
| `64559dc` | Intrinsics across the three tiers (WIP checkpoint). |
| `b7da7e2` | Cross-tier fixture + native-tier coverage. |
| `3ff504e` | Audit fixes, boxed-tier docs, CHANGELOG, `Any`→`boxed` blocker hint. |
| `204e616` | Fuzz-battery exhaustive-match fix. |
| `47634be` | Randomized boxed coverage in the brutal battery. |

## The one decision everything rests on

A handle is a **1-based index into a per-call root table on `Ctx`** (`Ctx.boxed:
RefCell<Vec<LispVal>>`); word `0` is boxed `NIL`.

Not a raw `*const LispVal`. The arena's pointer-stability guarantee comes from
`Box<[u64]>`; a `Vec<LispVal>` has no such guarantee, so a raw pointer handed to Cranelift
would be a bug that tests would not catch. An index cannot dangle across `Vec` growth, and
the table drops with the arena when the top-level call returns.

**The aliasing property.** The table holds a *cloned* `LispVal`. For `Shared`-backed
variants (`Array`, `Cons`, `Symbol`, …) that clone aliases the caller's object, so a
`store` through a handle mutates the caller's own array with **no write-back step**. Inline
variants (`Number`, `Float`, `Char`, `Nil`) copy, which is sound because they are
immutable. If you change the root table's representation, this property is the thing to
preserve — `brutal_boxed_array_ops_are_panic_free_and_alias` will catch you if you don't.

**Two handles may denote the same object.** Hence: no `Cmp` node is ever elaborated at
boxed type. Identity questions go through a deref intrinsic, never a handle-word compare.

## Three findings that corrected the original plan

Recorded because each would otherwise be rediscovered the hard way.

1. **`Infer::unify` needed a `(Boxed, Boxed)` arm.** Without it two boxed values fail to
   unify and *every* signature using `boxed` is rejected by the checker. The plan named
   `resolve` and `is_compileable` and missed this.
2. **`verify_core` had no invariant to extend.** `Core::Bin`/`Core::Cmp` carry a `NumKind`
   (exhaustively `{I, F}`), never a `Ty` — a boxed comparison is *structurally
   unconstructible* once `Core` exists, not merely disallowed. Enforcement lives in
   `elaboration.rs`, the last layer where `Ty` survives. `num_kind_has_no_boxed_variant`
   pins it with an exhaustive match that fails to **compile** if `NumKind` ever gains a
   boxed-like variant.
3. **Native codegen needed nothing for movement.** `native.rs` never branches on `Ty` for
   word-size lowering — every value is already a plain I64 word. Verified by differential
   test reaching `Tier::Native`, not assumed.

## Process trap, worth knowing before you add the next variant

`tests/brutal_correctness.rs` compiles **only** under `--features fuzz`. `cargo test`,
`cargo build --no-default-features`, and `cargo clippy --workspace --all-targets` are all
blind to it. Six commits went out green on those gates while that file did not compile.

Worse: `scripts/gauntlet.sh` runs the fuzz leg as `cargo test … && echo FUZZ-GREEN`, so a
failure **omits the marker and the script still exits 0**. Check the four verdict markers
(`DEFAULT-GREEN`, `NDF-GREEN`, `FUZZ-GREEN`, `CLIPPY-GREEN`), never the exit code.

## Testing, and what it does and does not prove

There is deliberately **no Rust oracle** for boxed. An oracle would have to be
`PartialEq for LispVal` and `crate::hash_code` — the same functions the intrinsics call,
since they were made shared on purpose so builtin and intrinsic cannot drift. Comparing
them to themselves would pass while proving nothing.

So the fuzz checks are metamorphic and differential:

- **`EQUAL a b ⇒ hash-code a == hash-code b`** — the real contract, oracle-free and
  falsifiable. The test asserts the EQUAL-pair count is nonzero so the law cannot go
  vacuous. Last run: 2000 pairs, 456 EQUAL and hashing identically.
- **Three-way tier agreement** per call: compiled edition, typed-core reference
  interpreter, tracing interpreter.
- Round-trip identity, movement through `if`/`let`, the aliasing property, and
  panic-freedom on adversarial indices and non-array receivers. Index draws are half
  in-range, half hostile — a purely adversarial draw left `fetch`'s success path at 99 hits
  against 2123 errors; it is now 652 vs 1579.

The generator keeps a **pool so it can hand back aliased values**. This is load-bearing:
array/hash-table equality is identity (`Shared::ptr_eq`), so two structurally identical
distinct arrays are not `EQUAL` and constrain nothing. Without aliasing the hash law would
be vacuous for every identity type.

## Gate status at hand-off

`scripts/gauntlet.sh` green on `47634be`'s code: **all four markers** —
`DEFAULT-GREEN`, `NDF-GREEN`, `FUZZ-GREEN`, `CLIPPY-GREEN`. The fuzz leg ran 11 tests
(up from 9), i.e. both new boxed batteries executed inside the gauntlet rather than only
standalone. `7401de2` is docs-only, so that verdict covers the current tip's code.

## Known limitations, stated rather than buried

- **Tier agreement is three code paths, not all four.** `TypedFn::invoke_once` prefers the
  native edition when present, so the *closure* edition is shadowed under `--features jit`.
  It is covered by the `--no-default-features` leg of the gauntlet — across runs, not
  within one. Closing this needs a way to force the closure edition; there is no public API
  for it today.
- **`call_by_id`'s `all_scalar` fast path gates on parameter types only**, so a
  scalar-parameter function returning `boxed` takes it. Sound — `from_word` unboxes while
  the `Ctx` is alive — but the name oversells the guarantee. `src/jit/registry.rs:1714`.
- **`(array boxed)` as a typed-array parameter gets no write-back**, so `ASET` into it does
  not reach the caller. Distinct from `store` through a handle-to-an-array, which does.
  This is `is_flat_scalar_array` excluding boxed by design; a functional gap, not a safety
  one. `src/jit/registry.rs:1827`.
- **Pre-existing, out of scope, worth fixing sometime:** `jit::parse::value_to_lispval`
  encodes a `bool` result as `Number(0|1)` while `evaluator::functions::typed_to_lispval`
  uses `NIL`/`T`. Two membranes, two bool encodings. `Number(0)` is *truthy* in Lisp, so
  anything reading the first membrane's bool for NIL-ness is wrong. This cost me a test bug;
  it will cost someone else a real one.

## v1 non-goals (deliberate, do not "fix" accidentally)

No `CAR`/`CDR` or any compiled introspection. No cons allocation. No boxed arithmetic. No
`Cmp` at boxed type. No narrowing to a known constructor set. No checked `boxed → int64`
coercion. Wanting any of these means you have left v1 — open a follow-up issue.

## What is left: the payoff, which is NOT in this branch

#476 delivers the *type*. The motivating consumer is `lib/45-hashtable.lisp` (LHT, #458 /
#472), which lives on the **unmerged** branch `origin/claude/issue-458-lamedh-hashtable`
(4 commits, tip `e3fab3c`). Nothing here has been measured against it, and this PR claims
no benchmark numbers.

Once #458 lands, the follow-up work is:

1. **Rewrite `LHT-PROBE` to return a packed `int64`** instead of `(list 'status idx
   payload)`. This is the blocker #476 does not remove: a cons per probe step keeps the
   function interpreted no matter what the type lattice says. Give it the signature
   `((array int64) boxed int64 boxed int64 int64 int64) -> int64` and have
   `LHT-FIND`/`-GET`/`-PUT!`/`-HAS-KEY-P`/`-REMOVE!` decode. **Without this step, #476 buys
   LHT nothing.**
2. **Replace `LHT-HASH`'s body with `(hash-code v)`** (#474, merged as `4e37e4c`).
   Verified safe against LHT's stated contract: `lisp_float_hash_bits` (`src/lib.rs:2812`)
   already collapses every NaN to one bit pattern and both signed zeros to `+0.0`, which is
   exactly the `0.0`/`-0.0`-same-key and NaN-finds-NaN guarantee LHT hand-rolls via
   `PRIN1-TO-STRING`. If the float/NaN assertions in `tests/lisp/72-lamedh-hashtable.lisp`
   fail after the swap, **stop** — that is a real disagreement, not a test to edit.
   The swap also closes the gap LHT's own header calls out as its honest cost: arrays,
   hash tables and environments currently all collide in `$lht-tag-opaque`'s single bucket.
   **One trade to decide explicitly:** `Hash for LispVal` hashes a `Symbol` by its interned
   `Shared` pointer, while LHT hashes symbols by printed *name*. A name is stable across
   `fork_world`'s deep copy; a pointer is not. Theoretical in practice (a `LispVal` does not
   cross worlds without host code moving it), and the benchmark keys are 3000 interned
   symbols, so it is exactly the path that would notice. Cheap mitigation if it ever bites:
   keep the by-name arm for symbols only.
3. **Leave `LHT-FIND`/`-PUT!`/`-GET` interpreted.** They read mixed-type slots out of the
   8-slot general array `ht`, which needs a checked `boxed → int64` coercion (a v1
   non-goal), and they run once per *operation* while the probe loop runs once per *step*.
4. **Measure.** `benchmarks/hashtable/bench.lisp`, release build, before and after. The
   honest claim after all this is "the probe loop compiles" — not the #472 headline
   (~178x/60x), which is the whole `lht` vs native gap including per-operation interpreted
   overhead this work does not touch. If the number does not move, that is a finding worth
   reporting, not a reason to widen scope.

## Full plan

The original implementation plan, with the §-numbers referenced above, is at
`/tmp/claude-0/.../scratchpad/plan-476-boxed.md` in the authoring session — ephemeral. Its
substance is reproduced here and in PR #480; nothing depends on recovering it.
