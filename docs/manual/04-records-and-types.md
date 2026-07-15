# 4. Records and Types

Every other Lisp makes you choose a record system when the project gets
serious enough: `defstruct` for speed, an alist or plist for flexibility, a
hand-rolled vector-with-tag for a homegrown object system, and a type
checker bolted on from outside because none of the above were checked in
the first place. Lamedh gives you one form, `defrecord`, and it does not
make you choose. It defines a branded, nominal record type; it
participates in a structural (row-polymorphic) type system that infers
itself with no annotations; it degrades gracefully to dynamic, unchecked
storage on any field the checker cannot express; and the compiler silently
promotes it to native code when every field is a scalar. One seed form,
and the checker stays honest about exactly how much of it it can prove.

This chapter covers `defrecord` end to end, the row-polymorphic type system
underneath it, the `defun*`/`defun-typed` function-definition forms that
feed the same checker, `defvariant` and the closed sum types it builds
(including the stdlib's `Option`/`Result`), the HM-style generics that make
records and variants parametric, self- and mutually-recursive record
types, and `definterface`, the Go-style method-set layer on top of
everything.

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

The one-element list `(defrecord parcel (contents))` means the same thing
as the bare symbol — a field with no type is an `any` field either way.

`defrecord` always generates:

| Symbol | Purpose |
|---|---|
| `make-Name` | constructor, one positional argument per field |
| `Name-p` | predicate — true only for values with this exact brand |
| `Name-field` | one accessor per field |
| `validate-Name` | runs the `:invariant` (default: always true); `make-Name` enforces it at construction |

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
; => "type error: `POINT-X` arg 0 expects POINT, got VEC2"
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
                   (list (record-with (make-point 3 4) (quote x) 99)
                         (point-p (record-with (make-point 1 2) (quote x) 9))))'
; => (#S(POINT 99 4) T)
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

Not every field type maps into the checker's type language, and the two
ways a field type can fail to map are treated differently on purpose. A
*bare structural word* used without its required arguments — `list`,
`array`, `pair`, or `record` written alone, not applied to anything — says
nothing checkable by itself and degrades *that field only* to `any`. A
*compound form* whose head the checker does not recognize as `list`,
`array`, `pair`, `record`, `->`, or a registered generic also degrades to
`any`. This is per-field, not all-or-nothing: the rest of the record keeps
its checked types.

```lisp
lamedh -s '(progn (defrecord widget (x list) (y (mystery-compound int64)))
                   (list (see-type (quote widget-x)) (see-type (quote widget-y))))'
; => ((DECLARED (-> (WIDGET) ANY)) (DECLARED (-> (WIDGET) ANY)))
```

A bare *symbol* that is not one of those structural words, though, is
never degraded — it is a **nominal record or variant reference**, resolved
by name, whether or not that name has been defined yet. This is what makes
self- and mutually-recursive records (§4.14) work with no forward-`declare`
step: an unknown bare symbol becomes a forward-declared phantom brand
instead of a silent `any`, and a genuine typo surfaces as a type error at
the first place the field is actually used, rather than as quietly-dropped
checking:

```lisp
lamedh -s '(progn (defrecord widget (x bogus-type) (y (mystery-compound int64)))
                   (list (see-type (quote widget-x)) (see-type (quote widget-y))))'
; => ((DECLARED (-> (WIDGET) BOGUS-TYPE)) (DECLARED (-> (WIDGET) ANY)))
```

```lisp
lamedh -s '(progn (defrecord widget (x bogus-type) (y (mystery-compound int64)))
                   (check-type (+ 1 (widget-x (make-widget 1 2)))))'
; => "type error: in call to `MAKE-WIDGET`: cannot unify int64 with BOGUS-TYPE"
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

`:invariant` attaches a validity predicate over the record's own fields.
It is **enforced at construction** — `make-Name` refuses a violating
value — and also available as the generated judgment `validate-Name`:

```lisp
lamedh -s '(progn (defrecord acct (bal int64) (:invariant (>= bal 0)))
                   (list (validate-acct (make-acct 5))
                         (errorset (quote (make-acct -5)))))'
