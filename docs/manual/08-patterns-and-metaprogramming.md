# 8. Patterns and Metaprogramming

Lamedh represents code as data, so it can search, rewrite, and edit code with
the same tools it uses to search, rewrite, and edit any other list. This
chapter covers four layers, each defined in `lib/`:

- **The pattern language** (`lib/23-match.lisp`) — one matcher (`pat-match`),
  exposed through `match`, `destructuring-bind`, `sgrep`, and `rewrite`.
- **`sgrep`** — structural search over forms, source text, and files.
- **`rewrite`** — structural, bottom-up code transformation.
- **The rulebook optimizer** (`lib/24-rules.lisp`) — optimization passes as
  data, feeding `optimize-form`.
- **The condensation change plane** (`lib/20-condensation.lisp`) —
  provenance, checker verdicts, and a minimum-change editing verb (`edit!`)
  for live definitions.

Every example below was run against `target/debug/lamedh`. Commands that read
files pass `--capability READ-FS` explicitly, since filesystem access is
sandboxed by default.

## 8.1 The Pattern Language

`pat-match` matches a *pattern* (ordinary data) against a *datum* (ordinary
data) and returns an alist of bindings, or the sentinel `$match-fail` (test
with `match-fail-p`). Patterns are:

| Pattern | Meaning |
|---|---|
| `?x` | element variable — binds the datum; a repeated occurrence must be `equal` to the first |
| `?_` | wildcard — matches anything, binds nothing |
| `??xs` | segment variable — inside a list, matches zero or more consecutive elements, binding the sublist (`??_` is the non-binding form) |
| `(?is ?x pred)` | matches when `(pred datum)` is truthy; `pred` is a function name or `(lambda ...)` |
| `(?and p...)` | all subpatterns must match; bindings accumulate |
| `(?or p...)` | first matching subpattern wins |
| `(?not p)` | matches when `p` does not (no bindings escape) |
| `(quote x)` | literal escape — matches a datum `equal` to `x` |
| any atom | literal — matches by `equal` |
| `(p . ps)` | cons pattern, including dotted tails |

```
$ lamedh -s "(list (pat-match '(?a ?b) '(1 2)) (match-fail-p (pat-match '(?a ?a) '(1 2))))"
; => (((?B . 2) (?A . 1)) T)
```

Segment variables backtrack, shortest match first — `??a` tries the empty
span, `??b` absorbs the rest, and that succeeds immediately, so `??a` binds
`nil`. A segment variable that recurs must match the *same* span each time,
which gets you palindrome patterns for free:

```
$ lamedh -s "(pat-match '(1 ??xs 4) '(1 2 3 4))"
; => ((??XS 2 3))

$ lamedh -s "(pat-match '(??a ??b) '(1 2 3))"
; => ((??B 1 2 3) (??A))

$ lamedh -s "(pat-match '(??xs 0 ??xs) '(1 2 0 1 2))"
; => ((??XS 1 2))

$ lamedh -s "(pat-match '(??xs 0 ??xs) '(1 2 0 1 3))"
; => $MATCH-FAIL
```

`?is`/`?and`/`?or`/`?not` compose predicates and alternatives; `quote`
matches a literal symbol like `?x` instead of treating it as a variable
(matters once your data itself contains pattern syntax); dotted patterns
destructure cons cells directly:

```
$ lamedh -s "(pat-match '(?is ?x numberp) 5)"
; => ((?X . 5))

$ lamedh -s "(pat-match '(?or (?is ?x numberp) (?is ?x symbolp)) 'foo)"
; => ((?X . FOO))

$ lamedh -s "(pat-match '(?not (?is ?_ numberp)) 'foo)"
; => ()      ; a genuine match with no bindings

$ lamedh -s "(pat-match ''?x '?x)"
; => ()

$ lamedh -s "(pat-match '(?h . ?t) '(1 2 3))"
; => ((?T 2 3) (?H . 1))
```

### `match`

`match` evaluates an expression once, tries each clause's pattern in order,
and evaluates the first matching clause's body with the pattern's variables
lexically bound. A clause may carry a `:when` guard; `?_` is the idiomatic
catch-all final pattern. One wrinkle: a bound variable keeps its
`?`-prefixed *name*. `?a` binds a lexical variable literally named `?a`, not
`a` — the body must refer to `?a`:

