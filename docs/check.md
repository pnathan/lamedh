# Static Checking (`lamedh --check`)

`lamedh --check` statically verifies one or more `.lisp` files **without
executing them**. It parses each file, then lints for the two mistakes that
otherwise force you to run the code to discover: a call to a function that is
bound nowhere (usually a typo or a Common-Lisp-ism), and a call whose argument
count cannot possibly match a function whose arity is statically known.

It exists to close the edit/run loop — especially for LLMs, which have almost
no Lamedh in their training data and would otherwise learn the language by
trial and error. Diagnostics double as in-context teaching: an unbound call
carries a did-you-mean suggestion or a "that's Common Lisp, use X instead"
redirect.

## Usage

```text
lamedh --check file.lisp [file2.lisp ...]
lamedh --check --error-format=sexpr file.lisp
```

* Multiple files are checked **together**: a definition in one file satisfies a
  reference in another. Pass files that are meant to be loaded as a unit.
* `--check` with no files is an error.

### Exit codes

| Code | Meaning                                             |
|------|-----------------------------------------------------|
| 0    | Clean — no findings.                                |
| 1    | One or more **warnings** (lint findings).           |
| 2    | A file failed to **parse** or could not be read.    |

A hard error (exit 2) dominates: if any file has a parse/read error, the exit
code is 2 even if other files produced warnings.

## Output formats

`--error-format=human` (the default) prints one line per finding:

```text
file.lisp:12: warning: unbound function FOOBAR — did you mean FOO-BAR?
file.lisp:20: error: parse error: unexpected end of input (unclosed '(' ...)
```

`--error-format=sexpr` prints one readable s-expression per finding, for
programmatic consumption.

### The sexpr schema

Each finding is an association list with a **stable** set of keys, always in
this order:

```lisp
((file . "path/to/file.lisp")
 (line . 12)
 (column . 0)
 (severity . warning)
 (kind . unbound-function)
 (symbol . FOOBAR)
 (message . "unbound function FOOBAR — did you mean FOO-BAR?"))
```

| Key        | Type                | Notes                                                      |
|------------|---------------------|------------------------------------------------------------|
| `file`     | string              | As passed on the command line.                             |
| `line`     | integer (1-based)   | The offending top-level form's starting line (or the parse-error line). |
| `column`   | integer (1-based)   | Only meaningful for parse errors; otherwise `0`.           |
| `severity` | symbol              | `warning` or `error`.                                      |
| `kind`     | symbol              | `parse-error`, `unbound-function`, `arity-mismatch`, or `file-error`. |
| `symbol`   | string or `nil`     | The offending symbol, when there is one.                   |
| `message`  | string              | A self-contained, human-readable explanation.             |

Strings escape `"` and `\`. `symbol` is `nil` when the finding has no
associated symbol (e.g. a parse error). New keys may be **appended** in future
versions; existing keys and their order will not change.

## What it catches

**Parse errors** (severity `error`). The reader's 1-based line/column is
reported. As with file loading, checking a file stops at its first parse error.

**Unbound operator** (`unbound-function`, severity `warning`). A symbol in
operator/function position that is bound neither in the standard library, nor
by any definition anywhere in the checked file(s), nor by an enclosing local
binding form. The message is enriched via the same "teaching errors" machinery
the runtime uses: a Levenshtein did-you-mean list, or a targeted redirect for a
known Common-Lisp-ism (`LOOP`, `DEFSTRUCT`, `MULTIPLE-VALUE-BIND`, …).

**Arity mismatch** (`arity-mismatch`, severity `warning`). A call to a function
whose arity is statically knowable — a function defined in the checked files,
or a plain stdlib lambda — that **cannot** match: too few required arguments,
or too many with no `&rest`/`&key` tail. Only provable impossibilities are
reported.

## The conservativeness contract (zero false positives)

> A checker that cries wolf is worse than none.

If `lamedh --check` produced spurious findings, an LLM would learn to distrust
it and ignore the real ones. So the checker is deliberately biased toward
**silence when in doubt** — it prefers a false negative (missing a real
problem) over a false positive (inventing one). The regression net is a test
that runs the checker over every stdlib, example, and benchmark file in the
repository and requires **zero** findings.

Concretely:

* A full standard-library environment is built so every builtin, stdlib
  function, macro, and operative is a *known* name. **The user's file is never
  evaluated.**
* A first pass collects every top-level definition across all checked files, so
  forward references and cross-file references never look unbound. This includes
  the names generated by `defrecord` (`make-N`, `N-p`, `N-field`, `validate-N`)
  and `defvariant` (each bare constructor, its predicate, and its accessors).
* The linter recurses **only** through forms whose binding and evaluation
  semantics are known exactly:
  * `quote` is skipped entirely; `quasiquote` is skipped as well (its unquotes
    are not descended — a deliberate coverage sacrifice for safety).
  * `defun` / `defun*` / `defun-typed` / `lambda` bind their parameters
    (including `&optional` / `&key` / `&rest`) before their bodies are walked.
  * `let` / `let*` / `prog` / `for` / `label` / `handler-case` bind their
    variables; `cond` / `if` / `and` / `or` / `progn` / `when` / `unless` /
    `setq` / `block` / `catch` / `throw` / `unwind-protect` are walked as plain
    expressions.
  * **Modules are understood** (`defmodule` / `with-module` / `import`). A
    `defmodule`'s `:export` list is recorded; a `with-module` body is descended
    with the module context pushed, so its definitions are collected under
    their qualified `MODULE:NAME` spelling and a bare operator inside the body
    resolves as `MODULE:operator` before being called unbound — mirroring
    `with-module`'s runtime rewrite. `(import M)` folds `M`'s exports into the
    known unqualified names. If a module named by `import` cannot have its
    exports enumerated (an unknown or computed module name), the checker goes
    **permissive** for the rest of that file, suppressing further
    unbound-function findings rather than risking a false positive.
* For an operator that is a **known macro or operative** (`vau`/fexpr) not on
  that whitelist, the checker does **not** descend into the call — a macro body
  can bind or introduce arbitrary names. This loses coverage but never invents
  a finding. The same applies to `defmacro` / `defexpr` / `defvau` bodies.

### What it deliberately does *not* catch

* **Unbound variables** (non-operator symbols). Dynamic variables, forward
  globals, and macro-introduced bindings make variable-position checking too
  false-positive-prone, so only operator-position names are checked.
* **Anything inside a macro/operative call body** that is not on the descent
  whitelist above.
* **Host-registered natives.** A file that relies on functions registered from
  Rust by an embedding host (e.g. a game engine exposing `entity-x`,
  `move-entity!`, …) is not self-contained; the checker cannot see host
  bindings and will report them as unbound. Check self-contained files — ones
  that define everything they call, or call only the standard library.
* **Arity of builtins and compiled stdlib membranes**, whose parameter lists
  are not reliably introspectable. Only file-defined functions and plain stdlib
  lambdas are arity-checked.

These are conscious trade-offs in service of the one property that makes the
tool worth trusting: when `lamedh --check` reports something, it is real.
