# v0.3.0 — 2026-07-10

condensation

Everything from the v0.2.0 tag to here. The headline: **one record
definition form** over one type story — records, sums, HM generics,
guards, processes, patterns, modules, and the checker meeting in one
language. Sections below, roughly newest first.

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

## Sequence protocols: map and for-each

- `(map coll fn)` — kind-preserving map as a protocol (list → list,
  array → array, string → string); `(for-each coll fn)` visits for
  effect over list/array/string/hash, with the hash instance receiving
  `(fn key value)`. Collection FIRST (protocols dispatch on their first
  argument — and it matches the container convention). Extend either
  with `definstance`.
- Ruled: `filter`/`reduce` stay list-specific and fn-first (the
  `mapcar`/`every` heritage class); protocols dispatch on the first
  argument, so protocolizing them would flip their argument order.
  Convert at the edge (`array->list`, `string->list`) for other kinds.
- **Breaking**: the Lisp 1.5 appendix's `map` (apply f to successive
  TAILS, return nil) is renamed **`map-tails`** — the bare name now means
  what every modern reader expects. Documented appendix deviation.

## Stdlib staples

Gap-probe additions (each follows its class's argument order):

- `(sort-by lst keyfn [pred])` — sort by extracted key, collection first
  like `sort`; `pred` (default `#'<`) compares keys.
- `(enumerate lst [start])` — index/element pairs, `zip` shape.
- `(frequencies lst)` — `(element . count)` alist, first-seen order;
  typed `(forall (a) (-> ((list a)) (list (pair a int64))))`.
- `(string-pad-left s width [pad])` / `string-pad-right` /
  `(string-repeat s n)` — padding never truncates.

## Dotted pairs in the checker

- `(cons 'k 2)` now types as `(pair symbol int64)` instead of erroring:
  a cons whose tail is a known non-list ground type takes the
  dotted-pair view, and `car`/`cdr` project known pairs. Unknown tails
  keep the list-cons view (the recursion default), so existing derived
  schemes are unchanged. The alist-cell idiom types end to end.
- New example: `examples/wordcount.lisp` — the classic word-frequency
  report, dogfooding the 0.3 staples (`frequencies`, `sort-by`,
  `enumerate`, padding, `for-each`, Option, `variant-case`). Found both
  the missing pair rule and the staples gaps.

## Typed protocols

```lisp
(defprotocol volume "loudness of a thing")
(definstance volume ((n int64)) int64 (* n 2))
(definstance volume ((s string)) int64 (string-length s))
```

- One name, many typed instances, three resolutions: the CHECKER selects
  the instance whose shape matches the first argument's inferred type and
  gives its precise scheme (a known type with no instance is a static
  error — "no `volume` instance for (list int64)"); the RUNTIME
  dispatches on the value's kind (list/string/array/hash/scalars, record
  brands, variants); the COMPILER treats each instance body as an
  ordinary defun, so eligible instances go native through the one-door
  pipeline. When every instance agrees on one ground result type, even a
  gradual call site derives it: `(defun n-items (x) (length x))` checks
  as `(forall (a) (-> (a) int64))`.
- `defprotocol` captures any prior binding as the fallback instance, so
  protocolizing a builtin is seamless. **`length` is the shipped pilot**:
  lists/strings/arrays/hash tables out of the box, and your own types
  join with one `definstance`.
- Instance implementations live under reader-unnameable hidden names —
  they cannot be shadowed or called around the dispatcher from source.

## The census, batches 2–3: one name, and the type table

- **`delete-key`/`delete-key-bang` removed; `remhash` is the one hash
  removal name** (collection first, kernel-direct).
- **`length` covers every sized collection**: lists, strings, arrays, and
  now hash tables.
- **The type table** (`lib/28-types.lisp`): verified declared schemes for
  ~45 builtins and stdlib functions — predicates
  `(forall (a) (-> (a) bool))`, integer-only predicates strict both ways,
  list functions (`member`/`filter`/`mapc`/`every`/`exists`/`notany`),
  strings and conversions (`string-upcase`, `princ-to-string : a →
  string`, `intern`, `implode`...), math with known results (`sqrt → 
  float64`, `floor → int64`). Two honesty rules exclude entries: NIL-ON-
  MISS functions never claim a result type (`nth`, `assoc`,
  `string->number` stay gradual — a declared "hit" type would let checked
  code consume a legal ()), and variadic/multi-arity functions can't carry
  fixed-arity schemes (they get kernel checker rules instead). Net effect:
  string/list/math pipelines derive full schemes with zero annotations,
  and misuse errors at the call site.

## The census, batch 1: variadicity (breaking)

The full special-form/builtin/stdlib census lives in docs/audit-0-3.md;
this batch fixes the variadicity findings.

