# 3. Data Structures

Chapter 2 covered lists in depth — cons cells, `car`/`cdr`, `list`, and the
classic recursive style. Lists remain the universal structure in Lamedh: most
control flow, most stdlib helpers, and most of the reader's own output are
lists of symbols and lists. Everything else in this chapter — arrays, hash
tables, strings, property lists, sets, and records — is built either as a
Rust-level primitive with a thin Lisp wrapper, or entirely in Lisp on top of
lists and hash tables. Reach for a list first; reach for one of these when you
specifically need indexed access, key/value lookup, mutability, or a typed
aggregate.

## Arrays

Arrays are fixed-size, mutable, zero-indexed vectors. Create one with
`array`, which takes a single non-negative size and fills every slot with
`nil`:

```
(array 5)
; => <array:5>

(let ((a (array 3))) (fetch a 0))
; => ()
```

There is no array literal syntax in the reader — you always build an array
with `array` (optionally followed by `store` calls) or convert a list with
`list->array`.

Read and write slots with `fetch`/`store`, or their aliases `aref`/`aset`
(same argument order, `(aref array index)` / `(aset array index value)`):

```
(let ((a (array 3)))
  (store a 0 'x)
  (store a 1 'y)
  (store a 2 'z)
  (array->list a))
; => (X Y Z)

(let ((a (array 3)))
  (aset a 0 'hi)
  (aref a 0))
; => HI
```

`store` mutates the array in place and returns the stored value — this is
one of the few places in Lamedh where in-place mutation is allowed (cons
cells are immutable by design; array cells are not, see `lib/17-arrays.lisp`).

Out-of-bounds access is a runtime error rather than `nil` or a crash:

```
(let ((a (array 3))) (fetch a 5))
; => Error: fetch: index 5 out of bounds (length 3)

(let ((a (array 3))) (store a 5 'x))
; => Error: store: index 5 out of bounds (length 3)
```

`array-length` reports the size, and `arrayp` tests the type:

```
(array-length (array 4))
; => 4

(arrayp (array 2))
; => T
```

`lib/17-arrays.lisp` layers convenience functions on top of the four
primitives (`array` / `fetch` / `store` / `array-length`):

- `array->list` / `list->array` — convert both directions.
- `array-map` — return a new array with a function applied to every element.
- `array-fill` — set every slot to a value, mutating in place.
- `array-copy` — a fresh array with the same contents.
- `subarray` — a fresh array holding a sub-range.

Growable vectors (push/pop-style resizing) are intentionally out of scope;
arrays are fixed-size once created.

## Hash Tables

Hash tables map arbitrary keys (compared with `equal`) to values. Create one
with `make-hash-table`:

```
(make-hash-table)
; => <hash-table>
```

Write with `sethash` (an alias for the primitive `set-bang`) or read with
`gethash`. Both accept the hash table and key in either order — `(gethash
table key)` is the historical Lamedh order, `(gethash key table)` is CL
style, and the hash table is recognized by type either way:

```
(let ((h (make-hash-table)))
  (sethash h 'a 1)
  (gethash h 'a))
; => 1

(let ((h (make-hash-table)))
  (set-bang h 'a 1)
  (gethash 'a h))
; => 1
```

Looking up a missing key returns `nil` rather than signaling an error, so it
is indistinguishable from a key whose value really is `nil`. Use `has-key-p`
or `gethash-or` (from `lib/15-sets-hash.lisp`) when that distinction matters:

```
(gethash (make-hash-table) 'missing)
; => ()

(gethash-or (make-hash-table) 'z 'default)
; => DEFAULT
```

Remove a key with `delete-key-bang` or its CL alias `remhash` (also
either-order):

```
(let ((h (make-hash-table)))
  (set-bang h 'a 1)
  (remhash 'a h)
  (keys h))
; => ()
```

Other primitives and helpers:

- `keys` — a list of a table's keys, in insertion order.
- `hash-table-p` — type predicate.
- `hash-table-count` — number of entries.
- `maphash` — call `(fn key value)` for each entry; accepts `(maphash table
  fn)` or CL-style `(maphash fn table)`.
