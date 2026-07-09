# 5. Functions, Macros, and Evaluation

## 5.1 Lambda and Function Values

`lambda` builds an anonymous function value. Calling it applies the body
to its arguments:

```lisp
((lambda (x y) (+ x y)) 3 4)
; => 7
```

`defun` is the everyday way to give a lambda a name — it is a macro that
expands to `(def name (lambda params body...))`, plus some bookkeeping
covered in 5.6:

```lisp
(defun square (x) (* x x))
(square 6)
; => 36
```

A function is a first-class value like any other. You can hold it in a
variable, pass it as an argument, and return it from another function:

```lisp
(let ((f (lambda (x) (* x 2))))
  (funcall f 21))
; => 42
```

## 5.2 `#'`, `funcall`, and `apply`

`#'name` (equivalently `(function name)`) fetches a symbol's function
value without calling it — the same role `'name` (quote) plays for data,
but for the operator position:

```lisp
#'car
; => <builtin>
(function car)
; => <builtin>
```

`funcall` calls a function value with an explicit argument list written
out one at a time:

```lisp
(funcall #'+ 1 2 3)
; => 6
```

`apply` is like `funcall` but its last argument is a list that gets
spread as the trailing arguments — any earlier arguments are passed
as-is:

```lisp
(apply #'+ '(1 2 3))
; => 6
(apply #'+ 1 2 '(3 4))
; => 10
```

`apply` is how you turn a runtime-built list of arguments into a call
without knowing its length ahead of time — the classic use is forwarding
a `&rest` parameter (5.3) to another function.

## 5.3 `&rest` Parameters

A lambda list ending in `&rest name` collects any extra arguments into a
list bound to `name`:

```lisp
((lambda (a &rest more) more) 1 2 3)
; => (2 3)
```

Combine `&rest` with `apply` to write variadic wrappers:

```lisp
(defun sum-all (&rest xs) (apply #'+ xs))
(sum-all 1 2 3 4)
; => 10
```

## 5.4 Closures

A lambda captures its enclosing lexical environment. Each call to a
function that returns a lambda produces a fresh, independent closure over
its own bindings:

```lisp
(defun make-counter ()
  (let ((n 0))
    (lambda () (setq n (+ n 1)))))

(def a (make-counter))
(def b (make-counter))
(a)
(a)
(b)
(list (a) (b))
; => (3 2)
```

`a` and `b` each close over their own `n`; mutating one (via `setq`, which
mutates the captured binding, not a copy) never touches the other's.

## 5.5 Higher-Order Style

`lib/13-functional.lisp` and `lib/01-list.lisp` supply the usual toolkit,
all function-first (predicate/function argument before the collection),
which nests cleanly:

```lisp
(mapcar (lambda (x) (* x x)) '(1 2 3 4))
; => (1 4 9 16)
(filter (lambda (x) (> x 2)) '(1 2 3 4))
; => (3 4)
(reduce #'+ '(1 2 3 4))
; => 10
(reduce #'+ '(1 2 3 4) 100)
; => 110
```

`reduce` takes an optional seed as a third argument (via `&rest`); with
no seed it folds left starting from the first element, and an empty list
with no seed calls the function with zero arguments (`(reduce #'+ '())`
is `0`, because `(+)` is `0`).

Function-combining helpers live alongside the rest:

```lisp
(funcall (compose (lambda (x) (* x 2)) (lambda (x) (+ x 1))) 5)
; => 12
(funcall (curry #'+ 10) 5)
; => 15
((complement #'evenp) 3)
; => T
(funcall (constantly 42) 1 2 3)
; => 42
```

`compose` builds `(f (g args...))` from two functions; `curry` prepends
fixed leading arguments to a function; `complement` negates a predicate;
`constantly` ignores its arguments and always returns a fixed value.
`identity` (returns its argument unchanged) rounds out the set.

