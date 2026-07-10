# The 0.3 Language Census

A systematic review of every special form, kernel builtin, and stdlib
definition, examining each against its expectations: arity/variadicity,
argument order, naming, duplication, and checker knowledge. Every claim
below was probed against the built interpreter. This document drives the
close-of-0.3 batches; items marked ✔ are done, → are batch assignments.

Surface counted: 40 special forms, ~238 kernel-registered names, ~496
stdlib definitions.

## Conventions (ratified)

- **Variadic operators**: anything that folds associatively is variadic —
  `+ - * and or max min list concat` ✔, comparisons chain ✔ (0.3).
- **Argument order**: container operations are COLLECTION FIRST
  (`gethash/sethash/remhash/fetch/store/getp/putp` ✔); search functions
  are NEEDLE FIRST (`member/assoc/exists` — Lisp heritage, coherent as a
  class). `alist-get` is a container op (collection first) — both orders
  are correct under the convention; documented.
- **Naming**: predicates end `-p`; mutators end `!`; conversions use `->`.

## Findings: variadicity (→ batch 1)

| name | expectation | observed | action |
|---|---|---|---|
| `append` | variadic | 2-ary errors on 3 | variadic + checker rule |
| `logand` | variadic | variadic ✓ | type rule |
| `logior`/`logxor` | variadic like logand | 2-ary! (inconsistent) | variadic + parity |
| `gcd`/`lcm` | variadic fold | 2-ary | variadic |
| `mapcar` | N lists (zip) | 1 list only | multi-list mapcar |
| `and`/`or`/`max`/`min`/`concat`/`list` | variadic | ✓ | — |

## Findings: duplicates and near-duplicates (→ batch 2)

| names | verdict |
|---|---|
| `remhash` / `delete-key` | duplicate; **remhash canonical**, delete-key removed |
| `sethash` / `set-bang` | set-bang is the primitive, sethash the documented name; keep both, one doc home |
| `subseq` / `substring` | intentional CL-compat alias; keep |
| `fetch`/`aref`, `store`/`aset` | intentional aliases; keep |
| `null` / `not` | same function (Lisp 1.5); keep |
| `remp` | gone already (remprop canonical) ✔ |
| `string-length`/`array-length` vs generic `length` | keep as *typed* specifics (length stays gradual-generic) |
| `hash-count` | MISSING despite docs; `length` doesn't cover hash | extend `length` to hash tables |
| `DECLARE-TYPED` vs `declare-type!` | different things, confusable names (forward decl vs axiom). Documented; rename deferred |
| `DEFINE` vs `DEF` vs `DEFVAR` | DEFINE = Lisp 1.5 batch form; DEFVAR = DEFDYNAMIC alias; distinct, keep |
| `BLOCK/RETURN-FROM`, `CATCH/THROW`, `PROG/RETURN/GO` | three escape mechanisms — heritage tiers, all tested; keep |

## Findings: genericity (→ batch 4)

Already generic: `length` (list/string/array — NOT hash), `elt`
(list/string/array), `reverse` (list/string), `record-ref` (any record).
Missing: `length` on hash; generic `map`/`filter`/`reduce`/`for-each`
(list-only today: mapcar/filter/reduce; array has array-map; hash has
maphash) — one sequence story: results follow the input's kind
(list→list, array→array, string→string; hash iterates (key . value)
pairs).

✔ Shipped as the `length`/`map`/`for-each` typed protocols
(lib/29-protocols.lisp): `length` covers hash; `map` is kind-preserving
over list/array/string; `for-each` visits list/array/string/hash (hash
gets `(fn key value)`).

✔ Access protocols (Paul's "functions with type names built in"
observation, post-census): the remaining cross-type duplicate operations
are unified as protocols — **`ref`** (strict read at index/key over
list/array/string/hash/records; absence ERRORS, which is what lets every
instance carry an honest result type — the lenient nil-on-miss reads
keep their old names: `gethash`/`nth`/`elt`), **`put!`**
(array/hash write, returns the value), **`copy`** (list/array/string/
hash + the Lisp 1.5 structure copy as atom fallback; `copy-hash` was
MISSING entirely and is new). The type-prefixed names remain as the
monomorphic substrate that instances dispatch to and compile through;
the bare protocol names are the taught vocabulary.

**Ruling — `char-code` vs `char->code`: keep both.** Probed: not
duplicates — `char-code` is the strict kernel primitive (string-only),
`char->code` the coercing wrapper (char, one-char string, or int
passthrough). Same relationship as `parse-integer` vs `string->number`.

**Ruling — `filter`/`reduce` stay list-specific, fn-first.** They belong
to the fn-first heritage class (`mapcar`/`mapc`/`every`/`exists`), and
protocols dispatch on the FIRST argument, so protocolizing them would
either flip their argument order out from under every existing caller or
bless two argument orders permanently. The generic sequence story is the
collection-first protocols; when you need a generic filter/reduce today,
go through `map`/`for-each` or convert at the edge (`array->list`,
`string->list`). Revisit only if 0.4's dispatch can key on a non-first
argument.

## Findings: checker knowledge (→ batch 3, "typing with vigor")

The checker natively understands: arithmetic, comparisons, cons/car/cdr/
list/null, records (record-ref/-with/-new), let/progn/cond/when/quote.
Everything else degrades to `Any` at call sites. High-value honest
schemes to declare (each verified against evaluator behavior first):

- **Predicates** (`consp numberp stringp symbolp floatp charp zerop ...`):
  `(forall (a) (-> (a) bool))` — clears the largest VACUOUS frontier.
- **List functions**: `mapcar : (forall (a b) (-> ((-> (a) b) (list a)) (list b)))`,
  `filter`, `reduce : (forall (a b) (-> ((-> (b a) b) (list a) b) b))`
  (verified acc-first), `member`, `assoc`, `nth`, `reverse` (list case is
  the declared one? no — reverse is generic, stays gradual), `exists`,
  `every`, `notany`.
- **String functions**: `substring : (-> (string int64 int64) string)`,
  `string-length`, `string-upcase/downcase`, `string-index-of`,
  `string-split : (-> (string string) (list string))`, `concat` (native
  variadic rule: strings → string; verified concat rejects non-strings).
- **Math library**: `sqrt/sin/cos/...` return `float64` (int args coerce —
  verified `(sqrt 4)` = 2.0), `floor/ceiling/round/truncate → int64`,
  `abs` numeric, `gcd/lcm → int64`.
- **Variadic operators** get native checker RULES (declared schemes are
  fixed-arity): `append` (all `(list a)` → `(list a)`), `concat`,
  `min/max` (numeric chain), `logand/logior/logxor` (int64), `gcd/lcm`.

## Ruled on and left alone

- Loop zoo (`while/for/prog/dotimes/dolist/label`): tiers of heritage,
  all metered, all tested.
- Print family (`print/princ/terpri/prin1-to-string/princ-to-string`):
  distinct semantics (read-back vs display).
- Equality zoo (`eq/equal/=`): identity / structural / numeric — three
  real relations; `string-equal` is case-insensitive comparison, not a
  duplicate.
- `format` vs `format-str`: print vs build — distinct.
