# Standard Library

This chapter documents the Lamedh standard library, which is loaded automatically at startup from the `lib/` directory.

---

## Overview

The standard library provides essential functions and macros that extend the built-in functionality. Files are loaded in alphabetical order:

| File | Contents |
|------|----------|
| `00-core.lisp` | `DEFUN` macro |
| `01-list.lisp` | List utilities |
| `02-cxr.lisp` | CAR/CDR compositions |
| `03-meta.lisp` | `DOCUMENTATION` |
| `04-predicates.lisp` | `EQUAL` |
| `05-math.lisp` | Math utilities |
| `07-shell.lisp` | Shell result helpers |
| `08-vau.lisp` | Kernel-style vau derived forms |
| `09-lisp15.lisp` | Lisp 1.5 appendix functions |
| `10-testing.lisp` | xUnit-style testing helpers |
| `11-optimizer-vau.lisp` | Lisp-level optimizer wrappers |
| `12-control.lisp` | Control-flow helpers |
| `13-functional.lisp` | Functional utilities |
| `14-strings.lisp` | String utilities |
| `15-sets-hash.lisp` | Set/hash helpers |
| `16-conditions.lisp` | Condition helpers |
| `17-arrays.lisp` | Array utilities |
| `18-format.lisp` | `FORMAT` subset |
| `19-typeclasses.lisp` | Explicit typeclass dictionaries |
| `20-condensation.lisp` | Condensation metadata and concept helpers |
| `97-doc-renderer.lisp` | Help renderer |
| `98-help-system.lisp` | `(HELP ...)` interface |
| `99-help-data.lisp` | Structured help database |

---

## 00-core.lisp: Core Macros

### DEFUN

**Syntax:** `(defun name (params...) &optional docstring body...)`

Defines a named function with optional docstring.

```lisp
(defun square (x)
  (* x x))

(defun factorial (n)
  "Compute factorial of N."
  (if (= n 0)
      1
      (* n (factorial (- n 1)))))
```

