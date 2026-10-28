# Chapter 4: Syntax and Evaluation

This chapter describes how Lamedh parses and evaluates expressions.

---

## 4.1 S-Expressions

All Lamedh code is represented as **S-expressions** (symbolic expressions). An S-expression is either:

1. An **atom** (number, string, symbol, or NIL)
2. A **cons cell** (pair of S-expressions)

### 4.1.1 Atoms

```lisp
42                  ; Number
3.14                ; Float
"hello"             ; String
foo                 ; Symbol
:key                ; Keyword symbol
nil                 ; NIL
t                   ; T (true)
```

### 4.1.2 Lists

Lists are chains of cons cells ending in NIL:

```lisp
(a b c)             ; A list of three symbols
(1 2 3)             ; A list of numbers
((a b) (c d))       ; Nested lists
()                  ; Empty list (same as NIL)
```

### 4.1.3 Dotted Pairs

The dot notation shows explicit cons cells:

```lisp
(a . b)             ; A cons with car=A, cdr=B
(a b . c)           ; Improper list: (A . (B . C))
```

---

## 4.2 Reader Syntax

### 4.2.1 Comments

Comments begin with semicolon and extend to end of line:

```lisp
; This is a comment
(+ 1 2)  ; This is also a comment
```

### 4.2.2 Quote

Single quote is shorthand for the `QUOTE` special form:

```lisp
'foo        ; Same as (QUOTE FOO)
'(a b c)    ; Same as (QUOTE (A B C))
```

### 4.2.3 Quasiquote

Backquote enables template expressions with selective evaluation:

```lisp
`(a b c)           ; Same as (QUASIQUOTE (A B C))
`(a ,x c)          ; Evaluate X, insert result
`(a ,@lst c)       ; Evaluate LST, splice its elements in place
```

### 4.2.4 Unquote and unquote-splicing

Comma inside a quasiquote evaluates its argument and inserts the single result.
Comma-at (`,@`) evaluates its argument — which must yield a list — and splices
that list's elements into the surrounding list:

```lisp
(def x 42)
`(the answer is ,x)    ; => (THE ANSWER IS 42)

(def xs '(2 3 4))
`(1 ,@xs 5)            ; => (1 2 3 4 5)
`(start ,@'() end)     ; => (START END)
```

---

## 4.3 Evaluation Rules

### 4.3.1 Self-Evaluating Forms

These evaluate to themselves:

| Type | Example | Result |
|------|---------|--------|
| Numbers | `42` | `42` |
| Floats | `3.14` | `3.14` |
| Strings | `"hello"` | `"hello"` |
| Keywords | `:key` | `:KEY` |
| NIL | `nil` | `NIL` |
| T | `t` | `T` |

### 4.3.2 Symbol Evaluation

Symbols evaluate to their bound value:

```lisp
(def x 42)
x          ; => 42

y          ; Error: Unbound variable Y
```

Keyword symbols begin with `:` and evaluate to themselves:

```lisp
:op        ; => :OP
(list :op 'eqv)
; => (:OP EQV)
```

### 4.3.3 List Evaluation

Lists are evaluated as function calls:

1. The first element (the **operator**) is evaluated
2. The remaining elements (the **arguments**) are evaluated left-to-right
3. The operator is applied to the arguments

```lisp
(+ 1 2 3)
; 1. Evaluate +  => <builtin-plus>
; 2. Evaluate 1  => 1
; 3. Evaluate 2  => 2
; 4. Evaluate 3  => 3
; 5. Apply <builtin-plus> to (1 2 3)
; => 6
```

### 4.3.4 Special Form Evaluation

Special forms have their own evaluation rules. Arguments may not be evaluated:

```lisp
(if (> 3 2) "yes" "no")
; Evaluate (> 3 2) => T
; Since T, evaluate "yes" only
; => "yes"
; "no" is never evaluated

(quote (+ 1 2))
; Do not evaluate (+ 1 2)
; => (+ 1 2)
```

---

## 4.4 Quoting

### 4.4.1 QUOTE

Prevents evaluation and returns the expression as data:

```lisp
(quote foo)         ; => FOO
(quote (+ 1 2))     ; => (+ 1 2)
'foo                ; => FOO (shorthand)
'(a b c)            ; => (A B C)
```

