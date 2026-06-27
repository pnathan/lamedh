# Appendix C: Known Limitations

This appendix documents known limitations, edge cases, and areas for caution in Lamedh.

---

## Numeric Limitations

### Integer Range

Integers are 64-bit signed:
- Minimum: -9,223,372,036,854,775,808
- Maximum: 9,223,372,036,854,775,807

Arithmetic is bounded. Some integer operations wrap and set the `OVERFLOW`
condition flag; others signal an overflow error. Do not treat Lamedh integers as
arbitrary precision bignums.

```lisp
(+ 9223372036854775807 1)
(flag-set-p 'overflow)  ; Check after arithmetic that may overflow
```

### Float Precision

Floats are 64-bit IEEE 754:
- Approximately 15-17 significant decimal digits
- Subject to rounding errors

```lisp
(= (+ 0.1 0.2) 0.3)  ; May be NIL due to floating point
```

### Special Float Values

- NaN (Not a Number) is canonicalized for hash keys, but numeric comparisons
  still follow IEEE 754 behavior
- `-0.0` and `0.0` compare equal and hash together as `LispVal` values
- Floating-point rounding still makes floats a poor choice for semantic keys

---

## String Limitations

### Limited Operations

Strings support escapes (`\n`, `\t`, `\r`, `\\`, `\"`, `\0`), substring,
length, character-code conversion, and value-to-string rendering. Missing
string operations include:

- String search
- Case conversion
- Regular expressions
- Locale-aware collation and Unicode normalization
- A full Common Lisp string comparison family

---

## Control Flow Limitations

### Tail Calls Are Recognized, Not Universal

The evaluator trampolines known tail positions (`IF`, `COND`, `PROGN`, `LET`,
`LET*`, function bodies, and related paths). Non-tail recursion still consumes
Rust stack frames and is protected by a recursion-depth guard.

```lisp
(defun count-down-tail (n)
  (if (zerop n)
      0
      (count-down-tail (- n 1))))  ; Tail-recursive

(defun count-down-nontail (n)
  (if (zerop n)
      0
      (+ 1 (count-down-nontail (- n 1)))))  ; Not tail-recursive
```

### PROG is Limited

PROG provides basic imperative control, but:
- No nested RETURN to outer PROG
- No exception handling within GO
- Labels must be unique

---

## File I/O Limitations

### Capability-Gated, Not Stream-Based

Lamedh has capability-gated file reading, writing, metadata queries, directory
listing, file mutation, and temporary-file helpers. It does not yet provide a
Common Lisp-style stream system, `WITH-OPEN-FILE`, append/update modes, or true
binary byte-vector I/O. Text APIs assume UTF-8 or lossy UTF-8 conversion.

---

## Data Structure Limitations

### No Circular Structures

Creating circular lists may cause infinite loops:

```lisp
;; If RPLACA/RPLACD mutated in place:
;; (def x (cons 'a nil))
;; (rplacd x x)  ; Circular!
;; (length x)    ; Would hang
```

Currently RPLACA/RPLACD create new cells, avoiding this.

### Hash Table Keys

Problematic key types:
- Floats (rounding and NaN semantics can surprise, even though `Eq`/`Hash` are
  internally consistent)
- Complex structures

Recommended keys:
- Symbols
- Strings
- Integers

---

## Macro Limitations

### No Hygiene

Macros can capture variables:

```lisp
(defmacro bad-twice (x)
  `(let ((temp ,x))
     (+ temp temp)))

(let ((temp 5))
  (bad-twice (+ temp 1)))  ; Captures outer 'temp'
```

Use GENSYM to avoid:

```lisp
(defmacro good-twice (x)
  (let ((g (gensym)))
    `(let ((,g ,x))
       (+ ,g ,g))))
```

### Limited Expansion

- No MACROEXPAND-ALL (only one level)
- No compiler macros
- No symbol macros

---

## Error Handling Limitations

### Basic Mechanism

- No stack traces
- No restarts
- No full Common Lisp condition system

### ERRORSET is Binary

Only returns success/failure, not error details:

```lisp
(errorset '(/ 1 0))    ; => NIL (but why?)
(errorset '(car 42))   ; => NIL (same result)
```

---

## Environment Limitations

### Global Symbol Table

All symbols are interned globally:
- No packages/namespaces
- Name conflicts possible across files
- Cannot have local symbol tables

### SETQ Creates Variables

Unlike some Lisps, SETQ creates variables if undefined:

```lisp
(setq new-var 42)  ; Creates rather than errors
```

---

## Performance Considerations

### Optimization Is Partial

Lamedh has a source optimizer, tail-call trampolining, and an experimental typed
JIT/type-checking path for monomorphic typed islands. The ordinary dynamic
language remains a boxed tree-walking interpreter.

Missing or incomplete performance work includes:

- No whole-program compiler
- No inline cache for ordinary dynamic calls
- No generational garbage collector
- Limited optimizer coverage outside simple source-level rewrites

### Interpretation Overhead

Each evaluation involves:
- Environment lookup
- Function dispatch
- Value boxing

Expect ordinary dynamic code to be much slower than compiled native code.

### Memory

- No generational GC
- Large lists copied fully by many operations
- Hash tables may not shrink

---

## Compatibility Notes

### Differences from Common Lisp

| Feature | Common Lisp | Lamedh |
|---------|-------------|--------|
| Multiple return values | Yes | No |
| Complex numbers | Yes | No |
| Rationals | Yes | No |
| Packages | Yes | No |
| CLOS | Yes | No |
| FORMAT | Yes | Partial subset |
| Conditions/Restarts | Yes | No |

### Differences from Lisp 1.5

| Feature | Lisp 1.5 | Lamedh |
|---------|----------|--------|
| Lexical scope | No (dynamic) | Yes |
| Strings | No | Yes |
| Floats | Limited | Yes |
| Hash tables | No | Yes |
| Macros | FEXPR only | Both |

---

## Workarounds

### Deep Recursion

Convert to iterative with PROG:

```lisp
(defun count-down-iter (n)
  (prog ()
   loop
    (if (zerop n) (return 0))
    (setq n (- n 1))
    (go loop)))
```

### String Comparison

Intern and use EQ:

```lisp
(defun string-equal (s1 s2)
  (eq (intern s1) (intern s2)))
```

### Checking for Errors

Wrap in ERRORSET:

```lisp
(if (errorset '(risky-operation))
    (princ "Success")
    (princ "Failed"))
```

---

## Future Improvements

Potential additions:
- [ ] Stack traces
- [ ] Circular structure detection
- [ ] Packages/namespaces
- [ ] Stream I/O
- [ ] Full condition/restart system
- [ ] Broader string search/case operations
- [ ] Hardened native-code/JIT release path

---

**See Also:**
- [Differences from Lisp 1.5](appendix_differences.md)
- [Error Handling](functions/errors.md)
