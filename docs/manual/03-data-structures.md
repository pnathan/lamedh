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

`array-length*` reports the size, and `arrayp` tests the type. (The
trailing `*` is the 0.3 substrate convention: a starred name is the
direct per-type implementation behind a generic protocol — here
`length` — and the bare protocol name is the one to reach for first.
Chapter 4 covers protocols.)

```
(array-length* (array 4))
; => 4

(arrayp (array 2))
; => T
```

`lib/17-arrays.lisp` layers convenience functions on top of the four
primitives (`array` / `fetch` / `store` / `array-length*`):

- `array->list` / `list->array` — convert both directions.
- `array-map*` — return a new array with a function applied to every element.
- `array-fill` — set every slot to a value, mutating in place.
- `array-copy*` — a fresh array with the same contents.
- `subarray` — a fresh array holding a sub-range.

Growable vectors (push/pop-style resizing) are intentionally out of scope;
arrays are fixed-size once created.

### Typed arrays

A plain `array` holds boxed `LispVal`s — any element can be any type. A
**typed array** instead holds unboxed numbers in a flat buffer, declared as
`int64` or `float64` at creation:

```
(let ((a (typed-array 3 'int64)))
  (store a 0 7)
  (fetch a 0))
; => 7

(typed-array-p (typed-array 4 'float64))
; => T
```

`fetch`/`store`/`aref`/`aset`/`array-length*` behave identically to a plain
array, and `arrayp` is `T` for a typed array (use `typed-array-p` for the
narrow test). The difference is the representation, and it only matters at
the typed JIT boundary. A typed array's buffer has the same layout the JIT
uses for its own `(array int64)`/`(array float64)` parameters, so passing one
to a natively compiled function that expects a matching element type crosses
the membrane **by pointer, with no copy** — the callee's in-place writes are
visible to the caller afterward. A plain array crossing the same boundary is
copied in and copied back. Chapter 9 covers the membrane; reach for a typed
array when you are feeding a large numeric buffer to compiled code and want
to avoid the per-call copy.

## Hash Tables

Hash tables map arbitrary keys (compared with `equal`) to values. Create one
with `make-hash-table`:

```
(make-hash-table)
; => <hash-table>
```

Write with `sethash` (an alias for the primitive `set-bang`) and read with
`gethash`. One argument order everywhere (0.3 regularity): **collection
first**, like `fetch`, `store`, and `getp` — `(gethash table key)`,
`(remhash table key)`:

```
(let ((h (make-hash-table)))
  (sethash h 'a 1)
  (gethash h 'a))
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

Remove a key with `remhash` — the one removal name as of 0.3
(`delete-key`/`delete-key-bang` were removed), collection first like every
container operation. `length` counts hash entries too:

```
(let ((h (make-hash-table)))
  (set-bang h 'a 1)
  (remhash h 'a)
  (list (keys h) (length h)))
; => (() 0)
```

Other primitives and helpers:

- `keys` — a list of a table's keys, in insertion order.
- `hash-table-p` — type predicate.
- `hash-table-count*` — number of entries.
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
implements string operations on top of the Rust primitives `string-length*`,
`substring`, `char-code`, `code-char`, `string->number`, `number->string`,
and `concat`.

```
(concat "foo" "bar")
; => "foobar"

(string-length* "hello")
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
`string-last-index-of` (rightmost occurrence), `string-count` (occurrence
count), `string-replace`/`string-replace-all` (all occurrences) and
`string-replace-first` (first occurrence only), `string-lessp`
(code-point order), `string=` (an alias for `equal`), `string-trim-left`/
`string-trim-right` (one-sided trims — `string-trim` does both), and
`reverse`/`subseq`/`elt` (from the CL-compat layer, below — these work on
strings as well as lists; `string-reverse` is a named alias for `reverse`).

Construction and access (0.3, issue #254): `(make-string 5 "x")` is
`"xxxxx"` (default fill is a space); `(string-empty-p "")` is `T`;
`(string-concat "a" "b" "c")` is `"abc"` (a named alias for `concat`);
`(char-at "hello" 1)` is `"e"` and signals a clear error naming the index
and the string's length when the index is out of range (unlike
`substring`, which clamps).

The full comparison family (0.3, issue #254): case-sensitive ordering —
`string<`, `string>`, `string<=`, `string>=` (CL's names; already
case-sensitive in CL, so no divergence) — plus `string-ne` for
inequality (this reader does not allow `/` inside a symbol, so CL's
`string/=` cannot be spelled that way). A parallel Unicode-aware,
locale-independent case-insensitive family uses an explicit `-ci` infix
instead of CL's names: `string-ci=`, `string-ci<`, `string-ci>`,
`string-ci<=`, `string-ci>=`, `string-ci-ne`. (`string-lessp` already
existed with case-*sensitive* semantics before 0.3 grew this family, so
reusing CL's case-insensitive names for the new functions would have
been its own accidental divergence — see `lib/14-strings.lisp`'s header
and `docs/cl-divergences.md`.) None of the comparison functions take
optional start/end ranges; `substring` first if you need one.

`string-capitalize` uppercases the first character of every word (a
maximal alphanumeric run, as in CL) and lowercases the rest:
`(string-capitalize "hELLO world")` is `"Hello World"`.

Padding (0.3) never truncates: `(string-pad-left "42" 5 "0")` is
`"00042"`, `(string-pad-right "ab" 4)` is `"ab  "`, and both return the
string unchanged when it is already wide enough. `string-repeat` is the
building block: `(string-repeat "ab" 3)` is `"ababab"`.

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

### The TEXT module: UTF-8 <-> Array<Char>

`lib/30-text.lisp` (0.3, issue #254) carries the explicit boundary
between `String` (Unicode text) and `Array<Char>` (the language-level
byte-vector surface — `Char` is exactly a byte, 0-255, never a Unicode
scalar; see `docs/cl-divergences.md` item 3). This is genuinely new
library surface, not a completion of the flat `lib/14-strings.lisp`
Prelude, so per the modules story (`lib/27-modules.lisp`) it lives in a
`TEXT` module instead of growing the flat namespace:

```
(text:string->utf8 "héllo")
; => an Array<Char> of the 6 UTF-8 bytes

(text:utf8->string (text:string->utf8 "héllo"))
; => "héllo"

(import text)
(utf8->string-lossy (list->array (list (make-char 104) (make-char 128))))
; => "h<U+FFFD>"
```

`string->utf8` never fails (every Lisp `STRING` is valid Unicode).
`utf8->string` is strict: invalid UTF-8 signals an error naming the
offending byte offset. `utf8->string-lossy` instead substitutes the
Unicode replacement character (U+FFFD) for invalid sequences. There is
no implicit coercion between `String` and `Array<Char>` anywhere else in
the language — crossing the boundary is always one of these three calls.

### format

`lib/18-format.lisp` implements a useful subset of Common Lisp's `format`.
The first argument is the destination: `nil` returns the formatted string,
`t` prints it to stdout (and returns `nil`), and a PORTS port (see
[Ports and I/O](11-ports-and-io.md)) writes the UTF-8 bytes to it (and
returns `nil`).

```
(format nil "~a and ~s~%" 42 "str")
; => "42 and \"str\"\n"

(format t "hi ~a~%" 'world)
; => prints "hi WORLD\n", returns ()

(format (ports:open-output-bytes) "~a" 42)
; => writes "42" to the port, returns ()
```

Supported directives:

| Directive | Meaning |
|---|---|
| `~a` / `~A` | human (`princ`) rendering of the next argument |
| `~s` / `~S` | readable (`prin1`) rendering of the next argument |
| `~d` / `~D` | the next argument as a decimal datum |
| `~f` / `~F` | fixed-point float; `~,<n>f` (e.g. `~,4f`, CL's digit-count slot) rounds/pads to exactly n digits after the decimal point; CL's width form `~4f` is not implemented and errors |
| `~x` / `~X` | the next argument (an integer) in hexadecimal |
| `~o` / `~O` | the next argument (an integer) in octal |
| `~b` / `~B` | the next argument (an integer) in binary |
| `~c` / `~C` | the next argument, a one-character string or a `CHAR`, as the bare character |
| `~%` | newline |
| `~&` | a newline, unless this call's output so far already ends in one ("fresh-line") |
| `~~` | a literal tilde |
| `~{...~}` | iteration: the next argument is a list; repeat the enclosed directives over its elements until it is exhausted |
| `~^` | inside `~{...~}` (or at top level), stop early if there are no more arguments -- the `~a~^, ` idiom for joining a list without a trailing separator |

```
(format nil "~{~a~^, ~}" '(1 2 3))
; => "1, 2, 3"

(format nil "~x ~,4f" 255 3.14159)
; => "FF 3.1416"
```

An unrecognized directive -- including any of the above written with an
unsupported numeric/colon/at-sign prefix, e.g. `~3a` or `~:d` -- is a hard
error naming the offending directive, not a silent pass-through: a typo
should not degrade gracefully into wrong output. See
[cl-divergences.md](../cl-divergences.md) for the exact rule.

`READ-LINE` (read one line, from an explicit port or, if none is given,
the process's stdin under the `IO` capability), `WITH-OUTPUT-TO-STRING`
(capture writes to a fresh in-memory port as a string), and
`READ-SEXPR-FILE` / `WRITE-SEXPR-FILE` (round-trip a list of s-expressions
through a file, under `READ-FS` / `CREATE-FS`) are also defined in
`lib/18-format.lisp`, built on the [PORTS module](11-ports-and-io.md) and
the existing whole-file `READ-FILE`/`WRITE-FILE` builtins.

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
`cadr`/`caddr`), `remove`, `count`/`count-if`, `copy-list*` and
`list-length*` (the trailing `*` marks protocol substrate — the normative
names are `copy` and `length`; CL's bare spellings are gone),
`reverse` (extended to strings), `nreverse` (a non-destructive alias — Lamedh
lists can't be reversed in place), `rem` (truncating remainder, contrast
`mod`), and `defparameter` (a `defdynamic` alias for defining dynamic
variables).
