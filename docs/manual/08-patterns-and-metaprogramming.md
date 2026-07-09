# 8. Patterns and Metaprogramming

Lamedh represents code as data, so it can search, rewrite, and edit code with
the same tools it uses to search, rewrite, and edit any other list. This
chapter covers four layers built on that idea, each in `lib/`:

- **The pattern language** (`lib/23-match.lisp`) — one matcher (`pat-match`),
  exposed through `match`, `destructuring-bind`, `sgrep`, and `rewrite`.
- **`sgrep`** — structural search over forms, source text, and files.
- **`rewrite`** — structural, bottom-up code transformation.
- **The rulebook optimizer** (`lib/24-rules.lisp`) — optimization passes as
  data, feeding `optimize-form`.
- **The condensation change plane** (`lib/20-condensation.lisp`) — provenance,
  checker verdicts, and a minimum-change editing verb (`edit!`) for live
  definitions.

Every example below was run against `target/debug/lamedh`; commands that read
files pass `--capability READ-FS` explicitly, since filesystem access is
sandboxed by default (see Chapter on Sandboxing).

## 8.1 The Pattern Language

`pat-match` matches a *pattern* (ordinary data) against a *datum* (ordinary
data) and returns an alist of bindings, or the sentinel `$match-fail` (test it
with `match-fail-p`). Patterns are:

| Pattern | Meaning |
|---|---|
| `?x` | element variable — binds the datum; a second occurrence must be `equal` to the first |
| `?_` | wildcard — matches anything, binds nothing |
| `??xs` | segment variable — inside a list, matches zero or more consecutive elements, binding the sublist |
| `??_` | non-binding segment wildcard |
| `(?is ?x pred)` | matches when `(pred datum)` is truthy; `pred` is a function name or `(lambda ...)` |
| `(?and p...)` | all subpatterns must match; bindings accumulate |
| `(?or p...)` | first matching subpattern wins |
| `(?not p)` | matches when `p` does not (no bindings escape) |
| `(quote x)` | literal escape — matches a datum `equal` to `x` |
| any atom | literal — matches by `equal` |
| `(p . ps)` | cons pattern, including dotted tails |

```
$ ./target/debug/lamedh -s "(pat-match '(?a ?b) '(1 2))"
; => ((?B . 2) (?A . 1))

$ ./target/debug/lamedh -s "(pat-match '(?a ?a) '(1 1))"
; => ((?A . 1))

$ ./target/debug/lamedh -s "(match-fail-p (pat-match '(?a ?a) '(1 2)))"
; => T
```

A repeated pattern variable is unification-lite: it must bind to an `equal`
datum every time it recurs, so `(?a ?a)` matches `(1 1)` but not `(1 2)`.

Segment variables backtrack, shortest match first:

```
$ ./target/debug/lamedh -s "(pat-match '(1 ??xs 4) '(1 2 3 4))"
; => ((??XS 2 3))

$ ./target/debug/lamedh -s "(pat-match '(??a ??b) '(1 2 3))"
; => ((??B 1 2 3) (??A))
```

The second call shows the backtracking order: `??a` first tries the empty
span, `??b` absorbs the rest, and that succeeds immediately — so `??a` binds
`nil` and `??b` binds the whole list. A segment variable that recurs must
match the *same* span each time, which makes palindrome-style patterns work
for free:

```
$ ./target/debug/lamedh -s "(pat-match '(??xs 0 ??xs) '(1 2 0 1 2))"
; => ((??XS 1 2))

$ ./target/debug/lamedh -s "(pat-match '(??xs 0 ??xs) '(1 2 0 1 3))"
; => $MATCH-FAIL
```

`?is`/`?and`/`?or`/`?not` compose predicates and alternatives:

```
$ ./target/debug/lamedh -s "(pat-match '(?is ?x numberp) 5)"
; => ((?X . 5))

$ ./target/debug/lamedh -s "(pat-match '(?or (?is ?x numberp) (?is ?x symbolp)) 'foo)"
; => ((?X . FOO))

$ ./target/debug/lamedh -s "(pat-match '(?not (?is ?_ numberp)) 'foo)"
; => ()
```

`quote` lets a pattern match the literal symbol `?x` instead of treating it as
a variable — useful once your data contains pattern-language syntax itself:

```
$ ./target/debug/lamedh -s "(pat-match ''?x '?x)"
; => ()
```