; => (T ())
```

Multiple expressions in `:invariant` are implicitly `and`ed together. The
invariant is arbitrary Lisp, checked dynamically (the validator's own
signature, `(-> (Name) bool)`, is declared in lockstep) — `defrecord` does
not try to prove your invariant, only to run it. Two roads bypass the
constructor door — `record-with` updates and `#S` reader literals — so
validate explicitly after either when the invariant matters:

```lisp
lamedh -s '(progn (defrecord acct (bal int64) (:invariant (>= bal 0)))
                   (validate-acct (record-with (make-acct 5) (quote bal) -1)))'
; => ()
```

`:derive` generates deterministic support code from the same field list.
Three targets exist:

| Target | Generates |
|---|---|
| `equality` | `Name-equal` |
| `printer` | `Name->plist` |
| `lens` | `Name->plist`, `plist->Name`, and a law `Name-lens-roundtrip` |

```lisp
lamedh -s '(progn (defrecord point (x int64) (y int64))
                   (derive point equality) (derive point printer)
                   (list (point-equal (make-point 1 2) (make-point 1 2))
                         (point-equal (make-point 1 2) (make-point 3 4))
                         (point->plist (make-point 1 2))))'
; => (T () ((X . 1) (Y . 2)))
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
so it participates in `see-type` and interface conformance (§4.15) for
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
- `(list T)`, `(array T)`, `(pair A B)` — `cons` produces whichever fits:
  a tail already known to be a non-list ground type makes a dotted pair
  (`(cons 'k 2)` is `(pair symbol int64)`, the alist-cell idiom, and
  `car`/`cdr` project it), while unknown or list tails take the
  list-cons view
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

## 4.11 Sum types: `defvariant` and `variant-case`

`defrecord` gives you a single, open-ended shape. Sometimes what you want
is the opposite: a *closed*, exhaustively-enumerable set of shapes — a
value that is a circle or a rectangle and nothing else, ever. That is
`defvariant`:

```lisp
(defvariant shape
  (circle (r int64))
  (rect   (w int64) (h int64)))
```

Each constructor — `circle`, `rect` — is an ordinary branded record under
the hood (one `#S`-printable `StructObj`, same representation `defrecord`
uses), but the constructor *function* is the bare name itself: `(circle
3)`, not `(make-circle 3)`. `shape` itself becomes a second, denotable name
in the type language — the checker-level union of every constructor brand.
The union predicate, `shape-p`, is true for a value built by any
constructor of the variant and false for everything else:

```lisp
lamedh -s '(progn (defvariant shape (circle (r int64)) (rect (w int64) (h int64)))
                   (list (circle 3) (shape-p (circle 3)) (shape-p (rect 1 1)) (shape-p 5)))'
; => (#S(CIRCLE 3) T T ())
```

Fields normalize exactly like `defrecord`'s, and a constructor with no
fields at all is legal and is called the same way it is defined, with zero
arguments — `(none)`, shown later in this section, is one.

`variant-case` dispatches on a value's constructor brand and binds its
fields positionally, clause by clause:

```lisp
lamedh -s "(progn (defvariant shape (circle (r int64)) (rect (w int64) (h int64)))
                   (defun area (s) (variant-case s (circle (r) (* 3 (* r r))) (rect (w h) (* w h))))
                   (list (area (circle 3)) (area (rect 2 4))))"
; => (27 8)
```

The distinguishing feature is exhaustiveness: unless the case has an
`else` clause, every constructor of the variant must be covered, or
`variant-case` errors and names exactly what is missing:

```lisp
lamedh -s "(progn (defvariant shape (circle (r int64)) (rect (w int64) (h int64)))
                   (variant-case (circle 3) (circle (r) r)))"
; => Error: variant-case over SHAPE is not exhaustive; missing: (RECT)
```

```lisp
lamedh -s "(progn (defvariant shape (circle (r int64)) (rect (w int64) (h int64)))
                   (variant-case (rect 1 2) (circle (r) r) (else 'other)))"
; => OTHER
```

This is a real safety property, not just a style nicety: add a new
constructor to a variant next month, and every unmarked `variant-case` over
it that lacks an `else` starts failing loudly at the call site instead of
silently falling through.

