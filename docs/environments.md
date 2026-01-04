# Chapter 5: Environments and Scope

This chapter describes how Lamedh manages variable bindings, scoping, and symbol properties.

---

## 5.1 Environments

An **environment** is a mapping from symbol names to values. Environments form a chain, with each environment potentially having a parent.

### 5.1.1 Environment Structure

```
┌─────────────────┐
│ Global Env      │
│ x = 10          │
│ y = 20          │
│ +, -, car, ...  │  (builtins)
└────────┬────────┘
         │ parent
┌────────▼────────┐
│ Function Env    │
│ a = 1           │
│ b = 2           │
└────────┬────────┘
         │ parent
┌────────▼────────┐
│ Let Env         │
│ temp = 100      │
└─────────────────┘
```

### 5.1.2 Variable Lookup

When looking up a variable:

1. Check current environment
2. If not found, check parent
3. Continue up the chain
4. If not found in any environment, error

```lisp
(def x 10)           ; In global env

(defun foo (a)       ; Function creates new env with a
  (let ((b 20))      ; Let creates new env with b
    (+ a b x)))      ; a from function, b from let, x from global
```

---

## 5.2 Lexical Scope

Lamedh uses **lexical (static) scoping**. A function's environment is determined by where it's defined, not where it's called.

### 5.2.1 Closures

Functions capture their defining environment:

```lisp
(defun make-adder (n)
  (lambda (x) (+ x n)))  ; Captures n

(def add-5 (make-adder 5))
(def add-10 (make-adder 10))

(add-5 3)   ; => 8
(add-10 3)  ; => 13
```

### 5.2.2 Closure Example

```lisp
(defun make-counter ()
  (let ((count 0))
    (lambda ()
      (setq count (+ count 1))
      count)))

(def counter1 (make-counter))
(def counter2 (make-counter))

(counter1)  ; => 1
(counter1)  ; => 2
(counter2)  ; => 1 (separate state)
```

---

## 5.3 Binding Forms

### 5.3.1 DEF

Creates a binding in the **current** environment:

```lisp
(def x 42)                    ; Bind x to 42
(def pi 3.14159 "The ratio")  ; With docstring
```

### 5.3.2 LET

Creates a new environment with local bindings:

```lisp
(let ((x 1)
      (y 2))
  (+ x y))   ; => 3

; x and y are not visible here
```

LET bindings are evaluated in the **outer** environment:

```lisp
(def x 10)
(let ((x 1)
      (y x))     ; y gets outer x (10), not the new x (1)
  (+ x y))       ; => 11
```

### 5.3.3 LAMBDA Parameters

Lambda parameters create bindings in a new environment:

```lisp
(lambda (a b c)
  (+ a b c))

; When called with (1 2 3):
; Creates env with a=1, b=2, c=3
```

### 5.3.4 SETQ

Modifies an existing binding or creates one:

```lisp
(def x 10)
(setq x 20)     ; Modify x to 20
x               ; => 20

(setq y 30)     ; Creates y if not exists
y               ; => 30
```

**Note:** Unlike some Lisps, Lamedh's SETQ will create a variable if it doesn't exist.

---

## 5.4 The Global Environment

### 5.4.1 Builtin Bindings

The global environment is initialized with all builtin functions:

```lisp
+, -, *, /              ; Arithmetic
car, cdr, cons          ; List operations
eq, atom, null          ; Predicates
print, read             ; I/O
; ... and many more
```

### 5.4.2 Standard Library

After builtins, the standard library is loaded from `lib/`:

```lisp
defun                   ; Macro for defining functions
append, reverse, length ; List utilities
equal                   ; Structural equality
abs, max, min           ; Math functions
```

### 5.4.3 Constants

```lisp
t    ; => T (truth)
nil  ; => NIL (false/empty list)
```

---

## 5.5 Symbol Interning

### 5.5.1 The Symbol Table

All symbols are stored in a **global symbol table**. This ensures:

