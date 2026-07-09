# v0.3.0

condensation

Everything from the v0.2.0 tag to here (83 commits). The headline: **one
record definition form**, and a language that condenses — records, guards,
processes, patterns, and the checker meeting in one story.

## Records — one definition form (breaking)

```lisp
(defrecord npc
  (tag symbol) (hp int64) (loot (list string))
  (:invariant (>= hp 0))
  (:derive equality lens))
```

- **`defrecord` is the only way to define a record**, and it does
  everything `defconcept`/`defstruct`/`defrecord` did together: branded,
  checker-denotable, NOMINAL, row-subsumable types over one runtime
  representation (StructObj), tier-dispatched — all-native fields compile,
  anything else runs dynamic with the same surface
  (`(record-compiled-p 'name)` reports the tier). Generates `make-Name`,
  `Name-p`, `Name-field`, `validate-Name`; optional `(:invariant ...)` and
  `(:derive equality|printer|lens ...)` sections; bare field names mean
  `(field any)`; the condensation trace (`condense-trace`, `edit!`,
  `deflaw`, `example`, fingerprints) works on every record.
- **Removed: `defconcept`** — use `defrecord`; a legacy
  `(:fields ((f ty)...))` section is still accepted to ease migration.
- **Removed: untyped `defstruct`** — with its keyword constructor protocol
  and `set-NAME-FIELD!` mutators. Records are values: update with
  `record-with`. `setf` accessor places still expand to
  `(set-<accessor>! obj v)` against user-defined mutators.
- **Removed: the positional record representation** (`(BRAND f1 ...)`
  lists). `defstruct-typed` remains only as internal machinery behind the
  compiled tier.
- `record-ref` / `record-with`: by-name field read and functional update
  with checker-native row rules — `(defun worth (x) (record-ref x 'value))`
  derives `(forall (a b) (-> ((record ((value a)) b)) a))` with no axioms.
- Records print as `#S(BRAND v ...)` and the reader accepts `#S(...)`
  literals: print/read round-trip (spawn and channel serialization), usable
  as source syntax.

## Sum types

```lisp
(defvariant shape
  (circle (r int64))
  (rect   (w int64) (h int64)))
```

