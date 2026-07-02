# Worked Example: Polymorphic NPCs

`examples/game/npcs.lisp` is a small, runnable answer to the question "what
does polymorphism look like in Lamedh now?" — after the typeclass dictionary
layer was removed, condensation shipped as the definition substrate, and the
checker learned rows. There is no class hierarchy, no dictionary, and no
dispatch table anywhere in it. Run it with:

```sh
cargo run -- -i examples/game/npcs.lisp
```

Every transcript line below is what that command prints.
`tests/test_npc_example.rs` pins the behavior.

## The cast

Two kinds of NPC, one seed each. The shared fields come first and in the same
order — `(name string) (hp int64)` — then each kind's own fields:

```lisp
(defconcept goblin
  (:fields ((name string) (hp int64) (ferocity int64)))
  (:invariant (>= hp 0))
  (:derive equality))

(defconcept merchant
  (:fields ((name string) (hp int64) (gold int64)))
  (:invariant (and (>= hp 0) (>= gold 0)))
  (:derive equality))
```

Each seed generates its constructor, predicate, accessors, validator, and
equality (`make-goblin`, `goblin-p`, `goblin-hp`, `validate-goblin`,
`goblin-equal`, ...), records the full provenance trace on the symbol's
plist, and — because every field type maps into the checker's type language —
installs declared row schemes for the generated operations:

```lisp
(see-type 'goblin-hp)
; => (DECLARED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) INT64)))
```

Read that as: `goblin-hp` accepts *any record with an int64 `hp`*, whatever
else it carries. The concept's name is nominal spelling; the checker's view
is structural. That one fact powers everything below.

## The shared-but-specialized method: `greet`

The method set is declared once, Go-style — an interface is signatures, and
a method of `TYPE` for op `OP` is the ordinary function `TYPE-OP`:

```lisp
(definterface npc
  (:ops ((name  (-> (self) string))
         (hp    (-> (self) int64))
         (greet (-> (self) string)))))
```

`name` and `hp` are satisfied for free: the accessors `goblin-name`,
`merchant-hp`, ... that `defconcept` generated already follow the `TYPE-OP`
naming convention. Generated code and hand-written code are method-eligible
on exactly the same terms.

`greet` is the op each kind specializes — same name, one plain function per
kind, nothing registered anywhere:

```lisp
(defun goblin-greet (self)
  (concat "Grr. " (goblin-name self) " waves a rusty knife."))

(defun merchant-greet (self)
  (concat "Welcome! " (merchant-name self) " opens a pack of "
          (princ-to-string (merchant-gold self)) " gold worth of wares."))

(implements! 'goblin 'npc)
(implements! 'merchant 'npc)
```

`implements!` is the Rust-flavored explicit assertion over the Go-flavored
structural check: verify now, record the claim on both symbols' plists, error
loudly if an op is missing or contradicts its declared signature.

Calling a specialized method is `method` — one deterministic name
computation from the value's concept tag (`(goblin ...)` → `GOBLIN-GREET`),
not a table lookup:

```lisp
(method 'greet (make-goblin "Snag" 7 3))
; => "Grr. Snag waves a rusty knife."
(method 'greet (make-merchant "Oren" 12 250))
; => "Welcome! Oren opens a pack of 250 gold worth of wares."
```

Because the method is an ordinary function, it realizes, type-checks,
`edit!`s, and traces like any other definition. There is nothing between you
and it.

## The shared method, flavor 1: row-typed (`wounded-p`)

One definition that works on every kind — and the checker *proves* it. Give
the shared projection a shared name; the row scheme is inferred, not
declared:

```lisp
(defun npc-hp (n) (goblin-hp n))
(defun wounded-p (n) (< (npc-hp n) 3))

(see-type 'npc-hp)
; => (CHECKED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) INT64)))

(wounded-p (make-goblin "Snag" 2 3))      ; => T
(wounded-p (make-merchant "Oren" 12 250)) ; => NIL
```

`npc-hp` is spelled through `goblin-hp`, but its inferred type is "any record
with an int64 `hp`" — goblins, merchants, training dummies, and every future
kind that keeps the shared fields. No annotation, no instance declaration,
no subscription: carrying the field *is* the membership.

And misuse does not wait for runtime. A merchant has gold; a goblin does
not, and the goblin's constructor returns a *closed* record:

```lisp
(defun rob () (merchant-gold (make-goblin "Snag" 7 3)))
(see-type 'rob)
; => (TYPE-ERROR "in call to `MERCHANT-GOLD`: closed record lacks field(s) gold")
```

## The shared method, flavor 2: late-bound (`introduce`)

