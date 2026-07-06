# Appendix D: Differences from Lisp 1.5

This appendix documents how Lamedh differs from the original Lisp 1.5 specification.

---

## Overview

Lamedh is a "relaxed superset" of Lisp 1.5. It:

- Implements core Lisp 1.5 semantics
- Adds modern conveniences
- Changes some behaviors for safety or practicality

---

## Scoping Model

### Lisp 1.5: Dynamic Scope

Variables are looked up in the calling environment:

```lisp
;; In original Lisp 1.5:
(def x 1)
(defun foo () x)
(defun bar ()
  (let ((x 10))
    (foo)))
(bar)  ; => 10 (uses caller's x)
```

### Lamedh: Lexical Scope

Variables are looked up in the defining environment:

```lisp
;; In Lamedh:
(def x 1)
(defun foo () x)
(defun bar ()
  (let ((x 10))
    (foo)))
(bar)  ; => 1 (uses definer's x)
```

**Rationale:** Lexical scope is easier to reason about and enables closures.

---

## Data Types

### Added Types

| Type | Lisp 1.5 | Lamedh |
|------|----------|--------|
| Strings | No | Yes |
| Floats | Limited | Full IEEE 754 |
| Hash Tables | No | Yes |

### Number Representation

**Lisp 1.5:**
- Machine-dependent integers
- Limited floating point
- Octal notation (`1750Q`)

**Lamedh:**
- 64-bit signed integers
- 64-bit IEEE 754 floats
- Lisp 1.5 octal notation (`1750Q`) plus `#o`, `#b`, `#x`, and `H`-suffix
  radix literals

---

## Special Forms

### DEFUN

**Lisp 1.5:** Not a special form (used DEFINE)

**Lamedh:** Macro for function definition:
```lisp
(defun name (args) body)
```

### DEFMACRO

**Lisp 1.5:** Not present (used FEXPR)

**Lamedh:** Full macro system:
```lisp
(defmacro name (args) body)
```

### LET

**Lisp 1.5:** Not present

**Lamedh:** Local binding form:
```lisp
(let ((x 1) (y 2)) (+ x y))
```

---

## Functions

### Argument Order

**MAPCAR:**
```lisp
;; Lamedh uses function-first order:
(MAPCAR fn list)
```

### Function Names

| Operation | Lisp 1.5 | Lamedh |
|-----------|----------|--------|
| Property get | `GET` | `GET` / `GETP` |
| Property put | `PUT` | `PUT` / `PUTP` |
| Hash get | N/A | `GETHASH` |
| Hash set | N/A | `SET-BANG` / `SETHASH` |

### New Functions

Functions not in Lisp 1.5:

- String operations: `CONCAT`, `INDEX`
- Hash tables: `MAKE-HASH-TABLE`, `GETHASH`, `SET-BANG`, `SETHASH`
- Type predicates: `STRINGP`, `FLOATP`, `FUNCTIONP`
- Error handling: `ERRORSET`

---

## PROG Feature

### Lisp 1.5 PROG

```lisp
(PROG (var1 var2)
  statement1
  label
  statement2
  (GO label)
  (RETURN value))
```

### Lamedh PROG

Same syntax, but:
- Variables are lexically scoped
- Labels are collected when `PROG` is evaluated
- Duplicate labels warn and the later label is used

---

## Property Lists

### Lisp 1.5

Properties accessed via indicators:
```lisp
(GET sym indicator)
(PUT sym indicator value)
```

### Lamedh

Similar, but different names:
```lisp
(GETP sym indicator)
(PUTP sym indicator value)
```

---

## Error Handling

### Lisp 1.5

- `ERROR` function
- Limited recovery options

### Lamedh

- `ERROR` function
- `ERRORSET` for catching errors
- Partial condition handling, but no restart system

---

## Input/Output

### Lisp 1.5

- Card reader input
- Printer output
- Tape operations

### Lamedh

- Console I/O: `READ`, `PRINT`, `PRIN1`, `PRINC`, `TERPRI`
- File loading: `LOAD-FILE`
- Capability-gated file reading, writing, metadata, directory, mutation, and
  temporary-file helpers
- No Common Lisp-style stream system yet

---

## Evaluation

### EVAL in Lisp 1.5

Takes expression and association list:
```lisp
(EVAL expr alist)
```

### EVAL in Lamedh

Takes only expression (uses current environment):
```lisp
(EVAL expr)
```

---

## Quote Syntax

### Lisp 1.5

Only `(QUOTE x)` syntax.

### Lamedh

Both forms:
```lisp
(quote x)
'x        ; Reader macro
```

Plus quasiquote:
```lisp
`(a ,b c)
```

---

## Symbol Names

### Lisp 1.5

Strict rules:
- Start with letter
- Only letters and digits
- Uppercase only

### Lamedh

Relaxed rules:
- More special characters allowed: `-`, `*`, `+`, etc.
- Case insensitive (stored uppercase)

---

## T and NIL

### Lisp 1.5

- `*T*` for true
- `NIL` for false/empty list
- `F` sometimes used

### Lamedh

- `T` for true
- `NIL` for false/empty list
- `()` equivalent to `NIL`

---

## Missing Features

Features in Lisp 1.5 not in Lamedh:

| Feature | Notes |
|---------|-------|
| `TRACE`/`UNTRACE` execution hooks | The compatibility functions only mark symbol plists; the evaluator does not emit traces |
| `OBLIST` | Symbol table access |
| `REMOB` | Remove from symbol table |
| User-defined reader macros | Built-in quote, quasiquote/unquote, function shorthand, character, comment, and radix syntax is fixed |
| S-expression I/O | Punch cards, tapes |

---

## Semantic Differences

### CAR/CDR of NIL

**Lisp 1.5:** Undefined or error

**Lamedh:** Returns NIL:
```lisp
(car nil)  ; => NIL
(cdr nil)  ; => NIL
```

### RPLACA/RPLACD

**Lisp 1.5:** Mutates the cons cell

**Lamedh:** Returns new cons (non-destructive):
```lisp
(def x '(a . b))
(rplaca x 'c)  ; => (C . B)
x              ; => (A . B) still
```

---

## Compatibility Mode

For closer Lisp 1.5 compatibility:

```lisp
;; Define traditional names
(def get getp)
(def put putp)

;; But note: scoping is still lexical
```

---

## References

- McCarthy, J. et al. (1962). *LISP 1.5 Programmer's Manual*
- [MIT AI Memo 57](http://www.softwarepreservation.org/projects/LISP/book/LISP%201.5%20Programmers%20Manual.pdf)

---

**See Also:**
- [Known Limitations](appendix_limitations.md)
- [Special Forms](special_forms.md)
- [Standard Library](standard_library.md)
