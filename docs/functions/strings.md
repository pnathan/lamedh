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

## Limitations

Lamedh's string support is basic compared to modern Lisps:

- **No escape sequences** - Cannot include quotes or special characters
- **No string comparison** - Use symbols for comparison
- **No substring extraction** - Only single character access
- **No string search** - No FIND, POSITION, etc.
- **No case conversion** - No UPCASE, DOWNCASE
- **No string formatting** - No FORMAT function

For text processing, consider:
- Converting to symbols with INTERN
- Using EXPLODE for character-level work
- Building strings with CONCAT

---

## Examples

### Building Strings

```lisp
(defun greet (name)
  (concat "Hello, " name "!"))

(greet "World")   ; => "Hello, World!"
```

### String Length via EXPLODE

```lisp
(defun string-length (s)
  "Get length of string S."
  (length (explode (intern s))))

; Note: This only works for strings that are valid symbol names
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
| Substring | `(subseq s start end)` | Not available |
| String= | `(string= a b)` | Use `(eq (intern a) (intern b))` |
| Length | `(length s)` | `(length (explode (intern s)))` |
| Format | `(format nil "~A" x)` | Not available |

---

**See Also:**
- [Data Types - Strings](../data_types.md#34-strings)
- [Predicates](predicates.md)