- `append` is a variadic kernel builtin — `(append)`, `(append xs)`,
  `(append a b c ...)`, dotted final tail preserved — replacing the 2-ary
  stdlib recursion (it is also faster and checker-known now).
- `gcd`/`lcm` are variadic folds with the CL identities (`(gcd)` = 0,
  `(lcm)` = 1).
- **`logor` is renamed `logior`** — the name every Lisp reader (and
  model) expects; `logand`/`logxor` were already variadic and now the
  trio is uniform.
- `mapcar` takes N lists Common-Lisp style, zipping and stopping at the
  shortest.
- Checker-native rules for the variadic family: `append` (all `(list a)`
  → `(list a)`; dotted-tail append stays dynamic-only), `concat`
  (strings → string), `min`/`max` (numeric chain), `logand`/`logior`/
  `logxor`/`gcd`/`lcm` (int64). Side effect of the `concat` rule: the NPC
  example's greet methods left the dynamic frontier — their row schemes
  now derive fully.

## HM generics — parametric records and variants

```lisp
(defrecord (duo a b) (first a) (second b))
(defvariant (option a) (some (value a)) (none))
(defrecord (node a) (val a) (next (option (node a))))
```

- Records and variants take TYPE PARAMETERS, as proper HM type
  application: `make-duo : (forall (a b) (-> (a b) (duo a b)))`,
  instantiated freshly at every use — `(duo-first (make-duo 1 "s"))` is
  `int64`, `(unwrap-or (some "s") 0)` is a static error, `(none)`
  instantiates freely like nil. Nominal by name with pairwise argument
  unification; constructor applications absorb into their variant's
  application; SIBLING constructors of one variant unify (an `if`
  building `(some x)` / `(none)` types cleanly); parametric record
  applications subsume into rows with instantiated field types.
- **Recursion composes**: `(node a)` referencing `(option (node a))`
  gives `node-next : (forall (a) (-> ((node a)) (option (node a))))`.
- **Option and Result are parametric now** with precise helper schemes
  (`unwrap-or : (forall (a) (-> ((option a) a) a))`, monadic
  `option-then`/`result-then`, `result : (result a e)`).
- A BARE generic name in a type is sugar for the all-`any` application
  (`option` ≡ `(option any)`) — the gradual reading, and what
  pre-parametric code already meant.
- Erased at runtime: every value is the same branded StructObj; generics
  never compile (checker-only), monomorphic records keep their tiers.
- Built-in type-constructor names (`pair`, `list`, `array`, `record`,
  scalars) are REJECTED as record/variant names — they'd silently shadow
  the built-in meaning in type surfaces.

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

## Stack traces

- Runtime errors carry a **backtrace of named frames**: the toplevel prints
  `Error: boom` followed by `in: INNER ← MIDDLE ← OUTER` (innermost first),
  and handlers read the frames of the error they caught via
  `(last-backtrace)`. Tail calls collapse into one frame (Scheme-style TCO
  traces — a 100k-deep tail loop is a single entry). Recording is
  pay-mostly-on-error: overhead on hot benchmarks is within noise. Direct
  toplevel errors format exactly as before. Host API:
  `lamedh::format_error_with_backtrace`.

## Dynamic-extent guard fences (breaking)

- `with-fuel` and `with-capabilities` are KERNEL SPECIAL FORMS now, with a
  thread-local capability mask and RAII save/restore. Attenuation follows
  the CALL, not the fence's lexical body: **helpers called from inside a
  fence are fenced**, eval'd code is fenced, and kernel capability checks
  consult the mask on every gated operation. There is no Lisp-callable way
  to widen either state (`kernel-fuel-set!` is narrow-only inside a
  fence; no capability-mask setter exists).
- Escaped closures follow the same law in reverse: a closure created
  under a fence but called outside runs with the caller's authority —
  the semantics `spawn` always had, now uniform.
- Under an armed fuel budget, one-door native membranes take their
  interpreted fallback (compiled internal loops never returned to the
  metered trampoline — the fuel escape is closed); `jit-optimize` returns
  `COMPILE-DISABLED-BY-GUARD` and `defun-typed` errors while armed.
- The tick-instrumentation walker, lexical seal shadows, and eval
  re-sealing hatches are deleted from `lib/22` (~150 lines); the
  gated-builtin table remains for static `capabilities-needed` manifests.
- New read-only introspection: `(capability-mask-allows-p 'CAP)` — how
  custom (module-provided) capabilities attenuate through the same mask.

## Checker honesty

- Arithmetic and comparisons reject KNOWN non-numeric operands statically
  (`(+ "a" "b")` used to check as `string`); char arithmetic and char
  comparisons stay legal (evaluator parity), variables and `any` stay
  gradual — no scheme changes. Char literals now check as `char`.