(An empty alist here is a genuine match with no bindings — `?x` inside
`quote` is a literal, not a variable.)

Dotted patterns destructure cons cells directly:

```
$ ./target/debug/lamedh -s "(pat-match '(?h . ?t) '(1 2 3))"
; => ((?T 2 3) (?H . 1))
```

### `match`

`match` is the control form: it evaluates an expression once, tries each
clause's pattern in order, and evaluates the first matching clause's body
with the pattern's variables lexically bound. A clause may carry a `:when`
guard; use `?_` as a catch-all final pattern.

One wrinkle worth knowing up front: the bound variable keeps its `?`-prefixed
*name*. `?a` in a pattern binds a lexical variable literally named `?a`, not
`a` — so the body must refer to `?a`, not `a`:

```
$ ./target/debug/lamedh -s "(match '(1 2) ((?a ?b) (+ ?a ?b)))"
; => 3

$ ./target/debug/lamedh -s "(match 5 ((?is ?n numberp) :when (> ?n 10) 'big) ((?is ?n numberp) 'small))"
; => SMALL

$ ./target/debug/lamedh -s "(match 50 ((?is ?n numberp) :when (> ?n 10) 'big) ((?is ?n numberp) 'small))"
; => BIG

$ ./target/debug/lamedh -s "(match 'foo ((?is ?n numberp) 'num) (?_ 'other))"
; => OTHER
```

With no matching clause, `match` returns `nil`.

### `destructuring-bind`

`destructuring-bind` is `match`'s single-pattern cousin: bind a pattern's
variables against one expression's value, or signal an error if it doesn't
match.

```
$ ./target/debug/lamedh -s "(destructuring-bind (?a ?b) '(10 20) (+ ?a ?b))"
; => 30

$ ./target/debug/lamedh -s "(destructuring-bind (?a ?b) '(10 20 30) (+ ?a ?b))"
Error: destructuring-bind: (10 20 30) does not match pattern (?A ?B)
```

### `instantiate`

`instantiate` is the inverse of `pat-match`: given a template and a bindings
alist, it fills in the bound variables (segment variables splice their
sublists into the surrounding list). It is the engine behind `rewrite` and
the rulebook, and is useful standalone:

```
$ ./target/debug/lamedh -s "(instantiate '(+ ?a ?b) '((?a . 1) (?b . 2)))"
; => (+ 1 2)

$ ./target/debug/lamedh -s "(instantiate '(list ??xs) '((??xs 1 2 3)))"
; => (LIST 1 2 3)
```

## 8.2 Structural Search: `sgrep`

`sgrep` walks a form depth-first and collects every subform matching a
pattern, as `(subform . bindings)` pairs:

```
$ ./target/debug/lamedh -s "(sgrep '(+ ?a ?b) '(let ((x (+ 1 2))) (* x (+ 3 4))))"
; => (((+ 1 2) (?B . 2) (?A . 1)) ((+ 3 4) (?B . 4) (?A . 3)))
```

`sgrep-fn` runs `sgrep` over a function's own source, via `see-source` — grep
a definition the way you'd grep a file:

```
$ ./target/debug/lamedh -s "(defun f (x) (+ x 1)) (sgrep '(+ ?a ?b) (see-source 'f))"
; => (((+ X 1) (?B . 1) (?A . X)))
```

### Positions: `sgrep-source` and `read-all-positioned`

`read-all-positioned` reads a whole source text into `(form line col)`
triples (1-based); `sgrep-source` runs `sgrep` over each top-level form and
reports hits as `(line col subform bindings)`:

```
$ ./target/debug/lamedh -s '(read-all-positioned "(defun f (x) x)
(defun g (y) y)")'
; => (((DEFUN F (X) X) 1 1) ((DEFUN G (Y) Y) 2 1))

$ ./target/debug/lamedh -s '(sgrep-source (quote (defun ?name ?_ ??_)) "(defun f (x) x)
(defun g (y) y)")'
; => ((1 1 (DEFUN F (X) X) ((?NAME . F))) (2 1 (DEFUN G (Y) Y) ((?NAME . G))))
```

Because a hit is itself a flat list, it destructures with `match`:

```
$ ./target/debug/lamedh -s '(mapcar (lambda (hit) (match hit ((?line ?col ?form ?bs) (list ?line ?col ?form)))) (sgrep-source (quote (defun ?name ?_ ??_)) "(defun f (x) x)
(defun g (y) y)"))'
; => ((1 1 (DEFUN F (X) X)) (2 1 (DEFUN G (Y) Y)))
```

