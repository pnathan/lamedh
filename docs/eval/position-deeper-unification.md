# One Thing, Three Identities — Is There a Deeper Unification Under Interfaces, Rows, and Condensation?

A position developed against the code on this branch (`lib/20-condensation.lisp`,
`lib/21-interfaces.lisp`, the row machinery in `src/jit/`), the worked example
that exercises all three layers at once (`examples/game/npcs.lisp`,
`docs/npc_polymorphism.md`), and four empirical probes reproduced below. It
follows the discipline `response-first-class-intent.md` established: read the
merge record first, admit only what lowers repair cost, gate speculative type
machinery behind the benchmark.

**Thesis in one line:** yes, there is a deeper unification, and the code has
almost said it out loud — *a concept value is a branded row*, and the three
layers are three identity representations of that one thing (provenance,
structure, behavior). The layers disagree only at the two places where they
answer the same question in different representations without converting.
The unification worth building is the conversion, not a merger of the
libraries — and the full theoretical collapse (interfaces as rows over
method space) should be named as the destination and then explicitly gated.

---

## 1. What the value already is

Look at the runtime representation `defconcept` chose:

```lisp
(make-goblin "Snag" 7 3)   ; => (GOBLIN "Snag" 7 3)
```

The `car` is a **brand** — nominal identity, what `condense-type-of` reads
and `method` computes names from. The `cdr` is a **row** — the fields in
declared order, what the accessors project and what the declared record
schemes describe to the checker. And the defining symbol carries the
**trace** — provenance identity, what condensation records and re-verifies.

So the three layers are not three rival theories of type. They are three
identities of one value:

| identity | representation | owner | question it answers |
|---|---|---|---|
| provenance | seed + trace on the plist | condensation | *where did this come from; is it still what the seed says?* |
| structural | `(record fields row)` in the checker | rows | *what can this safely flow into?* |
| behavioral | the `TYPE-OP` function namespace | interfaces | *what can this do?* |

Some unification is already real: the honesty vocabulary is shared end to
end (`condense-classify` grades checker verdicts; `iface-op-status` reuses
`condense-vacuous-p` and folds the same grades into
CONFORMS/UNPROVEN/MISMATCH/MISSING); generated accessors are
method-eligible by the naming convention with no adapter; and the brand that
`method` dispatches on is the same fact the constructor's closed record
states statically. The seams are where a layer answers another layer's
question in its own representation and the conversion is missing.

## 2. Seam A: conformance interrogates the wrong type language — and inverts the incentive

> **Status: fixed on this branch.** The two changes below are implemented —
> `scheme-subsumes?` is a kernel builtin over the checker's row unifier
> (`src/jit/registry.rs`, `src/evaluator/introspection.rs`), and
> `iface-op-status` now substitutes `self` with the concept's record type and
> asks it (`lib/21-interfaces.lisp`). `goblin-power`, the derived
> `invoice-equal`, and the `name`/`hp` accessors of `examples/game/npcs.lisp`
> now grade `CONFORMS`; only genuinely opaque methods (`greet`) stay
> `UNPROVEN`. The analysis below is retained as the record of *why*.

`implements?` substitutes `self` with the concept **symbol** and unifies
`(-> (goblin) int64)` against the verdict. But for a row concept the verdict
lives in the **record** language. The probe:

```lisp
(defconcept goblin (:fields ((name string) (hp int64) (ferocity int64))))
(defun goblin-power (self) (+ (goblin-hp self) (goblin-ferocity self)))
(see-type 'goblin-power)
; => (CHECKED (FORALL (A) (-> ((RECORD ((FEROCITY INT64) (HP INT64)) A)) INT64)))

(definterface fighter (:ops ((power (-> (self) int64)))))
(implements? 'goblin 'fighter)
; => (() (POWER MISMATCH GOBLIN-POWER (FORALL (A) (-> ((RECORD ...)) INT64))))
```

