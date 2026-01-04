# Metaprogramming Functions

This chapter documents functions for code manipulation, evaluation, and reflection in Lamedh.

---

## Evaluation

### EVAL

**Syntax:** `(eval expression)`

Evaluates an expression in the current environment.

```lisp
(eval '(+ 1 2))          ; => 3
(eval '(list 'a 'b 'c))  ; => (A B C)

(def x 10)
(eval 'x)                ; => 10
(eval '(* x 2))          ; => 20
```

**Arguments:**
- `expression` - Any Lisp expression

**Returns:** Result of evaluating the expression

**Use for:** Dynamic code execution, interpreters, DSLs

---

### APPLY

**Syntax:** `(apply function args)`

Applies a function to a list of arguments.

```lisp
(apply #'+ '(1 2 3))     ; => 6
(apply #'list '(a b c))  ; => (A B C)
(apply #'cons '(x (y z))) ; => (X Y Z)
```

**Arguments:**
- `function` - A function or function reference
- `args` - A list of arguments

**Returns:** Result of applying function to args

**Example:**
```lisp
(defun sum-list (lst)
  (apply #'+ lst))

(sum-list '(1 2 3 4 5))  ; => 15
```

---

### FUNCALL

**Syntax:** `(funcall function arg...)`

Calls a function with the given arguments.

```lisp
(funcall #'+ 1 2 3)      ; => 6
(funcall (lambda (x) (* x x)) 5)  ; => 25

(def my-fn #'+)
(funcall my-fn 10 20)    ; => 30
```

**Arguments:**
- `function` - A function
- `arg...` - Zero or more arguments

**Returns:** Result of calling function with args

**Difference from APPLY:**
- FUNCALL: `(funcall fn a b c)` - individual args
- APPLY: `(apply fn '(a b c))` - args as list

---

## Macro Operations

### MACROEXPAND

**Syntax:** `(macroexpand form)`

Expands a macro form (one level).

```lisp
(defmacro when (test &rest body)
  `(if ,test (progn ,@body) nil))