### `sgrep-file`: searching a real file

`sgrep-file` reads a file (requires `READ-FS`) and runs `sgrep-source` over
its text. `examples/npcs.lisp` in the repo defines three `defrecord` kinds and
several `record-ref`-based accessors — good real-world targets:

```
$ ./target/debug/lamedh --capability READ-FS -s "(mapcar (lambda (h) (list (car h) (car (cdr h)))) (sgrep-file '(defrecord ?name ??_) \"examples/npcs.lisp\"))"
; => ((26 1) (31 1) (36 1))
```

Three `defrecord` forms, at lines 26, 31, and 36 — `goblin`, `merchant`, and
`wisp`. Searching for a different shape finds the hand-written row-typed
accessors:

```
$ ./target/debug/lamedh --capability READ-FS -s "(sgrep-file '(record-ref ?_ ??_) \"examples/npcs.lisp\")"
; => ((49 1 (RECORD-REF SELF (QUOTE NAME)) ()) (50 1 (RECORD-REF SELF (QUOTE HP)) ()))
```

## 8.3 `rewrite`: Structural Transformation

`(rewrite pattern template form)` replaces every subform matching `pattern`
with `template` instantiated against that match's bindings. It runs
bottom-up, single pass: children are rewritten before their parent is
checked, so a nested match is already transformed by the time it's carried
into an enclosing template — but a freshly instantiated replacement is not
re-searched at its own node, so a template can echo the pattern's own shape
without looping.

A single rule call:

```
$ ./target/debug/lamedh -s "(rewrite '(+ ?a 0) '?a '(+ (+ x 0) 5))"
; => (+ X 5)

$ ./target/debug/lamedh -s "(rewrite '(* ?a 1) '?a '(* (* y 1) 2))"
; => (* Y 2)
```

### Worked example: simplifying arithmetic

Because `rewrite` is bottom-up, a single pattern combining several algebraic
identities with `?or` simplifies an arbitrarily nested expression in one
call — the recursion visits every subform, so nested `+0`/`*1` wrappers all
get peeled in the same pass:

```
$ ./target/debug/lamedh -s "(defun simplify (form) (rewrite '(?or (+ ?a 0) (* ?a 1)) '?a form)) (simplify '(+ (* (+ x 0) 1) 0))"
; => X
```

`(+ (* (+ x 0) 1) 0)` — an `x` wrapped in `+0`, then `*1`, then `+0` again —
collapses straight to `X`.

## 8.4 The Rulebook Optimizer

`lib/24-rules.lisp` turns optimization passes into data. A rule is a
`pat-match` pattern plus an `instantiate` template, optionally guarded by a
form evaluated with the pattern's variables bound to their *matched forms*
(as code, not values) — so a guard can ask "is this subform pure?" without
evaluating it.

```lisp
(defrule name pattern template)
(defrule name pattern template :when guard)
```

Three rules ship by default, visible via `list-rules`:

```
$ ./target/debug/lamedh -s "(list-rules)"
; => ((APPEND-NIL (APPEND ?X ()) ?X) (CDR-OF-CONS (CDR (CONS ?A ?B)) ?B) (CAR-OF-CONS (CAR (CONS ?A ?B)) ?A))
```

`apply-rules` rewrites a form bottom-up, retrying rules at each node to a
bounded fixpoint (`quote`/`quasiquote` subtrees are data and pass through
untouched):

```
$ ./target/debug/lamedh -s "(apply-rules '(car (cons 1 2)))"
; => 1

$ ./target/debug/lamedh -s "(apply-rules '(append (list 1 2) nil))"
; => (LIST 1 2)
```

Defining your own rule and using it:

```
$ ./target/debug/lamedh -s "(defrule double-add (+ ?x ?x) (* 2 ?x)) (apply-rules '(+ (foo) (foo)))"
; => (* 2 (FOO))
```

`undefrule` removes a rule by name:

```
$ ./target/debug/lamedh -s "(defrule double-add (+ ?x ?x) (* 2 ?x)) (undefrule 'double-add) (list-rules)"
; => ((APPEND-NIL (APPEND ?X ()) ?X) (CDR-OF-CONS (CDR (CONS ?A ?B)) ?B) (CAR-OF-CONS (CAR (CONS ?A ?B)) ?A))
```

