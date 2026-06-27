# Lamedh Reference Manual

**Lamedh** (Hebrew: ל, "Lamed") — a Lisp 1.5 interpreter written in Rust.  
Version 0.2.0 · License AGPL-3.0

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Project Layout and Build](#2-project-layout-and-build)
3. [Getting Started](#3-getting-started)
4. [Data Types](#4-data-types)
5. [Syntax and the Reader](#5-syntax-and-the-reader)
6. [Evaluation Rules](#6-evaluation-rules)
7. [Special Forms](#7-special-forms)
8. [Built-in Functions](#8-built-in-functions)
9. [Standard Library](#9-standard-library)
10. [Environments and Scoping](#10-environments-and-scoping)
11. [Functions, Macros, Fexprs, and Vau](#11-functions-macros-fexprs-and-vau)
12. [PROG, GO, and RETURN](#12-prog-go-and-return)
13. [Loops: FOR and WHILE](#13-loops-for-and-while)
14. [Hash Tables](#14-hash-tables)
15. [Arrays](#15-arrays)
16. [Property Lists](#16-property-lists)
17. [Structs](#17-structs)
18. [Error Handling](#18-error-handling)
19. [Condition Flags](#19-condition-flags)
20. [Capabilities and Sandboxing](#20-capabilities-and-sandboxing)
21. [The Optimizer](#21-the-optimizer)
22. [Testing Framework](#22-testing-framework)
23. [The Help System](#23-the-help-system)
24. [Embedding Lamedh in Rust](#24-embedding-lamedh-in-rust)
25. [Cargo Workspace and Dependencies](#25-cargo-workspace-and-dependencies)
26. [Architecture Reference](#26-architecture-reference)
27. [Appendix A: Differences from Lisp 1.5](#appendix-a-differences-from-lisp-15)
28. [Appendix B: Known Limitations](#appendix-b-known-limitations)
29. [Appendix C: Quick Reference Card](#appendix-c-quick-reference-card)

---

## 1. Introduction

Lamedh is a complete, embeddable Lisp 1.5 interpreter written in Rust. It
provides:

- A tree-walking evaluator with trampolining tail-call optimisation (TCO)
- Lexical closures, macros, fexprs, and Kernel-style vau operatives
- Both lexical and dynamic (special) variable scoping
- An extensible type system for host-defined Rust values
- A capability-gated sandbox (filesystem, shell, stdin — all off by default)
- A full standard library embedded in the binary (no `.lisp` files required)
- An interactive REPL with line editing and history

### Design philosophy

> **Prefer the Lisp layer; keep the Rust kernel small.**  
> When an optimisation can be expressed as a Lisp-to-Lisp transform, implement
> it as an optimizer pass in `lib/11-optimizer-vau.lisp` rather than growing
> the Rust evaluator.  The kernel stays a minimal set of primitives; the Lisp
> layer does the rest.

Lamedh is not yet a 1.0 production Lisp. It has an experimental typed
checker/JIT path, but it does not provide full Common Lisp or Scheme
compatibility, a mature debugger, packages, streams, or a full condition system.
It *is* a faithful, practical, embeddable Lisp 1.5 dialect with modern
extensions.

---

## 2. Project Layout and Build

### Cargo workspace

```
lamedh/                  ← workspace root
  Cargo.toml             ← workspace manifest + library [package]
  src/
    lib.rs               ← public API, LispVal, LispError, From/TryFrom impls
    reader.rs            ← nom-based s-expression parser
    evaluator.rs         ← eval loop, special forms, 100+ builtins
    environment.rs       ← variable binding, symbol table, scoping
    printer.rs           ← LispVal → String formatter
    optimizer.rs         ← constant-folding source optimizer
  lib/                   ← Lisp standard library (embedded at compile time)
    00-core.lisp … 99-help-data.lisp
  cli/                   ← lamedh-cli crate (binary named `lamedh`)
    Cargo.toml
    src/main.rs          ← argument parsing (clap) + REPL (rustyline)
  tests/                 ← integration tests
  benchmarks/            ← performance benchmarks (excluded from workspace)
```

The two workspace members are:

| Crate | Type | Description |
|-------|------|-------------|
| `lamedh` | library | Reusable interpreter. Default features include the typed JIT backend; use `--no-default-features` for the dependency-light typed checker. |
| `lamedh-cli` | binary (`lamedh`) | CLI/REPL driver. Depends on `lamedh`, `clap`, `rustyline`. |

Benchmark crates under `benchmarks/*/rust` are **excluded** from the workspace
and built directly by `benchmarks/run_benchmarks.sh`.

### Build commands

```bash
cargo build                        # build both crates
cargo build --release              # release build
cargo run                          # launch interactive REPL
cargo run -- -i myfile.lisp        # load file before REPL
cargo run -- -i mylib/             # load directory of .lisp files
cargo run -- -s "(+ 1 2)"          # evaluate expression and exit
cargo test                         # run all tests
cargo test <name>                  # run specific test
cargo clippy --workspace --all-targets
cargo fmt --all
cargo doc --no-deps                # build rustdoc
```

> Always run `cargo fmt --all` and `cargo clippy --workspace --all-targets`
> before committing. A clean clippy run is part of "done".

---

## 3. Getting Started

### Interactive REPL

```bash
cargo run
```

The prompt is `(ל)> ` (the Hebrew letter Lamed). Exit with Ctrl-D or Ctrl-C.

```
(ל)> (+ 1 2 3)
6
(ל)> (defun square (x) (* x x))
SQUARE
(ל)> (square 7)
49
(ל)> (mapcar (lambda (x) (* x x)) '(1 2 3 4 5))
(1 4 9 16 25)
```

### Loading files

```bash
cargo run -- -i hello.lisp
cargo run -- -i lib/ -i main.lisp
```

Files in a directory are loaded in sorted filename order, so numeric prefixes
(`00-`, `01-`, …) control load order.

### Command-line expression evaluation

```bash
cargo run -- -s "(+ 1 2 3)"
# prints: 6

cargo run -- -s "(defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 10)"
# prints: 55
```

### In-REPL help

```lisp
(help)                    ; overview
(help 'car)               ; help for a specific function
(help 'categories)        ; list all categories
(help 'category 'lists)   ; functions in a category
(documentation 'defun)    ; raw docstring
```

---

## 4. Data Types

All Lisp values are instances of the `LispVal` enum.

### 4.1 Nil

The empty list and boolean false. Written `()` or `NIL`. It is the only falsy
value — everything else is truthy.

```lisp
()          ; nil / false
nil         ; same
```

### 4.2 Symbol

An interned, uppercased identifier. Two occurrences of the same name share one
allocation; `EQ` is pointer equality.

```lisp
foo         ; → FOO (uppercased)
*my-var*    ; dynamic variable convention
+           ; operator symbol
```

`T` is the canonical true value.

### 4.3 Number (integer)

A 64-bit signed integer (`i64`).

```lisp
42
-17
0
```

Octal notation (Lisp 1.5): `177Q` = 127₁₀.

### 4.4 Float

A 64-bit IEEE 754 double (`f64`).

```lisp
3.14
-2.5e10
1.0
```

Arithmetic auto-promotes to float when any argument is a float.

### 4.5 String

UTF-8 string literals, double-quoted.

```lisp
"hello, world"
"line one\nline two"
"tab\there"
```

Supported escape sequences: `\n \t \r \\ \" \0`.

### 4.6 Cons cell

The fundamental compound structure. `car` holds the head; `cdr` holds the tail
(usually another cons or nil).

```lisp
(cons 'a 'b)        ; => (A . B)  dotted pair
(cons 1 '(2 3))     ; => (1 2 3)  proper list
```

Cons cells are immutable (`RPLACA`/`RPLACD` return new cells).

### 4.7 Lambda / Fexpr / Macro / Vau

Callable values created by special forms. See [section 11](#11-functions-macros-fexprs-and-vau).

### 4.8 Builtin

Internal tag for Rust-implemented primitives. Printed as `<builtin>`.

### 4.9 Hash Table

Mutable key-value store. See [section 14](#14-hash-tables).

### 4.10 Array

Mutable 0-indexed vector. See [section 15](#15-arrays).

### 4.11 Environment

A first-class environment obtained via `(the-environment)` or
`(make-environment)`. Can be passed to `(eval expr env)`.

### 4.12 Native

A host-registered Rust closure callable from Lisp. See [section 24](#24-embedding-lamedh-in-rust).

### 4.13 Extension

A host-defined opaque Rust value. See [section 24](#24-embedding-lamedh-in-rust).

### Truthiness

```lisp
; Only NIL is false:
(if nil "false" "true")   ; => "true"
(if 0   "false" "true")   ; => "true"   ← 0 is truthy
(if ""  "false" "true")   ; => "true"   ← empty string is truthy
(if ()  "false" "true")   ; => "true"   ← () = NIL is false
```

---

## 5. Syntax and the Reader

The reader parses text into `LispVal` trees. All symbols are **uppercased**
during interning.

### 5.1 Atoms

```lisp
; Integers
42   -17   0

; Octal (Lisp 1.5 notation)
177Q        ; = 127 decimal

; Floats
3.14   -2.5e10   1.0

; Strings
"hello"   "line\none"

; Symbols
foo   FOO   *var*   +   ->string   set!
```

### 5.2 Lists

```lisp
(a b c)         ; proper list: (a . (b . (c . nil)))
(a . b)         ; dotted pair
(1 2 . 3)       ; improper list
()              ; nil
```

### 5.3 Reader macros

| Input | Expansion |
|-------|-----------|
| `'x` | `(QUOTE x)` |
| `` `x `` | `(QUASIQUOTE x)` |
| `,x` | `(UNQUOTE x)` |
| `#'f` | `(FUNCTION f)` |

### 5.4 Quasiquotation

```lisp
(def x 10)
(def y 20)
`(sum is ,(+ x y))          ; => (SUM IS 30)
`(list ,x ,y ,(* x y))      ; => (LIST 10 20 200)
```

Nested quasiquotes are supported; unquote evaluates in the surrounding scope.

### 5.5 Comments

```lisp
; This is a line comment — everything after ; to end of line is ignored
```

### 5.6 Symbol naming rules

Valid symbol characters: letters, digits, `-`, `*`, `?`, `!`, `+`, `=`, `<`,
`>`, `&`, `$`. Must start with a letter, `&`, or `$` (or be a pure operator
sequence like `+`, `<=`, `->`, etc.). Earmuff symbols `*name*` are a naming
convention for dynamic (special) variables.

---

## 6. Evaluation Rules

Lamedh uses **applicative-order** evaluation (evaluate arguments before calling
the function), with these rules:

1. **Self-evaluating atoms**: numbers, floats, strings, nil → themselves.
2. **Symbol**: look up the binding in the current environment.
3. **Quoted form** `'x` → `x` unevaluated.
4. **Special form** `(special-form args…)` → handled by the evaluator directly
   (arguments may or may not be evaluated depending on the form).
5. **Function call** `(f arg1 arg2 …)` → evaluate `f`, evaluate all args, apply.

### Tail-call optimisation (TCO)

The evaluator uses a trampoline so tail calls in `IF` branches, `PROGN` last
forms, `LET` bodies, and lambda bodies consume no additional Rust stack frames.
Deeply recursive programs should use tail-recursive style or iterative loops.

### Recursion depth limit

The default limit is 10,000 eval frames (not tail calls). Exceeded → recoverable
`LispError::Generic`. Adjust with `lamedh::set_eval_depth_limit(n)` in Rust or
run on a reduced-depth workload.

---

## 7. Special Forms

Special forms are handled before argument evaluation. They receive the raw
s-expression structure.

### 7.1 QUOTE

```lisp
(quote x)
'x          ; reader shorthand
```

Returns `x` unevaluated.

```lisp
'(a b c)    ; => (A B C)  — a list, not a function call
'foo        ; => FOO
```

### 7.2 QUASIQUOTE / UNQUOTE

```lisp
(quasiquote template)
`template   ; reader shorthand
,expr       ; UNQUOTE within quasiquote
```

Like QUOTE but holes marked with `,` are evaluated:

```lisp
(def n 5)
`(the answer is ,n)        ; => (THE ANSWER IS 5)
`(doubled: ,(* n 2))       ; => (DOUBLED: 10)
```

### 7.3 IF

```lisp
(if condition then-expr else-expr)
```

Evaluates `condition`; if truthy evaluates `then-expr`, otherwise `else-expr`.
Both branches are in tail position.

```lisp
(if (> x 0) "positive" "non-positive")
(if (null lst) 0 (+ 1 (length (cdr lst))))
```

### 7.4 COND

```lisp
(cond (pred1 expr1…)
      (pred2 expr2…)
      (t     default…))
```

Tests predicates in order; evaluates the body of the first truthy clause.  The
last expression in a clause is in tail position.  If no clause matches, returns
`NIL`.

```lisp
(cond ((< n 0) "negative")
      ((= n 0) "zero")
      (t       "positive"))
```

### 7.5 AND

```lisp
(and expr…)
```

Short-circuit AND. Returns the last truthy value, or `NIL` on the first falsy.
Zero arguments returns `T`.

### 7.6 OR

```lisp
(or expr…)
```

Short-circuit OR. Returns the first truthy value, or `NIL` if all are falsy.
Zero arguments returns `NIL`.

### 7.7 DEF

```lisp
(def name value)
(def name value "docstring")
```

Defines a global variable. The optional docstring is stored on the symbol's
property list under key `"docstring"`.

```lisp
(def pi 3.14159)
(def greeting "Hello" "The greeting string.")
```

### 7.8 DEFDYNAMIC / DEFVAR

```lisp
(defdynamic *name* initial-value)
(defvar     *name* initial-value)
(defdynamic *name* initial-value "docstring")
```

Like `DEF` but marks the variable as **dynamic** (special). Future lookups of
this name walk the call stack rather than the lexical chain. Use the `*earmuff*`
naming convention as a signal to readers.

```lisp
(defdynamic *print-level* 10)
```

### 7.9 SETQ

```lisp
(setq name value)
(setq name1 val1 name2 val2 …)
```

Assign to an existing variable (search lexical then dynamic chain). If not
found, creates it in the current frame. Accepts an even number of arguments.

```lisp
(setq x 42)
(setq x 1 y 2 z 3)
```

### 7.10 LAMBDA

```lisp
(lambda (param…) body…)
(lambda (param… &rest rest-param) body…)
```

Creates a lexical closure. Multiple body forms are sequenced; the last is the
return value.

```lisp
(lambda (x) (* x x))
(lambda (x y) (+ x y))
(lambda (x &rest rest) (cons x rest))
```

### 7.11 DEFEXPR

```lisp
(defexpr name (args-name) body…)
(defexpr name (args-name) body… "docstring")
```

Create a named fexpr — a function that receives its arguments **unevaluated**
as a list bound to `args-name`. Unlike a macro, the body result is the return
value directly (not re-evaluated).

```lisp
(defexpr my-quote (args) (car args))
(my-quote foo)   ; => FOO — args is (FOO), (car args) = FOO
```

### 7.12 DEFMACRO

```lisp
(defmacro name (param… &rest rest) body…)
(defmacro name (param… &rest rest) body… "docstring")
```

Create a macro. Arguments are received unevaluated; the body must return a
Lisp form which is then evaluated in the caller's environment.

```lisp
(defmacro when (test &rest body)
  `(if ,test (progn ,@body) nil))

(defmacro while (test &rest body)
  `(prog ()
     loop
     (if (not ,test) (return nil))
     ,@body
     (go loop)))
```

> `&REST` in macros collects remaining parameters into a list.  
> Use `,@list` (unquote-splicing) to splice a list into a quasiquote.

### 7.13 FUNCTION

```lisp
(function name)
#'name         ; reader shorthand
```

Returns the function value of `name` (useful to pass built-ins as values).

```lisp
(mapcar #'(lambda (x) (* x x)) '(1 2 3 4))
```

### 7.14 LABEL

```lisp
(label name (lambda ...))
```

Creates a recursive function by evaluating the literal lambda in a child
environment and then binding `name` to the resulting closure in that same child
environment. The payload must be a `LAMBDA` expression; malformed nested `LABEL`
graphs are rejected instead of being re-evaluated as delayed expressions.

```lisp
((label fact (lambda (n) (if (zerop n) 1 (* n (fact (sub1 n)))))) 5)
; => 120
```

### 7.15 DEFINE

```lisp
(define ((name1 value1) (name2 value2) …))
```

Batch global definition; stores values under the `EXPR` property on each
symbol's plist (Lisp 1.5 style).

### 7.16 DEFSTRUCT

```lisp
(defstruct struct-name field1 field2 …)
```

Generates a constructor, predicate, field accessors, and field mutators.
Structs are implemented as hash tables with a `__type__` key.

```lisp
(defstruct point x y)

(def p (make-point :x 3 :y 4))
(point-p p)           ; => T
(point-x p)           ; => 3
(set-point-x! p 10)   ; mutate
(point-x p)           ; => 10
```

Generated names:
- Constructor: `make-point`
- Predicate: `point-p`
- Accessor: `point-field`
- Mutator: `set-point-field!`

### 7.17 PROGN

```lisp
(progn expr…)
```

Evaluate expressions in sequence; return the value of the last. The last
expression is in tail position.

```lisp
(progn
  (print "step 1")
  (print "step 2")
  42)            ; => 42
```

### 7.18 LET

```lisp
(let ((var1 val1) (var2 val2) …) body…)
```

Parallel bindings — all `val` expressions are evaluated in the outer scope
before any variable is bound. Body is in tail position.

```lisp
(let ((x 1)
      (y 2))
  (+ x y))        ; => 3
```

### 7.19 LET*

```lisp
(let* ((var1 val1) (var2 val2) …) body…)
```

Sequential bindings — each `val` is evaluated with the preceding variables
already bound.

```lisp
(let* ((x 1)
       (y (+ x 1)))
  (* x y))        ; => 2
```

### 7.20 PROG

```lisp
(prog (var…) statement…)
```

Execute a block with local variables (initialised to `NIL`) and optional labels
for `GO`. See [section 12](#12-prog-go-and-return).

### 7.21 RETURN

```lisp
(return value)
```

Exit the enclosing `PROG`, returning `value`. Only valid inside `PROG`.

### 7.22 GO

```lisp
(go label)
```

Jump to `label` inside the enclosing `PROG`. `label` must be a symbol atom
directly in the `PROG` body.

### 7.23 FOR

```lisp
(for (var start end) body…)
(for (var start end step) body…)
```

Inclusive integer loop. `var` runs from `start` to `end` (inclusive) in
increments of `step` (default 1). No per-iteration frame allocation.

```lisp
(for (i 1 10)
  (print i))           ; prints 1 2 3 … 10

(for (i 0 20 2)
  (print i))           ; prints 0 2 4 … 20
```

Returns `NIL`.

### 7.24 WHILE

```lisp
(while condition body…)
```

Loop while `condition` is truthy. No per-iteration frame allocation.

```lisp
(def n 0)
(while (< n 5)
  (setq n (+ n 1)))
n   ; => 5
```

### 7.25 VAU / $VAU

```lisp
(vau  (operands-param env-param) body…)
($vau (operands-param env-param) body…)
```

Kernel-style operative. `operands-param` is bound to the **unevaluated** operand
cons list; `env-param` is bound to the **caller's environment** as a
`LispVal::Environment`. The body runs in the vau's lexical environment.

```lisp
(def my-if
  (vau (ops e)
    (if (eval (car ops) e)
        (eval (cadr ops) e)
        (eval (caddr ops) e))))
```

---

## 8. Built-in Functions

### 8.1 Arithmetic

All arithmetic auto-promotes to float when any argument is a float.

| Function | Syntax | Description |
|----------|--------|-------------|
| `+` / `PLUS` | `(+ n…)` | Sum (0 args → 0) |
| `-` / `DIFFERENCE` | `(- n…)` | Subtract (1 arg → negate) |
| `*` / `TIMES` | `(* n…)` | Product (0 args → 1) |
| `/` / `QUOTIENT` | `(/ n…)` | Divide |
| `EXPT` | `(expt base exp)` | Power |
| `REMAINDER` | `(remainder n m)` | Integer remainder (sign of dividend) |
| `MOD` | `(mod n m)` | Modulo (sign of divisor) |
| `ADD1` / `1+` | `(add1 n)` | Increment by 1 |
| `SUB1` / `1-` | `(sub1 n)` | Decrement by 1 |
| `ABS` | `(abs n)` | Absolute value |
| `MAX` | `(max n…)` | Maximum |
| `MIN` | `(min n…)` | Minimum |
| `RANDOM` | `(random n)` | Random integer 0…n−1 |

```lisp
(+ 1 2 3)           ; => 6
(- 10 3)            ; => 7
(* 2 3 4)           ; => 24
(/ 10 3)            ; => 3  (integer division)
(/ 10.0 3)          ; => 3.3333…
(expt 2 10)         ; => 1024
(remainder 17 5)    ; => 2
(mod -7 3)          ; => 2
```

Overflow: checked arithmetic; on overflow the `"OVERFLOW"` condition flag is
set and wrapping semantics apply.

### 8.2 Numeric predicates

| Function | Description |
|----------|-------------|
| `ZEROP` | True if argument is 0 |
| `PLUSP` | True if argument > 0 |
| `MINUSP` | True if argument < 0 (stdlib) |
| `ONEP` | True if argument = 1 (stdlib) |
| `EVENP` | True if integer is even |
| `ODDP` | True if integer is odd |
| `FIXP` | True if integer |
| `FLOATP` | True if float |
| `NUMBERP` | True if integer or float |

### 8.3 Comparison

| Function | Syntax | Description |
|----------|--------|-------------|
| `=` / `EQUAL-NUMBER` | `(= a b)` | Numeric equality |
| `<` / `LESSP` | `(< a b)` | Numeric less-than |
| `>` / `GREATERP` | `(> a b)` | Numeric greater-than |
| `FLOAT-EQUAL` | `(float-equal a b)` | Float equality |
| `FLOAT-LESSP` | `(float-lessp a b)` | Float less-than |
| `FLOAT-GREATERP` | `(float-greaterp a b)` | Float greater-than |

### 8.4 Bitwise operations

| Function | Description |
|----------|-------------|
| `LOGOR` | Bitwise OR |
| `LOGAND` | Bitwise AND |
| `LOGXOR` | Bitwise XOR |
| `LOGNOT` | Bitwise NOT |
| `LEFTSHIFT` | Left shift (negative = right shift) |
| `ASH` | Arithmetic shift |
| `ROT` | Rotate bits |

```lisp
(logor  #b1010 #b0101)   ; => 15
(logand #b1111 #b1010)   ; => 10
(leftshift 1 4)           ; => 16
```

### 8.5 List operations

| Function | Syntax | Description |
|----------|--------|-------------|
| `CONS` | `(cons a b)` | Build a cons cell |
| `CAR` | `(car list)` | First element |
| `CDR` | `(cdr list)` | Rest of list |
| `LIST` | `(list x…)` | Build a proper list |
| `ATOM` | `(atom x)` | True if not a cons cell |
| `CONSP` | `(consp x)` | True if cons cell (stdlib) |
| `NULL` | `(null x)` | True if NIL (stdlib) |
| `NTH` | `(nth n lst)` | N-th element (0-based) |
| `NTHCDR` | `(nthcdr n lst)` | N-th CDR |
| `LAST` | `(last lst)` | Last element |
| `EFFACE` / `DELETE` | `(efface x lst)` | Remove first occurrence of `x` |
| `LENGTH` | `(length lst)` | Number of elements (stdlib) |
| `REVERSE` | `(reverse lst)` | Reverse list (stdlib) |
| `APPEND` | `(append l1 l2)` | Concatenate lists (stdlib) |
| `MEMBER` | `(member x lst)` | Search by EQUAL (stdlib) |
| `ASSOC` | `(assoc key alist)` | Search association list by EQUAL |
| `SUBST` | `(subst new old tree)` | Substitute in tree |
| `SUBLIS` | `(sublis alist tree)` | Batch substitute |
| `MAPCAR` | `(mapcar fn lst)` | Map function over list |
| `MAPLIST` | `(maplist fn lst)` | Map over tails |
| `RPLACA` | `(rplaca cons val)` | Return new cons with new car |
| `RPLACD` | `(rplacd cons val)` | Return new cons with new cdr |

```lisp
(car '(a b c))             ; => A
(cdr '(a b c))             ; => (B C)
(cons 1 '(2 3))            ; => (1 2 3)
(list 1 2 3)               ; => (1 2 3)
(nth 2 '(a b c d))         ; => C
(mapcar #'add1 '(1 2 3))   ; => (2 3 4)
(assoc 'b '((a 1) (b 2)))  ; => (B 2)
```

### 8.6 CXR compositions (stdlib)

`CAAR`, `CADR`, `CDAR`, `CDDR` (2-level), `CAAAR`…`CDDDR` (3-level),
`CAAAAR`…`CDDDDR` (4-level) — 30 functions total.

```lisp
(cadr '(a b c))    ; => B   (= (car (cdr lst)))
(caddr '(a b c))   ; => C
(caadr '((a b) c)) ; => A
```

### 8.7 Predicates

| Function | Description |
|----------|-------------|
| `EQ` | Pointer equality (atoms/symbols) |
| `EQUAL` | Structural equality (stdlib) |
| `ATOM` | True if not a cons |
| `SYMBOLP` | True if symbol |
| `STRINGP` | True if string |
| `FUNCTIONP` | True if callable (lambda/fexpr/macro/builtin/native) |
| `MACROP` | True if macro |
| `ARRAYP` | True if array |
| `EXTENSION-P` | True if extension value |
| `BOUNDP` | True if symbol has a binding |
| `NOT` | Logical negation |

```lisp
(eq 'foo 'foo)          ; => T   (same interned symbol)
(eq '(1 2) '(1 2))      ; => ()  (different allocations)
(equal '(1 2) '(1 2))   ; => T   (structural equality)
(symbolp 'x)            ; => T
(stringp "hi")          ; => T
```

### 8.8 String and symbol functions

| Function | Syntax | Description |
|----------|--------|-------------|
| `CONCAT` | `(concat s…)` | Concatenate strings |
| `INDEX` | `(index s n)` | Character at position n (as symbol) |
| `EXPLODE` | `(explode sym)` | Symbol name → list of char symbols |
| `IMPLODE` | `(implode lst)` | List of char symbols → symbol |
| `MAKNAM` | `(maknam lst)` | Concatenate symbol names |
| `GENSYM` | `(gensym)` | Generate unique uninterned symbol |
| `INTERN` | `(intern str)` | Intern a string as a symbol |

```lisp
(concat "hello" ", " "world")   ; => "hello, world"
(explode 'abc)                  ; => (A B C)
(implode '(h e l l o))         ; => HELLO
(gensym)                        ; => G0001 (unique each call)
```

### 8.9 I/O (requires `IO` capability)

| Function | Description |
|----------|-------------|
| `READ` | Read one s-expression from stdin |
| `PRIN1` | Print with quotes/escapes (no newline) |
| `PRINC` | Print without quotes (no newline) |
| `PRINT` | Print all args (no newline) |
| `TERPRI` | Print a newline |
| `SPACES` | Print N spaces |

`READ` requires the `IO` feature enabled. `PRIN1`, `PRINC`, `PRINT`, `TERPRI`,
`SPACES` do not require a feature flag.

### 8.10 Evaluation and application

| Function | Syntax | Description |
|----------|--------|-------------|
| `EVAL` | `(eval expr)` or `(eval expr env)` | Evaluate expression |
| `APPLY` | `(apply fn args-list)` | Call function with args from list |
| `FUNCALL` | `(funcall fn arg…)` | Call function with evaluated args |
| `EVLIS` | `(evlis lst env)` | Evaluate list of expressions |
| `EVCON` | `(evcon clauses env)` | Evaluate conditional clauses |
| `MACROEXPAND` | `(macroexpand form)` | Expand macro one step |

```lisp
(eval '(+ 1 2))                  ; => 3
(apply #'+ '(1 2 3))             ; => 6
(funcall #'+ 1 2 3)              ; => 6
(macroexpand '(when t (print 1))) ; => (IF T (PROGN (PRINT 1)) NIL)
```

### 8.11 Environment functions

| Function | Description |
|----------|-------------|
| `THE-ENVIRONMENT` | Return current lexical environment |
| `MAKE-ENVIRONMENT` | Create a new environment (optionally with parent) |
| `CURRENT-ENVIRONMENT` | Return hash table of all visible bindings |

### 8.12 Error handling

| Function | Syntax | Description |
|----------|--------|-------------|
| `ERROR` | `(error msg)` | Raise a runtime error |
| `ERRORSET` | `(errorset expr)` | Catch errors |

```lisp
(errorset (+ 1 2))          ; => (3)    — list wraps success value
(errorset (error "oops"))   ; => ()     — NIL on error
(errorset (/ 1 0))          ; => ()     — caught division by zero
```

### 8.13 Feature/capability management

| Function | Description |
|----------|-------------|
| `FEATURE-ENABLED-P` | Test if capability is enabled |
| `FEATURES` | List all enabled capabilities |

```lisp
(feature-enabled-p "SHELL")   ; => T if the host granted it
(features)                    ; => enabled capability names
```

### 8.14 Shell (requires `SHELL` capability)

```lisp
(shell "ls -la")
; => (0 "total 48\ndrwxr-xr-x …\n" "")
;     exit-code  stdout              stderr
```

See shell helpers in [section 9.7](#97-shell-helpers).

### 8.15 File I/O (capability-gated)

```lisp
(read-file "notes.txt")          ; requires READ-FS
(write-file "out.txt" "hello")   ; requires CREATE-FS
(make-temp-file "lamedh-")       ; requires TEMP-FS
```

---

## 9. Standard Library

All modules are embedded in the binary at compile time.  They are loaded in
numbered order by `Environment::with_stdlib`.

### 9.1 `00-core.lisp` — Core macros

**`DEFUN`** — the primary function definition macro:

```lisp
(defun name (param…) body…)
(defun name (param…) "docstring" body…)
```

Desugars to `(def name (lambda (param…) body…))` with docstring stored on the
symbol plist.

```lisp
(defun square (x)
  "Return X squared."
  (* x x))

(square 7)   ; => 49
(documentation 'square)   ; => "Return X squared."
```

**`PROG2`** — evaluate all args, return second:

```lisp
(prog2 expr1 expr2 expr3…)
```

**`CSET` / `CSETQ`** — aliases for `SETQ`.

### 9.2 `01-list.lisp` — List operations

| Function | Description |
|----------|-------------|
| `APPEND` | Concatenate two lists |
| `MEMBER` | Find first element `EQUAL` to key |
| `LENGTH` | Count elements |
| `REVERSE` | Reverse a list |
| `PAIRLIS` | Zip two lists into an alist |
| `NULL` | Test for NIL |
| `CONSP` | Test for cons cell |
| `LISTP` | Test for list (cons or nil) |
| `NCONC` | Alias for APPEND |
| `COPY` | Deep copy structure |
| `SASSOC` | Search alist with fallback thunk |
| `MAPC` | Map for side effects (returns original list) |
| `MAPCON` | Map over tails, concatenate results |

```lisp
(append '(a b) '(c d))           ; => (A B C D)
(member 'b '(a b c))             ; => (B C)
(length '(a b c d))              ; => 4
(reverse '(1 2 3))               ; => (3 2 1)
(pairlis '(a b) '(1 2))         ; => ((A . 1) (B . 2))
```

### 9.3 `02-cxr.lisp` — CAR/CDR compositions

All 30 compositions from `CAAR` to `CDDDDR`, generated via the `DEFCXR` macro.

```lisp
(cadr '(a b c))       ; => B
(caddr '(a b c))      ; => C
(caadr '((1 2) 3))    ; => 1
(cddr '(a b c d))     ; => (C D)
```

### 9.4 `03-meta.lisp` — Metaprogramming

**`DOCUMENTATION`** — retrieve the docstring of a symbol:

```lisp
(documentation 'car)       ; => "Return the first element…"
(documentation 'square)    ; => "Return X squared."
```

### 9.5 `04-predicates.lisp` — Predicates

**`EQUAL`** — structural (deep) equality:

```lisp
(equal '(1 2 3) '(1 2 3))   ; => T
(equal "hi" "hi")            ; => T
(equal 'a 'a)                ; => T (same as EQ for symbols)
```

### 9.6 `05-math.lisp` — Math utilities

| Function | Description |
|----------|-------------|
| `ONEP` | True if = 1 |
| `MINUSP` | True if < 0 |
| `ADD1` | n + 1 |
| `SUB1` | n − 1 |
| `MAX` | Maximum of arguments |
| `MIN` | Minimum of arguments |
| `ABS` | Absolute value |

### 9.7 Shell helpers (`07-shell.lisp`)

Requires `SHELL` feature enabled. Wraps the `SHELL` builtin:

| Function | Description |
|----------|-------------|
| `SHELL-EXIT-CODE` | Extract exit code from shell result |
| `SHELL-STDOUT` | Extract stdout string |
| `SHELL-STDERR` | Extract stderr string |
| `SHELL-OK-P` | True if exit code = 0 |
| `SH` | Run command; return stdout or signal error |

```lisp
; Requires the host or CLI to grant SHELL first.
(sh "echo hello")          ; => "hello\n"
(shell-ok-p (shell "ls"))  ; => T
```

### 9.8 Kernel vau forms (`08-vau.lisp`)

Kernel-style derived control forms:

| Form | Description |
|------|-------------|
| `$IF` | Vau-based if |
| `$AND` | Vau-based short-circuit and |
| `$OR` | Vau-based short-circuit or |
| `$SEQUENCE` | Vau-based sequence |

### 9.9 Lisp 1.5 Appendix A (`09-lisp15.lisp`)

Historical Lisp 1.5 functions:

| Function | Description |
|----------|-------------|
| `PAIR` | Zip two lists (like PAIRLIS without CONS) |
| `ATTRIB` | Append to property list |
| `PROP` | Search property list |
| `FLAG` | Set flag on list of symbols |
| `REMFLAG` | Remove flag from symbols |
| `MAP` | Apply to successive tails |
| `SEARCH` | Search with predicate |
| `RECIP` | `1 / x` as float |
| `SELECT` | Fexpr-based switch statement |
| `TRACE` / `UNTRACE` | Mark traced functions (stubs) |

### 9.10 Testing framework (`10-testing.lisp`)

```lisp
(deftest my-test
  (assert-equal (+ 1 2) 3)
  (assert-true  (> 5 0))
  (assert-false (= 1 2))
  (assert-nil   nil))

(run-tests)    ; => runs all registered tests, prints report
(clear-tests)  ; => clear the test registry
```

Assertion macros:

| Macro | Description |
|-------|-------------|
| `ASSERT-EQUAL` | Fail if values not EQUAL |
| `ASSERT-TRUE` | Fail if value is falsy |
| `ASSERT-FALSE` | Fail if value is truthy |
| `ASSERT-NIL` | Fail if value is not NIL |

### 9.11 Source optimizer (`11-optimizer-vau.lisp`)

Lisp-level optimizer passes that call the built-in `OPTIMIZE` constant folder:

```lisp
(optimize-form '(+ 1 2))          ; => 3
(optimize-form '(if t 42 99))     ; => 42
($opt (+ 1 2 3))                  ; evaluate with optimization
```

Passes applied:
- Dead-binding removal (pure init, zero uses)
- Atom inlining (single use, not mutated)
- `PROGN` flattening
- `IF` constant-condition detection

---

## 10. Environments and Scoping

### 10.1 Lexical scoping (default)

Functions capture their definition environment. A variable reference walks the
lexical parent chain:

```lisp
(def x 10)
(defun get-x () x)      ; x captured from definition scope
(let ((x 99))
  (get-x))              ; => 10  (lexical, not 99)
```

### 10.2 Dynamic (special) scoping

Variables declared with `DEFDYNAMIC` / `DEFVAR` are looked up on the call
stack:

```lisp
(defdynamic *indent* 0)

(defun show (msg)
  (princ (make-string *indent* #\space))
  (princ msg)
  (terpri))

(defun with-indent (body-fn)
  (let ((*indent* (+ *indent* 2)))
    (funcall body-fn)))

(show "top")
(with-indent (lambda () (show "indented")))
```

The `*earmuffs*` naming convention is a signal but not enforced — use
`DEFDYNAMIC` to register.

### 10.3 SETQ scoping

`SETQ` searches the lexical chain first, then the dynamic chain. If the variable
is not found anywhere, it is created in the current frame (supports interactive
top-level definitions).

### 10.4 First-class environments

```lisp
(def e (the-environment))
(eval '(+ x y) e)          ; evaluate in captured env
(make-environment)          ; fresh root env
(make-environment e)        ; child of e
```

### 10.5 Symbol interning

All symbols are stored once per name in the global `SymbolTable`. Two
occurrences of `FOO` share the same `Rc<RefCell<Symbol>>`.  `EQ` is therefore
O(1) pointer equality:

```lisp
(eq 'foo 'foo)    ; => T  (same Rc pointer)
```

---

## 11. Functions, Macros, Fexprs, and Vau

### Calling conventions summary

| Type | Args received | Body produces | Result |
|------|--------------|---------------|--------|
| Lambda | Evaluated | Any expression | Body value |
| Fexpr | Unevaluated list | Any expression | Body value directly |
| Macro | Unevaluated | A Lisp form | Form is evaluated |
| Vau | Unevaluated list + caller's env | Any expression | Body value directly |

### Lambda — lexical closure

```lisp
(lambda (x y) (+ x y))
(lambda (x &rest rest) (cons x rest))
```

Call creates a child environment whose lexical parent is the closure's captured
environment and whose dynamic parent is the caller's environment.

### Macro — code generator

```lisp
(defmacro swap! (a b)
  (let ((tmp (gensym)))
    `(let ((,tmp ,a))
       (setq ,a ,b)
       (setq ,b ,tmp))))

(def x 1) (def y 2)
(swap! x y)
x   ; => 2
y   ; => 1
```

Macro expansion happens at call time (not compile time). `MACROEXPAND` shows
the expansion:

```lisp
(macroexpand '(swap! x y))
; => (LET ((G0001 X)) (SETQ X Y) (SETQ Y G0001))
```

### Fexpr — value-returning special form

```lisp
(defexpr my-list (args) args)
(my-list a b c)   ; => (A B C)  — unevaluated symbols!

(defexpr my-and (args)
  (if (null args) t
      (if (eval (car args))
          (eval (cons 'my-and (cdr args)))
          nil)))
```

### Vau — full operative

```lisp
(def $define
  (vau (ops e)
    (eval (list 'def (car ops) (eval (cadr ops) e)) e)))

($define (answer) (* 6 7))
answer   ; => 42
```

---

## 12. PROG, GO, and RETURN

`PROG` provides labeled-statement control flow (Lisp 1.5 style).

```lisp
(prog (local-vars…)
  statement-or-label
  …)
```

- Atoms (symbols) in the body are **labels**.
- Non-atom forms are statements.
- `(GO label)` jumps to a label.
- `(RETURN value)` exits the PROG.
- Local variables are initialised to `NIL`.
- Returns `NIL` if control falls off the end.

```lisp
; Count to 10 with PROG/GO
(prog (i result)
  (setq i 1)
  (setq result 0)
  loop
  (if (> i 10) (return result))
  (setq result (+ result i))
  (setq i (+ i 1))
  (go loop))
; => 55
```

```lisp
; Early exit
(prog ()
  (print "before")
  (return 42)
  (print "never reached"))
; => 42
```

---

## 13. Loops: FOR and WHILE

### FOR — counted integer loop

```lisp
(for (var start end) body…)
(for (var start end step) body…)
```

- `start`, `end`, `step` must be integers.
- Range is **inclusive** on both ends.
- Negative `step` counts down.
- No per-iteration environment frame (efficient).
- Returns `NIL`.

```lisp
(def total 0)
(for (i 1 100)
  (setq total (+ total i)))
total   ; => 5050

; Sum of even numbers 0..20
(def sum 0)
(for (i 0 20 2)
  (setq sum (+ sum i)))
sum   ; => 110
```

### WHILE — conditional loop

```lisp
(while condition body…)
```

```lisp
(def n 10)
(def result 1)
(while (> n 0)
  (setq result (* result n))
  (setq n (sub1 n)))
result   ; => 3628800  (10!)
```

---

## 14. Hash Tables

Mutable key-value stores. Keys and values are arbitrary `LispVal`.

```lisp
(def h (make-hash-table))

(set-bang h 'x 42)          ; insert/update
(set-bang h "name" "Alice")

(gethash h 'x)              ; => 42
(get h 'x)                  ; same (GET = GETHASH for hash tables)

(keys h)                    ; => (X "name")  (order unspecified)

(delete-key-bang h 'x)      ; remove key
(gethash h 'x)              ; => ()  (NIL if not found)
```

`CURRENT-ENVIRONMENT` returns a hash table of all bindings visible in the
current environment.

---

## 15. Arrays

0-indexed mutable vectors (Lisp 1.5 style).

```lisp
(def a (array 5))           ; create array of length 5, all NIL

(store a 0 "zero")          ; set element
(store a 1 42)
(store a 2 '(a b c))

(fetch a 0)                 ; => "zero"
(fetch a 1)                 ; => 42

(array-length a)            ; => 5
(arrayp a)                  ; => T
```

---

## 16. Property Lists

Every interned symbol carries a **property list** (plist) — a `HashMap` of
string keys to `LispVal` values.

```lisp
(putp 'foo "color" "red")         ; set property
(getp 'foo "color")               ; => "red"
(remprop 'foo "color")            ; remove property
(plist 'foo)                      ; => (("color" . "red"))  all properties

; Docstrings are stored under "docstring"
(putp 'my-fn "docstring" "Does something.")
(documentation 'my-fn)            ; => "Does something."
```

**`DEFLIST`** — batch set a property on a list of symbols:

```lisp
(deflist '((foo value1) (bar value2)) 'myprop)
; equivalent to:
; (putp 'foo "MYPROP" value1)
; (putp 'bar "MYPROP" value2)
```

Lisp 1.5 flag operations (from `09-lisp15.lisp`):

```lisp
(flag '(foo bar baz) 'IMPORTANT)
; sets "IMPORTANT" property to T on foo, bar, baz

(remflag '(foo) 'IMPORTANT)
; removes "IMPORTANT" from foo
```

---

## 17. Structs

`DEFSTRUCT` auto-generates a complete struct interface backed by a hash table.

```lisp
(defstruct person name age email)

; Constructor (keyword arguments)
(def p (make-person :name "Alice" :age 30 :email "alice@example.com"))

; Type predicate
(person-p p)                  ; => T
(person-p "not a person")     ; => ()

; Accessors
(person-name p)               ; => "Alice"
(person-age p)                ; => 30

; Mutators
(set-person-age! p 31)
(person-age p)                ; => 31
```

Omitted fields default to `NIL`. The internal `__type__` key holds the struct
name as a symbol.

---

## 18. Error Handling

### Signalling errors

```lisp
(error "something went wrong")
(error (concat "bad value: " (prin1-to-string x)))
```

### Catching errors with ERRORSET

```lisp
(errorset expr)
; => (value)  on success — a list wrapping the value
; => ()       on any error — NIL
```

```lisp
(def result (errorset (/ 10 2)))
(if (null result)
    (print "error!")
    (print (car result)))     ; => 5

; Recover from a bad call
(def r (errorset (error "oops")))
(null r)   ; => T
```

### LispError variants (Rust)

| Variant | When |
|---------|------|
| `Generic(String)` | All user-visible runtime errors |
| `Return(LispVal)` | Internal: `(RETURN val)` signal from PROG |
| `Go(String)` | Internal: `(GO label)` signal from PROG |

`Return` and `Go` are control-flow signals, not true errors. They should not
escape a top-level `eval_str` call; if they do, it is a bug.

---

## 19. Condition Flags

Global boolean flags, distinct from feature flags. Used to pass state between
Lisp and host code, or to signal exceptional conditions (e.g. overflow).

```lisp
(set-flag "OVERFLOW")         ; set to true
(flag-set-p "OVERFLOW")       ; => T
(clear-flag "OVERFLOW")       ; set to false
(clear-all-flags)             ; clear all flags
```

The arithmetic system sets `"OVERFLOW"` when checked arithmetic overflows and
continues with wrapping semantics.

From Rust:

```rust
env.set_flag("DEBUG");
env.flag_set("DEBUG");    // → true
env.clear_flag("DEBUG");
```

---

## 20. Capabilities and Sandboxing

All potentially dangerous operations are gated behind **feature flags** that are
**off by default**. The host must opt in explicitly.

| Feature | Operations gated |
|---------|-----------------|
| `SHELL` | `(shell cmd)` — run subprocesses |
| `READ-FS` | `load-file`, `read-file`, metadata, and directory queries |
| `CREATE-FS` | file writes and filesystem mutation |
| `TEMP-FS` | temporary-file and temporary-directory creation |
| `IO` | `(read)` — read s-expression from stdin |

```lisp
; From Lisp, inspect capabilities:
(feature-enabled-p "SHELL")
(features)
```

```rust
// From Rust, grant capabilities explicitly:
env.enable_feature("SHELL");
env.enable_feature("READ-FS");
```

Because `SharedState` is shared across the whole environment chain, enabling a
feature anywhere enables it everywhere in that interpreter session.

Custom capabilities can be checked in host functions:

```rust
env.register_fn("my-op", |args, env| {
    if !env.feature_enabled("MY-FEATURE") {
        return Err(LispError::Generic("MY-FEATURE not enabled".into()));
    }
    // … safe to proceed
});
```

---

## 21. The Optimizer

### Built-in constant folder (Rust)

`(OPTIMIZE expr)` applies the Rust-level constant-folding pass:

```lisp
(optimize '(+ 1 2 3))             ; => 6
(optimize '(* 3 4))               ; => 12
(optimize '(+ x 0))               ; => X
(optimize '(* y 1))               ; => Y
(optimize '(if t 42 99))          ; => 42
(optimize '(if nil "a" "b"))      ; => "b"
(optimize '(progn 1 2 x))         ; => X  (pure 1, 2 dropped)
```

Transforms applied:
- Constant folding: `(+ 1 2)` → `3`
- Algebraic identities: `(+ x 0)` → `x`, `(* x 1)` → `x`, `(* x 0)` → `0`
- Branch elimination: `(if t a b)` → `a`, `(if nil a b)` → `b`
- PROGN pruning: pure non-final forms are dropped; `(progn x)` → `x`

Not applied: expressions inside fexpr/vau operands, macro expansion, any
transform with side effects.

### Lisp-level passes (`11-optimizer-vau.lisp`)

`(OPTIMIZE-FORM expr)` applies all Lisp-level passes then calls the Rust folder:

```lisp
(optimize-form '(let ((x 1)) (+ x 0)))
; dead-binding removal + algebraic identity → 1

($opt expr)    ; evaluate expr after optimization
```

Passes:
1. Dead-binding removal — pure binding, zero references
2. Atom inlining — single reference, not mutated
3. PROGN flattening
4. IF constant-condition detection

---

## 22. Testing Framework

```lisp
; Define a test
(deftest addition-works
  (assert-equal (+ 1 2) 3)
  (assert-equal (+ 0 0) 0)
  (assert-true  (> (+ 5 5) 9)))

(deftest list-operations
  (assert-equal (car '(1 2 3)) 1)
  (assert-equal (length '(a b c)) 3)
  (assert-nil   (cdr '(x))))

; Run all registered tests
(run-tests)
; Output:
;   Running 2 tests...
;   addition-works ... OK
;   list-operations ... OK
;   2/2 tests passed.

; Clear test registry
(clear-tests)
```

Assertion macros:

| Macro | Fails when |
|-------|-----------|
| `(assert-equal a b)` | `(not (equal a b))` |
| `(assert-true x)` | `x` is falsy |
| `(assert-false x)` | `x` is truthy |
| `(assert-nil x)` | `x` is not `NIL` |

---

## 23. The Help System

The help system is implemented entirely in Lisp (`98-help-system.lisp`,
`99-help-data.lisp`). Documentation is registered via `register-doc` calls in
`99-help-data.lisp`.

```lisp
(help)                          ; overview and category list
(help 'car)                     ; detailed help for CAR
(help 'categories)              ; list all categories
(help 'category 'lists)         ; all functions in the "lists" category
(documentation 'append)         ; raw docstring string
```

### Extending the help system

```lisp
(register-doc 'my-function
  (list
    (cons 'NAME        'my-function)
    (cons 'TYPE        'function)
    (cons 'SYNTAX      "(my-function arg)")
    (cons 'DESCRIPTION "Does something useful.")
    (cons 'EXAMPLES    '(((my-function 1) 2)))
    (cons 'SEE-ALSO    '(other-function))))
```

---

## 24. Embedding Lamedh in Rust

### Cargo dependency

```toml
[dependencies]
lamedh = { path = "/path/to/lamedh" }
# or, when published to crates.io:
# lamedh = "0.2"
```

### Minimal embedding

```rust
use lamedh::{eval_str, LispVal, environment::Environment};

fn main() {
    // LispVal/Environment are !Send — create them inside with_large_stack.
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();

        let val = eval_str("(+ 1 2 3)", &env).unwrap();
        assert_eq!(val, LispVal::Number(6));

        println!("{}", lamedh::printer::print(&val));
    });
}
```

### Stack size

The tree-walking evaluator uses large stack frames. Run inside
`lamedh::with_large_stack` (spawns a 512 MiB thread) to avoid stack overflow
before the recursion-depth guard fires.

Alternatives:
- Lower the limit: `lamedh::set_eval_depth_limit(1000)`
- Ensure your own thread has a large stack

### Creating environments

```rust
// Builtins only — no defun, list utilities, etc.
let env = Environment::new_with_builtins();

// Builtins + embedded standard library (recommended)
let env = Environment::with_stdlib();

// Explicitly sandboxed (same as new_with_builtins, communicates intent)
let env = Environment::new_sandboxed();
```

### Evaluating Lisp

```rust
use lamedh::{eval_str, eval_all};

// Single expression
let val: LispVal = eval_str("(+ 1 2)", &env)?;

// Multiple top-level forms
let vals: Vec<LispVal> = eval_all("(def x 1) (+ x 2)", &env)?;

// Load from a file (requires READ-FS if reached through Lisp code)
lamedh::load_file("mylib.lisp", &env)?;

// Load a directory of .lisp files
lamedh::load_directory("mylib/", &env)?;
```

### Value conversion

**Rust → Lisp** (`From<T>` impls):

```rust
LispVal::from(42i64)         // Number(42)
LispVal::from(3.14f64)       // Float(3.14)
LispVal::from(true)          // Symbol("T")
LispVal::from(false)         // Nil
LispVal::from("hello")       // String("hello")
LispVal::from("hello".to_string())

// Build a proper list from an iterator
LispVal::list([1i64, 2, 3])  // (1 2 3)
```

**Lisp → Rust** (`TryFrom<LispVal>` impls):

```rust
let n: i64   = i64::try_from(val)?;
let f: f64   = f64::try_from(val)?;    // also coerces Number
let b: bool  = bool::try_from(val)?;   // Nil → false
let s: String = String::try_from(val)?;
let v: Vec<LispVal> = Vec::try_from(val)?;  // proper list only
```

**Helper methods on `LispVal`**:

```rust
val.as_number()?     // → i64
val.as_float()?      // → f64 (coerces Number)
val.as_str_val()?    // → &str
val.as_list_vec()?   // → Vec<LispVal>
val.is_truthy()      // → bool (only Nil is false)
```

### Registering host functions

```rust
env.register_fn("add-one", |args, _env| {
    if args.len() != 1 {
        return Err(LispError::Generic("add-one: expected 1 arg".into()));
    }
    let n = args[0].as_number()?;
    Ok(LispVal::from(n + 1))
});

// Now callable from Lisp:
// (add-one 41)  => 42
```

The name is uppercased automatically.  The function receives evaluated arguments.

### Enabling capabilities from Rust

```rust
env.enable_feature("SHELL");
env.enable_feature("READ-FS");
env.enable_feature("CREATE-FS");
env.enable_feature("TEMP-FS");
env.enable_feature("IO");

// Check from Rust
if env.feature_enabled("SHELL") { /* … */ }

// Revoke
env.disable_feature("SHELL");
```

### Host-defined extension values

```rust
use lamedh::{LispVal, LispValExtension, LispError};
use std::hash::Hasher;

#[derive(Debug)]
struct MyPoint { x: f64, y: f64 }

impl LispValExtension for MyPoint {
    fn type_name(&self) -> &str { "point" }
    fn display(&self) -> String { format!("#<point {},{}>", self.x, self.y) }
    fn eq_ext(&self, other: &dyn LispValExtension) -> bool {
        other.as_any().downcast_ref::<MyPoint>()
            .map_or(false, |p| p.x == self.x && p.y == self.y)
    }
    fn hash_ext(&self, state: &mut dyn Hasher) {
        use std::hash::Hash;
        self.x.to_bits().hash(state);
        self.y.to_bits().hash(state);
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// Wrap for Lisp
let pt = LispVal::ext(MyPoint { x: 1.0, y: 2.0 });

// Retrieve in a host function
env.register_fn("point-x", |args, _env| {
    if let LispVal::Extension(e) = &args[0] {
        if let Some(p) = e.as_any().downcast_ref::<MyPoint>() {
            return Ok(LispVal::from(p.x));
        }
    }
    Err(LispError::Generic("expected point".into()))
});
```

### Error handling in Rust

```rust
match eval_str("(/ 1 0)", &env) {
    Ok(val)                    => { /* use val */ }
    Err(LispError::Generic(m)) => eprintln!("Lisp error: {m}"),
    Err(e)                     => eprintln!("internal: {e}"),
}
```

`LispError::Return` and `LispError::Go` are PROG control-flow signals and must
not escape a top-level `eval_str` call under normal use.

---

## 25. Cargo Workspace and Dependencies

### Root `Cargo.toml`

```toml
[package]
name        = "lamedh"
version     = "0.2.0"
edition     = "2024"
description = "An embeddable Lisp 1.5 interpreter written in Rust"
license     = "AGPL-3.0"

[lib]
name = "lamedh"
path = "src/lib.rs"

[dependencies]
nom = "=7.1.3"   # pinned parser dependency

# Native-code backend for the typed JIT. Enabled by default in 0.2.x.
cranelift-jit      = { version = "0.133", optional = true }
cranelift-module   = { version = "0.133", optional = true }
cranelift-codegen  = { version = "0.133", optional = true }
cranelift-frontend = { version = "0.133", optional = true }

[features]
default = ["jit"]
jit = [
    "dep:cranelift-jit",
    "dep:cranelift-module",
    "dep:cranelift-codegen",
    "dep:cranelift-frontend",
]

[workspace]
members         = ["cli"]
default-members = [".", "cli"]
exclude = [
    "benchmarks/fibonacci/rust",
    "benchmarks/loops/rust",
    "benchmarks/levenshtein/rust",
]
```

The library crate's dependency-light build uses `nom` for the reader:
`cargo build --no-default-features`. The default 0.2.x build also enables the
typed JIT's Cranelift backend.

### `cli/Cargo.toml`

```toml
[package]
name        = "lamedh-cli"
version     = "0.2.0"
edition     = "2024"
description = "REPL and command-line driver for the Lamedh Lisp 1.5 interpreter"

[[bin]]
name = "lamedh"
path = "src/main.rs"
doc = false

[dependencies]
lamedh    = { path = ".." }
rustyline = "14.0.0"
clap      = { version = "4.5.4", features = ["derive"] }
```

### Dependency summary

| Crate | Used by | Purpose |
|-------|---------|---------|
| `nom 7.1.3` | `lamedh` (lib) | Parser combinators for the reader |
| `cranelift-* 0.133` | `lamedh` default `jit` feature | Native-code backend for typed functions |
| `rustyline 14.0.0` | `lamedh-cli` | REPL line-editing and history |
| `clap 4.5.4` | `lamedh-cli` | Command-line argument parsing |

---

## 26. Architecture Reference

### Module pipeline

```
Source text
    │
    ▼
reader.rs ──── nom-based s-expression parser
                Interns symbols into SymbolTable during parse
    │
    ▼
evaluator.rs ── Tree-walking eval with TCO trampoline
                Special forms + 100+ builtins + user callables
    │   ▲
    │   └── environment.rs
    │         SymbolTable (interning), lexical chain, dynamic chain,
    │         SharedState (features, flags, dynamic var registry)
    │
    ▼
printer.rs ──── LispVal → String (PRIN1 semantics)

optimizer.rs ── Constant-folding source rewriter (pure, no eval)
lib.rs ─────── Public API: LispVal, LispError, eval_str, eval_all,
                load_file, load_directory, with_large_stack
```

### LispVal size

Large variants (`Lambda`, `Fexpr`, `Macro`, `Vau`) are `Box`-wrapped to keep
the enum size small. This matters because `LispVal` is cloned constantly during
evaluation.

### TCO trampoline

```
eval(expr, env)
  └── acquire DepthGuard  ← recoverable error if limit exceeded
       └── eval_impl loop
             eval_step(expr, env) → TcoStep::Done(val)
                                 → TcoStep::TailCall(new_expr, new_env)
                                   loop continues, no Rust frame consumed
```

Tail calls in `IF`, `COND`, `PROGN`, `LET`, `LET*`, `AND`, `OR`, and function
bodies all return `TcoStep::TailCall` and execute without growing the Rust stack.

### Environment structure

```rust
Environment {
    parent: Option<Rc<Environment>>,         // lexical parent
    bindings: Rc<RefCell<BindingMap>>,       // FxHash local frame
    shared: Rc<SharedState>,                 // one Rc across all frames
    dynamic_parent: Option<Rc<Environment>>, // call-stack parent
}

SharedState {
    symbols: SymbolTable,                    // global symbol interning
    condition_flags: HashMap<String, bool>,  // OVERFLOW etc.
    dynamic_vars: HashSet<String>,           // registered specials
    has_dynamic: Cell<bool>,                 // fast-path flag
    features: HashSet<String>,               // enabled capabilities
}
```

`SharedState` is shared across all environments in a session via a single `Rc`
clone — creating a child frame is one refcount bump, not four.

### Symbol lookup performance

| Path | Cost |
|------|------|
| Global symbol read (hot path) | O(1): read symbol value cell directly from interned `Rc` |
| Local frame lookup | O(depth): walk `BindingMap` (FxHash) per frame |
| Dynamic variable lookup | O(dynamic chain depth) |
| `EQ` comparison | O(1): `Rc::ptr_eq` |
| `EQUAL` comparison | O(tree size) |

---

## Appendix A: Differences from Lisp 1.5

| Feature | Lisp 1.5 | Lamedh |
|---------|----------|--------|
| Scoping | Dynamic | **Lexical** default; dynamic opt-in |
| Integers | Fixed-point | `i64` (64-bit signed) |
| Strings | Limited | Full UTF-8 |
| Hash tables | No | Yes (`make-hash-table`) |
| `RPLACA`/`RPLACD` | Destructive | **Non-destructive** (return new cell) |
| `CONS` sharing | Copy | `Rc` sharing (O(1) clone) |
| Macros | No | Yes (`defmacro`) |
| Fexprs | Yes (`FEXPR`) | Yes (`defexpr`) |
| Vau | No | Yes (Kernel-style operative) |
| TCO | No | Yes (trampolined) |
| Garbage collection | Mark-sweep | `Rc` reference counting |
| Tail calls | Stack overflow | Recovered (trampoline) |
| Recursion depth | Stack-limited | Guarded (default 10,000) |
| Array indexing | 1-based | 0-based |
| Floating point | Separate type | Promotes on mixed arithmetic |

---

## Appendix B: Known Limitations

- **No tail call in `COND` predicate position**: only the last expression of a
  matching clause is TCO.
- **No splicing in quasiquote** (`@,list`): unquote-splicing is not yet
  implemented in the reader/evaluator (use `append` instead).
- **No continuations**: `CALL/CC` is not available.
- **No tail-recursive `PROG`**: GO/RETURN use `LispError` signals through the
  Rust stack, not trampolining.
- **Single-threaded**: `LispVal`/`Environment` use `Rc` (not `Arc`); the
  interpreter is not `Send`.
- **No incremental GC**: reference counting with `Rc` means circular structures
  leak memory.
- **`TRACE`/`UNTRACE`** are defined but are stubs (no actual tracing output).
- **No `READ` from files**: `READ` reads stdin only; there is no
  `WITH-INPUT-FROM-FILE`.

---

## Appendix C: Quick Reference Card

### Defining things

```lisp
(def x 42)                            ; global variable
(def x 42 "docstring")               ; with documentation
(defun f (x y) (+ x y))              ; function
(defun f (x y) "doc" (+ x y))        ; with docstring
(defmacro m (a &rest b) `(,a ,@b))   ; macro
(defexpr e (args) (car args))         ; fexpr
(defdynamic *var* 0)                  ; dynamic variable
(defstruct point x y)                 ; struct
```

### Calling things

```lisp
(f 1 2)                    ; regular call
(funcall fn 1 2)           ; call via value
(apply fn '(1 2))          ; call with args as list
(eval '(+ 1 2))            ; evaluate form
```

### Control flow

```lisp
(if test then else)
(cond (t1 e1) (t2 e2) (t default))
(and a b c)
(or  a b c)
(not x)
(progn e1 e2 e3)           ; sequence
(let  ((x 1) (y 2)) body)  ; parallel binding
(let* ((x 1) (y (+ x 1))) body) ; sequential binding
(for  (i 1 10) body)       ; counted loop
(while cond body)          ; conditional loop
(prog (x) label (go label) (return x)) ; labeled block
```

### Lists

```lisp
(cons 'a '(b c))      ; (A B C)
(car '(a b c))        ; A
(cdr '(a b c))        ; (B C)
(cadr '(a b c))       ; B
(list 1 2 3)          ; (1 2 3)
(length '(a b c))     ; 3
(reverse '(1 2 3))    ; (3 2 1)
(append '(a) '(b))    ; (A B)
(mapcar #'add1 '(1 2 3)) ; (2 3 4)
(assoc 'b '((a 1)(b 2))) ; (B 2)
```

### Predicates

```lisp
(null x)     (atom x)    (consp x)    (listp x)
(numberp x)  (fixp x)    (floatp x)   (stringp x)
(symbolp x)  (functionp x) (macrop x)
(zerop n)    (plusp n)   (minusp n)   (evenp n)  (oddp n)
(eq a b)     (equal a b) (not x)      (boundp 'x)
```

### Quoting and quasiquoting

```lisp
'foo            ; (quote foo)
`(a ,b ,c)      ; (quasiquote (a (unquote b) (unquote c)))
#'fn            ; (function fn)
```

### Error handling

```lisp
(error "message")
(errorset expr)    ; => (val) or ()
```

### Hash tables

```lisp
(make-hash-table)
(set-bang h key val)
(gethash h key)
(keys h)
(delete-key-bang h key)
```

### Arrays

```lisp
(array n)          ; create
(store a i val)    ; set
(fetch a i)        ; get
(array-length a)   ; length
```

### Property lists

```lisp
(putp 'sym "key" val)
(getp 'sym "key")
(remprop 'sym "key")
(plist 'sym)
```

### Capabilities

```lisp
(feature-enabled-p "SHELL")
(feature-enabled-p "READ-FS")
(features)
(shell "ls")
(load-file "path.lisp")
(read)
```

---

*Lamedh — where ancient wisdom meets modern implementation.*  
*Source: <https://github.com/pnathan/lamedh>*