**Implementation:**
```lisp
(defmacro defun (name params &rest body)
  (if (stringp (car body))
    (let ((lambda-expr (cons 'lambda (cons params (cdr body)))))
      `(def ,name ,lambda-expr ,(car body)))
    (let ((lambda-expr (cons 'lambda (cons params body))))
      `(def ,name ,lambda-expr))))
```

---

## 01-list.lisp: List Utilities

### PAIRLIS

**Syntax:** `(pairlis keys values)`

Creates an association list from two parallel lists.

```lisp
(pairlis '(a b c) '(1 2 3))
; => ((A . 1) (B . 2) (C . 3))
```

---

### NULL

**Syntax:** `(null x)`

Returns T if x is NIL.

```lisp
(null nil)      ; => T
(null '())      ; => T
(null '(a))     ; => NIL
```

---

### APPEND

**Syntax:** `(append list1 list2)`

Concatenates two lists.

```lisp
(append '(a b) '(c d))  ; => (A B C D)
(append nil '(a))       ; => (A)
(append '(a) nil)       ; => (A)
```

---

### MEMBER

**Syntax:** `(member item list)`

Finds item in list; returns tail starting at match.

```lisp
(member 'b '(a b c))        ; => (B C)
(member 'x '(a b c))        ; => NIL
(member '(1) '((1) (2)))    ; => ((1) (2))  ; Uses EQUAL
```

---

### LENGTH

**Syntax:** `(length list)`

Returns number of elements in list.

```lisp
(length '(a b c))   ; => 3
(length nil)        ; => 0
```

---

### REVERSE

**Syntax:** `(reverse list)`

Returns list with elements in reverse order.

```lisp
(reverse '(a b c))  ; => (C B A)
(reverse nil)       ; => NIL
```

---

### CONSP

**Syntax:** `(consp x)`

Returns T if x is a cons cell.

```lisp
(consp '(a b))      ; => T
(consp nil)         ; => NIL
```

---

### LISTP

**Syntax:** `(listp x)`

Returns T if x is a list (cons or NIL).

```lisp
(listp '(a b))      ; => T
(listp nil)         ; => T
(listp 'a)          ; => NIL
```

---

## 02-cxr.lisp: CAR/CDR Compositions

Defines all 2, 3, and 4-level compositions of CAR and CDR:

### Two-Level

| Function | Expansion |
|----------|-----------|
| `CAAR` | `(car (car x))` |
| `CADR` | `(car (cdr x))` |
| `CDAR` | `(cdr (car x))` |
| `CDDR` | `(cdr (cdr x))` |

### Three-Level

| Function | Meaning |
|----------|---------|
| `CADR` | Second element |
| `CADDR` | Third element |
| `CADDDR` | Fourth element |

All 8 three-level combinations: CAAAR, CAADR, CADAR, CADDR, CDAAR, CDADR, CDDAR, CDDDR

### Four-Level

All 16 four-level combinations from CAAAAR to CDDDDR.

**Example:**
```lisp
(cadr '(a b c))      ; => B
(caddr '(a b c d))   ; => C
(cadddr '(a b c d e)) ; => D
```

---

## 03-meta.lisp: Metaprogramming

### DOCUMENTATION

**Syntax:** `(documentation symbol)`

Returns the docstring for a symbol.

```lisp
(defun square (x)
  "Compute the square of X."
  (* x x))

(documentation 'square)
; => "Compute the square of X."
```

Equivalent to `(getp symbol "docstring")`.

---

## 04-predicates.lisp: Predicates

### EQUAL

**Syntax:** `(equal a b)`

Tests structural equality recursively.

```lisp
(equal 'a 'a)              ; => T
(equal '(a b) '(a b))      ; => T
(equal '((1 2) 3) '((1 2) 3))  ; => T
(equal "hi" "hi")          ; => T
```

**Comparison:**
- `EQ` - Identity (same object)
- `EQUAL` - Structural (same content)
- `=` - Numeric equality

---

## 05-math.lisp: Math Utilities

### ONEP

**Syntax:** `(onep x)`

Returns T if x equals 1.

```lisp
(onep 1)    ; => T
(onep 2)    ; => NIL
```

---

### MINUSP

**Syntax:** `(minusp x)`

Returns T if x is negative.

```lisp
(minusp -5)  ; => T
(minusp 0)   ; => NIL
(minusp 5)   ; => NIL
```

---

### ADD1

**Syntax:** `(add1 x)`

Returns x + 1.

```lisp
(add1 5)    ; => 6
```

**Note:** Also available as builtin `1+`.

---

### SUB1

**Syntax:** `(sub1 x)`

Returns x - 1.

```lisp
(sub1 5)    ; => 4
```

**Note:** Also available as builtin `1-`.

---

### MAX

**Syntax:** `(max number...)`

Returns the largest argument.

```lisp
(max 1 5 3)      ; => 5
(max -1 -5 -3)   ; => -1
```

---

### MIN

**Syntax:** `(min number...)`

Returns the smallest argument.

```lisp
(min 1 5 3)      ; => 1
(min -1 -5 -3)   ; => -5
```

---

### ABS

**Syntax:** `(abs x)`

Returns the absolute value of x.

```lisp
(abs 5)     ; => 5
(abs -5)    ; => 5
(abs 0)     ; => 0
```

---

## 99-help-data.lisp: Builtin Documentation

This file registers structured help records for built-in functions:

```lisp
(register-doc 'list
  (list
    (cons 'NAME 'list)
    (cons 'DESCRIPTION "Constructs a list from its arguments.")))
;; etc.
```

You can access these with:
```lisp
(documentation 'list)  ; => "Constructs a list from its arguments."
```

---

## Extending the Standard Library

Add your own library files to `lib/`:

```lisp
;; lib/07-myutils.lisp
(defun double (x)
  "Double X."
  (* x 2))

(defun square (x)
  "Square X."
  (* x x))
```

Files are loaded in alphabetical order, so use numeric prefixes to control dependencies.

---

## Library Loading Details

### Automatic Loading

At startup, Lamedh:
1. Creates the global environment with builtins
2. Loads `prologue.lisp` (if present)
3. Loads all `.lisp` files in `lib/` alphabetically
4. Loads any files specified with `-i`

### Manual Loading

```lisp
(load-file "mylib.lisp")
```

### Load Order Matters

If `02-cxr.lisp` depends on functions from `01-list.lisp`, the numbering ensures correct order.

---

## Summary

| Category | Functions |
|----------|-----------|
| **Core** | `DEFUN` |
| **Lists** | `NULL`, `APPEND`, `MEMBER`, `LENGTH`, `REVERSE`, `PAIRLIS`, `CONSP`, `LISTP` |
| **CxR** | `CAAR`...`CDDDDR` (30 functions) |
| **Meta** | `DOCUMENTATION` |
| **Predicates** | `EQUAL` |
| **Math** | `ONEP`, `MINUSP`, `ADD1`, `SUB1`, `MAX`, `MIN`, `ABS` |

---

**See Also:**
- [List Functions](functions/lists.md)
- [Arithmetic Functions](functions/arithmetic.md)
- [Predicates](functions/predicates.md)
