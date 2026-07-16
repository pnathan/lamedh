# Lamedh — LLM reference (llms.txt)

Lamedh (ל) is an embeddable Lisp 1.5 interpreter written in Rust: a tree-
walking evaluator with lexical closures, macros, fexprs, Kernel-style `vau`
operatives, dynamic variables, capability sandboxing, hash tables, arrays,
branded records, sum types, typed protocols, conditions/restarts, and an
optional typed JIT (Cranelift) that compiles scalar/array-typed `defun`/
`defun*` bodies to native code. Version 0.4.0. Source:
https://github.com/pnathan/lamedh · Docs (mdBook):
https://pnathan.github.io/lamedh/ · This file:
https://pnathan.github.io/lamedh/llms.txt (also `llms.txt` at the repo
root). Generated from the live interpreter's help database — do not hand-edit;
regenerate with `scripts/generate-llms-txt.sh` (see `scripts/generate-docs.sh`).

Lamedh has near-zero presence in LLM training data. Trust this file and the
interpreter's own error messages (they carry did-you-mean and Common-Lisp-ism
guidance) over prior assumptions about "how Lisp works here."

## 1. Syntax & semantic gotchas

- Symbols intern **UPPERCASE**. `(defun foo ...)` and `FOO` name the same
  symbol; case only matters inside strings/chars.
- `if` takes **exactly three** arguments — no implicit `nil` else branch.
  Use `cond`/`when`/`unless` for that.
- Character literals are `'c'` (C-style, single-quoted, `\n \t \\ \'`
  escapes), **not** `#\c`. A char is a byte (0-255); multibyte text is a
  string. Reader subtlety: `'a'` is a char, `'a` is a quoted symbol.
- Integers are 64-bit and **wrap** (sets the `OVERFLOW` flag; no bignums).
  `(/ 7 2)` truncates to `3`; `(/ 7.0 2)` → `3.5`. No `float`/`coerce` —
  multiply by `1.0` to convert.
- `sort`, `rplaca`, `rplacd` are **non-destructive** (return new
  structure); `nreverse` is just `reverse`. Mutate via hash tables, arrays,
  `setq`/`setf`/`push`/`pop`/`incf`/`decf` (all present).
- Container access is **collection-first**: `(gethash table key)`,
  `(aref array index)` — opposite of some CL reflexes. HOFs stay
  function-first (`mapcar`, `filter`, `map`, `reduce`, `sort`).
- Records: **`defrecord` is the one form** — `(defrecord Name (field
  type)... [(:invariant expr...)] [(:derive equality|printer|lens ...)])`.
  No `defstruct`/`defclass` (removed in 0.3). Values are immutable;
  `record-with` returns an updated copy. `record-ref`/`record-with` are
  row-polymorphic (work on any record with the named field, by brand or by
  row type) — see Example 1 below.
- Dispatch: **`defprotocol` + `definstance`** is the one dispatch system
  (typed, dispatch-position-aware, fn-first HOFs like `map`/`length` are
  protocols too). `implements-p`/`implements!` check conformance. No
  CLOS, no `defgeneric`/`defmethod`.
- `defun*` accepts optional per-parameter type hints — `(defun* f ((x
  int64)) (* x x))` — and silently falls back to a plain (dynamic) lambda
  when inference is ambiguous; `(signature 'f)`, `(compiled-p 'f)`, and
  `(why-not-typed 'f)` introspect the result instead of guessing. Plain
  `defun` parameters must be bare symbols (no type hints) but the checker
  still attempts inference on the body — `see-type`/`explain-compile`
  report `(TYPED ... COMPILED)`, `(CHECKED ...)`, or `DYNAMIC`.
- Conditions are first-class values, not a class hierarchy: `(error msg
  data)` signals; `handler-case`/`handler-bind`/`restart-case` catch.
  Canonical restart shape: `restart-case` **wraps** a `handler-bind` that
  wraps the risky call (restarts established *inside* the protected code
  are already unwound by the time a `handler-bind` handler runs — see
  Example 5).
- Capability sandbox: the CLI enables every capability by default (use
  `--sandbox` to start with none); the library API (`Environment::new`)
  starts with none, enabled explicitly via `env.enable_feature("READ-FS")`
  etc. `--mcp` is sandboxed by default regardless (untrusted code).
