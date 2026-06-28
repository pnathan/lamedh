# I/O Functions

This chapter documents input/output functions in Lamedh.

---

## Console Output

### PRINT

**Syntax:** `(print object...)`

Prints objects to standard output without formatting.

```lisp
(print "hello")         ; Prints: hello
(print 42)              ; Prints: 42
(print 'foo)            ; Prints: FOO
(print 1 2 3)           ; Prints: 123
```

**Arguments:**
- `object...` - One or more values to print

**Returns:** NIL

**Note:** Does not add spaces or newlines between objects.

---

### PRIN1

**Syntax:** `(prin1 object)`

Prints object in readable (escaped) form. Strings include quotes.

```lisp
(prin1 "hello")         ; Prints: "hello"
(prin1 42)              ; Prints: 42
(prin1 '(a b c))        ; Prints: (A B C)
```

**Arguments:**
- `object` - Value to print

**Returns:** The object that was printed

**Use for:** Output that could be read back by READ.

---

### PRINC

**Syntax:** `(princ object)`

Prints object in character form. Strings print without quotes.

```lisp
(princ "hello")         ; Prints: hello
(princ 42)              ; Prints: 42
(princ '(a b c))        ; Prints: (A B C)
```

**Arguments:**
- `object` - Value to print

**Returns:** The object that was printed

**Use for:** Human-readable output.

---

### TERPRI

**Syntax:** `(terpri)`

Prints a newline character.

```lisp
(princ "Line 1")
(terpri)
(princ "Line 2")
; Output:
; Line 1
; Line 2
```

**Returns:** NIL

---

## Console Input

### READ

**Syntax:** `(read)`

Reads one S-expression from standard input.

```lisp
(read)
; User types: (+ 1 2)
; Returns: (+ 1 2)  ; The list, unevaluated

(read)
; User types: 42
; Returns: 42

(read)
; User types: hello
; Returns: HELLO
```

**Returns:** Parsed S-expression

**Errors:** If input is not valid S-expression syntax

---

## File Loading

### LOAD-FILE

**Syntax:** `(load-file filename)`

Loads and evaluates a Lisp source file.

```lisp
(load-file "mylib.lisp")
; => T

(load-file "utils/helpers.lisp")
; => T
```

Loaded files can include other files at top level:

```lisp
;; app.lisp
(include "utils/helpers.lisp")

(defun main () (helper))
```

Relative include paths resolve from the file containing the include. Include
cycles are reported as errors.

**Arguments:**
- `filename` - String path to file (relative or absolute)

**Returns:** T on success

**Errors:**
- File not found
- Read/parse error
- Evaluation error in file contents

**File Path Resolution:**
- Relative paths: Resolved from current working directory
- Absolute paths: Used as-is

---

## Output Patterns

### Formatted Line

```lisp
(defun println (x)
  "Print X followed by newline."
  (princ x)
  (terpri))

(println "Hello, World!")
```

### Labeled Output

```lisp
(defun show (label value)
  "Print LABEL: VALUE with newline."
  (princ label)
  (princ ": ")
  (prin1 value)
  (terpri))

(show "Result" 42)
; Prints: Result: 42
```

### List Printing

```lisp
(defun print-list (lst)
  "Print each element of LST on its own line."
  (if (null lst)
      nil
      (progn
        (prin1 (car lst))
        (terpri)
        (print-list (cdr lst)))))

(print-list '(a b c))
; Prints:
; A
; B
; C
```

---

## Input Patterns

### Interactive Prompt

```lisp
(defun prompt (message)
  "Display MESSAGE and read user input."
  (princ message)
  (read))

(def name (prompt "Enter your name: "))
```

### Read-Eval Loop

```lisp
(defun simple-repl ()
  "A simple read-eval-print loop."
  (prog ()
   loop
    (princ "> ")
    (let ((input (read)))
      (if (eq input 'quit)
          (return 'goodbye)
          (progn
            (prin1 (eval input))
            (terpri)
            (go loop))))))
```

---

## File Loading Patterns

### Safe Load

```lisp
(defun safe-load (filename)
  "Load file, returning T on success, NIL on failure."
  (if (errorset (list 'load-file filename))
      t
      (progn
        (princ "Failed to load: ")
        (princ filename)
        (terpri)
        nil)))
```

### Load with Feedback

```lisp
(defun load-verbose (filename)
  "Load file with progress message."
  (princ "Loading ")
  (princ filename)
  (princ "... ")
  (load-file filename)
  (princ "done")
  (terpri))
```

---

## Comparison: PRIN1 vs PRINC

| Input | PRIN1 Output | PRINC Output |
|-------|--------------|--------------|
| `"hello"` | `"hello"` | `hello` |
| `'foo` | `FOO` | `FOO` |
| `42` | `42` | `42` |
| `'(a b)` | `(A B)` | `(A B)` |

**Rule of thumb:**
- Use `PRIN1` when output should be read back by Lisp
- Use `PRINC` when output is for human reading

---

## Limitations

Lamedh has limited I/O capabilities:

**Not Available:**
- File writing (no WRITE-FILE, WITH-OPEN-FILE)
- Binary I/O
- Network I/O
- File metadata (existence check, size, etc.)
- Stream manipulation
- Formatted output (FORMAT)

**Available:**
- Read from stdin
- Write to stdout
- Load Lisp source files

---

## Automatic Loading

At startup, Lamedh automatically loads:

1. `prologue.lisp` (if present)
2. All `.lisp` files in `lib/` (alphabetically)
3. Files specified with `-i` flag

```bash
# Load order: lib/*, myconfig.lisp, main.lisp
cargo run -- -i myconfig.lisp -i main.lisp
```

---

**See Also:**
- [File I/O Details](../file_io.md)
- [Error Handling](errors.md)