### Purity guards

`car-of-cons` and `cdr-of-cons` only fire when the *dropped* half is pure
(`opt-pure-p`) — dropping an effectful subform would change what the program
does, so the rule must not fire:

```
$ ./target/debug/lamedh -s "(apply-rules '(car (cons 1 (print 2))))"
; => (CAR (CONS 1 (PRINT 2)))
```

`(cons 1 (print 2))`'s second element is not pure, so `car-of-cons` declines
and the form passes through unchanged — the `(print 2)` side effect is
preserved.

### Feeding the optimizer

`optimize-form` chains the Lisp-level passes, the rulebook, frame collapse,
and the builtin constant folder:

```
$ ./target/debug/lamedh -s "(optimize-form '(car (cons 1 2)))"
; => 1
```

### Termination

Rules retry at a node until none fires or a per-node cap (`$rules-node-cap`,
64) is hit, so a cyclic rulebook degrades to a bounded no-op instead of
hanging:

```
$ ./target/debug/lamedh -s "(defrule loopy (f ?x) (f (g ?x))) (apply-rules '(f 1))"
; => (F (G (G (G ... 64 Gs total ... 1))))
```

## 8.5 The Condensation Change Plane

`lib/20-condensation.lisp` treats a definition's history as ordinary data on
its symbol's property list: what seed produced it, what it expanded to, what
symbols it generated, and what the type checker actually knows about each
one. `defrecord` (Chapter on structs/records) populates this trace
automatically; `deflaw` and `example` extend it with executable contracts.

### Provenance: `condense-trace`, `see-source`, `condense-generated`

```
$ ./target/debug/lamedh -s "
(defrecord invoice
  (id int64) (amount int64) (status symbol)
  (:invariant (>= amount 0))
  (:derive equality lens))
(deflaw invoice-nonnegative (:for invoice) (:assert (>= amount 0)))
(example valid-draft (:for invoice) (:given (make-invoice 1 100 'draft))
                     (:expect (validate-invoice *it*)))
(print (mapcar #'car (condense-trace 'invoice)))
(print (cdr (assoc 'generated (condense-trace 'invoice))))
(print (cdr (assoc 'laws (condense-trace 'invoice))))
(print (cdr (assoc 'examples (condense-trace 'invoice))))
"
; => (KIND SOURCE EXPANSION GENERATED CONTRACTS LAWS CHECK-STATUS DYNAMIC-FRONTIER FIELDS INVARIANT DERIVATIONS EXAMPLES CONCEPT ASSERT GIVEN EXPECT EDITS LAST-DIFF STALE)
; => (MAKE-INVOICE INVOICE-P VALIDATE-INVOICE INVOICE-ID INVOICE-AMOUNT INVOICE-STATUS INVOICE-EQUAL INVOICE->PLIST PLIST->INVOICE INVOICE-LENS-ROUNDTRIP)
; => (INVOICE-LENS-ROUNDTRIP INVOICE-NONNEGATIVE)
; => (VALID-DRAFT)
```

`condense-trace` is the one-form read path: kind, source seed, generated
expansion, every generated symbol, attached laws/examples, checker
`check-status`, and the unproven `dynamic-frontier` (empty here — see below).
`condense-generated` and `see-source` are its building blocks — `see-source`
reconstructs a symbol's definition as a form, which is exactly what the
`sgrep`/`rewrite`/`edit!` machinery operates on:

```
$ ./target/debug/lamedh -s "(defun f (x) (+ x 1)) (see-source 'f)"
; => (LAMBDA (X) (+ X 1))
```

### `deflaw`, `example`, and `condense-check`

`deflaw` attaches a named predicate law to a `defrecord`-defined record;
`example` attaches an executable check (`*it*` is bound to `:given`'s value
inside `:expect`). `condense-check` runs a record's attached examples and
reports `(pass . results)`:

```
$ ./target/debug/lamedh -s "
(defrecord invoice (id int64) (amount int64) (status symbol)
  (:invariant (>= amount 0)))
(deflaw invoice-nonnegative (:for invoice) (:assert (>= amount 0)))
(example valid-draft (:for invoice) (:given (make-invoice 1 100 'draft))
                     (:expect (validate-invoice *it*)))
(condense-check 'invoice)
"
; => (T (VALID-DRAFT . T))
```

### Change is data: `sexpr-ref`/`sexpr-set`/`sexpr-patch`/`condense-diff`