- Matching: `match`/`pat-match`/`destructuring-bind` patterns are plain
  cons structure — `(add ?a ?b)`, not `(list 'add ?a ?b)`. `?x` binds,
  `??xs` is a segment var, `?_` is a wildcard/default clause.
- `dump-docs`/`render-function-index-md`/`render-llms-index` (this file's
  generator) all read `lib/99-help-data.lisp`'s `HELP-DB` at call time —
  `(help 'name)` in the REPL is the same data, interactively.

### Tooling one-liners

- `lamedh --check file.lisp...` — static verify without executing: parse
  errors, unbound-call did-you-mean/CL-ism hints, provable arity
  mismatches. Exit 0 clean / 1 warnings / 2 parse-or-read failure. Add
  `--error-format=sexpr` for machine-readable findings.
- `lamedh --fmt file.lisp...` / `--fmt-check` — canonical formatter
  (indentation/whitespace only, never reflows tokens or touches
  string/char/comment content); `--fmt-check` reports without writing.
- `lamedh --test file-or-dir...` — run every `deftest`-registered test;
  `test result: N passed; M failed` summary, exit 0/1/2.
- `lamedh --mcp [--capability X]...` — Model Context Protocol server over
  stdio (JSON-RPC 2.0), one persistent environment, sandboxed by default.
  Tools: `eval`, `check`, `doc`, `apropos`, `run-tests`, `introspect`. See
  §4.10 below and `docs/mcp.md`.
- `lamedh --fuel N` — kernel step budget backstop for one script/`-s`/REPL
  line; arms `WITH-FUEL`, disables native JIT for the metered unit.
- `lamedh -i file-or-dir` (repeatable, loads before REPL/`-s`), `-s
  '(expr)'` (repeatable, one shared env), `-c/--capability NAME`
  (repeatable), `script.lisp arg1 arg2` (batch mode, args in `*ARGV*`).

### Coming from Common Lisp

| CL reflex | Status in Lamedh |
|---|---|
| `loop`, `do` | absent — use `dotimes`, `dolist`, `while`, `for`, `mapcar`/`reduce`/`filter` |
| `defstruct` | removed in 0.3 — use `defrecord` |
| `defclass`/`defgeneric`/`defmethod` (CLOS) | absent — use `defrecord` + `defprotocol`/`definstance` |
| `defconstant` | absent — use `def` (no separate constant form) |
| `multiple-value-bind`, `values` | absent — return a `list` (or record) and use `destructuring-bind` |
| `with-open-file` | absent — use `with-open-port` |
| `labels` | absent — `flet`/`macrolet` exist; mutual local recursion needs top-level `defun` |
| `eql`, `equalp` | absent — `eq` compares numbers/chars by value, `equal` is structural, `string=` for strings |
| `#(1 2 3)` vectors | absent — `(make-array n init)`/`(array n)` + `aref`/`fetch`/`store` |
| `type-of` | absent — `see-type` (checker verdict), predicates (`stringp` etc.), `record-brand` |
| `setf`, `push`/`pop`, `incf`/`decf`, `subseq`, `elt` | present, work as expected |
| `catch`/`throw`, `block`/`return-from`, `unwind-protect`, `prog`/`go` | present |
| `#'`, `funcall`, `apply` with spread args | present (Lamedh is a Lisp-1: functions and values share one namespace, so `#'`/`funcall` are optional, not required) |

Full detail: `docs/cl-divergences.md`. Full function reference (verbose,
one entry per symbol): `docs/generated-reference.md`.

## 2. Module map