Absorption into the union is a checker fact, not just a runtime one, and
`variant-case` is a form the checker's static walker understands natively
(#350): the scrutinee unifies with the clause constructors' owning
variant, each clause binds its constructor's field types, and the clause
bodies join to one result. A function that consumes a variant through
`variant-case` gets its scheme **inferred** — no `declare-type!` needed —
and a value built by *any* of that variant's constructors — but only that
variant's constructors — type-checks against it:

```lisp
lamedh -s "(progn (defvariant shape (circle (r int64)) (rect (w int64) (h int64)))
                   (defun area (s) (variant-case s (circle (r) (* 3 (* r r))) (rect (w h) (* w h))))
                   (list (see-type 'area) (check-type (area (circle 3))) (check-type (area 5))))"
; => ((CHECKED (-> (SHAPE) INT64)) "int64"
;     "type error: in call to `AREA`: cannot unify int64 with SHAPE")
```

Parametric variants infer too — a consumer that never touches the payload
stays polymorphic, and one that does pins the parameter:

```lisp
lamedh -s "(progn (defun unwrap-or0 (o) (variant-case o (some (v) v) (none () 0)))
                   (see-type 'unwrap-or0))"
; => (CHECKED (-> ((OPTION INT64)) INT64))
```

A constructor from a *different* variant is not a shape, however many
fields it happens to line up with, and the checker names it by brand:

```lisp
lamedh -s "(progn (defvariant shape (circle (r int64)) (rect (w int64) (h int64)))
                   (defun area (s) (variant-case s (circle (r) (* 3 (* r r))) (rect (w h) (* w h))))
                   (declare-type! 'area '(-> (shape) int64))
                   (defvariant coin-flip (heads) (tails))
                   (check-type (area (heads))))"
; => "type error: in call to `AREA`: HEADS is not a constructor of variant SHAPE"
```

Nullary constructors are ordinary values, print and round-trip like any
other record, and compare `equal` the same way:

```lisp
lamedh -s '(list (none) (equal (none) (none)))'
; => (#S(NONE) T)
```

`none` is `Option`'s empty case — `Option` and `Result` are covered next.

`match` (§8, `lib/23-match.lisp`) also destructures constructors directly,
with a record pattern alongside its existing pattern language: `#S(BRAND
pat...)` matches any value whose brand is `BRAND`, binding each field
pattern against the corresponding field value. This is often nicer than
`variant-case` when you are matching one shape among several other
conditions rather than dispatching a whole function on the brand:

```lisp
lamedh -s "(progn (defvariant shape (circle (r int64)) (rect (w int64) (h int64)))
                   (match (rect 2 4) (#S(CIRCLE ?r) (list 'circ ?r)) (#S(RECT ?w ?h) (list 'rect ?w ?h))))"
; => (RECT 2 4)
```

`#S` patterns nest like any other pattern, so matching inside a matched
record's field works the same as matching inside a list:

```lisp
lamedh -s "(progn (defvariant shape (circle (r int64)) (rect (w int64) (h int64)))
                   (defvariant wrap (boxed (inner any)))
                   (match (boxed (circle 9)) (#S(BOXED #S(CIRCLE ?r)) ?r)))"
; => 9
```

## 4.12 `Option` and `Result`

The stdlib (`lib/25-variants.lisp`) defines `Option` and `Result` as
ordinary variants, built with nothing `defvariant` doesn't already give
you:

```lisp
(defvariant (option a)
  (some (value a))
  (none))

(defvariant (result a e)
  (ok (value a))
  (err (message e)))
```

(The `(option a)` head is the parametric form covered fully in §4.13; read
it here as "a variant with one type parameter.") `some`, `none`, `ok`, and
`err` are the ordinary bare-name constructors from §4.11, and the usual
inspection/combinator helpers come with them:

| Function | Behavior |
|---|---|
| `option-of` | `()` becomes `(none)`; anything else becomes `(some x)` |
| `unwrap` | the value inside `(some v)`; errors on `(none)` |
| `unwrap-or` | the value inside `(some v)`, or a supplied default |
| `option-map` | apply a function inside `(some v)`; `(none)` passes through |
| `option-then` | monadic bind: `(some v)` -> `(funcall f v)` (itself an option) |
| `unwrap-result` | the value inside `(ok v)`; errors with the message on `(err m)` |
| `result-or` | the value inside `(ok v)`, or a supplied default |
| `result-map` | apply a function inside `(ok v)`; `(err m)` passes through |
| `result-then` | monadic bind over `Result` |
| `try-call` | call a function, capturing a signaled condition as `(err message)` |

```lisp
lamedh -s '(list (unwrap-or (some 5) 0) (unwrap-or (none) 0))'
; => (5 0)
```

```lisp
lamedh -s "(list (option-map #'1+ (some 4)) (option-of ()) (option-of 3))"
; => (#S(SOME 5) #S(NONE) #S(SOME 3))
```

```lisp
lamedh -s '(list (result-or (ok 1) 99) (result-or (err "bad") 99))'
; => (1 99)
```

```lisp
lamedh -s '(unwrap-result (result-then (lambda (v) (ok (* v 10))) (ok 2)))'
; => 20
```

`try-call` is the bridge from the condition system (§6) into `Result`: call
`car` on a non-list and the condition it signals becomes an `(err ...)`
value instead of unwinding the stack; call it on something that actually
works and you get `(ok result)`.

```lisp
lamedh -s "(list (err-p (try-call #'car 5)) (try-call #'car (list 1 2)))"
; => (T #S(OK 1))
```

Typing is precise through every one of these helpers — payload and default
have to agree, the same way any other row-typed helper chain does (§4.9):

```lisp
lamedh -s '(check-type (+ 1 (unwrap-or (some 5) 0)))'
; => "int64"
```

```lisp
lamedh -s '(check-type (unwrap-or (some "s") 0))'
; => "type error: in call to `UNWRAP-OR`: cannot unify int64 with string"
```

`(some "s")` carries a `string` payload; `0` is an `int64` default;
`unwrap-or`'s declared scheme, `(forall (a) (-> ((option a) a) a))`,
demands they be the *same* `a` — mixing them is a genuine static error, not
a runtime surprise waiting to happen.

## 4.13 HM generics: parametric records and variants

`defrecord` and `defvariant` both accept a *parametric* head — a list
instead of a bare name, with type-parameter symbols after it — for a
record or variant whose field types mention type variables:

```lisp
lamedh -s "(progn (defrecord (duo a b) (first a) (second b))
                   (list (see-type 'make-duo) (see-type 'duo-first) (make-duo 1 \"s\") (duo-first (make-duo 1 \"s\"))))"
; => ((DECLARED (FORALL (A B) (-> (A B) (DUO A B)))) (DECLARED (FORALL (A B) (-> ((DUO A B)) A))) #S(DUO 1 "s") 1)
```

This is real type application in the Hindley-Milner sense, not row
polymorphism wearing a different hat (§4.3's `record-ref` already gave you
"any record with this field"; this is "a `duo` of exactly these two
types"). `make-duo`'s scheme quantifies over `a` and `b` and instantiates
them *fresh, per call site* — two calls to `make-duo` in the same program
can specialize to entirely different types without interfering:

```lisp
lamedh -s "(progn (defrecord (duo a b) (first a) (second b))
                   (list (check-type (+ 1 (duo-first (make-duo 1 \"x\"))))
                         (check-type (+ 1 (duo-first (make-duo \"x\" 2))))))"
; => ("int64" "type error: `+` operands disagree: Int64 vs Str")
```

Argument unification is nominal by *name*: `(duo int64 string)` and `(duo
string int64)` are different applications of the same generic, and a value
built as one does not flow where the other is demanded. `Option` and
`Result` (§4.12) are declared exactly this way — `some`'s real scheme is:

```lisp
lamedh -s "(see-type 'some)"
; => (DECLARED (FORALL (A) (-> (A) (SOME A))))
```

which is why the typing shown in §4.12 is exact rather than approximate: a
`(some 5)` really is a `(some int64)`, distinct from a `(some string)`, and
`unwrap-or`'s own `(forall (a) (-> ((option a) a) a))` scheme is what forces
its two arguments into agreement. Two different generics never unify with
each other, even when one happens to be a record and the other a variant
constructor:

```lisp
lamedh -s "(progn (defrecord (duo a b) (first a) (second b))
                   (check-type (unwrap-or (make-duo 1 2) 0)))"
; => "type error: in call to `UNWRAP-OR`: cannot unify (duo ?1 ?2) with (option ?0)"
```

A **bare** generic name, with no type arguments applied at all, means the
all-`any` application — the gradual reading, and exactly what pre-0.3 code
that used `option` before it was parametric still means today:

```lisp
lamedh -s "(progn (defrecord cell (v int64) (link option))
                   (list (see-type 'cell-link) (cell-v (make-cell 1 (some (make-cell 2 (none)))))))"
; => ((DECLARED (-> (CELL) (OPTION ANY))) 1)
```

Generic applications are also row-subsumable: a row-polymorphic function
that only names a subset of an instantiated generic's fields (§4.3) sees
the instantiated field type at the call site, not `any`:

```lisp
lamedh -s "(progn (defrecord (duo a b) (first a) (second b))
                   (defun the-first (x) (record-ref x 'first))
                   (check-type (+ 1 (the-first (make-duo 4 \"s\")))))"
; => "int64"
```

Sibling constructors of the same variant meet at the variant's own
instantiated application, the same way an `if`'s two branches meet at a
common type anywhere else in the checker:

```lisp
lamedh -s "(list (check-type (if t (some 5) (none)))
                 (check-type (+ 1 (unwrap-or (if nil (some 5) (none)) 0))))"
; => ("(some int64)" "int64")
```

Runtime is completely erased: a generic record or variant constructor is
the same `#S`-printed `StructObj` you would get from a non-generic
`defrecord`, with no type tag or dictionary passed at runtime. This has a
direct consequence for the compiled tier (§4.5): generics are checker-only
and are **never** native-compiled, regardless of how scalar their fields
turn out to be at any one instantiation — a `(duo int64 int64)` still runs
on the dynamic `StructObj` path.

One more restriction, and it exists precisely because generics are proper
type application: the names the checker already uses for its own type
constructors — `pair`, `list`, `array`, `record`, `->`, `forall`, and the
scalar names — cannot name a *parametric* record or variant:

```lisp
lamedh -s '(defrecord (pair a b) (first a) (second b))'
; => Error: `pair` is a built-in type name and cannot name a record or variant
```

```lisp
lamedh -s '(defvariant (list a) (lnil) (lcons (h a)))'
; => Error: `list` is a built-in type name and cannot name a record or variant
```

This check currently only fires on the *parametric* spelling — a
non-generic `(defrecord pair (x int64))` is accepted, since it never
reaches the type-constructor namespace that generic applications occupy —
but shadowing a built-in type name is exactly as confusing at the call
site either way, so avoid it regardless of which form you use.

## 4.14 Recursive records

A field's declared type can name its own record, or another record that
eventually names it back. Both self- and mutual recursion resolve
*nominally* — by brand name — rather than degrading to `any`, which is what
makes §4.4's "bare symbol = nominal reference" rule matter:

```lisp
lamedh -s "(progn (defrecord node (val int64) (next node))
                   (list (see-type 'node-next) (see-type 'make-node)))"
; => ((DECLARED (-> (NODE) NODE)) (DECLARED (-> (INT64 NODE) NODE)))
```

Access through the recursive field stays fully checked at every depth, and
so does a row read through it via `record-ref`:

```lisp
lamedh -s "(progn (defrecord node (val int64) (next node))
                   (def chain (make-node 1 (make-node 2 (make-node 3 'end))))
                   (list (node-val (node-next (node-next chain)))
                         (check-type (node-val (node-next chain)))
                         (check-type (record-ref (node-next chain) 'val))))"
; => (3 "int64" "int64")
```

(`chain`'s final `next` is the bare symbol `'end`, not another `node` —
`defrecord`'s constructor does not itself enforce the field's declared type
at construction time; that is what the checker's static verdicts above are
for, and what `check-type` at a real call site would catch.)

Mutual recursion works the same way through a forward reference — `tree`
names `branch` before `branch` is defined, via the two-phase registration
described in §4.4, and the later `defrecord branch` completes it:

```lisp
lamedh -s "(progn (defrecord tree (left branch) (v int64))
                   (defrecord branch (t1 tree))
                   (list (see-type 'tree-left) (see-type 'branch-t1)))"
; => ((DECLARED (-> (TREE) BRANCH)) (DECLARED (-> (BRANCH) TREE)))
```

An unconditionally self-referential field (`next node`, always another
`node`) has no way to terminate — you can describe an infinite type but
never build a finite value with a base case at that field. The blessed
idiom is to terminate with `Option` (§4.12), whether by the pre-0.3 gradual
bare name:

```lisp
lamedh -s "(progn (defrecord node (val int64) (next option))
                   (defun sum-nodes (n)
                     (+ (node-val n)
                        (variant-case (node-next n)
                          (some (rest) (sum-nodes rest))
                          (none () 0))))
                   (sum-nodes (make-node 1 (some (make-node 2 (some (make-node 3 (none))))))))"
; => 6
```

```lisp
lamedh -s "(progn (defrecord node (val int64) (next option))
                   (check-type (make-node 1 (none))))"
