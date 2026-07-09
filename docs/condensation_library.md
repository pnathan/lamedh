# Condensation

`lib/20-condensation.lisp` is one layer with one thesis: **the expensive part
of code is not writing it, it is changing it**. So the library makes three
things first-class data on ordinary symbol plists:

1. **Provenance** — what seed produced what expansion and which symbols;
2. **Verdicts** — what the HM checker actually knows about each generated
   symbol, classified honestly (a vacuous scheme is never called verified);
3. **Change** — a structural diff format that is also a patch format, and an
   editing verb (`edit!`) that applies a minimal path-addressed change with
   the checker as the barrier.

The surface reuses conventions the reader (human or model) already knows:
plists, conventional generated names, `deftest`-shaped laws and
examples. Nothing here is a new evaluation semantics.

## Records And Derivations

```lisp
(defrecord invoice
  (id int64) (amount int64) (status symbol)
  (:invariant (>= amount 0))
  (:derive equality lens))
```

One seed generates, deterministically: `make-invoice`, `invoice-p`,
`invoice-id`/`-amount`/`-status`, `validate-invoice` (the invariant with
fields bound), `invoice-equal`, `invoice->plist`, `plist->invoice`, and the
round-trip law `invoice-lens-roundtrip` — plus the full trace. The `:derive`
section is optional; `(derive invoice equality lens)` works separately and
is idempotent.

Derive targets:

| Target | Generates |
|--------|-----------|
| `printer` | `<c>->plist` |
| `equality` | `<c>-equal` |
| `lens` | `<c>->plist`, `plist-><c>`, and a `<c>-lens-roundtrip` law asserting `(equal (plist-><c> (<c>->plist self)) self)` |

Laws and examples attach as inspectable, executable contracts:

```lisp
(deflaw invoice-nonnegative (:for invoice) (:assert (>= amount 0)))
(example valid-draft (:for invoice) (:given (make-invoice 1 100 'draft))
                     (:expect (validate-invoice *it*)))
(condense-check 'invoice)   ; => (T (VALID-DRAFT . T))
```

## Honest Checker Verdicts

`see-type` (a builtin) reports the checker's verdict structurally, as data:

```lisp
(see-type 'inc)       ; => (CHECKED (-> (INT64) INT64))
(see-type 'my-typed)  ; => (TYPED (-> (INT64) INT64) COMPILED)
(see-type 'bad)       ; => (TYPE-ERROR "`+` operands disagree: Int64 vs Str")
(see-type 'car)       ; => (DYNAMIC "variadic or not a plain lambda")
```

`condense-classify` refines `CHECKED` into two grades. A scheme whose result
type contains a variable no argument constrains — e.g.
`(FORALL (A B C) (-> (A B) C))` — is **VACUOUS**: the checker found no
contradiction but proved nothing. The full status vocabulary is:

```
TYPED      registered typed function (the island; a real guarantee)
CHECKED    informative inferred scheme (a real guarantee)
DECLARED   generator-backed axiom (declare-type!, e.g. row schemes)
VACUOUS    no contradiction, no promise — unproven
DYNAMIC    variadic, builtin, or absent — unproven
TYPE-ERROR the checker rejects it
```

`condense-check-type` runs this over every generated symbol and stores the
results; everything not `TYPED`/`CHECKED`/`DECLARED` joins
`"condense.dynamic-frontier"` — the unproven remainder, visible in the
trace, never silently blended into "verified". `defrecord` and `derive` run
it automatically, so a fresh trace is honest from birth.

## Change Is Data: Diff, Patch, Edit

A change is a list of `(path old new)` triples, where a path is a list of
positions from the root of a form:

```lisp
(condense-diff '(defun f (x) (+ x 1)) '(defun f (x) (+ x 2)))
; => (((3 2) 1 2))

(sexpr-ref  '(defun f (x) (+ x 1)) '(3 2))     ; => 1
(sexpr-set  '(defun f (x) (+ x 1)) '(3 2) 9)   ; => (DEFUN F (X) (+ X 9))
(sexpr-patch old (condense-diff old new))       ; => new, always
```

`condense-diff` and `sexpr-patch` are inverses; each patch edit is guarded on
`old`, so a stale patch fails loudly instead of applying silently.

Edits may also name the subform instead of counting positions: a two-element
`(old new)` edit locates `old` uniquely via `sexpr-locate` (absence and
ambiguity are both errors). This is the ergonomic form for model authors —
the same unique-match contract as a string-replace tool, but over sexprs:

```lisp
(edit! 'price '(((* base qty) (* base (+ qty 1)))))
```

`edit!` is the minimum-change verb over live definitions:

```lisp
(defun price (base qty) (* base qty))
(edit! 'price '(((2) (* base qty) (* base (+ qty 1)))))
; => ((SYMBOL . PRICE) (WAS . CHECKED) (NOW . CHECKED) (APPLIED ...))
```

Paths address subforms of `(see-source sym)`. The HM checker is the barrier:
an edit that introduces a `TYPE-ERROR` into a definition that previously had
none is **rolled back and rejected**. An edit may still *repair* a broken
definition (`TYPE-ERROR` → anything is allowed). Every applied edit is
recorded under `"condense.edits"` — the change history is part of the trace.