Every `.lisp` row below is embedded in the binary; `with_stdlib()` loads
all of them, `with_prelude()` loads only Prelude rows, `(require 'name)`
pulls in an optional one by its "Requirable as" name on a `with_prelude()`
environment. Extracted from `src/lib.rs`'s doc comments (`scripts/generate-
llms-txt.sh` re-extracts this table on every regeneration).

{{MODULE_TABLE}}

## 3. Function index

Dense, one line per `HELP-DB` entry: `NAME [tag] SIGNATURE -- description`.
`[f]`=function, `[m]`=macro, `[s]`=special-form, `[v]`=variable. This is
the subset of the standard library with a registered `HELP-DB` doc entry
(`(help 'name)` in the REPL shows the full entry: args, return value,
worked examples, see-also). Generated live from the built interpreter —
see `lib/97-doc-renderer.lisp`'s `render-llms-index`. Modern-era forms not
yet in `HELP-DB` (`defrecord`, `defprotocol`/`definstance`, `defvariant`/
`variant-case`, `match`/`pat-match`, `defmodule`/`with-module`, guards/
fuel/spawn, conditions/restarts, the CL-compat layer, `regex:*`) are
covered by the module map above and the worked examples below instead —
signatures for those are in `docs/manual/`.

{{FUNCTION_INDEX}}

## 4. Worked examples

Every example below was verified with `lamedh --check <file>` (zero
findings) and by actually running it (`cargo run --release -q -- <file>`)
against this build.

### 4.1 Records + row polymorphism

One function reads an `x` field from *any* record that has one — no
interface declared, checker-inferred as `(forall (a b) (-> ((record ((x
a)) b)) a))`.

```lisp
(defrecord point (x int64) (y int64))
(defrecord box (x int64) (y int64) (w int64))

(defun worth (r) (record-ref r 'x))

(princ (list (worth (make-point 3 4)) (worth (make-box 1 2 9))))
(terpri)
(princ (record-with (make-point 3 4) 'x 99))
(terpri)
; => (3 1)
; => #S(POINT 99 4)
```

### 4.2 Typed protocols

`defprotocol` declares a dispatchable name; `definstance` supplies a typed
implementation per record brand. `implements!` asserts a brand honors a
set of protocols, or errors naming the gap.

```lisp
(defrecord goblin (name string) (hp int64))

(defprotocol greet "a one-line greeting")
(defprotocol damage "apply N damage, return the updated record")

(definstance greet ((self goblin)) string
  (concat (goblin-name self) " snarls."))

(definstance damage ((self goblin) (n int64)) goblin
  (record-with self 'hp (- (goblin-hp self) n)))

(princ (greet (make-goblin "Grix" 10)))          ; => Grix snarls.
(princ (goblin-hp (damage (make-goblin "Grix" 10) 3)))  ; => 7
(princ (implements! 'goblin 'greet 'damage))
; => ((GREET . INSTANCE) (DAMAGE . INSTANCE))
```

### 4.3 Structural pattern matching

Patterns are plain cons structure: `?x` binds, `??xs` is a segment
variable, `?_` is a wildcard/default clause, a bare atom matches
literally (no `(list ...)` wrapper needed).

```lisp
(defun classify (form)
  (match form
    ((add ?a ?b) (+ ?a ?b))
    ((neg ?a) (- 0 ?a))
    ((?head . ?rest) (list 'unknown-op ?head (length ?rest)))
    (?_ 'not-a-form)))

(princ (classify '(add 3 4)))   ; => 7
(princ (classify '(neg 5)))     ; => -5
(princ (classify '(mul 2 3)))   ; => (UNKNOWN-OP MUL 2)

(destructuring-bind (?a ?b . ?rest) (list 1 2 3 4)
  (princ (list ?a ?b ?rest)))   ; => (1 2 (3 4))
```

### 4.4 Sum types

`defvariant` + exhaustive `variant-case` (the checker requires every
constructor covered, or an explicit `else`). The stdlib's `Option`/
`Result` are ordinary `defvariant`s.

```lisp
(defvariant shape
  (circle (r int64))
  (rect (w int64) (h int64)))

(defun area (s)
  (variant-case s
    (circle (r) (* 3 (* r r)))
    (rect (w h) (* w h))))

(princ (list (area (circle 3)) (area (rect 4 5))))  ; => (27 20)

(princ (unwrap-or (option-map (lambda (x) (* x 2)) (some 5)) 0))  ; => 10
(princ (unwrap-or (option-map (lambda (x) (* x 2)) (none)) 0))    ; => 0
```

### 4.5 Conditions and restarts

Canonical shape: `restart-case` **wraps** a `handler-bind` that wraps the
risky call — a restart established *inside* the protected code is already
unwound by the time a `handler-bind` handler fires, so it must be
established around the handler instead.

