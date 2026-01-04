# Property List Functions

This chapter documents property list (plist) operations in Lamedh. Every symbol has an associated property list for storing metadata.

---

## Overview

Property lists provide a way to attach arbitrary key-value pairs to symbols:

```lisp
(putp 'my-symbol "color" "red")
(putp 'my-symbol "size" 42)
(getp 'my-symbol "color")   ; => "red"
```

Common uses:
- Documentation strings
- Type information
- Symbol metadata
- Custom attributes

---

## Basic Operations

### PUTP (PUT)

**Syntax:** `(putp symbol indicator value)` or `(put symbol indicator value)`

Sets a property on a symbol's property list.

```lisp
(putp 'foo "version" 1)
(putp 'foo "author" "Alice")

(put 'bar "type" 'function)  ; PUT is an alias
```

**Arguments:**
- `symbol` - The symbol to modify
- `indicator` - Property name (usually a string)
- `value` - Property value (any Lisp value)

**Returns:** The value

---

### GETP (GET)

**Syntax:** `(getp symbol indicator)`

Retrieves a property from a symbol's property list.

```lisp
(putp 'foo "color" "blue")
(getp 'foo "color")      ; => "blue"
(getp 'foo "missing")    ; => NIL
```

**Arguments:**
- `symbol` - The symbol to query
- `indicator` - Property name

**Returns:** Property value, or NIL if not found

---

### REMPROP

**Syntax:** `(remprop symbol indicator)`

Removes a property from a symbol's property list.

```lisp
(putp 'foo "temp" 123)
(remprop 'foo "temp")    ; => T
(getp 'foo "temp")       ; => NIL
(remprop 'foo "temp")    ; => NIL (already gone)
```

**Arguments:**
- `symbol` - The symbol to modify
- `indicator` - Property name to remove

**Returns:** T if property existed, NIL otherwise

---

### PLIST

**Syntax:** `(plist symbol)`

Returns the entire property list of a symbol.

```lisp
(putp 'foo "a" 1)
(putp 'foo "b" 2)
(plist 'foo)
; => ("a" 1 "b" 2) or similar
```

**Returns:** List of indicator-value pairs

---

## Batch Operations

### DEFLIST

**Syntax:** `(deflist pairs indicator)`

Sets the same property on multiple symbols.

```lisp
(deflist '((red 1)
           (green 2)
           (blue 3))
         "color-code")

(getp 'red "color-code")    ; => 1
(getp 'green "color-code")  ; => 2
(getp 'blue "color-code")   ; => 3
```

**Arguments:**
- `pairs` - List of (symbol value) pairs
- `indicator` - Property name to set on all symbols

**Returns:** NIL

---

## Documentation Strings

### Standard Docstring Property

The `"docstring"` property is used by `DEFUN` and `DEF` for documentation:

```lisp
(defun square (x)
  "Compute the square of X."
  (* x x))

(getp 'square "docstring")
; => "Compute the square of X."
```

### DOCUMENTATION

**Syntax:** `(documentation symbol)`

Retrieves the docstring for a symbol. (Standard library function)

```lisp
(defun add (a b)
  "Add two numbers."
  (+ a b))

(documentation 'add)  ; => "Add two numbers."
```

Equivalent to `(getp symbol "docstring")`.

---

## Examples

### Adding Metadata

```lisp
;; Mark functions with their category
(putp 'car "category" "list")
(putp 'cdr "category" "list")
(putp '+ "category" "arithmetic")
(putp '- "category" "arithmetic")

;; Query by category
(defun is-list-function (sym)
  (eq (getp sym "category") "list"))

(is-list-function 'car)  ; => T
(is-list-function '+)    ; => NIL
```

### Type Annotations

```lisp
(putp 'x "type" 'integer)
(putp 'name "type" 'string)

(defun get-type (sym)
  (or (getp sym "type") 'unknown))

(get-type 'x)      ; => INTEGER
(get-type 'y)      ; => UNKNOWN
```

### Version Information

```lisp
(defun set-version (sym major minor)
  "Set version info for SYM."
  (putp sym "version-major" major)
  (putp sym "version-minor" minor))

(defun get-version (sym)
  "Get version string for SYM."
  (let ((major (getp sym "version-major"))
        (minor (getp sym "version-minor")))
    (if (and major minor)
        (concat (prin1-to-string major) "." (prin1-to-string minor))
        "unknown")))

(set-version 'my-lib 1 5)
```

### Flag Properties

```lisp
;; Mark symbols as deprecated
(defun deprecate (sym message)
  (putp sym "deprecated" t)
  (putp sym "deprecation-message" message))

(defun check-deprecated (sym)
  (if (getp sym "deprecated")
      (progn
        (princ "Warning: ")
        (princ sym)
        (princ " is deprecated: ")
        (princ (getp sym "deprecation-message"))
        (terpri))))

(deprecate 'old-function "Use new-function instead")
(check-deprecated 'old-function)
; Warning: OLD-FUNCTION is deprecated: Use new-function instead
```

---

## Flag Operations

Lamedh provides convenience functions for boolean flags on symbols:

### SET-FLAG

**Syntax:** `(set-flag symbol flag-name)`

Sets a boolean flag to true.

```lisp
(set-flag 'my-sym "active")
```

### CLEAR-FLAG

**Syntax:** `(clear-flag symbol flag-name)`

Clears a boolean flag.

```lisp
(clear-flag 'my-sym "active")
```

### FLAG-SET-P

**Syntax:** `(flag-set-p symbol flag-name)`

Returns T if flag is set.

```lisp
(set-flag 'x "special")
(flag-set-p 'x "special")   ; => T
(flag-set-p 'x "other")     ; => NIL
```

### CLEAR-ALL-FLAGS

**Syntax:** `(clear-all-flags symbol)`

Removes all flags from a symbol.

---

## Property List vs Hash Table

| Aspect | Property List | Hash Table |
|--------|--------------|------------|
| Attached to | Symbols only | Standalone |
| Keys | Any value (usually strings) | Any value |
| Lookup | O(n) in plist size | O(1) average |
| Use case | Symbol metadata | General storage |

**Use plists for:**
- Documentation
- Symbol attributes
- Small amounts of metadata

**Use hash tables for:**
- Large data sets
- Non-symbol keys
- Frequent lookups

---

## Internal Representation

Property lists are stored as association lists:

```
Symbol: FOO
Plist: (("docstring" . "A foo") ("version" . 1) ("author" . "Bob"))
```

When accessed via PLIST, returned as flat list:
```lisp
(plist 'foo)
; => ("docstring" "A foo" "version" 1 "author" "Bob")
```

---

**See Also:**
- [Environments](../environments.md) - Symbol interning and scope
- [Hash Tables](hash_tables.md) - Alternative key-value storage
- [Data Types](../data_types.md#33-symbols) - Symbol structure
