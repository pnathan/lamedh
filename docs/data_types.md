# Chapter 3: Data Types

Lamedh provides the following data types, each with distinct properties and operations.

---

## 3.1 Type Hierarchy

```
LispVal
├── Atom
│   ├── Symbol
│   ├── Number (i64)
│   ├── Float (f64)
│   ├── Char (u8, 0-255)
│   ├── String
│   └── Nil
├── Cons (car, cdr)
├── Function
│   ├── Lambda
│   ├── Fexpr
│   ├── Macro
│   ├── Vau
│   └── Builtin
├── Array           (boxed LispVal vector)
├── TypedArray      (flat int64/float64 buffer)
├── Struct          (defrecord / defstruct-typed instance)
├── HashTable
├── Error           (first-class condition value)
└── Environment
```

---

## 3.2 Numbers

### 3.2.1 Integers

64-bit signed integers (`i64`).

**Range:** -9,223,372,036,854,775,808 to 9,223,372,036,854,775,807

**Syntax:**
```lisp
42
-17
0
9223372036854775807   ; Maximum value
```

**Type Predicate:** `FIXP`

```lisp
(fixp 42)      ; => T
(fixp 3.14)    ; => NIL
```

### 3.2.2 Floating-Point Numbers

64-bit IEEE 754 floating-point numbers (`f64`).

**Syntax:**
```lisp
3.14159
-2.5
1.0e10
6.022e23
```

**Type Predicate:** `FLOATP`

```lisp
(floatp 3.14)  ; => T
(floatp 42)    ; => NIL
```

### 3.2.3 Numeric Type Predicate

`NUMBERP` returns `T` for both integers and floats:

```lisp
(numberp 42)    ; => T
(numberp 3.14)  ; => T
(numberp "42")  ; => NIL
```

### 3.2.4 Mixed Arithmetic

When integers and floats are mixed in arithmetic, the result is a float:

```lisp
(+ 1 2.0)    ; => 3.0
(* 3 1.5)    ; => 4.5
```

---

## 3.3 Symbols

Symbols are named identifiers used for variables, function names, and as data.

### 3.3.1 Symbol Syntax

```lisp
foo
MY-VARIABLE
*global-setting*
+special+
:keyword
null
t
```

**Rules:**
- May contain letters, digits, and special characters: `-`, `*`, `+`, `/`, `<`, `>`, `=`, `?`, `!`, `@`
- May begin with `:` to form a keyword symbol, such as `:op`
- Cannot start with a digit
- Case-insensitive (stored as uppercase)

Keyword symbols are ordinary interned symbols whose names begin with `:`, but
they evaluate to themselves. They are useful as data tags in macros, property
lists, and DSL forms.

### 3.3.2 Symbol Interning

All symbols are **interned** in a global symbol table. Two symbols with the same name are always `EQ`:

```lisp
(eq 'foo 'FOO)     ; => T (same symbol)
(eq 'foo 'bar)     ; => NIL
```

### 3.3.3 Special Symbols

| Symbol | Meaning |
|--------|---------|
| `NIL` | The empty list and boolean false |
| `T` | Boolean true |

### 3.3.4 Property Lists

Every symbol has an associated **property list** (plist) for storing metadata:

```lisp
(putp 'my-sym "version" 1)
(getp 'my-sym "version")  ; => 1
(plist 'my-sym)           ; => ("version" 1)
```

### 3.3.5 Type Predicate

```lisp
(symbolp 'foo)     ; => T
(symbolp 42)       ; => NIL
(symbolp nil)      ; => T (NIL is a symbol)
```

---

## 3.4 Strings

Strings are sequences of characters enclosed in double quotes.

### 3.4.1 String Syntax

```lisp
"Hello, World!"
"Line one\nLine two"
"A quoted word: \"Lamedh\""
""                  ; Empty string
```

Supported escapes are `\n`, `\t`, `\r`, `\\`, `\"`, and `\0`. Unknown escape
sequences retain the backslash and following character.

### 3.4.2 String Operations

```lisp
(concat "Hello, " "World!")  ; => "Hello, World!"
(index "hello" 1)            ; => "e"
```

### 3.4.3 Type Predicate

```lisp
(stringp "hello")  ; => T
(stringp 'hello)   ; => NIL
```

---

## 3.5 Cons Cells and Lists

### 3.5.1 Cons Cells

A **cons cell** is a pair of values: the `car` (first) and `cdr` (second, "rest").

```lisp
(cons 'a 'b)       ; => (A . B)  ; Dotted pair
```

**Visual representation:**
```
[car|cdr]
  |   |
  A   B
```

### 3.5.2 Proper Lists

A **proper list** is a chain of cons cells ending in `NIL`:

```lisp
(cons 'a (cons 'b (cons 'c nil)))
; => (A B C)
```

**Visual representation:**
```
[A|•]─→[B|•]─→[C|NIL]
```

Shorthand syntax:
```lisp
(list 'a 'b 'c)    ; => (A B C)
'(a b c)           ; => (A B C)
```

### 3.5.3 Improper Lists

An **improper list** ends with a non-NIL value:

```lisp
(cons 'a (cons 'b 'c))
; => (A B . C)
```

### 3.5.4 List Operations

| Function | Description |
|----------|-------------|
| `CAR` | First element |
| `CDR` | Rest of list |
| `CONS` | Construct new cons cell |
| `LIST` | Create list from arguments |
| `LENGTH` | Number of elements |
| `NTH` | Get nth element |
| `LAST` | Last cons cell |

```lisp
(car '(a b c))     ; => A
(cdr '(a b c))     ; => (B C)
(cons 'x '(y z))   ; => (X Y Z)
(length '(a b c))  ; => 3
(nth 1 '(a b c))   ; => B
```