- Two symbols with the same name are identical (`EQ`)
- Efficient symbol comparison
- Memory efficiency for repeated symbols

```lisp
(eq 'foo 'foo)       ; => T (same object)
(eq 'foo 'FOO)       ; => T (case-insensitive)
```

### 5.5.2 Creating Symbols

```lisp
(intern "HELLO")     ; Find or create symbol HELLO
(gensym)             ; Create unique uninterned symbol
```

### 5.5.3 Symbol Names

```lisp
(explode 'hello)     ; => (H E L L O)
(implode '(A B C))   ; => ABC
```

---

## 5.6 Property Lists

Every symbol has an associated **property list** (plist) for storing metadata.

### 5.6.1 Structure

A plist is an association of indicators (keys) to values:

```
Symbol: FOO
Plist: ("docstring" "A test symbol"
        "version" 1
        "author" "Alice")
```

### 5.6.2 Accessing Properties

```lisp
(putp 'foo "color" "red")    ; Set property
(getp 'foo "color")          ; => "red"
(remprop 'foo "color")       ; Remove property
(plist 'foo)                 ; => All properties as list
```

### 5.6.3 Docstrings

Documentation strings are stored as the "docstring" property:

```lisp
(defun square (x)
  "Compute the square of X."
  (* x x))

(documentation 'square)   ; => "Compute the square of X."
(getp 'square "docstring") ; Same thing
```

### 5.6.4 DEFLIST

Set properties on multiple symbols at once:

```lisp
(deflist '((foo 1)
           (bar 2)
           (baz 3))
         "priority")

(getp 'foo "priority")  ; => 1
(getp 'bar "priority")  ; => 2
```

---

## 5.7 PROG and Local Variables

The `PROG` special form creates local variables initialized to NIL:

```lisp
(prog (x y z)
  (setq x 1)
  (setq y 2)
  (setq z (+ x y))
  (return z))
; => 3
```

---

## 5.8 Function Types and Environments

### 5.8.1 Lambda

Captures lexical environment, evaluates arguments:

```lisp
(def x 10)
(def f (lambda (a) (+ a x)))
(f 5)  ; => 15
```

### 5.8.2 Fexpr

Captures lexical environment, receives unevaluated arguments:

```lisp
(defexpr show-code (args)
  (progn
    (print "Code: ")
    (prin1 (car args))
    (terpri)
    (eval (car args))))

(show-code (+ 1 2))
; Prints: Code: (+ 1 2)
; => 3
```

### 5.8.3 Macro

Expands at call site, result is evaluated:

```lisp
(defmacro twice (x)
  `(progn ,x ,x))

(twice (print "hi"))
; Expands to: (PROGN (PRINT "hi") (PRINT "hi"))
; Prints "hi" twice
```

---

## 5.9 Environment Inspection

### 5.9.1 CURRENT-ENVIRONMENT

Returns a hash table of current bindings:

```lisp
(let ((x 1) (y 2))
  (current-environment))
; => Hash table with x=1, y=2 and inherited bindings
```

### 5.9.2 BOUNDP

Check if a symbol has a binding:

```lisp
(boundp 'car)    ; => T
(boundp 'xyz)    ; => NIL (probably)
```

---

## 5.10 Scope Examples

### 5.10.1 Shadowing

Inner bindings shadow outer ones:

```lisp
(def x 1)

(defun foo ()
  (let ((x 2))
    (let ((x 3))
      x)))    ; => 3

(foo)         ; => 3
x             ; => 1 (unchanged)
```

### 5.10.2 Free Variables

Functions access outer variables:

```lisp
(def multiplier 10)

(defun scale (x)
  (* x multiplier))

(scale 5)           ; => 50
(setq multiplier 100)
(scale 5)           ; => 500
```

### 5.10.3 Closure vs Global

```lisp
(def x 1)

(defun make-f ()
  (let ((x 10))
    (lambda () x)))

(def f (make-f))
(f)                 ; => 10 (uses closure's x, not global)
```

---

**Next:** [Special Forms](special_forms.md)
