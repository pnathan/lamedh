# 6. Conditions and Restarts

Lamedh's condition system has three layers, each built on the one below:

1. **Signaling** — `error` raises a condition; first-class error values carry
   a message and arbitrary data.
2. **Catching** — `errorset` (Lisp 1.5), `ignore-errors`, `handler-case`,
   `catch`/`throw`, and `unwind-protect` trap conditions and non-local exits.
3. **Restarts** — `restart-case` and `handler-bind`, built in pure Lisp on
   top of layers 1 and 2, let a handler choose a *named recovery* instead of
   just catching and giving up.

Only `error`, `errorset`, `handler-case`, `catch`, `throw`, and
`unwind-protect` are kernel special forms/builtins (they need non-local
control flow the tree-walker provides directly). Everything else —
`ignore-errors`, `restart-case`, `handler-bind`, and the standard restart
invokers — is defined in `lib/16-conditions.lisp` on top of those primitives.
There is no separate `signal` function and no condition-class hierarchy:
`error` is the one way to signal, and a first-class error value is the one
kind of condition.

## 6.1 Signaling errors

`(error message)` raises a condition and unwinds to the nearest handler.
`message` is normally a string; anything else is printed:

```lisp
(error "boom")
; => Error: boom
```

`error` actually has three forms:

- `(error)` — signal a generic condition with message `"Error"`.
- `(error an-error-value)` — re-signal an existing error value unchanged
  (this is how a declining handler re-raises the same condition; see
  below).
- `(error message data)` — signal a new condition with `message` and one
  attached `data` value (extra arguments beyond `data` are accepted but
  ignored).

```lisp
(handler-case (error "bad input" (list :code 42))
  (error (e) (list (error-message e) (error-data e))))
; => ("bad input" (:CODE 42))
```

A condition is a first-class value — `LispVal::Error` — with four accessors:

| Function | Result |
|---|---|
| `(make-error message data)` | builds an error value without signaling it |
| `(error-p x)` | `T` if `x` is an error value |
| `(error-message e)` | the message string |
| `(error-data e)` | the attached data, or `()` if none |

```lisp
(make-error "oops" (list 1 2 3))
; => #<error "oops" (1 2 3)>

(error-p (make-error "x" 1))   ; => T
(error-p 5)                    ; => ()
```

(`()` is how NIL prints; Lamedh has no separate `NIL` token in output.)

## 6.2 Catching, Lisp 1.5 style: `errorset`

`ERRORSET` is the original Lisp 1.5 primitive: it takes a *quoted* form,
evaluates it, and traps ordinary errors. On success it returns a
one-element list wrapping the value; on error it returns `()`. Wrapping the
success value in a list is what makes a `NIL` result distinguishable from a
failure — the wrapper list itself is truthy either way:

```lisp
(errorset '(+ 1 2))    ; => (3)
(errorset '(/ 1 0))    ; => ()
(errorset 'nil)        ; => (())
```

`ignore-errors` (in `lib/22-guard.lisp`) is the ergonomic wrapper: it
unwraps the `errorset` result for you and evaluates an implicit `progn`,
so you write `body...` instead of a quoted form:

```lisp
(ignore-errors (/ 1 0))    ; => ()
(ignore-errors (+ 1 2))    ; => 3
```

`ignore-errors` is defined as `(car (errorset '(progn body...)))`, so like
`errorset` it cannot tell you *why* something failed and it cannot recover
with a value chosen after the fact — use `handler-case` for that.

## 6.3 `handler-case`

`handler-case` is a kernel special form because it binds the handler
variable to the actual first-class condition value, which a macro over
`errorset` cannot do. Its shape:

```lisp
(handler-case expr
  (error (var) handler-body...))
```

`expr` is evaluated. If it signals, `var` is bound to the condition (an
error value, wrapping any Rust-level error too — division by zero, unbound
variable, etc. all arrive as ordinary error values) and `handler-body` runs
in place of `expr`'s value:

```lisp
(handler-case (/ 1 0)
  (error (e) (list 'caught (error-message e))))
; => (CAUGHT "Division by zero")

(handler-case (+ 1 2)
  (error (e) 'never))
; => 3
```

`handler-case` supports exactly one clause and does **no class-based
dispatch** — there is only one condition type, so the clause head is not
even checked against a type name:

```lisp
(handler-case (/ 1 0) (whatever-symbol (e) (list 'caught (error-message e))))
; => (CAUGHT "Division by zero")
```

Write `error` as the clause head anyway; it documents intent and matches
every other example in this manual and the standard library.

## 6.4 `catch`/`throw` and `unwind-protect`

`catch`/`throw` are general non-local exit, independent of the condition
system — the restart machinery in §6.6 is built on them:

```lisp
(catch 'foo (+ 1 (throw 'foo 99)))
; => 99
```

`unwind-protect` guarantees its cleanup forms run whether the body returns
normally, signals an error, or is unwound past by a `throw` or a restart
invocation:

```lisp
(let ((r nil))
  (ignore-errors (unwind-protect (error "x") (setq r 'cleaned)))
  r)
; => CLEANED
```

## 6.5 `handler-bind` and the documented deviation

`handler-bind` (in `lib/16-conditions.lisp`) runs a handler *function* for
its side effects — typically to invoke a restart — instead of replacing the
protected expression's value:

```lisp
(handler-bind ((error handler-fn) ...) body...)
```

If a handler returns normally instead of transferring control, it
**declines**: the condition is re-signaled to the next handler out.

```lisp
(handler-case
    (handler-bind ((error (lambda (e) nil)))   ; declines
      (error "still propagates"))
  (error (e) (list 'outer-saw (error-message e))))
; => (OUTER-SAW "still propagates")
```

**The deviation from Common Lisp, stated plainly:** `handler-bind` is
implemented as `handler-case` underneath — `(handler-case (eval body e)
(error (condition) (run-handlers condition) (error condition)))`. In CL,
a handler established by `handler-bind` runs *before* the stack unwinds,
so it can invoke a restart established anywhere between the signal point
and the handler. In Lamedh, because `handler-bind` is `handler-case` in
disguise, its handler function only runs *after* Rust-level error
propagation has already unwound the stack up to the `handler-bind` form —
including any dynamic bindings (like the live-restart list) established
*inside* that unwound extent.

Concretely: a restart established inside the function that signals is
already gone by the time the handler runs:

```lisp
(defun inner (s)
  (restart-case (error (concat "bad: " s))
    (use-value (v) v)))

(handler-bind ((error (lambda (e) (invoke-restart 'use-value 99))))
  (inner "x"))
; => Error: invoke-restart: no live restart named USE-VALUE
```

The **canonical shape** — restarts established *around* the
`handler-bind`, not inside the code it protects — works exactly as in CL,
because that dynamic extent is still live when the handler runs:

```lisp
(restart-case
    (handler-bind ((error (lambda (e) (invoke-restart 'use-value 99))))
      (error "bad"))
  (use-value (v) v))
; => 99
```

Always write restarts this way: `restart-case` wraps a `handler-bind` that
wraps the risky call, not the other way around.

## 6.6 The restart protocol

A restart is a named recovery, established with `restart-case` for the
dynamic extent of an expression:

```lisp
(restart-case expr
  (name (params...) body...)
  ...)
```

`expr`'s value is `restart-case`'s value — unless code running inside
`expr` (usually a `handler-bind` handler) calls `(invoke-restart 'name
args...)`. That transfers control straight back to the `restart-case` form,
running the clause's body with `args` bound to `params`, and *that* becomes
`restart-case`'s value:

```lisp
(restart-case (+ 1 2) (use-value (v) v))
; => 3

(restart-case
    (handler-bind ((error (lambda (er) (invoke-restart 'use-value 42))))
      (error "boom"))
  (use-value (v) v))
; => 42
```

Introspection functions see every restart currently live, innermost first:

```lisp
(restart-case
    (restart-case (mapcar (lambda (r) (restart-name r)) (compute-restarts))
      (retry () nil))
  (use-value (v) v))
; => (RETRY USE-VALUE)

(restart-case (restart-name (find-restart 'use-value)) (use-value (v) v))
; => USE-VALUE

(restart-case (find-restart 'nope) (use-value (v) v))
; => ()
```

`invoke-restart` on a name with no live restart signals its own catchable
error:

```lisp
(handler-case (invoke-restart 'nope) (error (e) (error-message e)))
; => "invoke-restart: no live restart named NOPE"
```

Restarts nest and shadow by name, innermost wins; a name reachable only
from an outer establisher still reaches it through the inner one. And since
`invoke-restart` unwinds through `throw`/`catch`, `unwind-protect` cleanups
still run when a restart transfers control past them (§6.4's example works
the same way with `restart-case`/`handler-bind` in place of `handler-case`).

