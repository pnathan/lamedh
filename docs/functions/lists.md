# List Functions

This chapter documents all list and cons cell operations in Lamedh.

---

## Basic List Operations

### CAR

**Syntax:** `(car list)`

Returns the first element of a list (the car of a cons cell).

```lisp
(car '(a b c))      ; => A
(car '((1 2) 3))    ; => (1 2)
(car nil)           ; => NIL
```

**Arguments:**
- `list` - A cons cell or NIL

**Returns:** The car of the cons, or NIL for empty list

**Errors:** If argument is not a cons or NIL

---

### CDR

**Syntax:** `(cdr list)`

Returns the rest of a list (the cdr of a cons cell).

```lisp
(cdr '(a b c))      ; => (B C)
(cdr '(a))          ; => NIL
(cdr nil)           ; => NIL
```

**Arguments:**
- `list` - A cons cell or NIL

**Returns:** The cdr of the cons, or NIL for empty list

---

### CONS

**Syntax:** `(cons car cdr)`

Creates a new cons cell with the given car and cdr.

```lisp
(cons 'a '(b c))    ; => (A B C)
(cons 'a 'b)        ; => (A . B)
(cons 1 nil)        ; => (1)
(cons 1 (cons 2 (cons 3 nil)))  ; => (1 2 3)
```

**Arguments:**
- `car` - Any value
- `cdr` - Any value

**Returns:** New cons cell

---

### LIST

**Syntax:** `(list item...)`

Creates a list from its arguments.

```lisp
(list)              ; => NIL
(list 1)            ; => (1)
(list 1 2 3)        ; => (1 2 3)
(list 'a '(b c))    ; => (A (B C))
```

**Arguments:**
- `item...` - Zero or more values

**Returns:** A proper list of the arguments

---

## CAR/CDR Compositions

These functions are compositions of CAR and CDR. They are defined in the standard library.

### Second Element

```lisp
(cadr '(a b c))     ; => B  (car (cdr x))
```

### Third Element

```lisp
(caddr '(a b c d))  ; => C  (car (cdr (cdr x)))
```

### Fourth Element

```lisp
(cadddr '(a b c d)) ; => D
```

### Complete List

| Function | Expansion | Example Result |
|----------|-----------|----------------|
| `CAAR` | `(car (car x))` | First of first |
| `CADR` | `(car (cdr x))` | Second element |
| `CDAR` | `(cdr (car x))` | Rest of first |
| `CDDR` | `(cdr (cdr x))` | After second |
| `CAAAR` | `(car (car (car x)))` | |
| `CAADR` | `(car (car (cdr x)))` | |
| `CADAR` | `(car (cdr (car x)))` | |
| `CADDR` | `(car (cdr (cdr x)))` | Third element |
| `CDAAR` | `(cdr (car (car x)))` | |
| `CDADR` | `(cdr (car (cdr x)))` | |
| `CDDAR` | `(cdr (cdr (car x)))` | |
| `CDDDR` | `(cdr (cdr (cdr x)))` | After third |

All 4-level compositions (CAAAAR through CDDDDR) are also available.

---

## List Access

### NTH

**Syntax:** `(nth n list)`

Returns the nth element of a list (0-indexed).

```lisp
(nth 0 '(a b c))    ; => A
(nth 1 '(a b c))    ; => B
(nth 2 '(a b c))    ; => C
(nth 5 '(a b c))    ; => NIL
```

**Arguments:**
- `n` - Non-negative integer index
- `list` - A list

**Returns:** Element at position n, or NIL if out of bounds

---

### NTHCDR

**Syntax:** `(nthcdr n list)`

Returns the list after n CDR operations.

```lisp
(nthcdr 0 '(a b c)) ; => (A B C)
(nthcdr 1 '(a b c)) ; => (B C)
(nthcdr 2 '(a b c)) ; => (C)
(nthcdr 3 '(a b c)) ; => NIL
```

---

### LAST

**Syntax:** `(last list)`

Returns the last cons cell of a list.

```lisp
(last '(a b c))     ; => (C)
(last '(a))         ; => (A)
(last nil)          ; => NIL
```

**Returns:** Last cons cell, not last element

---

## List Construction

### APPEND

**Syntax:** `(append list1 list2)`

Concatenates two lists. (Standard library function)

```lisp
(append '(a b) '(c d))    ; => (A B C D)
(append nil '(a b))       ; => (A B)
(append '(a b) nil)       ; => (A B)
```

**Note:** Creates new cons cells for list1; shares structure with list2.

---

### REVERSE

**Syntax:** `(reverse list)`

Returns a list with elements in reverse order. (Standard library function)