The other kind of sharing: a template written once whose *interior* is
specialized per kind. `introduce` composes the free accessors with the
specialized `greet`, all through `method`:

```lisp
(defun introduce (n)
  (concat (method 'name n)
          " [" (princ-to-string (method 'hp n)) " hp]: "
          (method 'greet n)))

(introduce (make-goblin "Snag" 2 3))
; => "Snag [2 hp]: Grr. Snag waves a rusty knife."
(introduce (make-merchant "Oren" 12 250))
; => "Oren [12 hp]: Welcome! Oren opens a pack of 250 gold worth of wares."
```

The two flavors trade against each other, and the honesty vocabulary says
which is which:

| | row-typed (`wounded-p`) | late-bound (`introduce`) |
|---|---|---|
| binding | early: the accessor is the function | late: name computed from the value's tag |
| checker verdict | `CHECKED`, an informative row scheme | `VACUOUS` — `method` is opaque to the checker |
| extends to new kinds | yes, if they carry the fields | yes, if they implement the ops |
| can call specialized code | no — fields only | yes — that is the point |

Use the row-typed form for anything expressible over shared *fields*; reach
for `method` only where the behavior genuinely differs per kind.

## Failing the contract, honestly

A training dummy has the shared fields but nobody taught it to speak:

```lisp
(defconcept training-dummy
  (:fields ((name string) (hp int64))))

(implements? 'training-dummy 'npc)
; => (() (NAME CONFORMS TRAINING-DUMMY-NAME (FORALL (A) ...))
;        (HP CONFORMS TRAINING-DUMMY-HP (FORALL (A) ...))
;        (GREET MISSING TRAINING-DUMMY-GREET))
```

`name` and `hp` `CONFORM` — the dummy carries the fields, so their generated
accessors' row schemes subsume the op signatures — but `greet` is `MISSING`,
the overall check fails, and `implements!` would refuse the claim. Meanwhile
`wounded-p` still works on a dummy — it only needs the field. The two
membership tests are different questions and stay separate.

Invariants travel with the seed, as executable validators:

```lisp
(validate-goblin (make-goblin "Snag" 7 3))  ; => T
(validate-goblin (make-goblin "Snag" -1 3)) ; => NIL
```

## Why `greet` is `UNPROVEN` but `name` is `CONFORMS`

Conformance grades by asking the checker, not by trusting that a function
exists. `implements?` substitutes `self` with the type's *structural*
identity — a goblin's closed record `(record ((name string) (hp int64)
(ferocity int64)))` — and asks the kernel's own row unifier whether the
method's scheme subsumes the op signature (the `scheme-subsumes?` builtin).

- `name`/`hp` carry `DECLARED` row schemes (`(forall (a) (-> ((record
  ((hp int64)) a)) int64))`); the closed goblin record is an instance, so
  they `CONFORM` — a real, checker-backed guarantee.
- `greet` builds a string with `concat`, which defeats inference: its scheme
  is `(forall (a b) (-> ((record ((name string)) a)) b))`, whose result `b`
  is unconstrained. That is *vacuous* — the checker found no contradiction
  but proved nothing — so it grades `UNPROVEN`, honestly, rather than being
  waved through as conformant. Give `greet` a checkable body (or a `DECLARED`
  scheme) and it too would `CONFORM`.

This is the fix for what was a documented seam: an earlier version
substituted `self` with the concept *symbol* and reimplemented unification in
Lisp, so a record-typed verdict — the strongest evidence the checker can
produce — could never confirm, and every row method graded `UNPROVEN`. See
`docs/eval/position-deeper-unification.md` §2 for the analysis.

## Access is by name, so layout does not lie

A row concept's declared scheme is by *name* — `goblin-hp` accepts "any record
with an int64 `hp`". Its accessor honors that: it reads the `hp` field of
whatever branded record it is handed, resolving the slot from the value's own
brand (`condense-field-ref`), not from a baked-in offset. So two kinds that
share a field name at different positions cannot read each other's wrong slot —
the runtime agrees with the declared type for every layout:

```lisp
(defconcept armored (:fields ((armor int64) (hp int64))))  ; hp at slot 2
(defconcept slime   (:fields ((hp int64))))                ; hp at slot 1
(armored-hp (make-slime 9))   ; => 9  (slime's hp, read by name), not NIL
```

This was Seam C in `docs/eval/position-deeper-unification.md` — a soundness
gap where the by-name axiom outran a by-offset accessor. It is now closed;
non-row concepts (unmappable field types, no declared axiom) keep the simpler
positional accessor, since nothing at the type level over-promises for them.