## 5.6 `defun`, `defun*`, and `defun-typed`: One-Door Compilation

Lamedh has three function-defining forms on one spectrum of typing
effort, all inspectable with `see-type`.

**`defun`** defines an ordinary function and then, quietly, tries to
compile it: HM inference runs in the background via `jit-optimize`, and
if the body is a fully-inferable typed island, the name is rebound to a
native membrane that fast-paths typed calls and falls back to the
original closure otherwise. If inference fails, `defun` silently keeps
the plain closure — no error:

```lisp
(defun sq (x) (* x x))
(see-type 'sq)
; => (CHECKED (FORALL (A) (-> (A) A)))
```

**`defun*`** is the form to reach for when you *want* typed compilation
attempted and want the syntax to say so. It accepts bare parameters,
`(param type)` pairs, and an optional return type before the body:

```lisp
(defun* scale (x int64) (y int64) (* x y))
(scale 6 7)
; => 42
(defun-typed (add2 int64) ((x int64) (y int64)) (+ x y))
(add2 3 4)
; => 7
(see-type 'add2)
; => (TYPED (-> (INT64 INT64) INT64) COMPILED)
```

When `defun*` can't produce a monomorphic typed body — untyped list
operations, `&rest`, a contradicted type pin — it falls back to an
ordinary lambda transparently; the function still runs correctly, it
just doesn't compile:

```lisp
(defun* mk (a b) (cons a b))
(mk 1 2)
; => (1 . 2)
(see-type 'mk)
; => (CHECKED (FORALL (A) (-> (A (LIST A)) (LIST A))))
```

**`defun-typed`** is the explicit, no-inference form: you give every
parameter and the return type up front, and the compiler either compiles
it natively or reports an error — no silent fallback. `see-type` reports
the three tiers seen above: `CHECKED` (type-checked, tree-walker),
`TYPED ... COMPILED` (native code), or an outright type error.

To opt a single `defun` out of the quiet compile attempt, put
`(declare (no-compile))` as the first form of the body (after an optional
docstring):

```lisp
(defun pinned (x)
  (declare (no-compile))
  (+ x 1))
(see-type 'pinned)
; => (CHECKED (-> () INT64))
```

To opt out globally for names defined after the declaration, use
`declaim`:

```lisp
(declaim (no-compile pinned2))
(defun pinned2 (x) (+ x 1))
(see-type 'pinned2)
; => (CHECKED (-> (INT64) INT64))
```

Both mechanisms also suppress a later explicit `(jit-optimize name)`
call. `defun-typed` is disallowed inside a guard fence
(`with-capabilities`/`sandboxed`) — sandboxed code can't request native
compilation.

## 5.7 `defmacro`: Templates and Hygiene