```
$ lamedh -s "(match '(1 2) ((?a ?b) (+ ?a ?b)))"
; => 3

$ lamedh -s "(match 5 ((?is ?n numberp) :when (> ?n 10) 'big) ((?is ?n numberp) 'small))"
; => SMALL

$ lamedh -s "(match 'foo ((?is ?n numberp) 'num) (?_ 'other))"
; => OTHER
```

With no matching clause, `match` returns `nil`.

### `destructuring-bind`

`destructuring-bind` binds a pattern's variables against one expression's
value, or signals an error if it doesn't match:

```
$ lamedh -s "(destructuring-bind (?a ?b) '(10 20) (+ ?a ?b))"
; => 30

$ lamedh -s "(destructuring-bind (?a ?b) '(10 20 30) (+ ?a ?b))"
Error: destructuring-bind: (10 20 30) does not match pattern (?A ?B)
```

### `instantiate`

`instantiate` is `pat-match`'s inverse: given a template and a bindings
alist, it fills in the bound variables (segment variables splice their
sublists into the surrounding list). It powers `rewrite` and the rulebook,
and is useful on its own:

```
$ lamedh -s "(instantiate '(+ ?a ?b) '((?a . 1) (?b . 2)))"
; => (+ 1 2)

$ lamedh -s "(instantiate '(list ??xs) '((??xs 1 2 3)))"
; => (LIST 1 2 3)
```

## 8.2 Structural Search: `sgrep`

`sgrep` walks a form depth-first and collects every subform matching a
pattern, as `(subform . bindings)` pairs:

```
$ lamedh -s "(match:sgrep '(+ ?a ?b) '(let ((x (+ 1 2))) (* x (+ 3 4))))"
; => (((+ 1 2) (?B . 2) (?A . 1)) ((+ 3 4) (?B . 4) (?A . 3)))
```