## Modules

```lisp
(defmodule geometry (:export area) (:provides FAST-MATH))
(with-module geometry
  (defun helper (x) (* x 3))
  (defun area (r) (helper (* r r))))
(geometry:area 2)   ; => 12
(import geometry)   ; binds AREA
```

- A module is a NAMING DISCIPLINE plus metadata over the flat global
  namespace: `with-module` stores definitions as `MODULE:SYMBOL` (the
  reader now accepts `:` as a non-initial symbol constituent) and
  qualifies module-local references; `import` binds a module's exports
  globally (snapshot semantics); `module-of`/`module-functions`/
  `module-exports`/`module-requires`/`module-provides` introspect.
- **Modules can provide capabilities** — conservatively: a `(:provides
  CAP)` clause registers a NEW capability name into the attenuable
  vocabulary. It is held by registration at the outermost level, gates
  only explicit `(require-capability 'CAP)` checks, attenuates through
  `with-capabilities`/`sandboxed`/`spawn` like a built-in, and can never
  grant kernel abilities (READ-FS and friends stay host-granted). The
  fence now shadows `require-capability` so the gate attenuates with it.
- `(:requires CAP...)` records a module's needs for introspection and
  manifests.

## Regularity (breaking, deliberately)

One convention where there were several; the breaks are the point.

- **Hash operations are COLLECTION FIRST, one order**: `(gethash table
  key)`, `(remhash table key)` — matching `sethash`/`fetch`/`store`/
  `getp`. The either-order type-guessing (#246) is removed.
- **Comparisons are variadic monotone chains** like `+`/`*`: `(< a b c)`,
  `(= x y z)`, `(<= ...)`, `(>= ...)`.
- **`defun` supports `&optional` and `&key`** (with defaults; later
  defaults see earlier parameters; composes with `&rest`). Expanded at the
  defun layer to a variadic lambda + `LET*` prologue, so such functions
  stay on the dynamic tier. Bare `lambda`/`defmacro` still take only
  `&rest`.
- **`(set sym val)`**: the value-level global setter (both arguments
  evaluated) — the computed-symbol twin of the quoting `cset` macro.
- **H-suffix hex literals must start with a decimal digit** (`0FFh`, not
  `FFh`) — the assembly convention — so `ch`, `each`, `deadh` are ordinary
  symbols again.
- **The LABEL variable-read hack is gone**: a list VALUE headed by the
  symbol `label` no longer auto-evaluates when read from a variable (it
  made `(list 'label ...)` data explode and reserved `label` as a field/
  parameter name everywhere). The `label` special form in operator
  position is unchanged.
- **`-s` is repeatable** on the CLI; each string evaluates in order in one
  shared environment.

## explain-compile

- `(explain-compile 'f)` reports the execution tier as data —
  `((TIER . COMPILED) (SIGNATURE ...))`, or `CHECKED` with the SCHEME and
  the CONCRETE blocker keeping it off the native tier (ambiguous operand
  types, non-storable list/record schemes, a `(declare (no-compile))`
  pin), or `DYNAMIC`/`TYPE-ERROR`. Side-effect-free (a dry-run twin of the
  codegen path — explaining never installs anything).

## Instrumentation: trace / time / step-count — and ONE fuel ruler

- `(trace 'f)` / `(untrace 'f)`: real call tracing (args in, value out,
  indented by depth) — replacing the Lisp 1.5 flag-only stubs. Natively
  compiled internals count as one call.
- `(time form)` prints wall milliseconds + kernel steps and returns the
  value; `(step-count form)` returns `(steps . value)`.
- **Breaking — one ruler**: `with-fuel N` is now denominated in KERNEL
  STEPS (one trampoline iteration each), the exact unit `step-count`
  measures: `(car (step-count form))` sizes the budget, tight to a handful
  of steps. The old unit (function entries, ×256 kernel backstop) is gone,
  along with the tick-instrumentation walker — the kernel counter is the
  single meter (the no-compile rewrites remain: in-fence definitions stay
  interpreted so native loops can't escape metering). `fuel-remaining`
  reads the kernel counter. Nested fences clamp to and spend from the
  enclosing remainder; fence setup charges the enclosing budget.
- `spawn`'s `:fuel` was already kernel steps; it now agrees with
  `with-fuel` instead of being 256× finer.
- New builtin: `(monotonic-micros)`.
- Soundness fix (found by TRACE's wrapper): a `&rest` closure created
  under an intervening `LET` read the let's slots instead of enclosing
  variables — `&rest` call frames now contribute an Opaque scope level so
  outer `LocalGet` depths count them.

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
