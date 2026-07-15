# Chapter 1: Introduction

This reference section documents Lamedh's data types, syntax, and forms
with exact signatures and return values.  For worked examples and design
rationale, see the [manual](manual/index.md).

## 1.1 Overview

Lamedh (Hebrew: ל, pronounced "LAH-med") is an implementation of Lisp 1.5 written in Rust. It provides a complete Lisp programming environment including:

- An interactive REPL (Read-Eval-Print Loop)
- File loading and evaluation
- A standard library of common functions
- Macro and fexpr support
- Hash tables and property lists
- A typed checker/JIT path for monomorphic typed code islands

## 1.2 History and Goals

Lisp 1.5, originally specified by John McCarthy and his team at MIT in 1962, was one of the first high-level programming languages. Its core ideas—s-expressions, garbage collection, conditional expressions, recursion, and symbolic computation—have influenced generations of programming languages.

Lamedh aims to:

1. **Faithfully implement Lisp 1.5 semantics** while providing a practical, usable system
2. **Extend where beneficial** with modern conveniences like strings and hash tables
3. **Provide a learning platform** for understanding the foundations of Lisp
4. **Be embeddable** in Rust applications

## 1.3 What Lamedh Is Not

Lamedh is not yet a 1.0 production Lisp system. It does not provide:

- Full Common Lisp or Scheme compatibility
- A mature debugger with step/trace integration
- Packages or streams
- Broad compiler optimization for ordinary dynamic code

## 1.4 Language Family

Lamedh belongs to the Lisp family of languages. Key relatives include:

| Language | Relationship |
|----------|--------------|
| Lisp 1.5 | Direct ancestor, primary inspiration |
| Common Lisp | Modern standardized Lisp (more features) |
| Scheme | Minimalist Lisp dialect |
| Clojure | Modern Lisp on JVM |
| Emacs Lisp | Lisp for the Emacs editor |

## 1.5 Key Concepts

### S-Expressions

All Lamedh code and data is represented as **S-expressions** (symbolic expressions):

```lisp
42                  ; A number
"hello"             ; A string
foo                 ; A symbol
(a b c)             ; A list
(+ 1 2)             ; A function call
```

### Code is Data

In Lisp, code and data share the same representation. This enables powerful metaprogramming:

```lisp
(def code '(+ 1 2))   ; A list representing code
(eval code)           ; Evaluate it => 3
```

### Lexical Scoping

Functions capture their lexical environment, enabling closures:

```lisp
(defun make-counter ()
  (let ((count 0))
    (lambda ()
      (setq count (+ count 1))
      count)))

(def counter (make-counter))
(counter)   ; => 1
(counter)   ; => 2
```

### Everything Returns a Value

Every expression in Lamedh returns a value:

```lisp
(if (> 3 2) "yes" "no")  ; => "yes"
(progn (+ 1 2) (* 3 4))  ; => 12 (last value)
```

## 1.6 Document Conventions

### Syntax Notation

```
expression  - Any valid Lisp expression
symbol      - A symbolic name
list        - A proper list
&rest       - Zero or more arguments
&optional   - Optional argument
```

### Examples

```lisp
(function arg1 arg2)
=> result
```

The `=>` notation indicates the return value of the expression.

### Error Conditions

Errors are shown as:

```lisp
(car 42)
;; Error: CAR requires a cons cell or NIL
```

## 1.7 Getting Help

Within the REPL:

```lisp
(documentation 'car)    ; Get docstring for a function
(plist 'my-symbol)      ; See symbol's properties
```

## 1.8 Source Code

Lamedh is implemented in Rust and consists of the following modules:

| Module | Purpose |
|--------|---------|
| `reader.rs` | Parsing S-expressions from text |
| `evaluator.rs` | Evaluating expressions |
| `environment.rs` | Variable bindings and scoping |
| `printer.rs` | Converting values to text |
| `lib.rs` | Core data types and library interface |
| `main.rs` | REPL and command-line interface |

---

**Next:** [Getting Started](getting_started.md)
