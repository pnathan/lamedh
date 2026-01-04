# Appendix C: Known Limitations

This appendix documents known limitations, edge cases, and areas for caution in Lamedh.

---

## Numeric Limitations

### Integer Range

Integers are 64-bit signed:
- Minimum: -9,223,372,036,854,775,808
- Maximum: 9,223,372,036,854,775,807

Overflow produces errors (or wraps in release builds):

```lisp
(+ 9223372036854775807 1)
;; Error or unexpected result
```

### Float Precision

Floats are 64-bit IEEE 754:
- Approximately 15-17 significant decimal digits
- Subject to rounding errors

```lisp
(= (+ 0.1 0.2) 0.3)  ; May be NIL due to floating point
```

### Special Float Values

- NaN (Not a Number) comparisons behave unexpectedly
- `-0.0` and `0.0` compare equal but may hash differently
- Avoid floats as hash table keys

---

## String Limitations

### No Escape Sequences

Strings cannot contain:
- Embedded quotes (`"`)
- Newlines
- Tab characters
- Other escape sequences

```lisp
"hello \"world\""   ; Does not work
```

### Limited Operations

Missing string operations:
- Substring extraction
- String search
- Case conversion
- Regular expressions
- String comparison (use `EQ` on interned symbols)

---

## Control Flow Limitations

### No Tail Call Optimization

Deep recursion will cause stack overflow:

```lisp
(defun count-down (n)
  (if (zerop n)
      0
      (count-down (- n 1))))

(count-down 100000)  ; Stack overflow
```

### PROG is Limited

PROG provides basic imperative control, but:
- No nested RETURN to outer PROG
- No CATCH/THROW
- No exception handling within GO
- Labels must be unique

---

## File I/O Limitations

### Read-Only

No file writing capabilities:
- No WRITE-FILE
- No WITH-OPEN-FILE
- No stream manipulation

### No File Queries

Cannot check:
- File existence
- File size
- File permissions
- Directory contents

### Text Only

- No binary file support
- Assumes UTF-8 encoding

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
- Floats (equality/hashing issues)
- Lists (compared by identity, not value)
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

- No exception types
- No stack traces
- No restarts
- No condition system

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

### No Optimization

- No compiler
- No inline expansion
- No constant folding
- No tail call elimination

### Interpretation Overhead

Each evaluation involves:
- Environment lookup
- Function dispatch
- Value boxing

Expect 100-1000x slower than compiled code.

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
| FORMAT | Yes | No |
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
- [ ] Tail call optimization
- [ ] String escape sequences
- [ ] File writing
- [ ] Stack traces
- [ ] Exception types
- [ ] Circular structure detection

---

**See Also:**
- [Differences from Lisp 1.5](appendix_differences.md)
- [Error Handling](functions/errors.md)
