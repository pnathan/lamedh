# Chapter 6: Special Forms

Special forms are expressions with evaluation rules different from normal function calls. Their arguments may be evaluated specially or not at all.

---

## 6.1 Overview

| Special Form | Purpose |
|--------------|---------|
| `QUOTE` | Prevent evaluation |
| `QUASIQUOTE` | Template with selective evaluation |
| `IF` | Two-way conditional |
| `COND` | Multi-way conditional |
| `AND` | Short-circuit logical and |
| `OR` | Short-circuit logical or |
| `DEF` | Define variable |
| `SETQ` | Assign variable |
| `LET` | Local bindings |
| `LAMBDA` | Anonymous function |
| `DEFUN` | Define named function (macro) |
| `DEFMACRO` | Define macro |
| `DEFEXPR` | Define fexpr |
| `LABEL` | Named recursive function |
| `FUNCTION` | Function reference |
| `DEFINE` | Batch definitions |
| `PROG` | Sequential with labels |
| `PROGN` | Sequential evaluation |
| `GO` | Jump to label (in PROG) |
| `RETURN` | Return from PROG |

---

## 6.2 QUOTE

**Syntax:** `(quote expression)` or `'expression`

Prevents evaluation of `expression` and returns it as data.

```lisp
(quote foo)           ; => FOO
(quote (+ 1 2))       ; => (+ 1 2)
'foo                  ; => FOO
'(a b c)              ; => (A B C)
```

**Evaluation Rule:** Does not evaluate its argument.

---

## 6.3 QUASIQUOTE and UNQUOTE

**Syntax:** `` `expression `` and `,subexpression`

Quasiquote is like quote but allows selective evaluation with unquote (comma).

```lisp
(def x 10)
(def y 20)

`(x is ,x and y is ,y)
; => (X IS 10 AND Y IS 20)
```

