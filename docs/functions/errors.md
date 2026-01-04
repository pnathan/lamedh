# Error Handling

This chapter documents error handling mechanisms in Lamedh.

---

## Overview

Lamedh provides simple error handling:

- `ERROR` - Raise an error
- `ERRORSET` - Catch errors from an expression

```lisp
;; Raise an error
(error "Something went wrong")

;; Catch potential errors
(errorset '(/ 10 0))    ; => NIL (error caught)
(errorset '(/ 10 2))    ; => (5) (success)
```

---

## ERROR

**Syntax:** `(error message)`

Raises an error with the given message.

```lisp
(error "Invalid input")
;; Error: Invalid input

(error (concat "Cannot process: " x))
```

**Arguments:**
- `message` - Any value (typically a string)

**Returns:** Never returns; signals an error

**Example - Validation:**
```lisp
(defun divide (a b)
  (if (zerop b)
      (error "Division by zero")
      (/ a b)))

(divide 10 2)    ; => 5
(divide 10 0)    ; Error: Division by zero
```

---

## ERRORSET

**Syntax:** `(errorset form)`

Evaluates a form, catching any errors. Returns a list containing the result on success, or NIL on error.

```lisp
(errorset '(+ 1 2))         ; => (3)
(errorset '(/ 1 0))         ; => NIL
(errorset '(error "oops"))  ; => NIL
(errorset '(car nil))       ; => (NIL)
```

**Arguments:**
- `form` - A quoted expression to evaluate

**Returns:**
- `(result)` - Single-element list on success
- `NIL` - On any error

**Note:** The form must be quoted because ERRORSET needs the unevaluated expression.

---

## Error Handling Patterns

### Safe Function Calls

```lisp
(defun safe-divide (a b)
  "Divide A by B, returning NIL on error."
  (let ((result (errorset (list '/ a b))))
    (if result
        (car result)
        nil)))

(safe-divide 10 2)    ; => 5
(safe-divide 10 0)    ; => NIL
```

### Default on Error

```lisp
(defun get-or-default (expr default)
  "Evaluate EXPR; return DEFAULT if error."
  (let ((result (errorset expr)))
    (if result
        (car result)
        default)))

(get-or-default '(/ 10 2) 0)    ; => 5
(get-or-default '(/ 10 0) 0)    ; => 0
```

### Error Recovery

```lisp
(defun try-operations (ops)
  "Try each operation; return first successful result."
  (if (null ops)
      (error "All operations failed")
      (let ((result (errorset (car ops))))
        (if result
            (car result)
            (try-operations (cdr ops))))))

(try-operations '((/ 1 0) (/ 2 0) (/ 6 2)))
; => 3 (third operation succeeds)
```

### Conditional Error

```lisp
(defun require-positive (n)
  "Error if N is not positive."
  (if (not (plusp n))
      (error "Expected positive number")
      n))

(require-positive 5)     ; => 5
(require-positive -3)    ; Error: Expected positive number
```

---

## Common Error Conditions

Lamedh raises errors for:

| Condition | Example |
|-----------|---------|
| Division by zero | `(/ 1 0)` |
| Type mismatch | `(+ 1 "a")` |
| Unbound variable | `undefined-var` |
| Arity error | `(car 1 2 3)` |
| File not found | `(load-file "missing.lisp")` |
| Invalid argument | `(nth -1 '(a b))` |

---

## Debugging Techniques

### Print Before Error

```lisp
(defun debug-divide (a b)
  (princ "Dividing ")
  (prin1 a)
  (princ " by ")
  (prin1 b)
  (terpri)
  (/ a b))
```

### Trace Results

```lisp
(defun trace-result (label expr)
  "Evaluate EXPR, print LABEL and result."
  (let ((result (errorset expr)))
    (princ label)
    (princ ": ")
    (if result
        (prin1 (car result))
        (princ "ERROR"))
    (terpri)
    (if result (car result) nil)))
```

### Assert

```lisp
(defun assert (condition message)
  "Error if CONDITION is false."
  (if (not condition)
      (error message)
      t))

(assert (> x 0) "x must be positive")
```

---

## Limitations

Lamedh's error handling is basic:

**Not Available:**
- Exception types/classes
- Stack traces
- CATCH/THROW
- UNWIND-PROTECT
- Restarts/conditions
- Error inheritance

**Available:**
- Raise errors with message
- Catch any error (no discrimination)
- Check success/failure

---

## Examples

### Safe File Loading

```lisp
(defun try-load (filename)
  "Try to load file; print message on failure."
  (if (errorset (list 'load-file filename))
      (progn
        (princ "Loaded: ")
        (princ filename)
        (terpri)
        t)
      (progn
        (princ "Failed: ")
        (princ filename)
        (terpri)
        nil)))
```

### Retry Logic

```lisp
(defun retry (n form)
  "Try FORM up to N times."
  (if (zerop n)
      (error "Retry limit exceeded")
      (let ((result (errorset form)))
        (if result
            (car result)
            (retry (- n 1) form)))))
```

### Validation Chain

```lisp
(defun validate-input (x)
  (cond ((not (numberp x))
         (error "Must be a number"))
        ((minusp x)
         (error "Must be non-negative"))
        ((> x 100)
         (error "Must be <= 100"))
        (t x)))

(validate-input 50)     ; => 50
(validate-input "hi")   ; Error: Must be a number
(validate-input -5)     ; Error: Must be non-negative
```

---

**See Also:**
- [I/O Functions](io.md) - File loading errors
- [Special Forms](../special_forms.md) - Control flow
