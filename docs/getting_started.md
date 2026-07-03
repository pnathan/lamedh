# Chapter 2: Getting Started

## 2.1 Building Lamedh

Lamedh requires Rust and Cargo. Build with:

```bash
cargo build --release
```

The executable will be at `target/release/lamedh`.

## 2.2 Running the REPL

Start the interactive REPL:

```bash
cargo run
```

You'll see:

```
Lamedh Lisp 1.5
>
```

Try some expressions:

```lisp
> (+ 1 2 3)
6
> (cons 'a '(b c))
(A B C)
> (defun square (x) (* x x))
SQUARE
> (square 5)
25
```

Multi-line forms work: while your parentheses are unbalanced the REPL shows
a ` ...>` continuation prompt. Ctrl-C cancels the input in progress; exit
with Ctrl-D or `(exit)`.

## 2.3 Command-Line Options

```bash
# Run a script; arguments after the path are available as *ARGV*
cargo run -- myscript.lisp arg1 arg2

# Load and run a file, then stay in the REPL
cargo run -- -i myfile.lisp

# Load multiple files
cargo run -- -i lib.lisp -i main.lisp

# Load a directory of .lisp files
cargo run -- -i mylib/

# Execute a single expression
cargo run -- -s "(+ 1 2)"
```

Scripts may start with a `#!` shebang line, and `(exit n)` sets the process
exit code — `(exit (if (run-tests) 0 1))` makes test files CI-ready.

## 2.4 Your First Program

Create `hello.lisp`:

```lisp
;; hello.lisp - A simple Lamedh program

(defun greet (name)
  "Print a greeting for NAME."
  (princ "Hello, ")
  (princ name)
  (princ "!")
  (terpri))

(greet "World")
```

Run it:

```bash
cargo run -- -i hello.lisp
```

Output:

```
Hello, World!
```

## 2.5 Project Structure

A typical Lamedh project:

```
myproject/
  lib/
    00-utils.lisp       ; Utility functions
    01-core.lisp        ; Core functionality
  main.lisp             ; Main program
```

Load order is alphabetical within directories.

## 2.6 Basic Syntax

### Comments

```lisp
; This is a comment (to end of line)
```

### Numbers

```lisp
42          ; Integer
-17         ; Negative integer
3.14159     ; Float
-2.5e10     ; Scientific notation
```

### Strings

```lisp
"Hello, World!"
"Line one"
```

### Symbols

```lisp
foo
my-variable
*global*
+special+
```

Note: All symbols are converted to uppercase internally.

### Lists

```lisp
(1 2 3)             ; List of numbers
(a b c)             ; List of symbols
((a 1) (b 2))       ; Nested lists
()                  ; Empty list (same as NIL)
```

### Dotted Pairs

```lisp
(a . b)             ; Cons cell
(1 2 . 3)           ; Improper list
```

## 2.7 Defining Things

### Variables

```lisp
(def x 42)                    ; Global variable
(def pi 3.14159 "The ratio")  ; With docstring
```

### Functions

```lisp
;; Simple function
(defun double (x)
  (* x 2))

;; With docstring
(defun factorial (n)
  "Compute the factorial of N."
  (if (= n 0)
      1
      (* n (factorial (- n 1)))))
```

### Macros

```lisp
(defmacro when (test &rest body)
  `(if ,test (progn ,@body) nil))

(when (> 3 2)
  (print "yes")
  "result")
```

## 2.8 Control Flow

### Conditionals

```lisp
;; IF
(if (> x 0) "positive" "non-positive")

;; COND (multi-way conditional)
(cond ((< x 0) "negative")
      ((= x 0) "zero")
      (t "positive"))
```

### Logical Operators

```lisp
(and a b c)   ; Short-circuit AND
(or a b c)    ; Short-circuit OR
(not x)       ; Boolean NOT
```

### Sequencing

```lisp
(progn
  (print "first")
  (print "second")
  "result")
```

## 2.9 Working with Lists

```lisp
;; Construction
(cons 'a '(b c))      ; => (A B C)
(list 1 2 3)          ; => (1 2 3)

;; Access
(car '(a b c))        ; => A
(cdr '(a b c))        ; => (B C)
(cadr '(a b c))       ; => B (second element)

;; Processing
(mapcar (lambda (x) (* x 2)) '(1 2 3))  ; => (2 4 6)
(length '(a b c))     ; => 3
(reverse '(a b c))    ; => (C B A)
(append '(a b) '(c d)) ; => (A B C D)
```

## 2.10 The Standard Library

Lamedh automatically loads files from `lib/` at startup:

| File | Contents |
|------|----------|
| `00-core.lisp` | `DEFUN` macro with docstring support |
| `01-list.lisp` | `APPEND`, `MEMBER`, `LENGTH`, `REVERSE`, etc. |
| `02-cxr.lisp` | `CADR`, `CADDR`, `CAADR`, etc. |
| `03-meta.lisp` | `DOCUMENTATION` |
| `04-predicates.lisp` | `EQUAL` |
| `05-math.lisp` | `ABS`, `MAX`, `MIN`, `ONEP`, `MINUSP` |
| `07-shell.lisp` ... `18-format.lisp` | Shell, vau, Lisp 1.5, testing, optimizer, control, functional, string, set/hash, condition, array, and format helpers |
| `97-doc-renderer.lisp` ... `99-help-data.lisp` | REPL help system and structured documentation database |

## 2.11 Example: Fibonacci

```lisp
(defun fib (n)
  "Compute the Nth Fibonacci number."
  (cond ((= n 0) 0)
        ((= n 1) 1)
        (t (+ (fib (- n 1))
              (fib (- n 2))))))

(fib 10)  ; => 55
```

## 2.12 Example: List Processing

```lisp
(defun sum-list (lst)
  "Sum all numbers in LST."
  (if (null lst)
      0
      (+ (car lst) (sum-list (cdr lst)))))

(sum-list '(1 2 3 4 5))  ; => 15
```

## 2.13 Example: Higher-Order Functions

```lisp
(defun filter (pred lst)
  "Return elements of LST satisfying PRED."
  (cond ((null lst) nil)
        ((funcall pred (car lst))
         (cons (car lst) (filter pred (cdr lst))))
        (t (filter pred (cdr lst)))))

(filter (lambda (x) (> x 2)) '(1 2 3 4 5))
; => (3 4 5)
```

---

**Next:** [Data Types](data_types.md)