A change is a list of `(path old new)` triples, where a path is a list of
positions counted from the root of a form. `condense-diff` produces such a
list from two forms; `sexpr-ref`/`sexpr-set` read and write a subform at a
path; `sexpr-patch` applies a list of edits, guarding each one on `old` so a
stale patch fails loudly:

```
$ ./target/debug/lamedh -s "(condense-diff '(defun f (x) (+ x 1)) '(defun f (x) (+ x 2)))"
; => (((3 2) 1 2))

$ ./target/debug/lamedh -s "(sexpr-ref '(defun f (x) (+ x 1)) '(3 2))"
; => 1

$ ./target/debug/lamedh -s "(sexpr-set '(defun f (x) (+ x 1)) '(3 2) 9)"
; => (DEFUN F (X) (+ X 9))

$ ./target/debug/lamedh -s "(let ((old '(defun f (x) (+ x 1))))
  (equal (sexpr-patch old (condense-diff old '(defun f (x) (+ x 2))))
         '(defun f (x) (+ x 2))))"
; => T
```

`sexpr-patch`'s edits may also skip the path and name the subform directly —
a two-element `(old new)` edit locates `old` uniquely via `sexpr-locate`
(absence and ambiguity are both errors, so an edit must name its site
unambiguously):

```
$ ./target/debug/lamedh -s "(defun f (x) (+ (+ x 1) (+ x 1))) (edit! 'f '(((+ x 1) (+ x 2))))"
Error: sexpr-locate: ambiguous, 2 occurrences of (+ X 1)
```

### `edit!`: minimum change, checker as the barrier

`edit!` applies edits to a live symbol's definition, re-evaluates it, and
re-checks it. For a plain function the HM checker is the barrier: an edit
that turns a clean definition into one with a type error is rolled back and
rejected.

A successful edit, using the path form:

```
$ ./target/debug/lamedh -s "
(defun price (base qty) (* base qty))
(edit! 'price '(((2) (* base qty) (* base (+ qty 1)))))
(list (price 10 3) (see-source 'price))"
; => PRICE
; => ((SYMBOL . PRICE) (WAS . CHECKED) (NOW . TYPED) (APPLIED ((2) (* BASE QTY) (* BASE (+ QTY 1)))))
; => (40 (LAMBDA (BASE QTY) (* BASE (+ QTY 1))))
```

The same edit works with the ergonomic `(old new)` form, since `(* base
qty)` is unambiguous in `price`'s body:

```
$ ./target/debug/lamedh -s "
(defun price (base qty) (* base qty))
(edit! 'price '(((* base qty) (* base (+ qty 1)))))
(price 10 3)"
; => 40
```

A refused edit — introducing a call `car` can't type — is rolled back:

```
$ ./target/debug/lamedh -s "
(defun f () (car (list 1 2 3)))
(edit! 'f '(((2 1) (list 1 2 3) 5)))"
; => F
Error: edit!: rejected, introduces a type error: `car` expects a list, got Int64
```

Wrapping the rejected call in `errorset` confirms the rollback: `f`'s source
and behavior are untouched.

```
$ ./target/debug/lamedh -s "
(defun f () (car (list 1 2 3)))
(errorset (list 'edit! ''f ''(((2 1) (list 1 2 3) 5))))
(list (see-source 'f) (see-type 'f) (f))"
; => (LAMBDA () (CAR (LIST 1 2 3))) (CHECKED (-> () INT64)) 1
```

Editing a **record** edits the seed instead of the expansion: the patched
`defrecord` source is re-evaluated, recorded derivations re-derived, attached
examples re-run, and checker statuses refreshed — one minimal edit
regenerates and re-verifies the whole artifact. `condense-source` returns the
`(defrecord ...)` seed form, so paths address it directly:

```
$ ./target/debug/lamedh -s "
(defrecord invoice (id int64) (amount int64) (status symbol)
  (:invariant (>= amount 0)) (:derive equality lens))
(deflaw invoice-nonnegative (:for invoice) (:assert (>= amount 0)))
(example valid-draft (:for invoice) (:given (make-invoice 1 100 'draft))
                     (:expect (validate-invoice *it*)))
(edit! 'invoice '(((5 1) (>= amount 0) (>= amount 1))))
(condense-check 'invoice)"
; => ((SYMBOL . INVOICE) (LAST-DIFF ((9 3 2 2 2) 0 1)) (DYNAMIC-FRONTIER)
;     (CHECKS T (VALID-DRAFT . T)) (APPLIED ((5 1) (>= AMOUNT 0) (>= AMOUNT 1))))
; => (T (VALID-DRAFT . T))
```

