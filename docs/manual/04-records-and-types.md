# 4. Records and Types

Every other Lisp makes you choose a record system when the project gets
serious enough: `defstruct` for speed, an alist or plist for flexibility, a
hand-rolled vector-with-tag for a homegrown object system, and eventually a
type checker bolted on from outside because none of the above were checked
in the first place. Lamedh gives you one form, `defrecord`, and it does not
make you choose. It defines a branded, nominal record type; it participates
in a structural (row-polymorphic) type system that infers itself with no
annotations; it degrades gracefully to dynamic, unchecked storage on any
field the checker cannot express; and the compiler silently promotes it to
native code when every field is a scalar. You get all of that from one
seed form, and the checker is honest about exactly how much of it it can
prove.

This chapter covers `defrecord` end to end, the row-polymorphic type system
underneath it, the `defun*`/`defun-typed` function-definition forms that
feed the same checker, and `definterface`, the Go-style method-set layer on
top of everything.

## 4.1 `defrecord`: the one door

```lisp
(defrecord point (x int64) (y int64))
```

This single form generates a constructor, a predicate, and one accessor per
field:

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (list (make-point 3 4) (point-p (make-point 3 4)) (point-x (make-point 3 4))))'
; => (#S(POINT 3 4) T 3)
```

The general shape is:

```lisp
(defrecord Name
  (field type) (field type) ...
  (:invariant expr...)   ; optional
  (:derive target...))   ; optional, targets: equality printer lens
```

A field written as a bare symbol (no parentheses, no type) means
`(field any)` — untyped, gradual storage:

```lisp
lamedh -s '(progn (defrecord parcel contents)
                   (parcel-contents (make-parcel (quote anything))))'
; => ANYTHING
```

Note the syntax: the bare form is a symbol standing on its own among the
field specs, not a one-element list — `(defrecord parcel (contents))` is a
different (and currently broken) thing, a field spec with no type element.
Stick to `(defrecord parcel contents)` or the explicit `(contents any)`.

`defrecord` always generates:

| Symbol | Purpose |
|---|---|
| `make-Name` | constructor, one positional argument per field |
| `Name-p` | predicate — true only for values with this exact brand |
| `Name-field` | one accessor per field |
| `validate-Name` | runs the `:invariant` (default: always true) |

`(:derive equality lens)` adds more, covered in §4.7. `defrecord` is built
on the condensation substrate, so every generated symbol is
provenance-tracked (`condense-kind`, `condense-generated`, `condense-trace`)
and the checker's verdict on it is computed and stored automatically —
nothing here is asserted, it is all checked at definition time.

There is exactly one record-defining form. Earlier releases had
`defconcept`, `defstruct`, and `defrecord` as three overlapping choices; as
of 0.3 they are unified into this one, with `defstruct-typed` remaining
only as internal machinery behind the compiled tier (§4.5) and untyped
mutable `defstruct` removed outright. Records are values — you don't mutate
a field in place, you build an updated copy with `record-with` (§4.3).

## 4.2 Records are nominal

Two records with identical field shapes are still different types. This
is not a structural type system pretending to be nominal — the brand is
part of the value, checked at the accessor:

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (defrecord vec2 (x int64) (y int64))
                   (check-type (point-x (make-vec2 1 2))))'
; => "type error: `POINT-X` arg 0 expects Struct(StructDef { name: \"POINT\", ... }),
;     got Struct(StructDef { name: \"VEC2\", ... })"
```

```lisp
lamedh -s '(progn (defrecord alpha (value int64))
                   (defrecord beta (value int64))
                   (alpha-p (make-beta 1)))'
; => ()
```

A `point` is never a `vec2`, whatever their fields look like. Accessors are
brand-checked at the type level (this is a static type error, not just a
runtime predicate failure) — you get the safety a nominal, class-based
language gives you, without writing a class.

## 4.3 Row polymorphism: `record-ref` and `record-with`

Nominal accessors are strict on purpose. When you actually want "any record
that happens to have this field," reach for `record-ref` — a checker-native
primitive that reads a field *by name* rather than by brand:

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (defun worth (p) (record-ref p (quote x)))
                   (worth (make-point 3 4)))'
; => 3
```

The interesting part is what the checker infers for `worth` with zero
annotations:

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (defrecord box (x int64) (y int64) (w int64))
                   (defun worth (p) (record-ref p (quote x)))
                   (see-type (quote worth)))'
; => (CHECKED (FORALL (A B) (-> ((RECORD ((X A)) B)) A)))
```

