# Lithhp Lisp Interpreter

Welcome to the documentation for Lithhp, a Lisp interpreter written in Rust.

## Introduction

Lithhp is a lightweight Lisp interpreter that supports a subset of the Lisp language. It is designed to be a simple, embeddable interpreter that can be used to run Lisp code.

## Building and Running

To build the project, run the following command from the `lithhp` directory:

```bash
cargo build
```

To run the REPL, use the following command:

```bash
cargo run
```

You can also execute a file containing Lisp code:

```bash
cargo run -- -i my-file.lisp
```

Or execute a single s-expression:

```bash
cargo run -- -s "(+ 1 2)"
```

## Built-in Functions

Lithhp provides a number of built-in functions:

### Arithmetic

*   `+`: Adds two or more numbers.
*   `-`: Subtracts two or more numbers.
*   `*`: Multiplies two or more numbers.
*   `/`: Divides two or more numbers.

### List Manipulation

*   `car`: Returns the first element of a list.
*   `cdr`: Returns the rest of a list.
*   `cons`: Constructs a new list.
*   `concat`: Concatenates two or more lists or strings.
*   `index`: Returns the element at a given index in a list.

### Hash Tables

*   `make-hash-table`: Creates a new hash table.
*   `get`: Gets a value from a hash table by key.
*   `set`: Sets a value in a hash table by key.
*   `delete-key`: Deletes a key-value pair from a hash table.
*   `keys`: Returns a list of keys in a hash table.

### Other

*   `eval`: Evaluates a Lisp expression.
*   `eq`: Checks if two values are equal.
*   `not`: Negates a boolean value.
*   `atom`: Checks if a value is an atom.
*   `current-environment`: Returns the current environment.
*   `print`: Prints its arguments.

## Examples

Here are some examples of Lisp code that can be run with Lithhp.

### Basic Arithmetic

```lisp
(+ 1 2 3)
; => 6

(- 10 5)
; => 5

(* 2 3 4)
; => 24

(/ 10 2)
; => 5
```

### Defining Functions

The `prologue.lisp` file, which is loaded by default, provides a `defun` macro for defining functions.

```lisp
(defun square (x)
  (* x x))

(square 5)
; => 25
```

### Working with Lists

```lisp
(car '(1 2 3))
; => 1

(cdr '(1 2 3))
; => (2 3)

(cons 0 '(1 2 3))
; => (0 1 2 3)

(concat '(1 2) '(3 4))
; => (1 2 3 4)

(index 1 '(a b c))
; => b
```

### Working with Hash Tables

```lisp
(def my-table (make-hash-table))
(set my-table "name" "Jules")
(get my-table "name")
; => "Jules"

(keys my-table)
; => ("name")
```

### Defining Macros

Macros allow you to extend the syntax of Lisp. Here is an example of an `unless` macro:

```lisp
(defmacro unless (condition true-branch false-branch)
  `(if (not ,condition)
       ,true-branch
       ,false-branch))

(unless (= 1 2)
  (concat "1 is not " "equal to 2")
  (concat "1 is " "equal to 2"))
; => "1 is not equal to 2"
```

### F-Expressions (defexpr)

F-expressions are similar to functions, but their arguments are not evaluated. They receive the unevaluated arguments as a list.

```lisp
(defexpr quote-args (args)
  args)

(quote-args (+ 1 2) "hello")
; => ((+ 1 2) "hello")
```
