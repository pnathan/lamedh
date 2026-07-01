# The Intent Layer

**Status: experimental spike.** This layer is a working sketch, not a settled
design. Read "Honest Limits" below before building on it; per the project's
own admission rule (`docs/eval/response-first-class-intent.md` §6), dispatch
indirection stays experimental until a repair benchmark shows it earns its
keep.

`lib/22-intent.lisp` makes intent a first-class, inspectable artifact: a named
**subject / means / outcome** triple over the typeclass dictionary layer
(`docs/typeclasses.md`). The design position behind it is
`docs/eval/response-first-class-intent.md`: change and intent become data with
an audit trail, not new evaluation semantics.

An intent names:

- **`:subject`** — *what* is acted on: a concept (`defconcept`), a ground
  builtin type (`int64`, `float64`, `char`, `string`, `symbol`, `list`), or
  any other symbol, which is treated as a type variable (a polymorphic
  intent).
- **`:means`** — *how*: a `(class op)` pair naming a typeclass operation.
  Intents sharing a means share dispatch through the same instance table.
- **`:outcome`** — *what must hold afterwards*: an optional contract form
  evaluated with `*it*` bound to the input subject and `*result*` to the
  result. The outcome is the intent's verifiability floor: a violated outcome
  is an error attributed to the intent by name.

```lisp
(defconcept invoice
  (:fields ((id int64) (amount int64)))
  (:invariant (>= amount 0)))

(derive invoice equality lens)          ; installs EQV and LENS instances

(defintent same-invoice
  (:subject invoice)
  (:means (eqv eqv)))

(defintent normalize
  (:subject invoice)
  (:means (lens view))
  (:outcome (consp *result*)))
```

## Two-Tier Dispatch

Dispatch mirrors the execution-tier architecture
(`docs/typed-region-design.md`): a dynamic path that always works, and a
static lowering that produces checkable code.

### Dynamic: `intent-apply`

```lisp
(intent-apply 'same-invoice a b)        ; ground subject: dispatch at INVOICE
(intent-apply 'normalize inv)           ; outcome checked after the call
```

For a polymorphic subject, the dispatch type is computed from the runtime
value — a concept-tagged list dispatches at its concept, other values at
their ground builtin type:

```lisp
(defintent same-thing (:subject a) (:means (eqv eqv)))
(intent-apply 'same-thing inv1 inv2)    ; resolves (EQV INVOICE)
(intent-apply 'same-thing rcpt1 rcpt2)  ; resolves (EQV RECEIPT)
```

Resolution is the explicit, shallow `typeclass-op` lookup — no implicit
search. A missing instance is a clear error.

### Static: `intent-realize`

A **ground** intent (subject is a concept or ground type) can be lowered to a
plain, dictionary-free function named after the intent:

```lisp
(intent-realize same-invoice)
(see-source 'same-invoice)
; => (LAMBDA (A B) (INVOICE-EQUAL A B))
```

The instance is resolved once, at realize time; the generated function calls
the concrete method directly, so the ordinary `check-type` surface sees a
plain function. Realization records the intent in the condensation registry
(`condense-record!` with kind `intent`), runs `condense-check-type` over the
generated symbols, and fingerprints them — a realized intent participates in
the same checked/dynamic-frontier accounting and staleness detection as any
condensed artifact.

The error discipline follows the typed-island rule "a missing ground instance
is a type error", enforced dynamically at the earliest possible moment:

- polymorphic subject → `intent-realize` is an error (stay on the dynamic
  path);
- ground subject with no instance → error at realize time, not a latent
  runtime failure;
- outcome contract → compiled into the realized function, enforced on every
  call.

This is deliberately the Lisp-layer half of the story. Checker-side
constrained schemes (`(forall (a) (=> ((EQV a)) ...))`) remain deferred per
the condensation eval (`eval/condensation-analysis`); when they are built,
`defintent`'s metadata — subject, means signature from the class `:ops`
declaration, outcome — is exactly the constraint information the checker
would consume.

## Sharing

Intents are registered and queryable by each leg of the triple:

```lisp
(intent-registry)                       ; all intents
(intents-for-subject 'invoice)
(intents-for-means '(eqv eqv))
(intents-for-outcome '(consp *result*))
```

Two intents sharing a means dispatch through the same instances; two intents
sharing a subject describe the same data; two intents sharing an outcome
satisfy the same contract. These are the seams along which condensed bundles
can later be grouped, compared, and reverified.

## Traces

```lisp
(intent-trace 'same-invoice)
; => ((KIND . INTENT) (SUBJECT . INVOICE) (MEANS EQV EQV) (OUTCOME)
;     (GROUND . T) (REALIZED . T) (SOURCE DEFINTENT ...))
```

After realization, `condense-trace` on the intent name shows the full
condensation record: source, expansion, generated symbols, checker status,
dynamic frontier, and staleness.

## Honest Limits

What this layer does **not** do, stated so the trace vocabulary cannot
oversell it:

- **Nothing here is statically typed.** Concepts are tagged lists; methods
  are dynamic functions. Nothing in this layer crosses the HM gate into the
  typed island. The `:ops` signatures in `deftypeclass` are unenforced
  metadata — the only thing consumed from them today is the *arity*, by
  `intent-realize`. `definstance` accepts any function; no check connects an
  instance to its declared signature.
- **`check-type` on a realized intent is weak evidence.** The checker infers
  a scheme like `(forall (a b c) (-> (a b) c))` for a realized intent over
  tagged-list concepts — a *vacuous* type: no contradiction found, nothing
  promised. The current status classifier files that as `checked`, which
  overstates it. Distinguishing informative from vacuous schemes needs
  structured `check-type` output (the standing consensus item), at which
  point the honest classification is four-valued:
  `checked / checked-vacuous / dynamic / type-error`.
- **Outcomes are runtime assertions, not proofs.** They fire per call. Laws
  and examples are per-value falsification with no quantification.
- **`intent-apply` is single-dispatch generic invocation** — structurally a
  restricted CLOS `defgeneric` (one operation, explicit table, no
  inheritance, no method combination). The parts that are *not* CLOS-shaped
  are the outcome contract (design-by-contract), `intent-realize`
  (an inspectable, dictionary-free lowering CLOS never exposes), and the
  condensation trace. Dispatch indirection raises the fault-localization
  span — a bug reachable through a polymorphic intent can originate in any
  instance — which is precisely the cost the condensation eval warned about.

The genuine type-checker tie-in, when wanted, is not more of this layer; it
is: (a) lowering ground concepts to `defstruct-typed`, (b) writing methods
with `defun-typed`, and (c) checking at `definstance` time that the method's
inferred type unifies with the class op signature instantiated at the
instance type — a registration-time conformance check that needs no
constrained schemes and no checker surgery. Until then, the dictionary layer
dispatches and the checker merely observes.

## Standard Typeclasses

`lib/21-typeclasses.lisp` declares the classes `derive` knows how to install
instances for:

| Class | Ops | Installed by |
|-------|-----|--------------|
| `eqv` | `eqv : (-> (a a) bool)` | `(derive c equality)` |
| `show` | `show : (-> (a) list)` | `(derive c printer)` |
| `lens` | `view : (-> (a) list)`, `build : (-> (list) a)` | `(derive c lens)` |