Read that scheme as: "for any type `A` and any row `B`, `worth` takes any
record with at least an `X` field of type `A` — plus whatever else, that's
what `B` is — and returns an `A`." `worth` runs on `point`, `box`, and any
record you define next month with an `x` field, and it type-checks all of
them without a single axiom. This is the money property of the design: one
function, written once, works — at the checker *and* at runtime — on
every record that has the field it asks for, differently shaped records
included:

```lisp
lamedh -s '(progn (defrecord coin (value int64))
                   (defrecord chest (value int64) (items (list string)))
                   (defun worth (x) (record-ref x (quote value)))
                   (list (worth (make-coin 5)) (worth (make-chest 9 (list "gold")))))'
; => (5 9)
```

`record-with` is the functional-update counterpart: it returns a new
record, same brand, with one field replaced.

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (record-with (make-point 3 4) (quote x) 99))'
; => #S(POINT 99 4)
```

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (point-p (record-with (make-point 1 2) (quote x) 9)))'
; => T
```

Both `record-ref` and `record-with` reject a field name the value's brand
does not have — this is a genuine static/dynamic row error, not a silent
`nil`:

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (check-type (record-with (make-point 3 4) (quote z) 99)))'
; => "type error: `record-with`: struct POINT has no field z"
```

## 4.4 The gradual frontier

Not every field type maps into the checker's type language. When a field's
declared type is a symbol or compound form the checker does not recognize —
naming no record and no known compound — `defrecord` degrades *that field
only* to `any`. This is per-field, not all-or-nothing: the rest of the
record keeps its checked types.

```lisp
lamedh -s '(progn (defrecord widget (x bogus-type) (y (mystery-compound int64)))
                   (list (see-type (quote widget-x)) (see-type (quote widget-y))))'
; => ((DECLARED (-> (WIDGET) ANY)) (DECLARED (-> (WIDGET) ANY)))
```

Compound types the checker *does* understand ride through unchanged —
`list`, `array`, `pair`, and `record` (including nested record brands):

```lisp
lamedh -s '(progn (defrecord thing (x (pair int64 string))) (see-type (quote thing-x)))'
; => (DECLARED (-> (THING) (PAIR INT64 STRING)))
```

```lisp
lamedh -s '(progn (defrecord inner (v int64)) (defrecord outer (i inner))
                   (outer-i (make-outer (make-inner 5))))'
; => #S(INNER 5)
```

The field is stored and accessed identically either way — `any` just means
the checker has stopped vouching for that one field. This is the gradual
frontier: static guarantees where the type language reaches, honest
dynamism everywhere else, with no migration step and no change to the
record's shape.

## 4.5 Tiers: compiled or dynamic, same surface

`defrecord` chooses, at definition time, whether a record compiles to a
native representation or runs on the dynamic `StructObj` path. The rule:
every field must be a scalar (`int64`, `float64`, `bool`, `char`) or an
array of scalars for the compiled tier; anything else — a `list`, a
`string`, a `symbol`, a nested record, a `pair` — runs dynamic.

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (defrecord chest (items (list string)))
                   (defrecord grid (cells (array int64)) (n int64))
                   (list (record-compiled-p (quote point))
                         (record-compiled-p (quote chest))
                         (record-compiled-p (quote grid))))'
; => (T () T)
```

`(array int64)` is natively storable, so `grid` compiles; `(list string)`
is not, so `chest` stays dynamic. The difference shows in the checker's
bookkeeping — a compiled-tier accessor is `TYPED`/`COMPILED`; a
dynamic-tier one is `DECLARED` (an axiom generated in lockstep with its
definition, since `record-ref` is a checker-native primitive rather than a
body the checker can re-derive a scheme from):

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (condense-check-type (quote point)))'
; => ((MAKE-POINT TYPED (TYPED (-> (INT64 INT64) POINT) COMPILED))
;     (POINT-P DECLARED (DECLARED (FORALL (A) (-> (A) BOOL))))
;     (VALIDATE-POINT DECLARED (DECLARED (-> (POINT) BOOL)))
;     (POINT-X TYPED (TYPED (-> (POINT) INT64) COMPILED))
;     (POINT-Y TYPED (TYPED (-> (POINT) INT64) COMPILED)))
```

None of this changes how you write code against the record. `make-Name`,
`Name-p`, and `Name-field` behave the same from the call site regardless of
tier — mix `chest` and `grid` in one function and never notice the
difference except in raw throughput. `record-compiled-p` is there to check,
e.g. before relying on a hot loop's performance.

## 4.6 Printing, reading, and structural equality

Records print in one readable form, `#S(BRAND field...)`, whatever their
tier:

```lisp
lamedh -s '(progn (defrecord chest (value int64) (items (list string)))
                   (make-chest 9 (list "gold" "gem")))'
; => #S(CHEST 9 ("gold" "gem"))
```

The reader accepts that same syntax back as source — `#S(...)` is a
literal, not just a printer artifact:

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64)) (point-x #S(POINT 7 8)))'
; => 7
```

Print and read round-trip through `equal`, the contract that makes records
safe to serialize across a `spawn` boundary or into a channel, and `equal`
on two records of the same brand and equal fields is already `T` by
structural comparison — you do not need `:derive equality` just to compare
two records; that derivation exists for when you want a *named*,
checker-declared function (§4.7):

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (list (equal (make-point 3 4) (read-from-string (prin1-to-string (make-point 3 4))))
                         (equal (make-point 1 2) (make-point 1 2))))'
; => (T T)
```

## 4.7 Invariants and derivations

`:invariant` attaches a validity predicate over the record's own fields,
checked by the generated `validate-Name`:

```lisp
lamedh -s '(progn (defrecord acct (bal int64) (:invariant (>= bal 0)))
                   (list (validate-acct (make-acct 5)) (validate-acct (make-acct -5))))'
; => (T ())
```

Multiple expressions in `:invariant` are implicitly `and`ed together. The
invariant is arbitrary Lisp, checked dynamically (the validator's own
signature, `(-> (Name) bool)`, is declared in lockstep) — `defrecord` does
not try to prove your invariant, only to run it.

`:derive` generates deterministic support code from the same field list.
Three targets exist:

| Target | Generates |
|---|---|
| `equality` | `Name-equal` |
| `printer` | `Name->plist` |
| `lens` | `Name->plist`, `plist->Name`, and a law `Name-lens-roundtrip` |

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64)) (derive point equality)
                   (list (point-equal (make-point 1 2) (make-point 1 2))
                         (point-equal (make-point 1 2) (make-point 3 4))))'
; => (T ())
```

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64)) (derive point printer)
                   (point->plist (make-point 1 2)))'
; => ((X . 1) (Y . 2))
```

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64)) (derive point lens)
                   (list (point-lens-roundtrip (make-point 1 2))
                         (plist->point (point->plist (make-point 5 6)))))'