A macro receives its arguments **unevaluated** and returns a new form
that gets evaluated in its place. `defmacro` pairs naturally with
quasiquote (`` ` ``), unquote (`,`), and unquote-splicing (`,@`) to build
the replacement form:

```lisp
(defmacro my-when (test &rest body)
  `(if ,test (progn ,@body) nil))
(macroexpand '(my-when t 1 2))
; => (IF T (PROGN 1 2) ())
```

`macroexpand` performs a single expansion step — it does not recursively
expand macros that appear in the result:

```lisp
(defmacro m1 (x) `(m2 ,x))
(defmacro m2 (x) `(+ ,x 1))
(macroexpand '(m1 5))
; => (M2 5)
```

Because a macro's arguments are unevaluated source, a branch that never
runs never gets evaluated — the substitution happens at expansion time,
before evaluation:

```lisp
(defmacro my-if (c a b) `(cond (,c ,a) (t ,b)))
(my-if t (princ "THEN") (princ "ELSE"))
; prints THEN, not ELSE
```

When a macro introduces its own temporary bindings, use `gensym` to avoid
capturing a variable the caller happens to be using with the same name —
the classic macro hygiene problem:

```lisp
(defmacro my-or (a b)
  (let ((g (gensym)))
    `(let ((,g ,a)) (if ,g ,g ,b))))
(macroexpand '(my-or 1 2))
; => (LET ((G0000 1)) (IF G0000 G0000 2))
(my-or nil 5)
; => 5
```

`gensym` returns a fresh, uninterned symbol on every call, so `G0000`
here can never collide with a symbol either the macro or the caller wrote
literally.

## 5.8 Fexprs: `defexpr`

A fexpr is the classic Lisp 1.5 mechanism for user-defined special forms:
when a fexpr is called, the evaluator does **not** evaluate the operands
first — the body receives the literal, unevaluated argument forms as a
single list, and decides for itself whether to `eval` them, ignore them,
or inspect them:

```lisp
(defexpr my-quote (x) (car x))
(my-quote (+ 1 2))
; => (+ 1 2)
```

The stdlib's own `select` (Lisp 1.5 Appendix A, `lib/09-lisp15.lisp`) is
a real fexpr: it evaluates a key expression and each candidate left to
right, unevaluated until it chooses which branch to run:

```lisp
(def x 2)
(select x (1 'one) (2 'two) (3 'three) 'other)
; => TWO
```

The important limitation: a `defexpr` body's `(eval form)` calls evaluate
against the **global** environment, not the caller's lexical scope — a
fexpr does not receive the caller's environment as a value the way a
`vau` operative does:

```lisp
(defexpr showenv (a) (eval (car a)))
(def y 5)
(showenv y)
; => 5
(let ((y 5)) (showenv y))
; Error: Unbound variable: Y
```

That's exactly why fexprs "compose poorly": code that only sees a fexpr
call can't tell what its operands mean, and the fexpr itself can't reach
into the caller's `let` bindings. Reach for `defmacro` when you're
generating code at expansion time, `lambda`/`defun` for ordinary runtime
abstraction, and `defexpr` only when you specifically want raw,
unevaluated source with no compile-time expansion step. If you also need
the caller's actual environment, use `vau` (5.9).

## 5.9 Kernel-Style `vau` Operatives

`vau` (also spelled `$vau`, following John Shutt's Kernel convention) is
a fexpr that additionally receives the **caller's environment** as an
explicit, first-class value — the operand list and the environment are
both just parameters:

```lisp
(def my-if
  ($vau (x e)
    (if (eval (car x) e)
        (eval (cadr x) e)
        (eval (caddr x) e))))
(my-if t 'yes 'no)
; => YES
```

Because the environment is explicit, `vau` can express `lambda`,
`defmacro`, and fexprs as special cases — it's the reflective kernel
underneath the higher-level forms. `defvau` is the named-definition
sugar, parallel to `defun`:

```lisp
(defvau unless (x e)
  (if (eval (car x) e)
      nil
      (eval (cons 'progn (cdr x)) e)))
(unless nil 1 2 3)
; => 3
```

The four `vau`-built operatives in `lib/08-vau.lisp` — `$if`, `$and`,
`$or`, `$sequence` — are worth reading as canonical small examples; each
recurses on `eval (... e)` over the caller's environment rather than
relying on the evaluator's own special-form dispatch. `(the-environment)`
captures the current environment as a value, and `make-environment`
builds a fresh one (optionally with a parent) — the raw material `vau`
operatives manipulate.

## 5.10 Dynamic Variables

Lexical variables (from `let`, `lambda` parameters, `def`) resolve
through the environment chain captured at closure-creation time. Dynamic
variables resolve through the *calling* stack instead — a `let` binding
of a dynamic variable is visible to every function called from within
that `let`, not just to code written lexically inside it.

Declare a dynamic variable with `defdynamic` (or its alias `defvar`):

```lisp
(defdynamic *x* 'global)
(defun get-x () *x*)
(get-x)
; => GLOBAL
(let ((*x* 'local)) (get-x))
; => LOCAL
(get-x)
; => GLOBAL
```

`get-x` refers to `*x*` free — it has no lexical binding for it — yet it
sees whatever dynamic binding is active when it's *called*, not when it
was *defined*. Contrast with 5.4: a closure's free variables are fixed at
creation time; a dynamic variable's value follows the live call stack.
Rebinding nests and unwinds correctly:

```lisp
(defdynamic *level* 0)
(defun get-level () *level*)
(let ((*level* 1)) (let ((*level* 2)) (get-level)))
; => 2
(get-level)
; => 0
```

By convention dynamic variable names are wrapped in asterisks
(`*name*`); `defdynamic` warns on stderr if you don't follow it, though
it still works:

```lisp
(defdynamic notstar 1)
; Warning: Dynamic variable 'NOTSTAR' does not follow naming convention *NAME*
```

There's no separate `dynamic-let` form — the same `let` you already use
for lexical bindings dynamically rebinds any symbol previously declared
with `defdynamic`.

## 5.11 The Evaluation Model

Lamedh's evaluator recognizes a few categories of "things in operator
position." They differ in one crucial way: what happens to the operand
forms before the body runs.

| Kind | Defined with | Operands evaluated? | Sees caller's env? |
|---|---|---|---|
| Special form | built into the evaluator (`if`, `quote`, `let`, ...) | form-specific | n/a (kernel) |
| Function | `lambda`, `defun`, `defun*`, `defun-typed` | yes, all, left to right (applicative order) | no — closes over definition-time env |
| Macro | `defmacro` | no — operands become a template's substitution values; the *expansion* is evaluated | expansion runs in caller's env |
| Fexpr | `defexpr` | no — operands arrive as literal source in one list | no — body's `eval` defaults to the global env |
| Vau operative | `vau`/`$vau`, `defvau` | no — operands arrive as literal source | yes — caller's env is passed explicitly |

Ordinary function application is applicative order: every argument is
evaluated exactly once, before the call, whether or not the body uses
it. Everything else on this table exists to escape that rule in a
controlled way — a macro escapes it at expansion time, fexprs and `vau`
escape it at call time.

## 5.12 `eval`, `read`, and Code as Data

Lisp code is data (s-expressions) before it's a program, so you can build
it, print it, and evaluate it programmatically. `read-from-string` parses
text into a form; `eval` evaluates a form:

```lisp
(eval (read-from-string "(+ 1 2)"))
; => 3
```

`eval` takes an optional second argument, the environment to evaluate
in — that's what `vau` operatives use to run operands in the caller's
scope (5.9); with no second argument, `eval` uses the global environment.

`prin1-to-string` prints a form the way the reader could read it back
(strings get their quotes); `princ-to-string` prints for human
consumption (no quotes on strings):

```lisp
(prin1-to-string '(1 2 3))
; => "(1 2 3)"
(prin1-to-string "hi")
; => "\"hi\""
(princ-to-string "hi")
; => "hi"
```

Round-tripping code through `prin1-to-string` and `read-from-string`
recovers the original structure — the basis for anything that generates,
serializes, or introspects Lisp forms at runtime, including `macroexpand`
(5.7) and `see-source`, which reconstructs the source form the evaluator
registered for a lambda, fexpr, macro, or vau operative.

## 5.13 `label`: Lisp 1.5 Self-Reference

`label` is the Lisp 1.5 heritage way to give a lambda a name usable
*inside its own body*, without a separate `def` — useful for an
anonymous recursive function you don't want to pollute the global
namespace:

```lisp
((label fact (lambda (n) (if (= n 0) 1 (* n (fact (- n 1))))))
 5)
; => 120
```

`label`'s argument order is `(label name lambda-expr)`, and the result is
a callable function value; `fact` is only bound inside that lambda's own
body, exactly the way `defun`'s named function is visible to its own
recursive calls, but without touching the global environment at all.
