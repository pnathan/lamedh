# v0.3.0

condensation

One record definition form. The `defconcept` / `defstruct` / `defrecord`
split is closed: **`defrecord` is the only way to define a record**, and it
does everything the three forms did together.

```lisp
(defrecord npc
  (tag symbol) (hp int64) (loot (list string))
  (:invariant (>= hp 0))
  (:derive equality lens))
```

## Added

- `defrecord` absorbs `defconcept`'s sections: optional `(:invariant ...)`
  and `(:derive equality|printer|lens ...)` after the field specs. Every
  record now also generates `validate-NAME` (invariant defaults to `T`) and
  records the condensation trace (`condense-trace`, fingerprints, `edit!`,
  `deflaw`, `example` all work on records).
- Bare field names: `(defrecord bag stuff)` means `(stuff any)` — the
  gradual frontier is per-field.
- Records print as `#S(BRAND v ...)` and the reader accepts `#S(...)`
  literals: values round-trip through print/read (spawn and channel
  serialization) and are usable as source syntax.
- Every record is a branded, checker-denotable, nominal, row-subsumable
  type over one runtime representation, compiled when all fields are
  natively storable (`(record-compiled-p 'name)` reports the tier).
- The checker derives schemes for unknown lambda callees on demand, so row
  types flow through helper functions with no `declare-type!` axioms.

## Removed (breaking)

- **`defconcept`** — use `defrecord` with the same sections inline. A
  legacy `(:fields ((f ty) ...))` section is still accepted to ease
  migration; the canonical form is inline field specs.
- **`defstruct`** (the untyped, mutable, array-backed form) — use
  `defrecord`. With it go its keyword constructor protocol
  (`(make-point :x 1)`) and its generated `set-NAME-FIELD!` mutators;
  records are values — update with `record-with` or `NAME-field` reads
  around construction. `setf`'s accessor-place convention still expands to
  `(set-<accessor>! obj v)`, now resolved against user-defined mutators.
- The positional record representation (`(BRAND f1 f2 ...)` lists) and its
  `record-ref`/`record-brand` bridges. All record values are native
  `StructObj`s.
- `condense-kind` for records is now `record` (was `concept`); `derive`,
  `deflaw`, and `example` say "record" in their error messages.

## Internal

- `defstruct-typed` remains as the kernel machinery behind `defrecord`'s
  compiled tier; it is no longer a documented user-facing form.

# v0.2.0

efficiency.

- Typed Cranelift JIT with a gradual HM checker (rows, structs, declared
  schemes), slot-frame locals, pre-compiled lambda bodies, TCO through the
  typed membrane.
- Guard fences (`with-fuel`, `with-capabilities`, `sandboxed`), `spawn`
  with capability attenuation, restarts, the pattern language
  (`match`/`sgrep`/`rewrite`), the rulebook optimizer, persistent REPL
  history and tab completion.