- `hash->alist` / `alist->hash` — convert to/from an association list.
- `clrhash` — remove every entry.

```
(let ((h (make-hash-table)))
  (set-bang h 'a 1)
  (set-bang h 'b 2)
  (hash->alist h))
; => ((B . 2) (A . 1))
```

## Strings

Strings are built-in atoms (not lists of characters). `lib/14-strings.lisp`
implements string operations on top of the Rust primitives `string-length`,
`substring`, `char-code`, `code-char`, `string->number`, `number->string`,
and `concat`.

```
(concat "foo" "bar")
; => "foobar"

(string-length "hello")
; => 5

(substring "hello world" 0 5)
; => "hello"
```

`substring` takes a start index (inclusive) and end index (exclusive), same
convention as `fetch`/`array` ranges elsewhere in the stdlib.

Number and string conversion:

```
(number->string 42)
; => "42"

(string->number "3.14")
; => 3.14

(parse-integer "42")
; => 42
```

`parse-integer` rejects strings with a fractional part (like `"3.14"`) and
returns `nil` rather than a float.

Splitting, joining, and searching:

```
(string-split "a,b,c" ",")
; => ("a" "b" "c")

(string-join (list "a" "b" "c") "-")
; => "a-b-c"

(starts-with-p "hello" "he")
; => T

(string-trim "  hi  ")
; => "hi"
```

Also available: `ends-with-p`, `contains-p`, `string-index-of`,
`string-replace`, `string-lessp` (code-point order), `string=` (an alias for
`equal`), and `reverse`/`subseq`/`elt` (from the CL-compat layer, below —
these work on strings as well as lists).

Case conversion and character classification accept either a one-character
string or an integer code point:

```
(string-upcase "hi there")
; => "HI THERE"

(digit-p "7")
; => T
```

`char-upcase`/`char-downcase` always return a one-character string.

Two printing primitives sit under `format` (next):

```
(princ-to-string 42)
; => "42"

(prin1-to-string "hi")
; => "\"hi\""
```

`princ-to-string` renders "for humans" (no quoting); `prin1-to-string`
renders "for the reader" (quoted, escaped — output that can be read back).

### format

`lib/18-format.lisp` implements a useful subset of Common Lisp's `format`.
The first argument is the destination: `nil` returns the formatted string,
`t` prints it (and returns `nil`).

```
(format nil "~a and ~s~%" 42 "str")
; => "42 and \"str\"\n"

(format t "hi ~a~%" 'world)
; => prints "hi WORLD\n", returns ()
```

Supported directives:

| Directive | Meaning |
|---|---|
| `~a` / `~A` | human (`princ`) rendering of the next argument |
| `~s` / `~S` | readable (`prin1`) rendering of the next argument |
| `~d` / `~D` | the next argument as a decimal datum (currently same as `~a`) |
| `~%` | newline |
| `~~` | a literal tilde |

Unrecognized directives pass through literally rather than erroring, so a
typo in a format string degrades gracefully instead of crashing.

## Property Lists on Symbols

Every symbol carries its own property list (Lisp 1.5 style), independent of
its value cell. Set a property with `putp`, read it with `getp`, and remove
it with `remprop`. All three take the symbol first, then the property name:

```
(putp 'foo 'color 'red)
; => T

(progn (putp 'foo 'color 'red) (getp 'foo 'color))
; => RED
```

`plist` returns the whole property list for a symbol as an alist-like list
of `(property value)` pairs (property names print as strings internally):

```
(progn (putp 'foo 'color 'red) (plist 'foo))
; => ("COLOR" RED)
```

`GET`/`PUT` are the classic Lisp 1.5 names and are bound as aliases for
`getp`/`putp`. `remprop` removes a property and returns `t` if one was
actually removed.

## Sets

Lamedh has no dedicated set type; `lib/15-sets-hash.lisp` treats an ordinary
list as a set (membership by `equal`) and provides the standard operations,
all non-mutating (they return new lists):

```
(union (list 1 2 3) (list 3 4 5))
; => (1 2 3 4 5)

(intersection (list 1 2 3) (list 2 3 4))
; => (2 3)

(set-difference (list 1 2 3) (list 2))
; => (1 3)

(adjoin 2 (list 1 2 3))
; => (1 2 3)

(adjoin 9 (list 1 2 3))
; => (9 1 2 3)
```