; => (T #S(POINT 5 6))
```

`point-lens-roundtrip` is a generated *law*: it asserts `(equal (plist->point
(point->plist self)) self)` for any point, and is itself a runnable
predicate. `(:derive equality lens)` inside the `defrecord` form does
exactly what calling `derive` afterward does — `derive` is idempotent and
can be called again if you add a target later. Every derived operation gets
a declared, branded scheme the same way the constructor and accessors do,
so it participates in `see-type` and interface conformance (§4.11) for
free.

## 4.8 The checker: verdicts, the type language, `declare-type!`

Everything above rides on one small, honest type checker. Its entry points
are two builtins:

- `check-type` — evaluate the given expression's declared/inferred type, or
  report a type error, as a string.
- `see-type` — report the checker's structural verdict on a *symbol* (a
  defined function), as data.

```lisp
lamedh -s '(list (check-type (+ 1 2)) (check-type (+ 1 "a")))'
; => ("int64" "type error: `+` operands disagree: Int64 vs Str")
```

`see-type` reports one of five verdict shapes:

```lisp
TYPED      (TYPED sig COMPILED|INTERPRETED)   ; registered typed/compiled function
CHECKED    (CHECKED scheme)                    ; inferred from the body — a real guarantee,
                                                ; unless VACUOUS (see below)
DECLARED   (DECLARED scheme)                   ; an axiom asserted by declare-type!
DYNAMIC    (DYNAMIC reason)                    ; variadic, builtin, or otherwise opaque
TYPE-ERROR (TYPE-ERROR msg)                    ; the checker rejects the definition
```

```lisp
lamedh -s "(progn (defun inc (x) (+ x 1)) (list (see-type 'inc) (see-type 'car)))"
; => ((TYPED (-> (INT64) INT64) COMPILED) (DYNAMIC "variadic or not a plain lambda"))
```

`condense-classify` (from the condensation library, `lib/20-condensation.lisp`)
refines `CHECKED` one step further: a scheme whose *result* type contains a
type variable that no argument constrains proves nothing about that result
— it is `VACUOUS`, not `CHECKED`, even though the checker found no
contradiction:

```lisp
lamedh -s "(list (condense-classify '(typed (-> (int64) int64) compiled))
                 (condense-classify '(checked (forall (a b c) (-> (a b) c))))
                 (condense-classify '(checked (-> (int64) int64)))
                 (condense-classify '(declared (-> (int64) int64)))
                 (condense-classify '(dynamic \"reason\"))
                 (condense-classify '(type-error \"msg\")))"
; => (TYPED VACUOUS CHECKED DECLARED DYNAMIC TYPE-ERROR)
```

`condense-check-type` runs this classification over every symbol a
`defrecord`/`derive` generated and files the unverified remainder under
`"condense.dynamic-frontier"` on the record's own plist — nothing is ever
silently counted as verified when it is merely unrefuted.

### The type language

The forms you will see in signatures and `declare-type!` calls:

- Scalars: `int64`, `float64`, `bool`, `char`, `string`, `symbol`, `any`
- `(list T)`, `(array T)`, `(pair A B)`
- `(record ((f T)...) [tail])` — a row type; the optional tail (a type
  variable) is what makes it *open* rather than a closed shape
- A record's own brand name (`point`, `invoice`, ...) as a nominal type
- `(-> (arg-types...) ret-type)` — a function signature
- `(forall (vars...) type)` — universal quantification over the vars used
  inside `type`

`declare-type!` is the escape hatch: assert a scheme for a symbol the
checker cannot see through — typically a Rust builtin, or code deliberately
kept dynamic. It is what `defrecord` itself uses internally to give every
generated dynamic-tier operation a real, checkable signature:

```lisp
lamedh -s "(progn (declare-type! 'my-len '(-> ((list any)) int64)) (see-type 'my-len))"
; => (DECLARED (-> ((LIST ANY)) INT64))
```

Reach for it yourself only when wrapping something the checker genuinely
cannot analyze; for ordinary Lisp code, prefer to let it infer.

## 4.9 Derived schemes: rows through helper chains

The checker does not stop at one function's own body. When it meets a call
to a plain `defun` it has not yet checked, it checks that callee's body on
demand (memoized, and safe against recursion via a monotype assumption),
and uses the *derived* scheme at the call site — instead of degrading the
call to `any`. This is what lets a row type flow through an arbitrary chain
of helpers with zero annotations anywhere:

```lisp
lamedh -s "(progn (defun the-hp (x) (record-ref x 'hp))
                   (defun half-hp (x) (/ (the-hp x) 2))
                   (defun weak-p (x) (< (half-hp x) 3))
                   (see-type 'weak-p))"
; => (CHECKED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) BOOL)))
```

Two layers of helpers (`the-hp` -> `half-hp` -> `weak-p`), one row scheme,
derived, no axioms. The static error at the bottom of the chain travels all
the way up to whoever calls it wrong:

```lisp
lamedh -s "(progn (defrecord disc (r int64))
                   (defun the-cost (x) (record-ref x 'cost))
                   (check-type (the-cost (make-disc 2))))"
; => "type error: in call to `THE-COST`: struct DISC has no field cost"
```

`disc` was never told about `the-cost` — the row demand ("a record with a
`cost` field") comes purely from `the-cost`'s own body, and fails against
`disc`'s actual row the moment you apply it. A broken helper degrades
gracefully rather than infecting its callers: a callee with a genuine
`TYPE-ERROR` reports it at its own definition, while a caller of it stays
gradual — that one call site degrades to `any` instead of inheriting a
confusing secondhand error.

## 4.10 `defun-typed` and `defun*`

Two more function-definition forms feed the same checker as `defun`/
`defrecord`, for when you want to be explicit rather than let inference
run.

`defun-typed` declares every argument type and the return type up front,
and always compiles to native code:

```lisp
lamedh -s "(progn (defun-typed (sq int64) ((x int64)) (* x x)) (list (sq 5) (see-type 'sq)))"
; => (25 (TYPED (-> (INT64) INT64) COMPILED))
```

`defun*` is the "try to infer, and don't complain if you can't" form —
described in `AGENTS.md` as the recommended default when HM-style
inference should be attempted automatically, falling back silently to a
plain lambda when types are ambiguous. Its argument list accepts bare
names, `(name type)` pairs, or a mix, and an optional return type before
the body:

```lisp
lamedh -s "(progn (defun* dot (x int64) (y int64) int64 (* x y)) (list (dot 3 4) (see-type 'dot)))"
; => (12 (TYPED (-> (INT64 INT64) INT64) COMPILED))
```

When `defun*` can fully resolve every argument and the body, it compiles,
same as `defun-typed`. When it cannot — an argument left untyped and used
in a way that stays generic — it falls back to a plain, checked (not
compiled) function instead of erroring:

```lisp
lamedh -s "(progn (defun* mysq (x) (* x x)) (list (mysq 5) (see-type 'mysq)))"
; => (25 (CHECKED (FORALL (A) (-> (A) A))))
```

Use `defun-typed` when you want a hard guarantee (and are willing to
annotate every argument). Use `defun*` as your everyday `defun` when you
want the checker's inference to run whenever it can, without ceremony and
without ever blocking on ambiguity. Use plain `defun` when you deliberately
do not want typed compilation attempted (or opt out explicitly with
`(declare (no-compile))`).

## 4.11 Interfaces: `definterface`, `implements?`, `implements!`

`definterface` declares a named method set — a Go-style, structurally
satisfied interface, not a CLOS-style dispatch table:

```lisp
(definterface counter
  (:ops ((bump (-> (self) self)))))
```

There is no registration step for a type to "become" an implementer. A
method for type `T` and operation `op` is just the ordinary function
`T-op` — `int64-bump`, `goblin-greet`, `invoice-equal`. `SELF` in a
signature stands for the implementing type; for a record, it substitutes
to that record's own closed row.

```lisp
lamedh -s "(progn (definterface counter (:ops ((bump (-> (self) self)))))
                   (defun-typed (int64-bump int64) ((self int64)) (+ self 1))
                   (list (implements? 'int64 'counter) (method 'bump 5)))"
; => ((T (BUMP CONFORMS INT64-BUMP (-> (INT64) INT64))) 6)
```

`implements?` returns a structural conformance report, `(pass . per-op)`.
Each operation is graded:

- `CONFORMS` — the method exists and its checker verdict unifies with the
  declared signature: a real guarantee.
- `UNPROVEN` — the method exists but its verdict is `VACUOUS`/`DYNAMIC`:
  nothing confirmed, nothing denied. This does not fail the check.
- `MISMATCH` — the method exists but its verdict conflicts with the
  signature.
- `MISSING` — no such function exists at all.

`method` is the call-site counterpart: it computes the same `TYPE-OP` name
and applies it. There is no dispatch table anywhere — `method` is one
deterministic name computation, so a method type-checks, edits, and
traces exactly like any other function.

Two record kinds implementing the same interface, verified:

```lisp
lamedh -s "(progn
  (definterface greeter (:ops ((greet (-> (self) string)))))
  (defrecord goblin (name string) (hp int64))
  (defrecord wisp (name string) (glow float64))
  (defun goblin-greet (self) (concat (goblin-name self) \" snarls.\"))
  (defun wisp-greet (self) (concat (wisp-name self) \" glimmers.\"))
  (implements! 'goblin 'greeter)
  (implements! 'wisp 'greeter)
  (list (method 'greet (make-goblin \"Grix\" 7))
        (method 'greet (make-wisp \"Sel\" 0.5))))"
; => ("Grix snarls." "Sel glimmers.")
```

`implements!` is the assert-now form: it runs the same structural check as
`implements?`, records the claim (fingerprinted, so a later incompatible
redefinition of a method is detectable via `implements-recheck!`), and
signals an error immediately if conformance fails:

```lisp
lamedh -s "(progn (definterface renderable (:ops ((render (-> (self) string)))))
                   (defrecord bag (item any))
                   (implements! 'bag 'renderable))"
; => Error: implements!: BAG does not implement RENDERABLE: ((RENDER MISSING BAG-RENDER))
```

A derived operation (§4.7) is itself interface-eligible, since it carries a
declared branded scheme just like a hand-written accessor — `(:derive
equality)` on a record is enough to make it `CONFORMS` against an
`eq-able` interface with no method written by hand.

`examples/npcs.lisp` and `examples/oo-patterns.lisp` in the repository work
through this whole stack together — shared row-polymorphic behavior for
every NPC kind, per-kind specialized methods dispatched through
`definterface`, and classic Gang-of-Four patterns (Strategy, Composite,
Decorator, Observer, State) reduced to a handful of lines once the row
type system says "any record with this field" directly instead of forcing
a class hierarchy to say it indirectly. Run either with `cargo run -- -i
examples/npcs.lisp`, or check the whole file's checker verdicts at once
with `(check-file! "examples/npcs.lisp")` under the `READ-FS` capability.
