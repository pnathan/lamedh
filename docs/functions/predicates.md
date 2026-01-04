# Predicates

This chapter documents all predicate functions in Lamedh. Predicates return T (true) or NIL (false).

---

## Equality Predicates

### EQ

**Syntax:** `(eq a b)`

Returns T if a and b are the same object (identity).

```lisp
(eq 'a 'a)          ; => T (same symbol)
(eq 'a 'b)          ; => NIL
(eq 42 42)          ; => T
(eq '(a) '(a))      ; => NIL (different cons cells)
(eq nil nil)        ; => T
```

**Use for:**
- Comparing symbols
- Comparing numbers (by value)
- Checking identity

---

### EQUAL

**Syntax:** `(equal a b)`

Returns T if a and b are structurally equivalent. (Standard library function)

```lisp
(equal 'a 'a)           ; => T
(equal '(a b) '(a b))   ; => T
(equal "hi" "hi")       ; => T
(equal '((1 2) 3) '((1 2) 3))  ; => T
```

**Recursively compares:**
- Atoms with EQ
- Lists element by element

---

### =

**Syntax:** `(= a b)`

Returns T if a and b are numerically equal.

```lisp
(= 1 1)             ; => T
(= 1 1.0)           ; => T
(= 1 2)             ; => NIL
```

---

## Type Predicates

### ATOM

**Syntax:** `(atom x)`

Returns T if x is not a cons cell.

```lisp
(atom 'a)           ; => T
(atom 42)           ; => T
(atom "hello")      ; => T
(atom nil)          ; => T
(atom '(a b))       ; => NIL
```

---

### SYMBOLP

**Syntax:** `(symbolp x)`

Returns T if x is a symbol.

```lisp
(symbolp 'foo)      ; => T
(symbolp nil)       ; => T
(symbolp t)         ; => T
(symbolp 42)        ; => NIL
(symbolp "foo")     ; => NIL
```

---

### NUMBERP

**Syntax:** `(numberp x)`

Returns T if x is a number (integer or float).

```lisp
(numberp 42)        ; => T
(numberp 3.14)      ; => T
(numberp "42")      ; => NIL
(numberp 'forty)    ; => NIL
```

---

### FIXP

**Syntax:** `(fixp x)`

Returns T if x is a fixed-point (integer) number.

```lisp
(fixp 42)           ; => T
(fixp -17)          ; => T
(fixp 3.14)         ; => NIL
```

---

### FLOATP

**Syntax:** `(floatp x)`

Returns T if x is a floating-point number.

```lisp
(floatp 3.14)       ; => T
(floatp 42)         ; => NIL
(floatp 1.0)        ; => T
```

---

### STRINGP

**Syntax:** `(stringp x)`

Returns T if x is a string.

```lisp
(stringp "hello")   ; => T
(stringp 'hello)    ; => NIL
(stringp "")        ; => T
```

---

### CONSP

**Syntax:** `(consp x)`

Returns T if x is a cons cell. (Standard library function)

```lisp
(consp '(a b))      ; => T
(consp '(a . b))    ; => T
(consp nil)         ; => NIL
(consp 'a)          ; => NIL
```

---

### LISTP

**Syntax:** `(listp x)`

Returns T if x is a list (cons or NIL). (Standard library function)

```lisp
(listp '(a b))      ; => T
(listp nil)         ; => T
(listp '())         ; => T
(listp 'a)          ; => NIL
```

---

### NULL

**Syntax:** `(null x)`

Returns T if x is NIL. (Standard library function)

```lisp
(null nil)          ; => T
(null '())          ; => T
(null '(a))         ; => NIL
(null t)            ; => NIL
```

---

## Function Type Predicates

### FUNCTIONP

**Syntax:** `(functionp x)`

Returns T if x is a function (lambda, fexpr, or builtin).

```lisp
(functionp (lambda (x) x))    ; => T
(functionp #'car)             ; => T
(functionp 'car)              ; => NIL (symbol, not function)
```

---

### MACROP