### Standard restart invokers

Four conventionally-named helpers just call `invoke-restart` for you, plus
one combinator that establishes a retry loop:

| Function | Effect |
|---|---|
| `(use-value v)` | invoke the innermost `use-value` restart with `v` |
| `(store-value v)` | invoke the innermost `store-value` restart with `v` |
| `(retry)` | invoke the innermost `retry` restart (no arguments) |
| `(abort-to-restart)` | invoke the innermost `abort` restart |
| `(with-retry-restart body...)` | run `body` with a `retry` restart that re-evaluates `body` from the top |

```lisp
(restart-case
    (handler-bind ((error (lambda (e) (store-value 7))))
      (error "x"))
  (store-value (v) (list 'stored v)))
; => (STORED 7)

(setq attempts 0)
(with-retry-restart
  (setq attempts (+ attempts 1))
  (handler-bind ((error (lambda (er) (if (< attempts 3) (retry) nil))))
    (if (< attempts 3) (error "flaky") (list 'ok attempts))))
; => (OK 3)
```

These are conventions, not kernel magic — `use-value` etc. are ordinary
names your `restart-case` clauses choose to define. `abort-to-restart`
invokes a restart literally named `abort` (not `abort-to-restart`).

## 6.7 Worked example: a parser with `use-value` and `retry`

A strict integer parser that signals on bad input, driven by a handler that
tries two different repairs before giving up:

```lisp
(defun parse-strict (s)
  "Parse string S as an integer, or signal a catchable error."
  (let ((n (parse-integer s)))
    (if n n (error (concat "not a number: " s)))))

(defun parse-line (s)
  "Drive PARSE-STRICT with USE-VALUE and RETRY restarts established
around a handler: blank input uses 0; a leading '#' is stripped and
the parse is retried; anything else falls back to 0."
  (restart-case
      (handler-bind
          ((error (lambda (e)
                    (cond ((string= s "") (use-value 0))
                          ((string= (substring s 0 1) "#")
                           (invoke-restart 'retry (substring s 1 (string-length s))))
                          (t (use-value 0))))))
        (parse-strict s))
    (use-value (v) v)
    (retry (new-s) (parse-strict new-s))))
```

```lisp
(list (parse-line "42") (parse-line "#42") (parse-line "") (parse-line "abc"))
; => (42 42 0 0)
```

Note the shape: `restart-case` is the outermost form in `parse-line`,
wrapping the `handler-bind` that wraps the actual call to `parse-strict` —
the canonical shape from §6.5. `parse-strict` itself knows nothing about
recovery; it just signals. The `retry` clause is a custom restart (defined
right here, taking one argument) — it is unrelated to the zero-argument
`retry`/`with-retry-restart` helpers in §6.6, which happen to share the
conventional name `retry` for the common "just run the body again" case.

## 6.8 Conditions and guard fences

`with-fuel` (`lib/22-guard.lisp`, covered fully in Chapter 7) bounds
execution to a step budget and signals when it runs out. That signal is an
**ordinary, catchable error** — not a special uncatchable condition. This
is a deliberate design choice: bounded work should have a programmatic
fallback, not an unrecoverable crash.

```lisp
(handler-case
    (with-fuel 50
      (defun spin (n) (if (< n 1) 'done (spin (- n 1))))
      (spin 1000000))
  (error (e) (error-message e)))
; => "fuel exhausted (budget 50)"
```

Because it is an ordinary condition, it composes with the restart protocol
exactly like any other error — a handler can choose a fallback value
instead of letting the whole computation die:

```lisp
(restart-case
    (handler-bind ((error (lambda (e) (use-value 'budget-fallback))))
      (with-fuel 50
        (defun spin2 (n) (if (< n 1) 'done (spin2 (- n 1))))
        (spin2 1000000)))
  (use-value (v) v))
; => BUDGET-FALLBACK
```

`(fuel-remaining)` reports the live budget inside a fence and `()` outside
any fence:

```lisp
(with-fuel 1000 (fuel-remaining))    ; => 1000
(fuel-remaining)                      ; => ()
```

The fence attenuates *authority and budget*, not *catchability* — a fuel
exhaustion is exactly as catchable, and exactly as capable of driving a
restart, as an `(error ...)` you wrote by hand.