; => "NODE"
```

or, fully checked end to end, with a parametric `node` and `Option`
instantiated to `(node a)` itself:

```lisp
lamedh -s "(progn (defrecord (node a) (val a) (next (option (node a))))
                   (see-type 'node-next))"
; => (DECLARED (FORALL (A) (-> ((NODE A)) (OPTION (NODE A)))))
```

```lisp
lamedh -s "(progn (defrecord (node a) (val a) (next (option (node a))))
                   (defun sum-nodes (n)
                     (+ (node-val n)
                        (variant-case (node-next n)
                          (some (r) (sum-nodes r))
                          (none () 0))))
                   (sum-nodes (make-node 1 (some (make-node 2 (none))))))"
; => 3
```

A well-formed chain checks precisely, `a` instantiated to `int64` the whole
way down:

```lisp
lamedh -s "(progn (defrecord (node a) (val a) (next (option (node a))))
                   (check-type (make-node 1 (some (make-node 2 (none))))))"
; => "(node int64)"
```

and a wrong payload anywhere in the chain — here, a `duo` where a `node`
was expected — is a genuine static error, not a value the checker waves
through because the field is "just a link":

```lisp
lamedh -s "(progn (defrecord (node a) (val a) (next (option (node a))))
                   (defrecord (duo a b) (first a) (second b))
                   (check-type (make-node 1 (some (make-duo 1 2)))))"