(macroexpand '(when (> x 0) (print x) x))
; => (IF (> X 0) (PROGN (PRINT X) X) NIL)
```

**Arguments:**
- `form` - A quoted form that may be a macro call

**Returns:** Expanded form, or original if not a macro

**Use for:** Debugging macros, understanding expansions

---

## Symbol Functions

### GENSYM

**Syntax:** `(gensym)`

Generates a unique uninterned symbol.

```lisp
(gensym)    ; => G0001
(gensym)    ; => G0002
(gensym)    ; => G0003
```

**Returns:** Unique symbol

**Use for:** Creating temporary variables in macros

```lisp
(defmacro with-temp (expr)
  (let ((temp (gensym)))
    `(let ((,temp ,expr))
       (print ,temp)
       ,temp)))
```

---

### INTERN

**Syntax:** `(intern string)`

Creates or finds a symbol with the given name.

```lisp
(intern "HELLO")         ; => HELLO
(eq (intern "FOO") 'foo) ; => T
```

**Arguments:**
- `string` - Symbol name as string

**Returns:** Interned symbol

---

### BOUNDP

**Syntax:** `(boundp symbol)`

Returns T if symbol has a value binding.

```lisp
(def x 42)
(boundp 'x)              ; => T
(boundp 'undefined)      ; => NIL
```

---

## Type Inspection

### FUNCTIONP

**Syntax:** `(functionp object)`

Returns T if object is a function.

```lisp
(functionp (lambda (x) x))  ; => T
(functionp #'car)           ; => T
(functionp 'car)            ; => NIL (symbol)
(functionp 42)              ; => NIL
```

---

### MACROP

**Syntax:** `(macrop object)`

Returns T if object is a macro.

```lisp
(defmacro my-macro (x) x)
;; Need to get the macro object, not the symbol
```

---

### SYMBOLP

**Syntax:** `(symbolp object)`

Returns T if object is a symbol.

```lisp
(symbolp 'foo)      ; => T
(symbolp 42)        ; => NIL
(symbolp nil)       ; => T
```

---

## Environment Access

### CURRENT-ENVIRONMENT

**Syntax:** `(current-environment)`

Returns a hash table of current bindings.

```lisp
(let ((x 1) (y 2))
  (keys (current-environment)))
; => List including X, Y, and inherited bindings
```

---

## Code Generation Patterns

### Building Code

```lisp
(defun make-adder-fn (n)
  "Create code for a function that adds N."
  `(lambda (x) (+ x ,n)))

(def add-5-code (make-adder-fn 5))
; => (LAMBDA (X) (+ X 5))

(def add-5 (eval add-5-code))
(add-5 10)   ; => 15
```

### Dynamic Dispatch

```lisp
(def operations (make-hash-table))
(set-bang operations 'add #'+)
(set-bang operations 'sub #'-)
(set-bang operations 'mul #'*)

(defun dispatch (op &rest args)
  (let ((fn (get operations op)))
    (if fn
        (apply fn args)
        (error "Unknown operation"))))

(dispatch 'add 1 2 3)   ; => 6
(dispatch 'mul 2 3 4)   ; => 24
```

### Code Walker

```lisp
(defun find-symbols (expr)
  "Find all symbols in EXPR."
  (cond ((null expr) nil)
        ((symbolp expr) (list expr))
        ((atom expr) nil)
        (t (append (find-symbols (car expr))
                   (find-symbols (cdr expr))))))

(find-symbols '(+ x (* y z)))
; => (+ X * Y Z)
```

---

## Macro Writing

### Basic Macro

```lisp
(defmacro unless (test &rest body)
  `(if (not ,test)
       (progn ,@body)
       nil))

(unless (> 1 2)
  (print "1 is not greater than 2"))
```

### Macro with Gensym

```lisp
(defmacro once-only (var expr &rest body)
  "Evaluate EXPR once, bind to VAR for BODY."
  (let ((temp (gensym)))
    `(let ((,temp ,expr))
       (let ((,var ,temp))
         ,@body))))
```

### Debugging Macros

```lisp
(defun show-expansion (form)
  (princ "Form: ")
  (prin1 form)
  (terpri)
  (princ "Expands to: ")
  (prin1 (macroexpand form))
  (terpri))

(show-expansion '(unless nil (print "hi")))
; Form: (UNLESS NIL (PRINT "hi"))
; Expands to: (IF (NOT NIL) (PROGN (PRINT "hi")) NIL)
```

---

## Reflection

### Inspecting Functions

```lisp
(documentation 'car)     ; Get docstring
(plist 'my-fn)           ; Get all properties
(getp 'my-fn "version")  ; Get specific property
```

### Runtime Type Checking

```lisp
(defun type-of (x)
  (cond ((null x) 'null)
        ((symbolp x) 'symbol)
        ((numberp x) (if (fixp x) 'integer 'float))
        ((stringp x) 'string)
        ((consp x) 'cons)
        ((functionp x) 'function)
        (t 'unknown)))

(type-of 42)        ; => INTEGER
(type-of '(a b))    ; => CONS
(type-of #'+)       ; => FUNCTION
```

---

## Common Idioms

### Self-Evaluating Check

```lisp
(defun self-evaluating-p (x)
  (or (numberp x)
      (stringp x)
      (null x)
      (eq x t)))
```

### Quote Needed?

```lisp
(defun needs-quote-p (x)
  (and (not (self-evaluating-p x))
       (not (and (consp x)
                 (eq (car x) 'quote)))))
```

---

**See Also:**
- [Special Forms](../special_forms.md) - DEFMACRO, LAMBDA
- [Property Lists](plists.md) - Symbol metadata
- [Environments](../environments.md) - Scope and binding