**Nested quasiquote:**
```lisp
`(a `(b ,(+ 1 2) ,,(+ 3 4)))
; The inner quasiquote remains quoted
```

**Evaluation Rule:**
- At the top level, traverse the quoted structure
- When `,` is encountered, evaluate the following expression
- Insert the result into the structure

---

## 6.4 IF

**Syntax:** `(if condition then-form else-form)`

Evaluates `condition`. If non-NIL, evaluates and returns `then-form`. Otherwise, evaluates and returns `else-form`.

```lisp
(if t "yes" "no")           ; => "yes"
(if nil "yes" "no")         ; => "no"
(if (> 3 2) "bigger" "smaller")  ; => "bigger"
```

**Evaluation Rule:**
1. Evaluate `condition`
2. If result is non-NIL, evaluate `then-form`
3. Otherwise, evaluate `else-form`
4. Only one branch is evaluated

---

## 6.5 COND

**Syntax:** `(cond clause1 clause2 ...)`

Where each clause is `(test expression...)`.

Multi-way conditional. Evaluates tests in order; when one is true, evaluates its expressions.

```lisp
(cond ((< x 0) "negative")
      ((= x 0) "zero")
      ((> x 0) "positive"))
```

**With T as default:**
```lisp
(cond ((= x 1) "one")
      ((= x 2) "two")
      (t "other"))    ; t always matches
```

**Evaluation Rule:**
1. For each clause, evaluate the test
2. If test is non-NIL, evaluate expressions in that clause
3. Return value of last expression
4. If no test succeeds, return NIL

---

## 6.6 AND

**Syntax:** `(and form1 form2 ...)`

Short-circuit logical AND. Returns the first NIL value, or the last value if all are non-NIL.

```lisp
(and t t t)           ; => T
(and 1 2 3)           ; => 3
(and t nil t)         ; => NIL
(and)                 ; => T (identity for AND)
```

**Evaluation Rule:**
1. Evaluate forms left-to-right
2. If any form evaluates to NIL, stop and return NIL
3. Otherwise, return the value of the last form

---

## 6.7 OR

**Syntax:** `(or form1 form2 ...)`

Short-circuit logical OR. Returns the first non-NIL value, or NIL if all are NIL.

```lisp
(or nil nil t)        ; => T
(or nil nil nil)      ; => NIL
(or 1 2 3)            ; => 1 (first non-NIL)
(or)                  ; => NIL (identity for OR)
```

**Evaluation Rule:**
1. Evaluate forms left-to-right
2. If any form evaluates to non-NIL, stop and return that value
3. Otherwise, return NIL

---

## 6.8 DEF

**Syntax:** `(def symbol value &optional docstring)`

Binds `symbol` to `value` in the current environment.

```lisp
(def x 42)
(def pi 3.14159 "The ratio of circumference to diameter")
```

**With docstring:**
```lisp
(def *debug* t "Enable debug mode")
(documentation '*debug*)  ; => "Enable debug mode"
```

**Evaluation Rule:**
1. Evaluate `value`
2. Bind `symbol` to the result
3. If `docstring` provided, store it in symbol's plist
4. Return `symbol`

---

## 6.9 SETQ

**Syntax:** `(setq symbol value)`

Assigns a new value to an existing variable.

```lisp
(def x 10)
(setq x 20)
x             ; => 20
```

**Multiple assignments:**
```lisp
(setq a 1)
(setq b 2)
(setq c 3)
```

**Evaluation Rule:**
1. Evaluate `value`
2. Find `symbol` in environment chain
3. Update its binding
4. If not found, create in current environment
5. Return the value

---

## 6.10 LET

**Syntax:** `(let ((var1 val1) (var2 val2) ...) body...)`

Creates local variable bindings for the duration of `body`.

```lisp
(let ((x 1)
      (y 2))
  (+ x y))   ; => 3
```

**Variables are not visible to each other during binding:**
```lisp
(def x 10)
(let ((x 1)
      (y x))   ; y gets the OUTER x (10)
  (list x y))  ; => (1 10)
```

**Evaluation Rule:**
1. Evaluate all value forms in the current environment
2. Create new environment with all bindings
3. Evaluate body forms in new environment
4. Return value of last body form

---

## 6.11 LAMBDA

**Syntax:** `(lambda (params...) body...)`

Creates an anonymous function.

```lisp
(lambda (x) (* x x))        ; Square function
(lambda (x y) (+ x y))      ; Addition function
(lambda () 42)              ; No-argument function
```

**Using lambda:**
```lisp
((lambda (x) (* x 2)) 5)    ; => 10

(mapcar '(1 2 3)
        (lambda (x) (* x x)))  ; => (1 4 9)
```

**Evaluation Rule:**
1. Capture the current environment (for closures)
2. Return a function object
3. When called, bind parameters to arguments, evaluate body

---

## 6.12 DEFUN

**Syntax:** `(defun name (params...) &optional docstring body...)`

Macro that defines a named function. Equivalent to `(def name (lambda ...))`.

```lisp
(defun square (x)
  (* x x))

(defun factorial (n)
  "Compute factorial of N."
  (if (= n 0)
      1
      (* n (factorial (- n 1)))))
```

**With docstring:**
```lisp
(defun add (a b)
  "Add two numbers A and B."
  (+ a b))

(documentation 'add)  ; => "Add two numbers A and B."
```

---

## 6.13 DEFMACRO

**Syntax:** `(defmacro name (params &rest rest) &optional docstring body...)`

Defines a macro. Macros transform code before evaluation.

```lisp
(defmacro when (test &rest body)
  `(if ,test (progn ,@body) nil))

(when (> 3 2)
  (print "yes")
  "result")
