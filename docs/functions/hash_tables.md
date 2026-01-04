# Hash Table Functions

This chapter documents hash table (dictionary) operations in Lamedh.

---

## Overview

Hash tables provide O(1) average-time key-value storage. They are mutable data structures.

```lisp
(def ht (make-hash-table))
(set-bang ht "name" "Alice")
(set-bang ht "age" 30)
(get ht "name")     ; => "Alice"
```

---

## Creation

### MAKE-HASH-TABLE

**Syntax:** `(make-hash-table)`

Creates a new empty hash table.

```lisp
(def my-table (make-hash-table))
```

**Returns:** New hash table

---

## Access

### GET

**Syntax:** `(get hash-table key)`

Retrieves the value associated with key.

```lisp
(def ht (make-hash-table))
(set-bang ht "x" 42)
(get ht "x")        ; => 42
(get ht "y")        ; => NIL (not found)
```

**Arguments:**
- `hash-table` - A hash table
- `key` - Any value (symbols and strings recommended)

**Returns:** Value for key, or NIL if not found

---

### SET-BANG

**Syntax:** `(set-bang hash-table key value)`

Sets the value for a key in the hash table.

```lisp
(def ht (make-hash-table))
(set-bang ht "name" "Bob")
(set-bang ht "name" "Carol")  ; Overwrites
(get ht "name")    ; => "Carol"
```

**Arguments:**
- `hash-table` - A hash table
- `key` - Any value
- `value` - Any value

**Returns:** The value

**Note:** Creates a new entry or overwrites existing.

---

### DELETE-KEY

**Syntax:** `(delete-key hash-table key)`

Removes a key-value pair from the hash table.

```lisp
(def ht (make-hash-table))
(set-bang ht "x" 1)
(delete-key ht "x")
(get ht "x")        ; => NIL
```

**Arguments:**
- `hash-table` - A hash table
- `key` - Key to remove

**Returns:** The removed value, or NIL if not found

---

### KEYS

**Syntax:** `(keys hash-table)`

Returns a list of all keys in the hash table.

```lisp
(def ht (make-hash-table))
(set-bang ht "a" 1)
(set-bang ht "b" 2)
(set-bang ht "c" 3)
(keys ht)           ; => ("a" "b" "c") (order may vary)
```

**Returns:** List of keys

**Note:** Order of keys is not guaranteed.

---

## Key Considerations

### Recommended Key Types

**Best:**
- Symbols (`'foo`)
- Strings (`"foo"`)
- Integers (`42`)

**Avoid:**
- Floats (equality issues with -0.0, NaN)
- Lists (compared by identity, not value)

### Symbol vs String Keys

```lisp
(def ht (make-hash-table))
(set-bang ht 'name "with symbol")
(set-bang ht "name" "with string")

(get ht 'name)      ; => "with symbol"
(get ht "name")     ; => "with string"
; These are different keys!
```

---

## Examples

### Simple Dictionary

```lisp
(def person (make-hash-table))
(set-bang person "name" "Alice")
(set-bang person "age" 30)
(set-bang person "city" "Boston")

(get person "name")  ; => "Alice"
(get person "age")   ; => 30
```

### Counter

```lisp
(defun make-counter-table ()
  "Create a hash table for counting."
  (make-hash-table))

(defun increment (table key)
  "Increment counter for KEY in TABLE."
  (let ((current (get table key)))
    (set-bang table key
              (if current (+ current 1) 1))))

(def counts (make-counter-table))
(increment counts 'a)
(increment counts 'a)
(increment counts 'b)
(get counts 'a)     ; => 2
(get counts 'b)     ; => 1
```

### Safe Get with Default

```lisp
(defun get-or-default (table key default)
  "Get KEY from TABLE, or return DEFAULT if not found."
  (let ((value (get table key)))
    (if value value default)))

(get-or-default person "email" "not provided")
; => "not provided"
```

### Iterate Over Table

```lisp
(defun print-table (table)
  "Print all key-value pairs in TABLE."
  (mapcar (keys table)
          (lambda (key)
            (princ key)
            (princ ": ")
            (prin1 (get table key))
            (terpri))))

(print-table person)
; name: "Alice"
; age: 30
; city: "Boston"
```

### Check if Key Exists

```lisp
(defun has-key (table key)
  "Return T if TABLE contains KEY."
  (member key (keys table)))

(has-key person "name")  ; => ("name" ...) - truthy
(has-key person "email") ; => NIL
```

---

## Implementation Notes

- Hash tables are implemented using Rust's `HashMap`
- Keys are hashed using Rust's default hasher
- Tables are mutable (unlike most Lisp values in Lamedh)
- Equality uses the same semantics as `EQ` for most types

---

## Comparison with Association Lists

| Aspect | Hash Table | Association List |
|--------|------------|------------------|
| Lookup | O(1) average | O(n) |
| Memory | More overhead | Minimal |
| Immutability | Mutable | Immutable |
| Syntax | Functions | Regular lists |
| Ordered | No | Yes (insertion order) |

**Use hash tables when:**
- Many lookups expected
- Large number of entries
- Need fast access

**Use alists when:**
- Few entries
- Immutability important
- Need ordered iteration

---

## Environment Access

### CURRENT-ENVIRONMENT

**Syntax:** `(current-environment)`

Returns a hash table containing current variable bindings.

```lisp
(let ((x 1) (y 2))
  (current-environment))
; => Hash table with X=1, Y=2, plus inherited bindings
```

**Returns:** Hash table of symbol-to-value mappings

---

**See Also:**
- [Data Types](../data_types.md)
- [Property Lists](plists.md) (alternative for symbol metadata)