`sgrep-fn` runs `sgrep` over a function's own source, via `see-source` — grep
a definition the way you'd grep a file: `(match:sgrep '(+ ?a ?b) (see-source 'f))`
for `(defun f (x) (+ x 1))` finds `(((+ X 1) (?B . 1) (?A . X)))`.

### Positions: `sgrep-source` and `read-all-positioned`

`read-all-positioned` reads a source text into `(form line col)` triples
(1-based); `sgrep-source` runs `sgrep` over each top-level form and reports
`(line col subform bindings)` hits — flat enough to destructure with `match`
itself:

```
$ lamedh -s '(match:sgrep-source (quote (defun ?name ?_ ??_)) "(defun f (x) x)
(defun g (y) y)")'
; => ((1 1 (DEFUN F (X) X) ((?NAME . F))) (2 1 (DEFUN G (Y) Y) ((?NAME . G))))

$ lamedh -s '(mapcar (lambda (hit) (match hit ((?line ?col ?form ?bs) (list ?line ?col ?form))))
  (match:sgrep-source (quote (defun ?name ?_ ??_)) "(defun f (x) x)
(defun g (y) y)"))'
; => ((1 1 (DEFUN F (X) X)) (2 1 (DEFUN G (Y) Y)))
```

### `sgrep-file`: searching a real file

`sgrep-file` reads a file (requires `READ-FS`) and runs `sgrep-source` over
its text. `examples/npcs.lisp` in the repo defines three `defrecord` kinds
and several `record-ref`-based accessors — good real targets:

```
$ lamedh --capability READ-FS -s "(list
  (mapcar (lambda (h) (list (car h) (car (cdr h)))) (match:sgrep-file '(defrecord ?name ??_) \"examples/npcs.lisp\"))
  (match:sgrep-file '(record-ref ?_ ??_) \"examples/npcs.lisp\"))"
; => (((26 1) (31 1) (36 1))
;     ((49 1 (RECORD-REF SELF (QUOTE NAME)) ()) (50 1 (RECORD-REF SELF (QUOTE HP)) ())))
```

Three `defrecord` forms, at lines 26, 31, and 36 — `goblin`, `merchant`, and
`wisp` — and two hand-written row-typed accessors at 49 and 50.

## 8.3 `rewrite`: Structural Transformation

`(match:rewrite pattern template form)` replaces every subform matching `pattern`
with `template` instantiated against that match's bindings. It runs
bottom-up, single pass: children are rewritten before their parent is
checked, so a nested match is already transformed by the time it reaches an
enclosing template — but a freshly instantiated replacement is not
re-searched at its own node, so a template can echo the pattern's own shape
without looping.

```
$ lamedh -s "(match:rewrite '(+ ?a 0) '?a '(+ (+ x 0) 5))"
; => (+ X 5)
```

### Worked example: simplifying arithmetic

Because `rewrite` visits every subform bottom-up, a single pattern combining
two algebraic identities with `?or` simplifies an arbitrarily nested
expression in one pass — nested `+0`/`*1` wrappers all get peeled together:

```
$ lamedh -s "(defun simplify (form) (match:rewrite '(?or (+ ?a 0) (* ?a 1)) '?a form))
  (simplify '(+ (* (+ x 0) 1) 0))"
; => X
```

`(+ (* (+ x 0) 1) 0)` — `x` wrapped in `+0`, then `*1`, then `+0` again —
collapses straight to `X`.

## 8.4 The Rulebook Optimizer

`lib/24-rules.lisp` turns optimization passes into data. A rule is a
`pat-match` pattern plus an `instantiate` template, optionally guarded by a
form evaluated with the pattern's variables bound to their *matched forms*
(as code, not values) — a guard can ask "is this subform pure?" without
evaluating it.

```lisp
(defrule name pattern template)
(defrule name pattern template :when guard)
```

Three rules ship by default:

```
$ lamedh -s "(list-rules)"
; => ((APPEND-NIL (APPEND ?X ()) ?X) (CDR-OF-CONS (CDR (CONS ?A ?B)) ?B) (CAR-OF-CONS (CAR (CONS ?A ?B)) ?A))
```

`apply-rules` rewrites a form bottom-up, retrying rules at each node to a
bounded fixpoint (`quote`/`quasiquote` subtrees are data and pass through
untouched). Defining your own rule, applying it, then removing it with
`undefrule`:

```
$ lamedh -s "(apply-rules '(car (cons 1 2)))"
; => 1

$ lamedh -s "(defrule double-add (+ ?x ?x) (* 2 ?x)) (apply-rules '(+ (foo) (foo)))"
; => (* 2 (FOO))

$ lamedh -s "(defrule double-add (+ ?x ?x) (* 2 ?x)) (undefrule 'double-add) (list-rules)"
; => ((APPEND-NIL (APPEND ?X ()) ?X) (CDR-OF-CONS (CDR (CONS ?A ?B)) ?B) (CAR-OF-CONS (CAR (CONS ?A ?B)) ?A))
```

### Purity guards

`car-of-cons` and `cdr-of-cons` fire only when the *dropped* half is pure
(`opt-pure-p`) — dropping an effectful subform would change what the program
does:

```
$ lamedh -s "(apply-rules '(car (cons 1 (print 2))))"
; => (CAR (CONS 1 (PRINT 2)))
```

`(cons 1 (print 2))`'s second element isn't pure, so `car-of-cons` declines
and the `(print 2)` side effect is preserved.

`optimize-form` chains the Lisp-level passes, the rulebook, frame collapse,
and the builtin constant folder — `(optimize-form '(car (cons 1 2)))` also
returns `1`. Rules retry at a node until none fires or a per-node cap
(`$rules-node-cap`, 64) is hit, so a cyclic rulebook degrades to a bounded
no-op instead of hanging: `(defrule loopy (f ?x) (f (g ?x))) (apply-rules
'(f 1))` returns `(f (g (g (g ... 1))))` nested exactly 64 `g`s deep, not an
infinite rewrite.

## 8.5 The Condensation Change Plane

`lib/20-condensation.lisp` treats a definition's history as ordinary data on
its symbol's property list: what seed produced it, what it expanded to,
what symbols it generated, and what the type checker actually knows about
each one. `defrecord` populates this trace automatically; `deflaw` and
`example` extend it with executable contracts.

### Provenance: `condense-trace`, `see-source`, `condense-generated`; `deflaw`/`example`

`deflaw` attaches a named predicate law to a `defrecord`-defined record;
`example` attaches an executable check (`*it*` is bound to `:given`'s value
inside `:expect`). Both feed the trace:

```
$ lamedh -s "(progn
  (defrecord invoice (id int64) (amount int64) (status symbol)
    (:invariant (>= amount 0)) (:derive equality lens))
  (deflaw invoice-nonnegative (:for invoice) (:assert (>= amount 0)))
  (example valid-draft (:for invoice) (:given (make-invoice 1 100 'draft))
                       (:expect (validate-invoice *it*)))
  (list (mapcar #'car (condense-trace 'invoice))
        (condense-generated 'invoice)
        (condense-check 'invoice)))"
; => ((KIND SOURCE EXPANSION GENERATED CONTRACTS LAWS CHECK-STATUS DYNAMIC-FRONTIER
;      FIELDS INVARIANT DERIVATIONS EXAMPLES CONCEPT ASSERT GIVEN EXPECT EDITS
;      LAST-DIFF STALE)
;     (MAKE-INVOICE INVOICE-P VALIDATE-INVOICE INVOICE-ID INVOICE-AMOUNT
;      INVOICE-STATUS INVOICE-EQUAL INVOICE->PLIST PLIST->INVOICE INVOICE-LENS-ROUNDTRIP)
;     (T (VALID-DRAFT . T)))
```

`condense-trace` is the one-form read path: kind, source seed, expansion,
every `condense-generated` symbol, attached laws/examples, checker
`check-status`, and the `dynamic-frontier` (covered below). `see-source` is
the trace's other building block — it reconstructs a symbol's definition as
a form, exactly what `sgrep`/`rewrite`/`edit!` operate on: `(see-source 'f)`
for `(defun f (x) (+ x 1))` returns `(LAMBDA (X) (+ X 1))`. `condense-check`
runs a record's attached examples and reports `(pass . results)` — `T` here,
since `valid-draft`'s invoice satisfies `validate-invoice`.

### Change is data: `sexpr-ref`/`sexpr-set`/`sexpr-patch`/`condense-diff`

A change is a list of `(path old new)` triples, a path being positions
counted from the root. `condense-diff` produces such a list from two forms;
`sexpr-ref`/`sexpr-set` read and write a subform at a path; `sexpr-patch`
applies edits, guarding each on `old` so a stale patch fails loudly instead
of applying silently — `sexpr-patch` and `condense-diff` are inverses:

```
$ lamedh -s "(list (condense-diff '(defun f (x) (+ x 1)) '(defun f (x) (+ x 2)))
  (sexpr-ref '(defun f (x) (+ x 1)) '(3 2))
  (let ((old '(defun f (x) (+ x 1))))
    (equal (sexpr-patch old (condense-diff old '(defun f (x) (+ x 2))))
           '(defun f (x) (+ x 2)))))"
; => ((((3 2) 1 2)) 1 T)
```

An edit may skip the path and name the subform directly — a two-element
`(old new)` edit locates `old` uniquely via `sexpr-locate`, erroring on
absence or ambiguity:

```
$ lamedh -s "(defun f (x) (+ (+ x 1) (+ x 1))) (edit! 'f '(((+ x 1) (+ x 2))))"
Error: sexpr-locate: ambiguous, 2 occurrences of (+ X 1)
```

### `edit!`: minimum change, checker as the barrier

`edit!` applies edits to a live symbol's definition, re-evaluates it, and
re-checks it. For a plain function, the HM checker is the barrier: an edit
that turns a clean definition into one with a type error is rolled back and
rejected. A successful edit, using the ergonomic `(old new)` form:

```
$ lamedh -s "(progn
  (defun price (base qty) (* base qty))
  (edit! 'price '(((* base qty) (* base (+ qty 1)))))
  (price 10 3))"
; => 40
```

A refused edit — wrapping `car`'s argument in something it can't type —
prints `Error: edit!: rejected, introduces a type error: \`car\` expects a
list, got Int64` and exits non-zero. Wrapping the attempt in `errorset`
confirms the rollback: `f`'s source, type, and behavior are all untouched:

```
$ lamedh -s "(progn
  (defun f () (car (list 1 2 3)))
  (errorset (list 'edit! ''f ''(((2 1) (list 1 2 3) 5))))
  (list (see-source 'f) (see-type 'f) (f)))"
; => ((LAMBDA () (CAR (LIST 1 2 3))) (CHECKED (-> () INT64)) 1)
```

Editing a **record** edits the seed instead of the expansion: the patched
`defrecord` source (from `condense-source`) is re-evaluated, derivations
re-derived, attached examples re-run, and checker statuses refreshed — one
minimal edit regenerates and re-verifies the whole artifact:

```
$ lamedh -s "(progn
  (defrecord invoice (id int64) (amount int64) (status symbol)
    (:invariant (>= amount 0)) (:derive equality lens))
  (deflaw invoice-nonnegative (:for invoice) (:assert (>= amount 0)))
  (example valid-draft (:for invoice) (:given (make-invoice 1 100 'draft))
                       (:expect (validate-invoice *it*)))
  (edit! 'invoice '(((5 1) (>= amount 0) (>= amount 1))))
  (condense-check 'invoice))"
; => (T (VALID-DRAFT . T))
```

The tightened invariant (`amount >= 1` instead of `>= 0`) is re-checked
against `valid-draft` immediately: an amount of `100` still passes.

### Fingerprints and staleness

Condensation is a one-way lens: you cannot recover the seed from an edited
expansion. Rather than prohibit hand edits to generated code, the library
detects them: every generated symbol is fingerprinted (via `see-source`) at
`defrecord`/`derive` time, and `condense-stale` reports which have drifted:

```
$ lamedh -s "(progn
  (defrecord invoice (id int64) (amount int64) (status symbol) (:invariant (>= amount 0)))
  (defun invoice-amount (self) (* 2 (record-ref self 'amount)))
  (condense-stale 'invoice))"
; => (INVOICE-AMOUNT)
```

Hand-redefining `invoice-amount` to double the field flags it stale
immediately. `condense-recheck!` bundles staleness, drift, `condense-check`,
and `condense-check-type` into one re-verification call. The sanctioned way
to change generated code is `edit!` on the seed, not hand-editing the
generated function.

### `condense-check-type` and the dynamic frontier

`see-type` (a builtin) reports the checker's verdict on a symbol as data:
`(TYPED sig tier)`, `(CHECKED scheme)`, `(DECLARED scheme)`, `(TYPE-ERROR
msg)`, or `(DYNAMIC reason)`. `condense-check-type` classifies every
generated symbol of a record — refining `CHECKED` into `CHECKED` vs.
`VACUOUS` depending on whether the result type is actually constrained by an
argument — and records the unproven remainder as
`"condense.dynamic-frontier"`.

`examples/npcs.lisp` is a good real demonstration: three `defrecord` kinds
share hand-written row-typed accessors (`npc-name`, `npc-hp`, via
`record-ref`) plus a `greet` method specialized per kind. The agent-facing
entry point, `check-file!`, loads a file and reports honest verdicts for
everything it defines, repeating the unproven/broken remainder under
`frontier`:

```
$ lamedh --capability READ-FS -s "(mapcar #'car
  (cdr (assoc 'frontier (check-file! \"examples/npcs.lisp\"))))"
; => (TAUNT-ALL SCUFFLE)
```

`npc-name` and `alive-p` (also defined in that file, over `record-ref`) are
`CHECKED` with a row scheme whose result type is pinned by an argument —
`(FORALL (A B) (-> ((RECORD ((NAME A)) B)) A))`. As of the 0.3 census the
greet methods left the frontier too: `concat` gained a checker-native rule
(variadic strings → string), so `goblin-greet` now derives a full row
scheme. `taunt-all` and `scuffle` remain — they dispatch through `method`
(a runtime name computation nothing static can pin), so `condense-classify`
reports them honestly as unproven rather than folding them in with the
proven functions. That is the frontier report's whole point: it never
silently blends an unproven definition into "verified."

`check-file!` is the workflow for agents that edit files with their own
tools rather than holding a live REPL image: edit the file, run
`check-file!`, and `condense-diff` two reports to see exactly what an edit
changed in the type story.

## 8.6 Regular Expressions

The pattern language above matches s-expression *structure*. For matching
*text*, `lib/44-regex.lisp` wraps Rust's `regex` crate (RE2 semantics:
guaranteed linear-time matching, Unicode-aware, no backreferences or
lookaround). It is an optional module — `(require 'regex)` — and needs no
capability, since matching is pure and cannot run away even on an untrusted
pattern.

Every function takes either a compiled regex (from `regex:compile`) or a raw
pattern string; hoist `regex:compile` out of a loop when reusing a pattern.
A match is reported as a `(TEXT START END)` triple whose indices are
character offsets, end-exclusive — the same convention as `substring`.

```lisp
(require 'regex)

(regex:match-p "^\\d+$" "12345")
; => T

(regex:find-all "\\w+" "one two three")
; => (("one" 0 3) ("two" 4 7) ("three" 8 13))

(regex:replace-all "\\s+" "a   b  c" "_")
; => "a_b_c"

(let ((re (regex:compile "(?P<user>\\w+)@(?P<host>\\w+)")))
  (regex:named-groups re "alice@example"))
; => (("user" "alice" 0 5) ("host" "example" 6 13))
```

The twelve functions are `compile`, `regex-p`, `pattern`, `escape`,
`match-p`, `find`, `find-all`, `groups`, `named-groups`, `replace`,
`replace-all`, and `split`. `import` them for unqualified names, or call
them `regex:`-qualified. See the [generated reference](../generated-reference.md)
for each one's full signature and semantics.