**Syntax:** `(macrop x)`

Returns T if x is a macro.

```lisp
(defmacro my-macro (x) x)
(macrop (symbol-function 'my-macro))  ; => T
```

---

### BOUNDP

**Syntax:** `(boundp symbol)`

Returns T if symbol has a value binding.

```lisp
(def x 42)
(boundp 'x)         ; => T
(boundp 'undefined) ; => NIL (probably)
```

---

## Numeric Predicates

### ZEROP

**Syntax:** `(zerop n)`

Returns T if n is zero.

```lisp
(zerop 0)           ; => T
(zerop 0.0)         ; => T
(zerop 1)           ; => NIL
```

---

### PLUSP

**Syntax:** `(plusp n)`

Returns T if n is positive (> 0).

```lisp
(plusp 1)           ; => T
(plusp 0)           ; => NIL
(plusp -1)          ; => NIL
```

---

### MINUSP

**Syntax:** `(minusp n)`

Returns T if n is negative (< 0). (Standard library function)

```lisp
(minusp -1)         ; => T
(minusp 0)          ; => NIL
(minusp 1)          ; => NIL
```

---

### ONEP

**Syntax:** `(onep n)`

Returns T if n equals 1. (Standard library function)

```lisp
(onep 1)            ; => T
(onep 2)            ; => NIL
(onep 1.0)          ; => T
```

---

### EVENP

**Syntax:** `(evenp n)`

Returns T if n is an even integer.

```lisp
(evenp 2)           ; => T
(evenp 0)           ; => T
(evenp 3)           ; => NIL
```

---

### ODDP

**Syntax:** `(oddp n)`

Returns T if n is an odd integer.

```lisp
(oddp 3)            ; => T
(oddp 2)            ; => NIL
(oddp 1)            ; => T
```

---

## Logical Operations

### NOT

**Syntax:** `(not x)`

Returns T if x is NIL, NIL otherwise.

```lisp
(not nil)           ; => T
(not t)             ; => NIL
(not 'a)            ; => NIL
(not '())           ; => T
```

---

## Comparison Predicates

### <  (LESSP)

**Syntax:** `(< a b)` or `(lessp a b)`

Returns T if a is less than b.

```lisp
(< 1 2)             ; => T
(< 2 1)             ; => NIL
(< 1 1)             ; => NIL
```

---

### >  (GREATERP)

**Syntax:** `(> a b)` or `(greaterp a b)`

Returns T if a is greater than b.

```lisp
(> 2 1)             ; => T
(> 1 2)             ; => NIL
(> 1 1)             ; => NIL
```

---

## Type Predicate Summary

| Predicate | True For |
|-----------|----------|
| `ATOM` | Numbers, strings, symbols, NIL |
| `SYMBOLP` | Symbols (including NIL, T) |
| `NUMBERP` | Integers and floats |
| `FIXP` | Integers only |
| `FLOATP` | Floats only |
| `STRINGP` | Strings |
| `CONSP` | Cons cells |
| `LISTP` | Cons cells and NIL |
| `NULL` | NIL only |
| `FUNCTIONP` | Lambdas, fexprs, builtins |
| `MACROP` | Macros |
| `BOUNDP` | Symbols with bindings |

---

## Examples

### Type Dispatch

```lisp
(defun type-of (x)
  (cond ((null x) 'null)
        ((symbolp x) 'symbol)
        ((numberp x) 'number)
        ((stringp x) 'string)
        ((consp x) 'list)
        (t 'unknown)))

(type-of 'foo)      ; => SYMBOL
(type-of 42)        ; => NUMBER
(type-of '(a b))    ; => LIST
```

### Safe Car

```lisp
(defun safe-car (x)
  "Return car of x, or NIL if not a cons."
  (if (consp x)
      (car x)
      nil))

(safe-car '(a b))   ; => A
(safe-car 42)       ; => NIL
```

---

**See Also:**
- [Data Types](../data_types.md)
- [Arithmetic Functions](arithmetic.md)
