# Lamedh Reference Manual

**A Lisp 1.5 Implementation in Rust**

---

## Interactive Help

From the REPL, use the built-in help system:

```lisp
(help)                  ; Show help overview
(help 'car)             ; Help for specific function
(help 'categories)      ; List all categories
(help 'category 'lists) ; Show functions in category
(documentation 'defun)  ; Get docstring
```

The help system reads from the Lisp documentation database in `lib/99-help-data.lisp`.

---

## Table of Contents

### Part I: Introduction
1. [Introduction](introduction.md) - Overview of Lamedh, history, and goals
2. [Getting Started](getting_started.md) - Installation, REPL usage, and first programs

### Part II: The Language
3. [Data Types](data_types.md) - Atoms, numbers, strings, symbols, cons cells, and lists
4. [Syntax and Evaluation](syntax.md) - S-expressions, evaluation rules, quoting
5. [Environments and Scope](environments.md) - Lexical scoping, symbol interning, property lists

### Part III: Special Forms
6. [Special Forms](special_forms.md) - Complete reference for all special forms

### Part IV: Functions Dictionary

**[Complete Function Reference](generated-reference.md)** - Auto-generated from `lib/99-help-data.lisp`

Individual category pages (hand-written with additional context):
- [Arithmetic Functions](functions/arithmetic.md) - `+`, `-`, `*`, `/`, `EXPT`, `REMAINDER`, etc.
- [List Functions](functions/lists.md) - `CAR`, `CDR`, `CONS`, `APPEND`, `MAPCAR`, etc.
- [Predicates](functions/predicates.md) - Type and value predicates
- [String Functions](functions/strings.md) - `CONCAT`, `INDEX`, `EXPLODE`, `IMPLODE`
- [I/O Functions](functions/io.md) - `READ`, `PRINT`, `PRIN1`, `PRINC`, `LOAD-FILE`
- [Hash Table Functions](functions/hash_tables.md) - `MAKE-HASH-TABLE`, `GETHASH`, `SET-BANG`, `SETHASH`
- [Property List Functions](functions/plists.md) - `GETP`, `PUTP`, `REMPROP`, `PLIST`
- [Bitwise Functions](functions/bitwise.md) - `LOGOR`, `LOGAND`, `LOGXOR`, `LEFTSHIFT`
- [Error Handling](functions/errors.md) - `ERROR`, `ERRORSET`
- [Metaprogramming](functions/meta.md) - `EVAL`, `APPLY`, `FUNCALL`, `MACROEXPAND`

### Part V: Standard Library
7. [Standard Library](standard_library.md) - Functions defined in `lib/`

### Part VI: Appendices
- [Appendix A: Generated Function Index](generated-function-index.md) - Alphabetical listing
- [Appendix B: Complete Special Form Index](appendix_special_forms_index.md)
- [Appendix C: Known Limitations](appendix_limitations.md)
- [Appendix D: Differences from Lisp 1.5](appendix_differences.md)
- [Appendix E: Divergences from Common Lisp](cl-divergences.md) - One page for CL reflexes
- [Roadmap To 1.0](roadmap_1_0.md) - Release gates while the version remains on `0.2.x`

---

## Quick Reference

### Notation Conventions

Throughout this manual:

- `symbol` - A Lisp symbol (shown in lowercase in examples, but case-insensitive)
- `SYMBOL` - Built-in function or special form name
- `expression` - Any valid Lisp expression
- `&rest args` - Zero or more arguments
- `&optional arg` - Optional argument
- `=> result` - Shows the return value

### Example Format

```lisp
(+ 1 2 3)           ; Add numbers
=> 6

(car '(a b c))      ; First element of list
=> A

(defun square (x)   ; Define a function
  (* x x))
=> SQUARE
```

---

## About This Manual

This reference manual documents **Lamedh** (Hebrew letter "Lamed", ל), a Lisp 1.5 implementation written in Rust. Lamedh aims to provide a faithful implementation of classic Lisp 1.5 semantics with modern extensions.

The manual is organized in the style of the [Common Lisp HyperSpec](http://www.lispworks.com/documentation/HyperSpec/Front/), providing comprehensive documentation for all language features.

### Version

This documentation covers Lamedh 0.2.x as of June 2026.

---

## Documentation Architecture

The Lamedh documentation system has two components:

1. **Lisp-native help database** (`lib/99-help-data.lisp`)
   - All function/form documentation stored as s-expressions
   - Queryable via `(help 'symbol)` in the REPL
   - Can be extended by adding more `register-doc` calls

2. **Markdown reference manual** (this `docs/` directory)
   - Hand-written conceptual documentation
   - Generated reference from the Lisp database
   - Run `./scripts/generate-docs.sh` to regenerate

To add documentation for a new function:

```lisp
(register-doc 'my-function
  (list
    (cons 'NAME 'my-function)
    (cons 'TYPE 'function)
    (cons 'SYNTAX "(my-function arg)")
    (cons 'DESCRIPTION "Does something useful.")
    (cons 'EXAMPLES '(((my-function 1) 2)))
    (cons 'SEE-ALSO '(other-function))))
```

---

*Lamedh - Where ancient wisdom meets modern implementation*