Editing a **record** edits the seed: the patched `defrecord` source is
re-evaluated, recorded derivations re-derived, attached examples re-run, and
checker statuses refreshed. One minimal edit regenerates and re-verifies the
whole artifact:

```lisp
(edit! 'invoice '(((3 1) (>= amount 0) (>= amount 1))))
; => ((SYMBOL . INVOICE) (LAST-DIFF ...) (DYNAMIC-FRONTIER ...)
;     (CHECKS T (VALID-DRAFT . T)) (APPLIED ...))
```

## Branded Rows

Every `defrecord` registers its name as a real, NOMINAL type in the
checker: generated accessors carry branded declared schemes,

```lisp
(see-type 'invoice-amount)
; => (DECLARED (-> (INVOICE) INT64))
```

and cross-record misuse is a **static type error** — caught by
`check-type`, `check-file!`, and the `edit!` barrier:

```lisp
(see-type (progn (defun bad (r) (receipt-total (make-invoice 1 5 'draft))) 'bad))
; => (TYPE-ERROR "`RECEIPT-TOTAL` arg 0 expects RECEIPT, got INVOICE")
```

Row polymorphism lives one level down, in the `record-ref` primitive.
An accessor written over it derives an open row scheme — "any record with
an `amount`, whatever else it carries" — with no annotation anywhere:

```lisp
(defun the-amount (x) (record-ref x 'amount))
(see-type 'the-amount)
; => (CHECKED (FORALL (A B) (-> ((RECORD ((AMOUNT A)) B)) A)))
```

and every record — invoice, receipt, anything with an `amount` field —
flows through it, both in the checker (struct-into-row subsumption) and at
runtime. Ordinary code over such accessors infers row-polymorphic schemes
transitively: the checker derives schemes for helper functions on demand,
so the rows travel through call chains without a single `declare-type!`.

The honesty rules: `DECLARED` is an axiom — generated in lockstep with the
implementation, trusted by the checker at call sites, but not derived from
the body (a deliberate membrane, like typed natives). Field types outside
the checker's language degrade per-field to `any` — the record stays
branded and checkable, the opaque field stays honest.

## Staleness: The One-Way Lens, Enforced By Detection

Condensation is a one-way lens — the seed cannot be recovered from an edited
expansion — so the discipline is regenerate-only, and it is enforced by
detection rather than prohibition. Every generated definition is
fingerprinted (via `see-source`) at `defrecord`/`derive` time:

```lisp
(condense-stale 'invoice)   ; generated symbols whose definitions drifted
(condense-drift 'invoice)   ; (symbol . diff) for each, localized
(condense-recheck! 'invoice); staleness + examples + checker statuses
```

A hand-edited generated function shows up in the trace's `stale` entry; the
trace never vouches for code the seed no longer describes. The sanctioned
channel for changing generated code is editing the seed with `edit!`.

## The Agent Loop: `check-file!`

`edit!` assumes a live image — a REPL workflow. Agents work differently: the
file is the source of truth, edits happen through the agent's own tools, and
verification is a fresh batch run. `check-file!` is that verification step:

```sh
lamedh --capability READ-FS -s '(check-file! "src.lisp")'
```

```lisp
(check-file! "src.lisp")
; => ((FILE . "src.lisp")
;     (DEFINITIONS (INC CHECKED (CHECKED (-> (INT64) INT64)))
;                  (BROKEN TYPE-ERROR (TYPE-ERROR "`+` operands disagree: ..."))
;                  (INVOICE-EQUAL VACUOUS (CHECKED (FORALL (A B C) (-> (A B) C))))
;                  ...)
;     (FRONTIER (BROKEN TYPE-ERROR ...) ...))
```

It evaluates every form, reports an honest verdict per definition (concepts
expand to their generated symbols), and repeats the unproven/broken remainder
under `FRONTIER`. Reports are data: run it before and after an edit and
`condense-diff` the two reports to see exactly what the edit changed in the
type story. The loop is: **edit the file with your own tools → `check-file!`
→ read the delta.**

## Trace

`(condense-trace sym)` returns the whole record as an alist: `kind`,
`source`, `expansion`, `generated`, `laws`, `examples`, `check-status`,
`dynamic-frontier`, `derivations`, `edits`, `last-diff`, `stale`, and the
per-kind fields. It is the read-path artifact: the context a maintainer (or
model) needs, compressed to one form.

## Metadata Keys

All state lives under string plist keys on the defining symbol:
`"condense.kind"`, `".source"`, `".expansion"`, `".generated"`,
`".contracts"`, `".laws"`, `".examples"`, `".check-status"`,
`".dynamic-frontier"`, `".fields"`, `".invariant"`, `".derivations"`,
`".fingerprints"`, `".edits"`, `".last-diff"`, plus `".concept"`,
`".assert"`, `".given"`, `".expect"` on law/example symbols. Higher-level
forms should build on `condense-put`/`condense-get`/`condense-record!`
instead of inventing private conventions.

## Not In This Layer

Typeclass dictionaries and the intent/dispatch experiment were removed after
review (see `docs/eval/response-first-class-intent.md` and the branch
history): dispatch indirection raises the fault-localization span, and
nothing in it served minimum-change editing. If generic dispatch returns, it
must earn its way in through a repair benchmark. The forward path for
*types* is not dispatch but promotion: lowering ground concepts to
`defstruct-typed` so generated operations land in the typed island and their
`TYPED` verdicts are real guarantees.