; => "type error: in call to `MAKE-NODE`: cannot unify (duo ?2 ?3) with (node ?0)"
```

Foreign brands are rejected the gradual, bare-`option` way too — a
different variant's constructor does not unify with `(option any)`:

```lisp
lamedh -s "(progn (defrecord node (val int64) (next option))
                   (defvariant color (red) (blue))
                   (check-type (make-node 1 (red))))"
; => "type error: in call to `MAKE-NODE`: cannot unify RED with (option any)"
```

Because a bare, unrecognized field-type symbol is nominal rather than
gradual (§4.4), a misspelled type name — `intt64` where `int64` was meant —
does not silently become `any`. It becomes a phantom brand named after the
typo, and the mistake surfaces the first time the field is actually used
somewhere the real type is demanded:

```lisp
lamedh -s "(progn (defrecord pt (x intt64) (y int64))
                   (list (see-type 'pt-x) (check-type (+ 1 (pt-x (make-pt 1 2))))))"
; => ((DECLARED (-> (PT) INTT64)) "type error: in call to `MAKE-PT`: cannot unify int64 with INTT64")
```

`pt-x`'s own declared scheme is not wrong — `pt` really does have an
`INTT64`-typed field, exactly as written. The error appears where it
belongs: at the point that tries to use a `pt-x` result as an `int64`.

## 4.15 Contracts: `implements!`, `implements-p`

There is exactly ONE dispatch system: protocols (§4.16). What people
reach for interfaces for — "does this type honor the whole contract?" —
is a conformance question over protocol instances, and it has two
answers: `implements-p` (a predicate) and `implements!` (assert now,
error loudly). A contract is nothing more than a set of protocol names.

```lisp
lamedh -s "(progn
  (defprotocol greet \"voice\")
  (defprotocol damage \"hp reduction\")
  (defrecord goblin (name string) (hp int64))
  (definstance greet ((self goblin)) string
    (concat (goblin-name self) \" snarls.\"))
  (definstance damage ((self goblin) (n int64)) goblin
    (record-with self 'hp (- (goblin-hp self) n)))
  (implements! 'goblin 'greet 'damage))"
; => ((GREET . INSTANCE) (DAMAGE . INSTANCE))
```

Each protocol in the report is graded:

- `INSTANCE` — a typed instance is registered for the brand and its
  implementation carries no checker error: a real guarantee.
- `MISMATCH` — an instance exists but its implementation's checker
  verdict is a type error.
- `MISSING` — no instance for this brand.

A `MISSING` or `MISMATCH` grade fails `implements!` with the offending
pairs named:

```lisp
lamedh -s "(progn (defprotocol render \"draw\")
                   (defrecord bag (item any))
                   (implements! 'bag 'render))"
; => Error: implements!: BAG fails ((RENDER . MISSING))
```

(Before 0.3 this niche was served by a second system — `definterface`
method sets with `TYPE-OP` naming-convention dispatch via `method`. Two
dispatch mechanisms was one too many; `defprotocol` is the survivor, and
the old forms are gone.)

`examples/npcs.lisp` and `examples/oo-patterns.lisp` in the repository work
through this whole stack together — shared row-polymorphic behavior for
every NPC kind, per-kind voices as protocol instances verified by
`implements!`, and classic Gang-of-Four patterns (Strategy, Composite,
Decorator, Observer, State) reduced to a handful of lines once the row
type system says "any record with this field" directly instead of forcing
a class hierarchy to say it indirectly. Run either with `cargo run -- -i
examples/npcs.lisp`, or check the whole file's checker verdicts at once
with `(check-file! "examples/npcs.lisp")` under the `READ-FS` capability.

## 4.16 Typed protocols: one name, many typed instances

`length` works on lists, strings, arrays, and hash tables — and on *your*
types, because it is a **protocol**: a name with many typed instances,
selected by inference at check time and by the value's kind at runtime.

```lisp
lamedh -s '(progn (defrecord playlist (songs (list string)))
                   (definstance length ((p playlist)) int64
                     (length (playlist-songs p)))
                   (list (length (make-playlist (list "a" "b")))
                         (check-type (length (make-playlist (list "a"))))))'
; => (2 "int64")
```

Three resolutions from one definition:

- **Checker**: a call site whose first argument's type is known selects
  the matching instance and gets its precise scheme. A known type with
  *no* instance is a static error — `(check-type (length 3.5))` says
  `no `LENGTH` instance for float64`. And when every instance agrees on
  one ground result type, even a gradual call site derives it:
  `(defun n-items (x) (length x))` checks as
  `(forall (a) (-> (a) int64))`.
- **Runtime**: the protocol name is a dispatcher keyed on the value's
  kind — record brands (and their variants), then list/string/array/hash/
  scalars.
- **Compiler**: each instance body is an ordinary function under a hidden
  name, so eligible instances compile natively through the one-door
  pipeline.

Define your own with `defprotocol` + `definstance`; `defprotocol` captures
any prior binding of the name as the fallback instance, which is how the
kernel's `length` kept everything it already handled. The shipped
protocols are `length`, `map`, `for-each`, `filter`, `ref`, `put!`, and `copy`.
Argument order follows Common Lisp: higher-order functions take the
FUNCTION first (`(map fn coll)`, like `mapcar`), access operations take
the collection first (`(ref coll k)`, like `aref`/`elt`). A protocol
declares which argument position it dispatches on — `(:dispatch 1)` for
the fn-first pair — and the default is 0:

```lisp
lamedh -s '(map (lambda (x) (* x x)) (list 1 2 3))'
; => (1 4 9)
```

`map` is kind-preserving (a list maps to a list, an array to an array, a
string to a string — `(map #'string-upcase "abc")` is `"ABC"`);
`for-each` visits for effect, and its hash instance receives
`(fn key value)`. The Lisp 1.5 appendix's tails-visiting `map` lives on as
`map-tails`.

The access protocols round out the vocabulary: `(ref coll k)` reads at
an index, key, or record field — strictly, so an absent index is an
*error* and every instance carries an honest result type (the lenient
nil-on-miss reads keep their old names: `gethash`, `nth`, `elt`);
`(put! coll k v)` writes the mutable containers (arrays, hash tables)
and returns `v`; `(copy x)` produces a fresh list, array, or hash
table. Underneath, the monomorphic per-type implementations remain as
the substrate the instances dispatch to and compile through, and each
carries a trailing `*` marking it visibly outside the normative
vocabulary: `string-length*`, `array-length*`, `array-copy*`,
`copy-hash*`, and so on. Write against the protocol names; call a
starred form when you've already committed to the type and want the
direct monomorphic call (the hot path, until the compiler splices
instances at call sites). `fetch`/`store` and the lenient reads
(`gethash`, `nth`, `elt`) are unstarred — not substrate duplicates but
distinct contracts that kept their names.
