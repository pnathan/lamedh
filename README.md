# Lithhp
A Lisp 1.5 implementation in Rust.

## Features

### Core Language
- **Read-Eval-Print Loop (REPL)** with rustyline
- **S-expression syntax** with reader macros: `'` (quote), `` ` `` (quasiquote), `,` (unquote)
- **Lexically scoped environments** with symbol interning
- **Property lists** for symbol metadata

### Data Types
- Numbers (64-bit integers and floats)
- Strings
- Symbols (interned)
- Cons cells and lists
- Hash tables
- Functions (lambdas, fexprs, macros)

### Special Forms
- **Control flow**: `IF`, `COND`, `AND`, `OR`, `PROG` (with `GO`/`RETURN`)
- **Functions**: `LAMBDA`, `DEFUN`, `LABEL`, `FUNCTION`
- **Macros**: `DEFMACRO`, `DEFEXPR` (fexprs)
- **Variables**: `DEF`, `SETQ`, `LET`
- **Quoting**: `QUOTE`, `QUASIQUOTE`/`UNQUOTE`

### Built-in Functions

**Arithmetic**: `+`, `-`, `*`, `/`, `REMAINDER`, `EXPT`, `<`, `>`, `ZEROP`

**Lists**: `CAR`, `CDR`, `CONS`, `SUBST`, `ASSOC`, `MAPLIST`, `MAPCAR`, `RPLACA`, `RPLACD`

**Strings**: `CONCAT`, `INDEX`

**I/O**: `READ`, `PRIN1`, `PRINC`, `TERPRI`, `PRINT`

**Error Handling**: `ERROR`, `ERRORSET`

**Type Predicates**: `ATOM`, `NUMBERP`, `FIXP`, `FLOATP`, `STRINGP`

**Bitwise**: `LOGOR`, `LOGAND`, `LOGXOR`, `LEFTSHIFT`

**Property Lists**: `GETP`, `PUTP`, `REMPROP`, `DEFLIST`

**Meta**: `EVAL`, `APPLY`, `LOAD-FILE`

See [Language Reference](docs/language_reference.md) for complete documentation. 
