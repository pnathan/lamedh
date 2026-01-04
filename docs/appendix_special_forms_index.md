# Appendix B: Special Forms Index

A complete index of all special forms in Lamedh.

---

## Quick Reference

| Special Form | Purpose |
|--------------|---------|
| `AND` | Short-circuit logical and |
| `COND` | Multi-way conditional |
| `DEF` | Define variable |
| `DEFINE` | Batch definitions |
| `DEFEXPR` | Define fexpr |
| `DEFMACRO` | Define macro |
| `DEFUN` | Define function (macro) |
| `FUNCTION` | Function reference |
| `GO` | Jump to label |
| `IF` | Two-way conditional |
| `LABEL` | Named recursive function |
| `LAMBDA` | Anonymous function |
| `LET` | Local bindings |
| `OR` | Short-circuit logical or |
| `PROG` | Imperative block |
| `PROGN` | Sequential evaluation |
| `QUASIQUOTE` | Template quotation |
| `QUOTE` | Prevent evaluation |
| `RETURN` | Return from PROG |
| `SETQ` | Assign variable |

---

## Detailed Reference

### AND

**Syntax:** `(and form...)`

Evaluates forms left-to-right until one returns NIL.

```lisp
(and t t t)       ; => T
(and t nil t)     ; => NIL (stops at nil)
(and 1 2 3)       ; => 3
```

---

### COND

**Syntax:** `(cond (test form...)...)`

Multi-way conditional.

```lisp
(cond ((< x 0) "negative")
      ((= x 0) "zero")
      (t "positive"))
```

---

### DEF

**Syntax:** `(def symbol value &optional docstring)`

Define a global variable.

```lisp
(def x 42)
(def pi 3.14159 "The ratio")
```

---

### DEFINE

**Syntax:** `(define ((name (params) body)...))`

Define multiple functions at once.

```lisp
(define ((double (x) (* x 2))
         (triple (x) (* x 3))))
```

---

### DEFEXPR

**Syntax:** `(defexpr name (args) &optional docstring body...)`

Define an fexpr (unevaluated args).

```lisp
(defexpr my-quote (args)
  (car args))
```

---

### DEFMACRO

**Syntax:** `(defmacro name (params &rest rest) &optional docstring body...)`

Define a macro.

```lisp
(defmacro when (test &rest body)
  `(if ,test (progn ,@body) nil))
```

---

### DEFUN

**Syntax:** `(defun name (params...) &optional docstring body...)`

Define a named function. (Implemented as macro)

```lisp
(defun square (x)
  "Square X."
  (* x x))
```

---

### FUNCTION

**Syntax:** `(function name)`

Get function object for a name.

```lisp
(function car)
(mapcar list (function square))
```

---

### GO

**Syntax:** `(go label)`

Jump to label in PROG.

```lisp
(prog ()
 loop
  (print "hi")
  (go loop))
```

---

### IF

**Syntax:** `(if test then else)`

Two-way conditional.

```lisp
(if (> x 0) "positive" "non-positive")
```

---

### LABEL

**Syntax:** `(label name function)`

Create named recursive function.

```lisp
((label fac (lambda (n)
              (if (= n 0) 1 (* n (fac (- n 1))))))
 5)  ; => 120
```

---

### LAMBDA

**Syntax:** `(lambda (params...) body...)`

Create anonymous function.

```lisp
(lambda (x) (* x x))
(lambda (x y) (+ x y))
```

---

### LET

**Syntax:** `(let ((var val)...) body...)`

Local variable bindings.

```lisp
(let ((x 1) (y 2))
  (+ x y))  ; => 3
```

---

### OR

**Syntax:** `(or form...)`

Short-circuit logical or.

```lisp
(or nil nil t)  ; => T
(or 1 2 3)      ; => 1
```

---

### PROG

**Syntax:** `(prog (vars...) statements...)`

Imperative block with labels.

```lisp
(prog (sum i)
  (setq sum 0 i 0)
 loop
  (if (> i 10) (return sum))
  (setq sum (+ sum i))
  (setq i (+ i 1))
  (go loop))
```

---

### PROGN

**Syntax:** `(progn form...)`

Evaluate forms sequentially.

```lisp
(progn
  (print "one")
  (print "two")
  42)  ; => 42
```

---

### QUASIQUOTE

**Syntax:** `` `expression `` or `(quasiquote expression)`

Template with unquote.

```lisp
(def x 10)
`(a ,x b)  ; => (A 10 B)
```

---

### QUOTE

**Syntax:** `'expression` or `(quote expression)`

Return expression unevaluated.

```lisp
'(+ 1 2)        ; => (+ 1 2)
(quote foo)     ; => FOO
```

---

### RETURN

**Syntax:** `(return value)`

Return from PROG.

```lisp
(prog (x)
  (setq x 42)
  (return x))  ; => 42
```

---

### SETQ

**Syntax:** `(setq symbol value)`

Assign to variable.

```lisp
(def x 1)
(setq x 2)
x  ; => 2
```

---

## Evaluation Summary

| Form | Evaluates Args? |
|------|-----------------|
| `QUOTE` | Never |
| `QUASIQUOTE` | Selectively (`,`) |
| `IF` | Conditionally |
| `COND` | Conditionally |
| `AND` | Until NIL |
| `OR` | Until non-NIL |
| `PROGN` | All, sequentially |
| `DEF` | Value only |
| `SETQ` | Value only |
| `LET` | Values only |
| `LAMBDA` | Never (closure) |
| `DEFMACRO` | Never (macro) |
| `DEFEXPR` | Never (fexpr) |
| `PROG` | Statements only |
| `GO` | Never |
| `RETURN` | Value only |

---

## See Also

- [Special Forms Chapter](special_forms.md) - Detailed documentation
- [Function Index](appendix_function_index.md) - All functions