### 3.5.5 Type Predicates

```lisp
(atom 'a)          ; => T (not a cons)
(atom '(a))        ; => NIL
(consp '(a b))     ; => T
(consp nil)        ; => NIL
(listp '(a b))     ; => T
(listp nil)        ; => T (NIL is the empty list)
(null nil)         ; => T
(null '())         ; => T
(null '(a))        ; => NIL
```

---

## 3.6 NIL and T

### 3.6.1 NIL

`NIL` represents:
- The empty list `()`
- Boolean false

```lisp
(eq nil '())       ; => T
(null nil)         ; => T
(car nil)          ; => NIL
(cdr nil)          ; => NIL
```

### 3.6.2 T

`T` represents boolean true. Any non-NIL value is considered true:

```lisp
(if t "yes" "no")    ; => "yes"
(if 'foo "yes" "no") ; => "yes"
(if nil "yes" "no")  ; => "no"
```

---

## 3.7 Functions

### 3.7.1 Lambda Functions

Anonymous functions created with `LAMBDA`:

```lisp
(lambda (x) (* x x))
```

### 3.7.2 Named Functions

Created with `DEFUN`:

```lisp
(defun square (x) (* x x))
```

### 3.7.3 Fexprs

Functions that receive unevaluated arguments:

```lisp
(defexpr my-quote (args)
  (car args))

(my-quote (+ 1 2))  ; => (+ 1 2) (not evaluated)
```

### 3.7.4 Macros

Code transformers that run at expansion time:

```lisp
(defmacro double (x)
  `(+ ,x ,x))

(double 5)  ; Expands to (+ 5 5), returns 10
```

### 3.7.5 Type Predicates

```lisp
(functionp (lambda (x) x))  ; => T
(functionp 'car)            ; => T (after lookup)
(macrop 'defun)             ; => T
```

---

## 3.8 Hash Tables

Mutable key-value stores.

### 3.8.1 Creating Hash Tables

```lisp
(def my-table (make-hash-table))
```

### 3.8.2 Operations

```lisp
(set-bang my-table "name" "Alice")
(gethash my-table "name")    ; => "Alice"
(keys my-table)              ; => ("name")
(remhash my-table "name")
```

### 3.8.3 Key Types

Any value can be used as a key, but symbols and strings are recommended for
stable semantic keys. Float keys have a lawful `Eq`/`Hash` implementation
(`0.0` and `-0.0` hash together; NaNs are canonicalized), but ordinary
floating-point rounding can still make them surprising.

---

## 3.9 Type Summary

| Type | Predicate | Example |
|------|-----------|---------|
| Integer | `FIXP` | `42` |
| Float | `FLOATP` | `3.14` |
| Number | `NUMBERP` | `42`, `3.14` |
| Symbol | `SYMBOLP` | `'foo` |
| String | `STRINGP` | `"hello"` |
| Cons | `CONSP` | `'(a . b)` |
| List | `LISTP` | `'(a b c)` |
| Atom | `ATOM` | anything not a cons |
| Function | `FUNCTIONP` | `(lambda (x) x)` |
| Macro | `MACROP` | defined with `DEFMACRO` |
| Bound | `BOUNDP` | symbol with a value |

---

## 3.10 Equality

| Function | Semantics |
|----------|-----------|
| `EQ` | Identity (same object) |
| `=` | Numeric equality |
| `EQUAL` | Structural equality (recursive) |

```lisp
(eq 'a 'a)              ; => T
(eq '(1 2) '(1 2))      ; => NIL (different conses)
(= 1 1.0)               ; => T
(equal '(1 2) '(1 2))   ; => T
```

---

## 3.11 Arrays

Fixed-size, mutable, zero-indexed vectors of boxed `LispVal`s. Create one
with `ARRAY` (alias `MAKE-ARRAY`), read and write with `FETCH`/`STORE`
(aliases `AREF`/`ASET`), query the size with `ARRAY-LENGTH*`, and test the
type with `ARRAYP`.

```lisp
(let ((a (array 3)))
  (store a 0 'x)          ; mutate in place, returns the stored value
  (fetch a 0))            ; => X

(array-length* (array 4)) ; => 4
(arrayp (array 2))        ; => T
```

Out-of-bounds access signals an error rather than returning `NIL`. There is
no array literal in the reader; build arrays with `ARRAY` or `LIST->ARRAY`.
`defstruct-typed`/`defrecord` instances (`STRUCT`) are arrays internally, so
`ARRAYP` is also `T` for them.

## 3.12 Typed Arrays

`TYPED-ARRAY` builds a flat buffer of unboxed `int64` or `float64` elements
rather than a vector of boxed values:

```lisp
(typed-array 4 'int64)          ; => <typed-array:int64:4>
(typed-array-p (typed-array 3 'float64))  ; => T
```

The element type is the symbol `'int64` or `'float64`; the size is capped at
16M elements. `FETCH`/`STORE`/`AREF`/`ASET`/`ARRAY-LENGTH*` work exactly as
on a plain array, and `ARRAYP` is `T` (use `TYPED-ARRAY-P` for the narrow
test).

The point of a typed array is its memory layout: it is a raw `[len, e0, e1,
…]` `u64` buffer identical to what the typed JIT allocates for its own array
parameters. When a typed array is passed to a natively compiled function
whose parameter's element type matches, it crosses the JIT membrane **by
pointer with no copy** — in-place `STORE`/`ASET` writes the callee performs
are visible to the caller. A plain `ARRAY` crossing the same membrane is
copied in and (if the parameter may be mutated) copied back out. See
[The Typed JIT and Embedding](manual/09-jit-and-embedding.md) for the
membrane semantics.

---

**Next:** [Syntax and Evaluation](syntax.md)