```lisp
(princ (handler-case (/ 1 0)
         (error (e) (list 'caught (error-message e)))))
; => (CAUGHT "Division by zero")

(princ
 (restart-case
     (handler-bind ((error (lambda (e) (invoke-restart 'use-value 99))))
       (error "bad input" (list :code 42)))
   (use-value (v) v)))
; => 99

(princ (ignore-errors (/ 1 0)))   ; => ()  (ERRORSET-backed; no reason string)
(princ (ignore-errors (+ 1 2)))   ; => 3
```

### 4.6 Arrays + `defun*` native compilation

`defun*` with scalar/array type hints compiles straight to native
Cranelift code — `see-type` reports `COMPILED`, not just `CHECKED`.

```lisp
(defun make-arr (lst)
  (let ((a (array (length lst))) (i 0))
    (mapcar (lambda (x) (progn (aset a i x) (setq i (+ i 1)))) lst)
    a))

(defun* array-sum ((a (array int64)) (n int64))
  (let ((total 0) (i 0))
    (while (< i n)
      (setq total (+ total (aref a i)))
      (setq i (+ i 1)))
    total))

(princ (see-type 'array-sum))
; => (TYPED (-> ((ARRAY INT64) INT64) INT64) COMPILED)
(princ (array-sum (make-arr (list 1 2 3 4 5)) 5))  ; => 15
```

### 4.7 Modules

`defmodule` + `with-module` is a naming discipline over the flat global
namespace (`with-module` stores definitions as `MODULE:SYMBOL`); `import`
snapshots a module's exports into the caller's namespace. (Calls below go
through `funcall` because `lamedh --check` — a static verifier that never
executes the file — does not yet trace names `with-module`/`import`
create at runtime; direct `geometry:area`/`area` calls work fine when the
file actually runs.)

```lisp
(defmodule geometry (:export area))

(with-module geometry
  (defun helper (x) (* x 3))
  (defun area (r) (helper (* r r))))

(princ (funcall 'geometry:area 2))   ; => 12
(import geometry)
(princ (funcall 'area 3))            ; => 27
```

### 4.8 Regex

The `regex:*` module (0.3.1); names are qualified (`regex:compile`, ...)
or brought in with `(import regex)`.

```lisp
(let ((re (regex:compile "[0-9]+")))
  (princ (regex:match-p re "order #42"))          ; => T
  (princ (regex:find-all re "a1 b22 c333"))
  ; => (("1" 1 2) ("22" 4 6) ("333" 8 11))
  (princ (regex:replace-all re "a1 b22" "N")))     ; => aN bN
```

### 4.9 Testing with `deftest`

```lisp
(deftest add-works
  (assert-equal (+ 1 2) 3))

(deftest strings-concat
  (assert-equal (concat "a" "b") "ab"))

(princ (run-tests))
; => (ASSERTIONS-PASSED 2 FAILED 0)
; => ALL-TESTS-PASSED
```

Same suite from the shell: `lamedh --test file.lisp` prints `test result:
2 passed; 0 failed` and exits 0.

### 4.10 `--mcp`: driving Lamedh as an agent tool

`lamedh --mcp` speaks newline-delimited JSON-RPC 2.0 over stdio against
one persistent, sandboxed (all capabilities off by default) interpreter.
A minimal session:

```json
→ {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}
← {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"lamedh","version":"0.4.0"}}}
→ {"jsonrpc":"2.0","method":"notifications/initialized"}
→ {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"eval","arguments":{"source":"(setq x 5)"}}}
← {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"5"}],"isError":false}}
→ {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"eval","arguments":{"source":"(lenght nil)"}}}
← {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"Error: Unbound variable: LENGHT — did you mean LENGTH?"}],"isError":true}}
```

Six tools: `eval` (fuel-fenced, persistent env), `check` (static, same
findings as `--check`), `doc` (REPL `help` text for a symbol), `apropos`
(substring symbol search), `run-tests` (fresh scratch env per call),
`introspect` (`signature`+`compiled-p`+`why-not-typed` in one call). Grant
capabilities with repeatable `--capability NAME` (`--sandbox` is a no-op —
all-off is already the default). Full protocol/tool schema: `docs/mcp.md`.
