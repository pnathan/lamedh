# String Functions

This chapter documents string manipulation functions in Lamedh.

---

## String Operations

### CONCAT

**Syntax:** `(concat string...)`

Concatenates all string arguments into a single string.

```lisp
(concat "Hello" " " "World")
; => "Hello World"

(concat "a" "b" "c")
; => "abc"

(concat)
; => ""

(concat "only")
; => "only"
```

**Arguments:**
- `string...` - Zero or more strings

**Returns:** Concatenated string

**Errors:** If any argument is not a string

---

### INDEX

**Syntax:** `(index string n)`

Returns the character at position n (0-indexed) as a single-character string.

```lisp
(index "hello" 0)    ; => "h"
(index "hello" 1)    ; => "e"
(index "hello" 4)    ; => "o"
```

**Arguments:**
- `string` - A string
- `n` - Non-negative integer index

**Returns:** Single-character string

**Errors:**
- If n is negative
- If n is >= string length

---

## Symbol/String Conversion

### EXPLODE

**Syntax:** `(explode atom)`

Converts an atom to a list of single-character symbols.

```lisp
(explode 'hello)
; => (H E L L O)

(explode 'abc)
; => (A B C)

(explode 42)
; => (4 2)
```

**Arguments:**
- `atom` - A symbol or number

**Returns:** List of character symbols

---

### IMPLODE

**Syntax:** `(implode char-list)`

Converts a list of character symbols to an interned symbol.

```lisp
(implode '(H E L L O))
; => HELLO

(implode '(A B C))
; => ABC
```

**Arguments:**
- `char-list` - List of single-character symbols

**Returns:** Interned symbol

---

### MAKNAM

**Syntax:** `(maknam char-list)`

Same as IMPLODE. Converts a list of character symbols to a symbol.

```lisp
(maknam '(F O O))
; => FOO
```

---

### INTERN

**Syntax:** `(intern string)`

Interns a string as a symbol in the global symbol table.

```lisp
(intern "HELLO")
; => HELLO

(eq (intern "FOO") 'foo)
; => T
```

**Arguments:**
- `string` - A string

**Returns:** Interned symbol

---

### GENSYM

**Syntax:** `(gensym)`

Generates a unique uninterned symbol.

```lisp
(gensym)    ; => G0001 (or similar)
(gensym)    ; => G0002
(gensym)    ; => G0003
```

**Returns:** Unique symbol

**Use for:** Creating symbols that won't conflict with user-defined names, especially in macros.

---

## Type Predicate

### STRINGP

**Syntax:** `(stringp x)`

Returns T if x is a string.

```lisp
(stringp "hello")   ; => T
(stringp 'hello)    ; => NIL
(stringp 42)        ; => NIL
(stringp "")        ; => T
```

---

## Additional String Primitives

### STRING-LENGTH*

**Syntax:** `(string-length* string)`

Returns the number of Unicode scalar values in `string`.

```lisp
(string-length* "hello")  ; => 5
(string-length* "")       ; => 0
```

### SUBSTRING

**Syntax:** `(substring string start)` or `(substring string start end)`

Returns characters from `start` inclusive to `end` exclusive. If `end` is
omitted, it defaults to the string length.

```lisp
(substring "hello" 1 3)  ; => "el"
(substring "hello" 2)    ; => "llo"
```

### CHAR-CODE, CODE-CHAR, MAKE-CHAR

`CHAR-CODE` converts a character value or one-character string to an integer.
`CODE-CHAR` converts an integer code point to a one-character string.
`MAKE-CHAR` converts an integer in `0..255` to Lamedh's byte-sized `Char` type.

### STRING->NUMBER and NUMBER->STRING

`STRING->NUMBER` parses integers and floats, returning `NIL` on failure.
`NUMBER->STRING` renders a number in decimal form.

### PRIN1-TO-STRING and PRINC-TO-STRING

These return the same text that `PRIN1` or `PRINC` would print. The stdlib
`FORMAT` builds on these rendering primitives.

---

## Current Limitations

Lamedh supports string escapes such as `\n`, `\t`, `\r`, `\\`, `\"`, and `\0`,
plus substring and value-to-string rendering. Remaining gaps compared with
Common Lisp include:

- No string search primitives such as `FIND` or `POSITION`
- No case conversion such as `STRING-UPCASE`
- No locale-aware collation or Unicode normalization
- No mutable strings

---

## Examples

### Building Strings

```lisp
(defun greet (name)
  (concat "Hello, " name "!"))

(greet "World")   ; => "Hello, World!"
```

### String Length

```lisp
(string-length* "Lamedh")  ; => 6
```

### Reversing a Symbol

```lisp
(defun reverse-symbol (sym)
  "Reverse the characters in symbol SYM."
  (implode (reverse (explode sym))))

(reverse-symbol 'hello)  ; => OLLEH
```

### Character Check

```lisp
(defun first-char (s)
  "Get first character of string S."
  (index s 0))

(first-char "hello")    ; => "h"
```

---

## Comparison with Common Lisp

| Operation | Common Lisp | Lamedh |
|-----------|-------------|--------|
| Concatenate | `(concatenate 'string ...)` | `(concat ...)` |
| Char access | `(char s n)` | `(index s n)` |
| Substring | `(subseq s start end)` | `(substring s start end)` |
| String= | `(string= a b)` | Use `(eq (intern a) (intern b))` |
| Length | `(length s)` | `(string-length* s)` |
| Format | `(format nil "~A" x)` | `(format nil "~A" x)` |

---

**See Also:**
- [Data Types - Strings](../data_types.md#34-strings)
- [Predicates](predicates.md)