`adjoin` only conses the item on if an `equal` element isn't already present
— it's the building block for de-duplicated accumulation. The same file also
has association-list helpers (`assoc` is a core builtin; `rassoc`,
`alist-get`, `alist-put` live here) for when you want ordered key/value pairs
instead of a hash table.

## Records

For structured, typed aggregate data — the "give me a `point` with named,
typed fields" case — lists, arrays, and hash tables are the wrong tool.
Lamedh's answer is `defrecord`, a gradually-typed record form that generates
a constructor, a type predicate, and per-field accessors from one
declaration:

```
(progn
  (defrecord point (x int64) (y int64))
  (let ((p (make-point 3 4)))
    (list (point-x p) (point-y p))))
; => (3 4)
```

`defrecord` is the primary aggregate type in Lamedh — nominal, row-typed,
and usable from both untyped and type-checked code. Its full treatment,
including invariants, derivations, functional update, and how it interacts
with the type checker, is Chapter 4.

## CL Compatibility Layer

`lib/21-cl-compat.lisp` closes the "reach for Common Lisp syntax by reflex"
gap: `setf` and friends, expressed over the primitives already covered in
this chapter.

### setf places

`setf` recognizes a fixed set of place forms and expands each to the
underlying primitive call:

| Place | Expands to |
|---|---|
| `(setf sym v)` | `(setq sym v)` |
| `(setf (gethash table k) v)` | a hash write (either `gethash` argument order) |
| `(setf (fetch a i) v)` / `(setf (aref a i) v)` | `(store a i v)` |
| `(setf (elt seq i) v)` | array/hash-aware store |
| `(setf (name obj) v)` | `(set-name! obj v)` — a user-defined mutator, by convention |

```
(let ((x 1)) (setf x 10) x)
; => 10

(let ((h (make-hash-table))) (setf (gethash h 'a) 5) (gethash h 'a))
; => 5

(let ((a (array 3))) (setf (aref a 0) 'z) (aref a 0))
; => Z
```

The accessor-convention row is a naming pattern, not a built-in mutator
generator: `(setf (thing obj) v)` looks for a function literally named
`SET-THING!`. `defrecord` does not generate one — records are updated
functionally with `record-with` (Chapter 4) — so this place only fires for
your own hand-written `set-x!` functions.

Because place subforms are evaluated once per *mention* rather than cached,
`push`/`pop`/`incf`/`decf` (which mention the place twice) require the place
expression to be side-effect-free. `car`/`cdr` are deliberately not places:
cons cells are immutable in Lamedh, so there is no `(setf (car x) v)`.

### push / pop / incf / decf

```
(let ((lst (list 1 2 3))) (push 0 lst) lst)
; => (0 1 2 3)

(let ((lst (list 1 2 3))) (pop lst) lst)
; => (2 3)

(let ((n 5)) (incf n) n)
; => 6

(let ((n 5)) (incf n 3) n)
; => 8

(let ((n 5)) (decf n 2) n)
; => 3
```

Since Lamedh lists are immutable, `push`/`pop` work by rebinding the place
(via `setf`), not by mutating shared structure — same caveat as `nreverse`
below.

### Sequence staples

`subseq` and `elt` work uniformly across lists, strings, and (for `elt`)
arrays:

```
(subseq (list 1 2 3 4 5) 1 3)
; => (2 3)

(subseq "hello world" 6)
; => "world"

(elt (list 10 20 30) 1)
; => 20

(elt "hello" 1)
; => "e"
```

Also present: `first`/`rest`/`second`/`third` (aliases for `car`/`cdr`/
`cadr`/`caddr`), `remove`, `count`/`count-if`, `copy-list`, `list-length`,
`reverse` (extended to strings), `nreverse` (a non-destructive alias — Lamedh
lists can't be reversed in place), `rem` (truncating remainder, contrast
`mod`), and `defparameter` (a `defdynamic` alias for defining dynamic
variables).

`remhash`, already covered above, also lives in this file as the CL-named
alias for `delete-key-bang`.