```lisp
(reverse '(a b c))  ; => (C B A)
(reverse nil)       ; => NIL
(reverse '(1))      ; => (1)
```

---

### PAIRLIS

**Syntax:** `(pairlis keys values)`

Creates an association list from two lists. (Standard library function)

```lisp
(pairlis '(a b c) '(1 2 3))
; => ((A . 1) (B . 2) (C . 3))
```

---

## List Searching

### MEMBER

**Syntax:** `(member item list)`

Searches for item in list using EQUAL. Returns the tail starting at the match. (Standard library function)

```lisp
(member 'b '(a b c))     ; => (B C)
(member 'd '(a b c))     ; => NIL
(member '(1 2) '((1 2) (3 4)))  ; => ((1 2) (3 4))
```

---

### ASSOC

**Syntax:** `(assoc key alist)`

Searches an association list for a pair with matching key.

```lisp
(assoc 'b '((a . 1) (b . 2) (c . 3)))
; => (B . 2)

(assoc 'x '((a . 1) (b . 2)))
; => NIL
```

**Arguments:**
- `key` - Key to search for
- `alist` - Association list (list of cons cells)

**Returns:** First pair with matching car, or NIL

---

## List Length

### LENGTH

**Syntax:** `(length list)`

Returns the number of elements in a list. (Standard library function)

```lisp
(length '(a b c))   ; => 3
(length nil)        ; => 0
(length '((1 2) 3)) ; => 2
```

---

### NULL

**Syntax:** `(null x)`

Returns T if x is NIL (the empty list). (Standard library function)

```lisp
(null nil)          ; => T
(null '())          ; => T
(null '(a))         ; => NIL
```

---

## Higher-Order List Functions

### MAPCAR

**Syntax:** `(mapcar list function)`

Applies function to each element, returns list of results.

```lisp
(mapcar '(1 2 3) (lambda (x) (* x 2)))
; => (2 4 6)

(mapcar '(a b c) (lambda (x) (list x x)))
; => ((A A) (B B) (C C))
```

**Note:** Argument order is (list function), not (function list).

---

### MAPLIST

**Syntax:** `(maplist list function)`

Applies function to successive tails of list.

```lisp
(maplist '(a b c) (lambda (x) (length x)))
; => (3 2 1)

(maplist '(1 2 3) (lambda (x) x))
; => ((1 2 3) (2 3) (3))
```

---

## Substitution

### SUBST

**Syntax:** `(subst new old tree)`

Replaces all occurrences of old with new in tree.

```lisp
(subst 'x 'a '(a b a c))
; => (X B X C)

(subst 'x 'a '(a (b a) c))
; => (X (B X) C)
```

Uses EQ for comparison.

---

## Destructive Operations

### RPLACA

**Syntax:** `(rplaca cons new-car)`

Returns a new cons with car replaced. (Non-mutating in Lamedh)

```lisp
(rplaca '(a . b) 'x)   ; => (X . B)
```

**Note:** Unlike traditional Lisp, this creates a new cons cell.

---

### RPLACD

**Syntax:** `(rplacd cons new-cdr)`

Returns a new cons with cdr replaced. (Non-mutating in Lamedh)

```lisp
(rplacd '(a . b) 'x)   ; => (A . X)
(rplacd '(a b c) '(x y)) ; => (A X Y)
```

---

### EFFACE (DELETE)

**Syntax:** `(efface item list)` or `(delete item list)`

Returns list with first occurrence of item removed.

```lisp
(efface 'b '(a b c b))  ; => (A C B)
(delete 'x '(a b c))    ; => (A B C)
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
(listp 'a)          ; => NIL
```

---

## Examples

### Building a List

```lisp
(defun iota (n)
  "Return list (0 1 2 ... n-1)"
  (if (= n 0)
      nil
      (append (iota (- n 1)) (list (- n 1)))))

(iota 5)  ; => (0 1 2 3 4)
```

### Filtering a List

```lisp
(defun filter (pred lst)
  "Return elements of lst satisfying pred."
  (cond ((null lst) nil)
        ((funcall pred (car lst))
         (cons (car lst) (filter pred (cdr lst))))
        (t (filter pred (cdr lst)))))

(filter (lambda (x) (> x 2)) '(1 2 3 4 5))
; => (3 4 5)
```

### Folding a List

```lisp
(defun reduce (fn init lst)
  "Reduce lst using fn, starting with init."
  (if (null lst)
      init
      (reduce fn (funcall fn init (car lst)) (cdr lst))))

(reduce (lambda (a b) (+ a b)) 0 '(1 2 3 4 5))
; => 15
```

---

**See Also:**
- [Data Types - Cons Cells](../data_types.md#35-cons-cells-and-lists)
- [Predicates](predicates.md)
