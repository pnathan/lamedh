# Error Handling

Lamedh has a three-layer condition system: signaling (`error`), catching
(`errorset`, `handler-case`, `catch`/`throw`, `unwind-protect`), and
restarts (`restart-case`, `handler-bind`).  See
[Chapter 6](../manual/06-conditions-and-restarts.md) of the manual for
worked examples and the design rationale.

This page documents each form's signature and return value.

---

## Signaling

### ERROR

**Syntax:**
- `(error)` — signal with message `"Error"`
- `(error message)` — signal with the given message
- `(error message data)` — signal with message and one attached data value
- `(error err-value)` — re-signal an existing error value unchanged

Raises a condition and unwinds to the nearest handler.  Never returns.

```lisp
(error "Invalid input")
;; Error: Invalid input

(error "bad key" '(:code 42))
;; Error: bad key   — data: (:CODE 42)
```

### MAKE-ERROR

**Syntax:** `(make-error message data)`

Builds a first-class error value *without* signaling it.

```lisp
(make-error "oops" (list 1 2 3))
; => #<error "oops" (1 2 3)>
```

### ERROR-P

**Syntax:** `(error-p value)`

Returns `T` if `value` is an error value, `()` otherwise.

### ERROR-MESSAGE / ERROR-DATA

**Syntax:** `(error-message err)`, `(error-data err)`

Read the message string or data payload from an error value.

```lisp
(handler-case (error "bad input" (list :code 42))
  (error (e) (list (error-message e) (error-data e))))
; => ("bad input" (:CODE 42))
```

---

## Catching

### ERRORSET

**Syntax:** `(errorset form)`

The original Lisp 1.5 primitive.  Evaluates a *quoted* form and traps
errors.  Returns a one-element list on success, `()` on error.

```lisp
(errorset '(+ 1 2))         ; => (3)
(errorset '(/ 1 0))         ; => ()
(errorset '(error "oops"))  ; => ()
(errorset 'nil)             ; => (())
```

The wrapper list makes `NIL` results distinguishable from failure.

### HANDLER-CASE

**Syntax:** `(handler-case expr (error (var) body...))`

Evaluates `expr`.  If it signals a condition, binds the error value to
`var` and runs `body`.  The handler runs *after* unwinding.

```lisp
(handler-case (/ 10 0)
  (error (e) (list 'caught (error-message e))))
; => (CAUGHT "division by zero")
```

### CATCH / THROW

**Syntax:** `(catch tag body...)`, `(throw tag value)`

Non-local exit by tag.  `catch` evaluates `body` normally unless a
`throw` with a matching tag (compared by `eq`) unwinds to it.

```lisp
(catch 'done
  (progn
    (throw 'done 42)
    (error "unreachable")))
; => 42
```

### UNWIND-PROTECT

**Syntax:** `(unwind-protect protected-form cleanup...)`

Evaluates `protected-form`, then evaluates `cleanup` forms whether
`protected-form` returned normally or unwound.

```lisp
(unwind-protect
    (error "boom")
  (princ "cleanup ran"))
;; prints "cleanup ran", then re-raises the error
```

### IGNORE-ERRORS

**Syntax:** `(ignore-errors body...)`

Evaluates `body` inside an implicit `errorset`.  Returns the result on
success, `()` on error.  Defined in `lib/22-guard.lisp`.

```lisp
(ignore-errors (/ 1 0))    ; => ()
(ignore-errors (+ 1 2))    ; => 3
```

---

## Restarts

Restarts let a handler choose a named recovery instead of just catching
and giving up.  They are built in pure Lisp (`lib/16-conditions.lisp`)
on top of `handler-case` and dynamic variables.

### RESTART-CASE

**Syntax:** `(restart-case expr (restart-name (var...) body...)...)`

Evaluates `expr` with the named restarts established.  If a handler
invokes one, its body runs and the result is returned from
`restart-case`.

```lisp
(handler-bind
    ((error (lambda (e)
              (invoke-restart 'use-value 0))))
  (restart-case (/ 10 0)
    (use-value (v) v)))
; => 0
```

### HANDLER-BIND

**Syntax:** `(handler-bind ((type handler-fn)...) body...)`

Like `handler-case` but the handler runs *before* unwinding.  The
handler can inspect the condition and invoke a restart, or decline by
re-signaling with `(error e)`.

### INVOKE-RESTART

**Syntax:** `(invoke-restart name arg...)`

Transfers control to the restart named `name`, passing `arg...` to its
body.

---

## Teaching Errors: Did-You-Mean and CL-ism Guidance

Lamedh has almost no presence in LLM training data, so unbound-symbol and
undefined-function errors carry extra guidance (`src/teaching_errors.rs`) —
the same machinery `lamedh --check` uses. Two mechanisms fire, and a CL-ism
redirect takes precedence over a spelling suggestion when both would apply.

**Did-you-mean.** For an unbound name, a Levenshtein search over the symbols
*actually bound* in the environment (not merely interned) appends up to three
suggestions. The distance threshold scales with name length: names of two
characters or fewer get no suggestions (too noisy), length-3 names allow
distance 1, and names of four or more allow distance 2.

```lisp
(lenght '(a b c))
;; Error: Unbound variable: LENGHT — did you mean LENGTH?
```

**Common-Lisp-ism guidance.** A small fixed table maps well-known Common Lisp
forms Lamedh deliberately lacks to their real replacement:

| CL form | Guidance |
|---|---|
| `LOOP` | use `DOTIMES`, `WHILE`, or `MAP` |
| `DEFSTRUCT` | removed in 0.3 — use `DEFRECORD` |
| `DEFCLASS` | use `DEFPROTOCOL` and `DEFINSTANCE` |
| `DEFMETHOD` | use `DEFINSTANCE` (with `DEFPROTOCOL`) |
| `DEFGENERIC` | use `DEFPROTOCOL` |
| `DEFCONSTANT` | use `DEF` (Lamedh has no separate constant-binding form) |
| `MULTIPLE-VALUE-BIND` | Lamedh has no multiple return values — return a `LIST` and use `DESTRUCTURING-BIND` |
| `VALUES` | Lamedh has no multiple return values — return a `LIST` directly |
| `WITH-OPEN-FILE` | use `WITH-OPEN-PORT` |

```lisp
(loop for x in '(1 2 3) collect x)
;; Error: Unbound variable: LOOP is Common Lisp, not Lamedh — use DOTIMES, WHILE, or MAP
```

The guidance appears wherever these errors surface: at runtime, and in
`lamedh --check`'s static findings (see [Static Checking](../check.md)).

## Common Error Conditions

| Condition | Example |
|-----------|---------|
| Division by zero | `(/ 1 0)` |
| Type mismatch | `(+ 1 "a")` |
| Unbound variable | `undefined-var` |
| Arity error | `(car 1 2 3)` |
| File not found | `(load-file "missing.lisp")` |
| Invalid argument | `(nth -1 '(a b))` |
| Capability denied | `(shell "ls")` without `SHELL` |

---

**See Also:**
- [Chapter 6: Conditions and Restarts](../manual/06-conditions-and-restarts.md) — full tutorial
- [Special Forms](../special_forms.md) — `handler-case`, `catch`, `throw`, `unwind-protect`