### 4.4.2 QUASIQUOTE and UNQUOTE

Quasiquote creates templates with holes filled by unquote:

```lisp
(def name "Alice")
(def age 30)

`(person ,name ,age)
; => (PERSON "Alice" 30)
```

Useful for macro definitions:

```lisp
(defmacro when (test &rest body)
  `(if ,test (progn ,@body) nil))
```

---

## 4.5 The Reader

The reader converts text into S-expressions.

### 4.5.1 Tokenization

Input is broken into tokens:

- Numbers: `42`, `3.14`, `-17`
- Strings: `"hello"`
- Symbols: `foo`, `+`, `my-var`
- Delimiters: `(`, `)`, `.`
- Quote marks: `'`, `` ` ``, `,`

### 4.5.2 Case Conversion

All symbols are converted to **uppercase** during reading:

```lisp
foo     ; Read as FOO
Foo     ; Read as FOO
FOO     ; Read as FOO
```

### 4.5.3 Number Parsing

Numbers are parsed in this order:
1. Try to parse as integer
2. Try to parse as float
3. Otherwise, treat as symbol

```lisp
42      ; Integer
3.14    ; Float
1e10    ; Float
+       ; Symbol (not a number)
```

---

## 4.6 The Evaluator

### 4.6.1 Evaluation Context

Each evaluation occurs in an **environment** that maps symbols to values:

```lisp
; Global environment
(def x 10)
(def y 20)

; New environment created by LET
(let ((x 100))
  (+ x y))    ; x=100, y=20 (from global)
; => 120
```

### 4.6.2 Function Application

When applying a function:

1. **Builtins**: Call the Rust implementation directly
2. **Lambdas**: Create new environment with parameter bindings, evaluate body
3. **Fexprs**: Pass unevaluated arguments, evaluate body
4. **Macros**: Expand form, then evaluate result

```lisp
;; Lambda application
((lambda (x y) (+ x y)) 3 4)
; 1. Evaluate arguments: 3 => 3, 4 => 4
; 2. Bind x=3, y=4 in new environment
; 3. Evaluate (+ x y) in new environment
; => 7
```

### 4.6.3 Tail Position

Lamedh implements tail-call optimization with an evaluator trampoline for known
tail positions: function bodies, `IF` branches, matching `COND` consequents,
`PROGN`, `LET`, `LET*`, and related special forms. Proper tail-recursive loops
can run without growing the Rust stack; non-tail recursion still consumes stack
and is guarded by the evaluator recursion limit.

---

## 4.7 Order of Evaluation

### 4.7.1 Left-to-Right

Arguments are evaluated left-to-right:

```lisp
(list (print 1) (print 2) (print 3))
; Prints: 1, then 2, then 3
; => (NIL NIL NIL)
```

### 4.7.2 Short-Circuit Evaluation

`AND` and `OR` evaluate only as needed:

```lisp
(and nil (error "never reached"))
; => NIL

(or t (error "never reached"))
; => T
```

---

## 4.8 Implicit PROGN

Many special forms have an implicit `PROGN` for multiple body expressions:

```lisp
(lambda (x)
  (print x)
  (print "done")
  (* x x))

(defun foo (x)
  (print "computing")
  (* x 2))

(let ((x 1))
  (print x)
  (+ x 1))
```

---

## 4.9 Error Handling

### 4.9.1 Errors

Errors terminate normal evaluation:

```lisp
(car 42)
;; Error: CAR requires a cons cell or NIL
```

### 4.9.2 ERRORSET

Catch errors and continue:

```lisp
(errorset '(car 42))   ; => NIL (error caught)
(errorset '(+ 1 2))    ; => (3) (success)
```

---

## 4.10 Summary of Evaluation Rules

| Form | Evaluation |
|------|------------|
| Number | Returns itself |
| String | Returns itself |
| Symbol | Look up value in environment |
| `NIL`/`T` | Returns itself |
| `(quote x)` | Returns `x` unevaluated |
| `(if t c a)` | Evaluate condition, then branch |
| `(fn args...)` | Evaluate fn and args, apply |
| Special form | Per-form evaluation rules |

---

**Next:** [Environments and Scope](environments.md)
