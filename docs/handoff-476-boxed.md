# Handoff: #476 `Ty::Boxed` — state, decisions, and the LHT payoff

Started on branch `claude/adoring-davinci-ag4p3a` (PR #480, 8 commits on top of
`4e37e4c`); continued on `claude/epic-keller-pgzanf`, which merges `main` (LHT, #472,
and the native `hash-code` swap, #477, had landed there) and adds the LHT payoff
below. Written for whoever picks this up next. Facts here were verified in-session;
where something is unverified it says so.

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

Worse: `scripts/gauntlet.sh` ran the fuzz leg as `cargo test … && echo FUZZ-GREEN`, so a
failure **omitted the marker while the script still exited 0**. The verdict markers
(`DEFAULT-GREEN`, `NDF-GREEN`, `FUZZ-GREEN`, `CLIPPY-GREEN`) remain the record, and the
script now also exits non-zero when any is missing. Clippy runs with `--features fuzz`
in the gauntlet, in CI's lint job and in the documented pre-commit command, so the fuzz
battery is linted; the first such run found three `cloned_ref_to_slice_refs` lints in
the boxed battery, which no earlier gate could see.

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

## The payoff: done on `claude/epic-keller-pgzanf`

`main` already carried LHT (`lib/45-hashtable.lisp`, #472) with `LHT-HASH` swapped to
`(hash-code v)` (#477), so step 2 of the original plan, including the `fork_world`
symbol-hashing trade, was decided on `main` before this branch touched it. What the
branch did:

1. **`LHT-PROBE` returns a packed `int64`** and is `defun-typed` with the signature
   `((array int64) boxed int64 boxed int64 int64 int64) -> int64`. Encoding, for
   capacity `cap`: `r >= 0` is a HIT at bucket `r` (payload `(fetch buckets r)`);
   `-cap <= r < 0` is a MISS with insertion bucket `(- -1 r)`; `r < -cap` is FULL.
   `LHT-FIND`/`-GET`/`-PUT!`/`-HAS-KEY-P`/`-REMOVE!` decode once per operation.
   `(see-type 'lht-probe)` reads `COMPILED`; a Lisp test pins it.
2. **`LHT-INSERT-EMPTY!`, `LHT-MIX64` and `LHT-HASH` are typed too.** The mixer is pure
   int64 arithmetic; its wrapping multiplies agree bit for bit with the interpreted body
   (test `lht-mix64-matches-reference-mixer`, over negatives and both int64 extremes),
   and the compiled edition sets the same `OVERFLOW` flag. `LHT-HASH` takes `boxed`, so
   `hash-code` on it is the intrinsic. `LHT-INDEX` moved from `defun*` to `defun-typed`,
   which also silences the `; defun* LHT-INDEX …` line every process printed at startup.
3. **`LHT-FIND`/`-PUT!`/`-GET` stay interpreted**, as planned: they read mixed-type
   slots out of the 8-slot table record, which needs the boxed-to-int64 coercion that is
   a v1 non-goal.
4. **Typed bodies cannot read globals**, so `-1`/`-2` (EMPTY/TOMBSTONE) and the mixer
   constants are literals in the typed bodies. Tests pin the globals to the literals
   (`lht-sentinels-match-typed-literals`, `lht-mix64-matches-reference-mixer`).
5. **Measured.** `benchmarks/hashtable/bench.lisp`, release, 3000 interned-symbol keys,
   three runs each, same machine and session:

   | | before | after |
   |---|---|---|
   | insert, us/op | 108-112 | 74-76 |
   | lookup, us/op | 27-29 | 17 |
   | insert, LHT / native | 50x-52x | 37x |
   | lookup, LHT / native | 12x-14x | 8x |

   The honest claim is the one the plan predicted: the probe loop compiles, and the
   per-operation cost dropped by about a third. The remaining gap to the native table is
   per-operation interpreted work (`LHT-FIND`'s frame, the record accessors, two
   membrane crossings per operation), not the probe loop. Closing it means either a
   checked `boxed -> int64` coercion so `LHT-FIND` can compile, or a typed record for
   the table itself; both are follow-up issues, not this branch.

## Full plan

The original implementation plan, with the §-numbers referenced above, is at
`/tmp/claude-0/.../scratchpad/plan-476-boxed.md` in the authoring session — ephemeral. Its
substance is reproduced here and in PR #480; nothing depends on recovering it.