- **`defvariant`**: a closed set of branded record constructors (bare
  constructor names — `(circle 3)`) plus a checker-level union type. A
  `CIRCLE` unifies where a `SHAPE` is demanded; a constructor of another
  variant is rejected by name ("HEADS is not a constructor of variant
  SHAPE").
- **`variant-case` is exhaustive**: missing a constructor without an
  `else` clause errors, naming the missing brands.
- **`match` destructures records/constructors with `#S` patterns**:
  `(match v (#S(CIRCLE ?r) ...) ...)`, nesting and all.
- **Option and Result** are ordinary variants in the stdlib
  (`some`/`none`, `ok`/`err`) with `unwrap`/`unwrap-or`/`option-map`/
  `option-then`/`result-or`/`result-map`/`result-then`/`option-of`, and
  `try-call` bridging the condition system into Result.
- Breaking: the `some` list quantifier in `lib/13` is renamed **`exists`**
  (`(exists #'evenp xs)`); `some` is Option's constructor now.
- `record-new` accepts zero field values (nullary constructors like
  `(none)`).

## Recursive records

- Self- and mutually-referential `defrecord` field types are NOMINAL now,
  not a silent `any`: `(defrecord node (val int64) (next node))` gives
  `node-next : (-> (NODE) NODE)`. Struct unification is by brand name;
  struct-into-row expansion re-resolves the definition through the
  registry; forward references get provisional definitions (a misspelled
  field type surfaces as a phantom brand at first unification instead of
  degrading silently). The blessed terminator idiom is Option:
  `(next option)` with `(none)`.

## Checker and rows

- Row types (Rémy-style, with a gradual `Any` frontier) ported into the
  checker; typed structs subsume into record rows; `declare-type!` declared
  schemes consulted at call sites; `see-type` reports
  TYPED/CHECKED/DECLARED/TYPE-ERROR/DYNAMIC verdicts as data.
- **Derived schemes at call sites**: the checker checks unknown lambda
  callees on demand (memoized, recursion-safe), so row types flow through
  helper functions with zero annotations.
- **One door**: plain `defun` quietly attempts typed compilation; opt out
  with `(declare (no-compile))` or `declaim`. `defun-typed`/`defun*` remain
  for explicit signatures/inference.

## Guards, fuel, and processes

- Composable guard fences, pure Lisp: `with-fuel`, `with-capabilities`,
  `sandboxed`; guarded code is introspective; static capability manifests
  (`capabilities-needed`) computed from the call graph; a kernel
  step-budget backstop makes fuel exhaustion un-catchable-by-accident.
- `spawn` / `spawn*` / `await`: share-nothing interpreter threads whose
  capabilities are the requested set intersected with the caller's
  effective set — attenuation all the way down (#140). Channels and
  `clone-interpreter` behind the `concurrency` feature.

## Conditions

- Restarts: `restart-case`, `invoke-restart`, `find-restart`,
  `compute-restarts`, `handler-bind`, plus `use-value`/`store-value`/
  `retry`/`abort-to-restart`/`with-retry-restart`. (Documented deviation:
  handler-bind handlers run post-unwind.)

## Patterns and the rulebook

- Structural pattern language: `pat-match` (`?x`, `??segments`, guards),
  `match`, `destructuring-bind`, `sgrep`/`sgrep-source`/`sgrep-file`
  (positioned hits via `read-all-positioned`), bottom-up `rewrite`.
- The rulebook optimizer: `defrule`/`list-rules`/`apply-rules` — optimizer
  passes as pattern data feeding `optimize-form`.
- Go-style interfaces: `definterface`, `implements?`/`implements!` verify
  method sets against checker verdicts with a row-aware unifier.

## Performance

- Slot frames with routing tables: sound compile-time lexical addressing
  for lambda params and LET binders (#200 M3); unified compile/tree-walker
  trampolines; typed-JIT tail calls (self, mutual, and general); lambda
  bodies pre-compiled at definition time; per-call allocation cuts
  (SmallVec operands, symbol-id frames, precomputed special-form dispatch,
  cached symbol flags, lazy defun analysis hooks); shallow binding for
  dynamic variables; optional `arc-val` atomic refcounting feature;
  COLLAPSE-FRAMES and purity-checker optimizer passes over a DEFUN call
  graph.

## Correctness and parity (the #210 audit and friends)

- Typed tiers now match the evaluator exactly on: Euclidean `MOD`,
  `OVERFLOW`/div-by-zero flag propagation through the membrane (including
  `MIN%-1` and flag-before-error ordering), `FETCH`/`STORE` out-of-bounds
  errors, `CODE-CHAR`/`CHAR-CODE`, variadic `AND`/`OR`, strict binary
  `/`/`MOD` arity, mutated array parameters written back.
- Soundness: gensym symbols bind by their own id in lambda params and
  `SETQ`; dynamic bindings preserved across tail calls; cross-namespace
  symbol-id remapping in local frames; checked `gcd`/`lcm`; reachable JIT
  panics converted to Lisp errors; reader recursion bounded;
  `RENAME-FILE` requires `READ-FS` in addition to `CREATE-FS`.

## Reader, CLI, and docs

- Reader: 1-based positions in parse errors, nesting block comments
  `#| |#`, CL-style radix literals (`#x`/`#b`/`#o`), shebang support.
- REPL: persistent history (saved on `(exit)` too) and symbol tab
  completion.
- Self-evaluating keyword symbols; runtime error messages include the
  offending value; `cargo doc` warning-free; docs refreshed to match
  behavior; classic OO patterns example on row types (`examples/`).

# v0.2.0

efficiency.

The typed JIT release: HM-lite type inference (#135), the typed membrane
with a native Cranelift backend (#124), `defun-typed`/`defstruct-typed`,
typed regions, and the `jit` feature on by default.