`goblin-power` carries the strongest evidence the system can produce — an
informative `CHECKED` row scheme that *proves exactly what the declared
signature means* — and it grades **MISMATCH**, failing `implements!`.
Meanwhile a method whose body defeats inference (a `concat` call, say) grades
UNPROVEN and *passes*. The better the checker evidence, the worse the
conformance grade. `DECLARED` accessor schemes hit the same wall a grade
softer: they fall to the else-branch and grade UNPROVEN (the "documented
seam" of the row commit), so in `examples/game/npcs.lisp` every op of a
fully-derived row concept reports UNPROVEN despite the checker holding
proofs for two of the three.

The cause is that `iface-unify` is a toy: one-sided, structural,
equal-length — it cannot absorb a row tail, so it cannot recognize that
`(record ((hp int64)) a)` accepts a goblin. The fix is the conversion the
thesis calls for, and both halves already exist:

1. **Substitute `self` with the concept's structural identity, not its
   name.** The closed record is sitting on the plist
   (`"condense.fields"`); `iface-substitute-self` should map
   `self ↦ (record ((name string) (hp int64) (ferocity int64)))` when the
   type is a row concept, and keep the symbol for ground types (`int64-bump`
   works today and must keep working).
2. **Ask the kernel's unifier, not a Lisp reimplementation.** The kernel
   already has row unification, instantiation, and zonking
   (`src/jit/infer.rs`). One small query builtin — `scheme-subsumes?`,
   taking a ground wanted type and a scheme, returning T/NIL — replaces
   `iface-unify` entirely. This respects the CLAUDE.md kernel rule: the
   kernel gains one *query*, the conformance *policy* stays in Lisp.

With that, `CONFORMS` became a checker verdict rather than a Lisp-side
approximation of one, `DECLARED` row schemes are confirming evidence (they are
axioms, trusted at call sites), and the inversion is gone. The one wrinkle the
implementation had to keep: a *vacuous* `CHECKED` scheme (result variable no
argument constrains — the `greet`/`concat` case) is still gated to `UNPROVEN`
*before* subsumption, because instantiating it would let its free result
variable unify with anything and falsely confirm. Honesty is preserved: the
method exists, but the checker proved nothing about it. This was a correctness
fix to an existing question, not new type machinery, so it did not need the
benchmark gate.

## 3. Seam B: interfaces are not condensation citizens, so claims can rot

`condensation_library.md` is explicit: *"Higher-level forms should build on
`condense-put`/`condense-get`/`condense-record!` instead of inventing private
conventions."* `definterface` invents `"interface.*"` keys, and
`interface-trace` is a parallel, poorer `condense-trace`.

The cost is not aesthetic. Condensation's whole staleness apparatus —
fingerprints, `condense-stale`, `condense-recheck!` — exists so the trace
never vouches for code that drifted. But `implements!` records its claim
**once** and nothing ever re-checks it: redefine `merchant-greet` to return
an integer and `(getp 'merchant "interface.implements")` still says
`(NPC)`. That is precisely the metadata-rot failure mode the fingerprint
machinery was built to kill, reintroduced one file later.

The unification is mechanical, pure Lisp, tens of lines:

- `definterface` records through `condense-record!` (kind `interface`,
  source, ops as the generated payload), making `condense-trace` the one
  read-path for interfaces too;
- `implements!` fingerprints each conforming method (same
  `see-source`-snapshot trick) and stores the graded report;
- `condense-recheck!` on a type re-runs `implements?` for every recorded
  claim and flags drift — a stale conformance claim joins the same `stale`
  entry hand-edited generated code already does.

A recorded `implements!` then has the same epistemic status as a `deflaw`:
an executable, re-verifiable contract with provenance — not a one-time
stamp.

## 4. Seam C: the axiom and the implementation disagree about what a field is

The row commit's honesty note says `DECLARED` schemes are "generated in
lockstep with the implementation." There is a hole in the lockstep, verified:

```lisp
(defconcept armored (:fields ((armor int64) (hp int64))))
(defconcept slime   (:fields ((hp int64))))
(defun probe () (armored-hp (make-slime 9)))
(see-type 'probe)   ; => (CHECKED (-> () INT64))   -- the checker is satisfied
(probe)             ; => ()                        -- nil, not an int64
```

The declaration speaks **by name** — any record with an int64 `hp` — but the
accessor executes **by position** (`armored-hp` is `(nth 2 self)`, and
slime's `hp` sits at `nth 1`). Two concepts that share a field at different
offsets unify statically and misread each other dynamically. The axiom
promises more than the implementation delivers, which is exactly the
"context-compression device that can lie" failure mode the intent response
ranked worst-in-class.

`examples/game/npcs.lisp` survives by convention — shared fields first, same
order — and the convention is stated in the file. But a convention the
system cannot see is a trap for the next author. Options, ranked:

1. **Name-directed access on the dynamic path** (recommended). The
   interpreted accessor looks the field's offset up from the *value's own
   brand*: read `(car self)`'s recorded field list, then `nth`. Rows never
   reach the native tier anyway (`is_compileable` rejects records), so this
   costs a lookup only where the code was already dynamic — and it makes the
   axiom true by construction, for every layout. Soundness fixes have
   precedent for merging without the benchmark (the #202 AND/OR fix).
2. **Brand the constructor's record** (make accessors nominal-only). Sound,
   but it deletes cross-kind row reuse — the payoff feature the row commit
   exists for. Reject.
3. **Enforce the convention mechanically**: `defconcept` warns when a field
   name it shares with an existing row concept lands at a different
   position. Cheapest, but it turns a semantic guarantee into a lint; take
   it only as a stopgap if (1) measures too slow.

## 5. The deeper unification, named — and gated

Seams A and B are conversions between existing identities. Seam C is the
observation that goes further. In the NPC example these two lines state the
same fact in two languages:

```lisp
(hp int64)              ; a row field, in defconcept
(hp (-> (self) int64))  ; an interface op, in definterface
```

A row **is** an interface whose ops are all projections. An interface **is**
a row over the type's method namespace — a record of functions keyed by op
name. That object, reified as a runtime value, is a typeclass dictionary;
kept as a naming convention, it is Go's method set; typed, it is a record
type in method space. Lamedh's collapse removed the reified dictionary
(right, per the merge record: dispatch indirection raised the
fault-localization span) and kept the other two views. The full unification
would type the method namespace with the row machinery the checker already
has: an interface becomes a type — `(record ((greet (-> (self) string))) m)`
over method space — conformance becomes row unification, `implements?`
becomes a checker query, and functions could take "anything that is an npc"
as a *checked* parameter, closing the `VACUOUS` hole on `method`-using
templates like `introduce`.

It is elegant, it is one mechanism instead of three, and it is exactly the
kind of constrained-type program the eval consensus froze: a checker-wide
investment whose repair-cost payoff is conjecture. The discipline holds. Name
it as the destination so the small steps stay collinear with it; build none
of it until the repair benchmark exists and shows that typed conformance
localizes faults better than graded conformance does. Note that Seam A's fix
is a strict prefix of this destination (it already converts `self` to record
types and asks the kernel about subsumption), so nothing done now is thrown
away if the destination is ever funded.

## 6. The program

Ranked, admissible-first:

1. **Fix Seam A** — ✅ **done on this branch.** `self ↦` the concept's record
   type; `scheme-subsumes?` kernel query over the checker's row unifier;
   `iface-unify` deleted. Killed the evidence-inversion (MISMATCH on
   proven-correct methods) and made `CONFORMS` mean what it says. Correctness
   fix; no gate.
2. **Fix Seam C by name-directed dynamic access** — make the `DECLARED`
   axiom true for every layout, not just the shared-prefix convention.
   Soundness fix; no gate. (The next admissible step.)
3. **Make interfaces condensation citizens (Seam B)** — record through
   `condense-record!`, fingerprint `implements!` claims, re-grade in
   `condense-recheck!`. Pure Lisp; converts one-time stamps into
   re-verifiable contracts.
4. **Write the method-row destination into the design record** (this
   document) **and freeze it** behind the repair benchmark, alongside the
   rest of the frozen constrained-type program.

The one-line summary the code was already whispering: *brand for dispatch,
row for checking, trace for repair — one value, three identities, and the
work is to make the conversions between them exact.*
