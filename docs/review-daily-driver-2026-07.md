# Review: Lamedh as a daily driver for LLM coding (July 2026)

Scope: full-repo assessment of gaps and clear defects, evaluated against one
goal — *using Lamedh as an effective daily driver for LLM-assisted coding*.
Method: code reading (kernel, stdlib, CLI, docs) plus ~80 hands-on probes
against the built binary, cross-checked against the open issue tracker so this
review does not duplicate the known JIT/optimizer soundness backlog
(#220–#234, #210, etc.).

Baseline health is genuinely good: the workspace builds warning-free, clippy is
completely clean, and every test binary in the suite passes. TCO is real
(1,000,000-deep tail recursion works), the recursion guard produces a clear
actionable message, strings are Unicode-correct (char-indexed `substring`/
`string-length*`/`string->list`), dynamic variables interact correctly with
`let` rebinding, `handler-case`/`errorset`/`unwind-protect` behave, `#'`
syntax / keywords / quasiquote with splicing / `&rest` macros all work, and the
typed JIT delivers (interpreted `fib 25` ≈ 0.9 s vs near-instant via
`defun-typed`). The capability sandbox (FS/shell/stdin off by default) is a
real strength for running LLM-generated code.

The findings below are ordered by how much they hurt the LLM-coding loop:
an agent writes code → runs it → reads the error → repairs. Anything that
makes step 3 vague or step 2 lie about success is a first-order problem.

---

## P0 — breaks the agent feedback loop

### 1. No source locations anywhere (parse or runtime)

The reader (`src/reader.rs`) tracks no line/column spans. Consequences:

- Parse errors are raw nom internals:
  `Error: Parse error: Parsing Error: Error { input: "(foo", code: Tag }`
- Runtime errors carry no file, line, offending form, or call stack:
  `Error: car requires a list` (which call, of hundreds?)

For a human at a REPL this is an annoyance; for an LLM repairing a 300-line
file it is the difference between one fix iteration and five. This is the
single highest-leverage improvement available. Minimum viable version:
line/column on parse errors, and "while evaluating `(CAR 5)` in `F`" one-frame
context on runtime errors. (`docs/appendix_limitations.md` already lists stack
traces as future work; the parse-error half is cheaper and just as valuable.)

### 2. A file with one bad form loads as *nothing*, and the CLI carries on

`load_file` parses the entire file before evaluating anything, so a parse
error at line 400 means the 399 good lines above it are *not* loaded — and the
error names the file but no line:

```
$ lamedh -i bad.lisp -s "(ok)"       # bad.lisp defines OK, then has a typo
Error loading file bad.lisp: Generic("Failed to parse file ...: code: Tag")
Error: Unbound variable: OK
```

Worse for automation: `-i` load errors (parse *and* runtime) are only
`eprintln!`'d (`cli/src/main.rs:109-127`); the process continues and can exit
0. An agent doing `lamedh -i src.lisp -s "(run-tests)"` can get a green exit
from a file that failed to load. `-i` failures should set a nonzero exit (or
at least a `--strict` flag should exist).

### 3. The REPL cannot accept multi-line input — or two forms on one line

Pasting a conventionally formatted defun fails line-by-line:

```
(ל)> (defun add2 (x)
Error: Parse error: ... code: Tag
(ל)>    (+ x 2))
Error: Parse error: Unexpected input: )
```

`eval_line` is called per line with no continuation on incomplete input
(`cli/src/main.rs:156-163`). Also inconsistent: `-s "(+ 1 2) (+ 3 4)"`
evaluates both forms via `eval_all`, but the same two forms on one REPL line
give `Unexpected input`. Both humans and LLM-driven tmux/expect sessions paste
multi-line code constantly. Fix: buffer input while the reader reports
"incomplete" (requires the reader to distinguish incomplete from malformed —
same span work as finding 1 makes this natural).

### 4. `run-tests` dies at the first error escaping a test body

```
(deftest good (assert-equal 1 1))
(deftest bad (car 5))          ; a *bug* in a test, the normal TDD case
(deftest good2 (assert-equal 2 2))
(run-tests)
=> Error: car requires a list      ; no summary, good tests unreported
```

`run-one-test` (`lib/10-testing.lisp:63-65`) funcalls the thunk with no error
trap. A test framework's core contract is isolating test failures; for
LLM-driven TDD the erroring test must be recorded as a failure and the run
must continue. One `errorset`/`handler-case` wrapper fixes it. Related gaps in
the same file: re-`deftest`ing the same name appends a duplicate (reloading a
test file in a session double-registers everything), and there is no way to
turn a `run-tests` result into a process exit code because **no `(exit n)` /
`(quit)` exists** — CI must string-match stdout.

---

## P1 — silent wrong behavior (worse than erroring)

### 5. `T` is an ordinary mutable binding

```
(setq t nil)  ; accepted
(if t 'yes 'no)               => NO
(let ((t nil)) (if t 'x 'y))  => Y
(defun f (t) ...)             ; accepted
```

`NIL` is protected only by accident (it reads as the `Nil` value, so
`(setq nil 5)` fails with "must be a symbol"). `T` is a plain symbol and can
be rebound globally, locally, or as a parameter — and LLMs *love* using short
names. Everything downstream of a clobbered `T` (`cond` defaults, predicate
returns, `case`'s `t` clause) breaks with no diagnostic. Guard `T` (and
ideally keywords) in `SETQ`/`DEF`/binders in `src/evaluator/special_forms.rs`.

### 6. `&optional` / `&key` are silently bound as positional parameters

```
(defun f (a &optional b) ...)   ; defines 3-ary f with a param named &OPTIONAL
(f 1 2)                          ; binds &OPTIONAL=2, then arity-errors on B
(defun h (&key x y) (list x y))
(h :x 1)  => Error: lambda expected 3 arguments, got 2
```

Only `&rest` is implemented. That's a legitimate design choice, but the other
lambda-list keywords must be *rejected at definition time* with a clear
message ("`&optional` is not supported; use `&rest`"), not absorbed as
parameter names. This is among the most common CL reflexes an LLM will emit,
and today it produces confusing failures far from the definition site.

### 7. The docs teach a keyword `defstruct` constructor that doesn't exist

`lamedh-manual.md` §17, `CLAUDE.md`, and the `src/lib.rs` rustdoc all show
`(make-point :x 1 :y 2)`. Reality (and `tests/test_defstruct.rs`) is
positional: `(make-point 1 2)`. The documented form fails with the opaque
`lambda expected 2 arguments, got 4`. For LLM use this is doubly bad — the
docs are exactly what gets stuffed into the model's context. Either implement
keyword construction or fix all three documents. (Manual also says structs are
"hash tables with a `__type__` key"; the implementation is `LispVal::Struct`.)

### 8. Integer arithmetic wraps silently by default

`(+ 9223372036854775807 1)` → `-9223372036854775808`, with only the passive
`OVERFLOW` condition flag set (which nothing checks unless asked). Documented
in the limitations appendix, but "documented footgun" still bites generated
code doing e.g. factorial. Consider signaling on overflow by default with a
`WRAPPING-ADD`-style escape hatch, or at minimum have the REPL print a
warning when the flag transitions.

---

## P2 — stdlib gaps and inconsistencies an LLM hits daily

### 9. Missing CL staples (each small, collectively the biggest friction)

Verified absent: `setf`, `push`/`pop`, `incf`/`decf`, `remove`, `count`,
`copy-list*`, `list-length*`, `nreverse`, `subseq`, `elt`, `rem`,
`defparameter`, `read-from-string`, two-argument `floor`/`truncate`,
`apply` with spread args (`(apply #'+ 1 2 '(3))` — apply is strictly 2-ary),
`length` on strings, `reverse` on strings, `(/ 1.0 0.0)` errors instead of
returning `inf` (while the printer/reader do handle `inf`). Present and
working (so close to parity): `remove-if`, `butlast`, `1+`/`1-`, `mapcan`,
`assoc`, `member`, `reduce`, `sort`, `string=`.

Almost all of these are Lisp-layer work per the project's own philosophy —
`setf` on symbols/hash-tables/arrays is expressible with the existing
macro/fexpr machinery, and `push`/`incf` follow from it. A `21-cl-compat.lisp`
would close most of the reflex gap in one file.

### 10. Argument-order conventions are internally inconsistent and diverge from CL

- `gethash` is *(table key)*; CL is *(key table)*.
- `maphash` is *(table fn)*; CL is *(fn table)* — and this contradicts the
  repo's own documented convention that the functional layer is
  "function-first (CL style)" (CLAUDE.md re `13-functional.lisp`).
- `alist-get` is *(alist key)*; Elisp (where the name is from) is *(key alist)*.
- Meanwhile `nth`, `assoc`, `member`, `mapcar`, `sort` follow CL order.

The failure mode is nasty: `(maphash fn h)` doesn't error cleanly, it errors
as `keys requires a hash table as its first argument` from an inner helper.
Pick one rule, add CL-order aliases or arg-type-based dispatch for the
collection functions, and add a "divergences from CL" cheat-sheet doc — that
one page is the highest-value doc for LLM contexts.

### 11. Error messages omit the offending value

`car requires a list`, `Math functions only accept numbers`,
`concat only accepts strings` — none say what they got. `(car 5)` should
report `CAR: expected a list, got 5`. Cheap, mechanical, and directly
improves agent repair accuracy. (Good counter-example already in tree: the
recursion-limit message names the limit and both remedies.)

### 12. Reader polish

- No block comments `#| ... |#` (LLMs emit them).
- No `#x10`/`#b101` radix literals (the `FFh`/`177Q` forms exist but no LLM
  will guess them).
- A leading `#!` shebang line is a parse error, blocking executable scripts.
- Char literals are bytes 0–255, so `'日'` is a parse error with an opaque
  message (documented, but the error should say "char literals are single
  bytes; use a string").

### 13. CLI scripting ergonomics

- No positional file argument: `lamedh script.lisp` is an error; must be `-i`.
- No way to pass arguments to a script (no `*argv*`).
- No `(exit n)` (see finding 4).
- Ctrl-C in the REPL exits the process instead of cancelling the current line
  (`cli/src/main.rs:164-167`).

### 14. Documentation drift (docs are LLM context — drift is load-bearing here)

Issue #166 tracks this generally; concrete instances found:

- `docs/appendix_limitations.md` "Missing string operations: string search,
  case conversion" — both exist now (`lib/14-strings.lisp`:
  `string-index-of`, `contains-p`, `string-upcase`, ...).
- Same file recommends `(defun string-equal (s1 s2) (eq (intern s1) ...))`
  as a workaround although `string=` exists.
- `defstruct` keyword-constructor claims (finding 7) in three places.
- CLAUDE.md's stdlib table stops at `18-format.lisp` + "97–99", omitting
  `19-call-graph.lisp` and `20-condensation.lisp` (the lib.rs table has them).

---

## Suggested priority order

1. Reader spans + parse errors with line/column (unlocks 1, 3, and 12's error
   quality; biggest single lever).
2. `-i` failures → nonzero exit; runtime errors name the top form; REPL
   multi-line continuation.
3. Test-runner error isolation + `(exit n)` builtin (makes Lamedh CI-able).
4. Reject unsupported lambda-list keywords; protect `T`.
5. `21-cl-compat.lisp` (setf/push/incf/remove/subseq/elt/spread-apply...) +
   the "divergences from CL" one-pager.
6. Doc-truth pass (fold into #166): defstruct, limitations appendix, CLAUDE.md.

None of these collide with the open soundness/perf backlog (#220–#234), which
covers a different layer (JIT parity, optimizer correctness, TCO memory).
