# Divergences from Common Lisp

One page for readers (human or LLM) arriving with Common Lisp reflexes.
Lamedh descends from **Lisp 1.5**, not CL — most CL muscle memory works, but
the exceptions below bite silently. Every claim here is verified against the
interpreter; when in doubt, probe (`lamedh -s '<expr>'`).

## The five that bite first

1. **Lamedh is a Lisp-1.** One namespace for functions and values.
   `(let ((f (lambda (y) (* y 2)))) (f 21))` → `42` — no `funcall` needed
   (though `funcall`, `apply`, and `#'` all exist and work, so CL-shaped
   code runs unchanged).

2. **Integers are 64-bit and wrap.** No bignums, no rationals, no complex
   numbers (and no `most-positive-fixnum` constant). Adding 1 to
   `9223372036854775807` wraps to the most negative value and sets the
   `OVERFLOW` flag (the REPL warns on the transition).
   `(/ 7 2)` → `3` — integer division truncates; write `(/ 7.0 2)` → `3.5`
   for a float. Mixed int/float arithmetic contaminates to float, like CL.
   There is no `float`/`coerce`; multiply by `1.0` or divide to convert.

3. **Character literals are `'a'`, not `#\a`.** C-style, single quotes,
   with `\n \t \\ \'` escapes; a char is a byte (0–255), so multibyte
   characters must be strings. `#\a` is a parse error. Note the reader
   subtlety: `'a'` is the char, `'a` is the quoted symbol.

4. **`sort`, `rplaca`, and `rplacd` do not mutate.** `sort` returns a new
   list and leaves its argument untouched (CL's is destructive);
   `rplaca`/`rplacd` return a new cons. `nreverse` exists but is just
   `reverse`. State lives in hash tables, arrays, and rebinding
   (`setq`/`setf`/`push`/`pop`/`incf` all exist and work as expected).

5. **Container accessors are collection-first.** `(gethash table key)` —
   the reverse of CL's `(gethash key table)`. The 0.3 convention: HOFs are
   function-first like CL (`mapcar`, `filter`, `map`, `reduce`, `sort`);
   container access is collection-first like CL's `aref`/`elt`
   (`gethash`, `ref`, `put!`, `copy`, `sort-by`); searches are needle-first
   like CL (`member`, `assoc`). Only the hash-table names moved.

## Absent — use the replacement

| CL reflex | Status | Use instead |
|---|---|---|
| `loop`, `do` | absent | `dotimes`, `dolist`, `while`, `for`, `mapcar`/`reduce`/`filter` |
| CLOS (`defclass`, `defgeneric`, `defmethod`) | absent | `defrecord` + `defprotocol`/`definstance` (brand dispatch, one dispatch position) |
| `defstruct` | removed in 0.3 | `defrecord` (branded, row-subsumable, checker-native) |
| Packages (`defpackage`, `in-package`) | absent | one global namespace; `defmodule` for grouping |
| Multiple values (`values`, `multiple-value-bind`) | absent | return a list or a record; `destructuring-bind` exists |
| `labels` | absent | `flet` and `macrolet` exist; mutual local recursion needs top-level `defun` |
| `eql`, `equalp` | absent | `eq` compares numbers/chars by value; `equal` is structural; `string=` for strings |
| `define-condition`, `signal` | absent | errors are first-class values; raise with `error`, catch with `handler-case`/`handler-bind`/`restart-case` (all present), or Lisp 1.5 `errorset` (takes a **quoted** form) |
| `#(1 2 3)` vector literals | absent | `(make-array n init)` / `(array ...)`; `aref`/`fetch`/`store` work |
| Two-argument `floor`/`truncate` | absent | `(floor x)` is one-argument; `mod`/`rem` exist |
| `string<` family | absent | `string=` exists; compare via `sort-by` keys |
| `type-of` | absent | `see-type` (checker verdicts), predicates (`stringp`, `floatp`, …), `record-brand` |

## Same words, different behavior

- **`defvar` declares a dynamic variable** (alias of `defdynamic`) and
  earmuffed `let` bindings rebind it dynamically, like CL specials —
  but plain `def` defines a lexical global, and there is no
  `(declare (special …))`.
- **`case`** takes `t`, `otherwise`, *or* `else` as the default clause.
- **`format`** supports exactly `~a ~s ~d ~% ~~`; anything else (`~f`,
  `~&`, `~{`) passes through **literally** rather than erroring.
  Destinations `nil` (string) and `t` (stdout) behave as in CL.
- **`error`** produces a first-class error value that flows through
  `handler-case` — there is no condition class hierarchy; discriminate on
  the error's message/payload.
- **`car`/`cdr` of `nil`** return `nil` (CL-compatible; this is where
  Lamedh diverges from Lisp 1.5, which errored) — but `(car 5)` errors.

## CL reflexes that just work

`setf` `push` `pop` `incf` `decf` `subseq` `elt` `assoc` `member` `mapcar`
(including multi-list) `mapcan` `reduce` `remove` `remove-if` `count`
`find-if` `position` `butlast` `last` `apply` with spread args
(`(apply #'+ 1 2 '(3))`), `&rest` `&optional` `&key`, dotted parameter
tails, `let`/`let*`/`flet`/`macrolet`, `catch`/`throw`,
`block`/`return-from`, `unwind-protect`, `prog`/`go`/`return`,
`prog1`/`prog2`, quasiquote, `#'`, `#| block comments |#`, `#x`/`#o`/`#b`
radix literals, uppercase symbol interning, `defmacro`, `gensym`,
`ignore-errors`, `defparameter`, `string-upcase`/`string-downcase`.

## Things CL has no word for

Kernel-style **fexprs and `vau`** (unevaluated-argument operatives), an
**HM-style type checker** over plain `defun` (`see-type`, `check-type`,
`defun*`), a **typed JIT**, **branded row-typed records** (`defrecord`,
`record-ref`, `record-with`), **sum types** (`defvariant`,
`variant-case`, Option/Result), **capability sandboxing** (file, shell,
and IO access are off by default), and **fuel-bounded evaluation**
(`with-fuel`). See the [manual](manual/README.md).