The tightened invariant (`amount >= 1` instead of `>= 0`) is re-checked
against the attached example immediately: `valid-draft`'s amount of `100`
still passes.

### Fingerprints and staleness

Condensation is a one-way lens: you cannot recover the seed from an edited
expansion. Instead of prohibiting hand edits to generated code, the library
detects them. Every generated symbol is fingerprinted (via `see-source`) at
`defrecord`/`derive` time; `condense-stale` reports which generated symbols
have since drifted, and `condense-drift` localizes the drift as a diff:

```
$ ./target/debug/lamedh -s "
(defrecord invoice (id int64) (amount int64) (status symbol)
  (:invariant (>= amount 0)))
(defun invoice-amount (self) (* 2 (record-ref self 'amount)))
(list (condense-stale 'invoice) (condense-recheck! 'invoice))"
; => (INVOICE-AMOUNT)
; => ((STALE INVOICE-AMOUNT) (DRIFT (INVOICE-AMOUNT ((2 0) RECORD-REF *) ((2 1) SELF 2)
;     ((2 2) (QUOTE AMOUNT) (RECORD-REF SELF (QUOTE AMOUNT))))) (CHECKS T)
;     (CHECK-STATUS ...))
```

Hand-redefining `invoice-amount` to double the field flags it stale
immediately. `condense-recheck!` bundles `condense-stale`, `condense-drift`,
`condense-check`, and `condense-check-type` into one re-verification call.
The sanctioned way to change generated code is `edit!` on the seed, not
hand-editing the generated function.

### `condense-check-type` and the dynamic frontier

`see-type` (a builtin) reports the checker's verdict on a symbol as data:
`(TYPED sig tier)`, `(CHECKED scheme)`, `(DECLARED scheme)`, `(TYPE-ERROR
msg)`, or `(DYNAMIC reason)`. `condense-check-type` classifies every
generated symbol of a record (refining `CHECKED` into `CHECKED` vs.
`VACUOUS`, depending on whether the result type is actually constrained by
an argument) and records the unproven remainder as
`"condense.dynamic-frontier"`.

`examples/npcs.lisp` is a good real demonstration: it defines three
`defrecord` kinds sharing hand-written row-typed accessors (`npc-name`,
`npc-hp`, via `record-ref`), plus a `greet` method specialized per kind. The
agent-facing entry point, `check-file!`, loads a file and reports honest
verdicts for everything it defines, repeating the unproven/broken remainder
under `frontier`:

```
$ ./target/debug/lamedh --capability READ-FS -s "
(mapcar #'car (cdr (assoc 'frontier (check-file! \"examples/npcs.lisp\"))))"
; => (GOBLIN-GREET MERCHANT-GREET WISP-GREET TAUNT-ALL SCUFFLE)
```

Checking the individual verdicts shows why: `npc-name` and `alive-p` are row
polymorphic and provably informative (`CHECKED`, and the result type — `A`,
`BOOL` — is pinned by an argument), while `goblin-greet` builds a `string`
result that no argument constrains, so it's classified `VACUOUS` — the
checker found no contradiction but proved nothing about the return value:

```
$ ./target/debug/lamedh --capability READ-FS -i examples/npcs.lisp -s "
(list (see-type 'npc-name) (see-type 'alive-p) (see-type 'goblin-greet))"
; => ((CHECKED (FORALL (A B) (-> ((RECORD ((NAME A)) B)) A)))
;     (CHECKED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) BOOL)))
;     (CHECKED (FORALL (A B C) (-> ((RECORD ((NAME A)) B)) C))))
```

`goblin-greet`'s scheme, `(forall (a b c) (-> ((record ((name a)) b)) c))`,
has an unconstrained `c` in the result position — nothing pins the return
type to `string`, so `condense-classify` reports it honestly as `VACUOUS`
rather than folding it into "verified." That's the frontier report's whole
point: it never silently blends an unproven function in with a proven one.

`check-file!` is the workflow for agents that edit files with their own
tools rather than holding a live REPL image: edit the file, run
`check-file!`, and `condense-diff` two reports to see exactly what an edit
changed in the type story.