; Expands to: (IF (> 3 2) (PROGN (PRINT "yes") "result") NIL)
```

**With `&rest`:**
```lisp
(defmacro unless (test &rest body)
  `(if (not ,test) (progn ,@body) nil))
```

**Evaluation Rule:**
1. When macro is called, bind parameters to unevaluated arguments
2. Evaluate macro body to produce expansion
3. Evaluate the expansion in the caller's environment

---

## 6.14 DEFEXPR (Fexpr)

**Syntax:** `(defexpr name (args-symbol) &optional docstring body...)`

Defines a function that receives its arguments unevaluated.

```lisp
(defexpr my-quote (args)
  (car args))

(my-quote (+ 1 2))  ; => (+ 1 2) (not 3)
```

**Implementing control structures:**
```lisp
(defexpr my-if (args)
  (if (eval (car args))
      (eval (cadr args))
      (eval (caddr args))))

(my-if (> 3 2) "yes" "no")  ; => "yes"
```

**Difference from macros:**
- Macros return code that is then evaluated
- Fexprs directly compute the result

---

## 6.15 LABEL

**Syntax:** `(label name (lambda ...))`

Creates a recursive function binding. Useful for anonymous recursion. The
payload must be a literal `LAMBDA` expression; malformed nested `LABEL` graphs
are rejected rather than re-evaluated as delayed expressions.

```lisp
((label fac (lambda (n)
              (if (= n 0)
                  1
                  (* n (fac (- n 1))))))
 5)
; => 120
```

**Evaluation Rule:**
1. Create a child environment
2. Evaluate the lambda in that child environment
3. Bind `name` to the resulting closure in the same child environment
4. Return the closure, whose body can reference `name`

---

## 6.16 FUNCTION

**Syntax:** `(function name)` or `#'name`

Returns the function value of a symbol. Used when passing functions as arguments.

```lisp
(function car)        ; The CAR function
(mapcar '(1 2 3) (function (lambda (x) (* x 2))))
```

**Note:** `#'` is Common Lisp syntax; Lamedh uses explicit `(function ...)`.

---

## 6.17 DEFINE

**Syntax:** `(define ((name1 (params1) body1) ...))`

Defines multiple functions at once (Lisp 1.5 style).

```lisp
(define ((double (x) (* x 2))
         (triple (x) (* x 3))))

(double 5)   ; => 10
(triple 5)   ; => 15
```

---

## 6.18 PROG

**Syntax:** `(prog (vars...) statements...)`

Provides local variables and labeled statements with GO/RETURN.

```lisp
(prog (sum i)
  (setq sum 0)
  (setq i 0)
 loop
  (if (> i 10) (return sum))
  (setq sum (+ sum i))
  (setq i (+ i 1))
  (go loop))
; => 55
```

**Components:**
- Variable list initialized to NIL
- Statements (evaluated sequentially)
- Labels (symbols used as targets for GO)
- GO jumps to a label
- RETURN exits with a value

---

## 6.19 PROGN

**Syntax:** `(progn form1 form2 ...)`

Evaluates forms in sequence, returns the last value.

```lisp
(progn
  (print "First")
  (print "Second")
  42)
; Prints: First Second
; => 42
```

---

## 6.20 GO

**Syntax:** `(go label)`

Transfers control to `label` within a PROG.

```lisp
(prog ()
 start
  (print "hello")
  (go start))  ; Infinite loop
```

**Evaluation Rule:**
- Only valid inside PROG
- Signals a non-local jump to the named label

---

## 6.21 RETURN

**Syntax:** `(return value)`

Returns from a PROG with the specified value.

```lisp
(prog (x)
  (setq x 42)
  (return x))
; => 42
```

**Evaluation Rule:**
- Evaluate `value`
- Signal return from enclosing PROG with that value

---

## 6.22 Special Form Summary

| Form | Evaluates Args? | Purpose |
|------|-----------------|---------|
| `QUOTE` | No | Return unevaluated |
| `IF` | Conditionally | Branch |
| `COND` | Conditionally | Multi-branch |
| `AND`/`OR` | Conditionally | Short-circuit logic |
| `DEF` | Yes (value) | Bind variable |
| `SETQ` | Yes (value) | Update variable |
| `LET` | Yes (values) | Local scope |
| `LAMBDA` | No | Create function |
| `DEFMACRO` | No | Define macro |
| `DEFEXPR` | No | Define fexpr |
| `LABEL` | Special | Named recursion |
| `FUNCTION` | No | Get function object |
| `PROG` | Special | Imperative block |
| `PROGN` | Yes | Sequence |
| `GO` | No | Jump |
| `RETURN` | Yes | Exit PROG |

---

**Next:** [Arithmetic Functions](functions/arithmetic.md)
