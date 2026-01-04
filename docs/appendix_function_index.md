# Appendix A: Complete Function Index

An alphabetical index of all functions and macros in Lamedh.

---

## Symbols

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `+` | Builtin | Addition | [Arithmetic](functions/arithmetic.md) |
| `-` | Builtin | Subtraction/negation | [Arithmetic](functions/arithmetic.md) |
| `*` | Builtin | Multiplication | [Arithmetic](functions/arithmetic.md) |
| `/` | Builtin | Division | [Arithmetic](functions/arithmetic.md) |
| `<` | Builtin | Less than | [Predicates](functions/predicates.md) |
| `>` | Builtin | Greater than | [Predicates](functions/predicates.md) |
| `=` | Builtin | Numeric equality | [Predicates](functions/predicates.md) |
| `1+` | Builtin | Increment | [Arithmetic](functions/arithmetic.md) |
| `1-` | Builtin | Decrement | [Arithmetic](functions/arithmetic.md) |

---

## A

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `ABS` | Stdlib | Absolute value | [Standard Library](standard_library.md) |
| `ADD1` | Builtin | Add one | [Arithmetic](functions/arithmetic.md) |
| `APPEND` | Stdlib | Concatenate lists | [Standard Library](standard_library.md) |
| `APPLY` | Builtin | Apply function to list | [Meta](functions/meta.md) |
| `ASH` | Builtin | Arithmetic shift | [Bitwise](functions/bitwise.md) |
| `ASSOC` | Builtin | Association list lookup | [Lists](functions/lists.md) |
| `ATOM` | Builtin | Test if not cons | [Predicates](functions/predicates.md) |

---

## B

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `BOUNDP` | Builtin | Test if bound | [Predicates](functions/predicates.md) |

---

## C

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `CAAR`...`CDDDDR` | Stdlib | CAR/CDR compositions | [Standard Library](standard_library.md) |
| `CAR` | Builtin | First element | [Lists](functions/lists.md) |
| `CDR` | Builtin | Rest of list | [Lists](functions/lists.md) |
| `CLEAR-ALL-FLAGS` | Builtin | Clear symbol flags | [Plists](functions/plists.md) |
| `CLEAR-FLAG` | Builtin | Clear a flag | [Plists](functions/plists.md) |
| `CONCAT` | Builtin | Concatenate strings | [Strings](functions/strings.md) |
| `CONS` | Builtin | Create cons cell | [Lists](functions/lists.md) |
| `CONSP` | Stdlib | Test if cons | [Predicates](functions/predicates.md) |
| `CURRENT-ENVIRONMENT` | Builtin | Get environment | [Hash Tables](functions/hash_tables.md) |

---

## D

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `DEFLIST` | Builtin | Set properties on symbols | [Plists](functions/plists.md) |
| `DELETE` | Builtin | Remove from list | [Lists](functions/lists.md) |
| `DELETE-KEY` | Builtin | Remove hash key | [Hash Tables](functions/hash_tables.md) |
| `DIFFERENCE` | Builtin | Subtraction | [Arithmetic](functions/arithmetic.md) |
| `DIVIDE` | Builtin | Division | [Arithmetic](functions/arithmetic.md) |
| `DOCUMENTATION` | Stdlib | Get docstring | [Standard Library](standard_library.md) |

---

## E

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `EFFACE` | Builtin | Remove first occurrence | [Lists](functions/lists.md) |
| `EQ` | Builtin | Identity test | [Predicates](functions/predicates.md) |
| `EQUAL` | Stdlib | Structural equality | [Standard Library](standard_library.md) |
| `ERROR` | Builtin | Raise error | [Errors](functions/errors.md) |
| `ERRORSET` | Builtin | Catch errors | [Errors](functions/errors.md) |
| `EVAL` | Builtin | Evaluate expression | [Meta](functions/meta.md) |
| `EVENP` | Builtin | Test if even | [Predicates](functions/predicates.md) |
| `EXPLODE` | Builtin | Symbol to char list | [Strings](functions/strings.md) |
| `EXPT` | Builtin | Exponentiation | [Arithmetic](functions/arithmetic.md) |

---

## F

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `FIXP` | Builtin | Test if integer | [Predicates](functions/predicates.md) |
| `FLAG-SET-P` | Builtin | Test if flag set | [Plists](functions/plists.md) |
| `FLOAT-EQUAL` | Builtin | Float equality | [Arithmetic](functions/arithmetic.md) |
| `FLOAT-GREATERP` | Builtin | Float greater | [Arithmetic](functions/arithmetic.md) |
| `FLOAT-LESSP` | Builtin | Float less than | [Arithmetic](functions/arithmetic.md) |
| `FLOATP` | Builtin | Test if float | [Predicates](functions/predicates.md) |
| `FUNCALL` | Builtin | Call function | [Meta](functions/meta.md) |
| `FUNCTIONP` | Builtin | Test if function | [Predicates](functions/predicates.md) |

---

## G

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `GENSYM` | Builtin | Generate symbol | [Strings](functions/strings.md) |
| `GET` | Builtin | Hash table get | [Hash Tables](functions/hash_tables.md) |
| `GETP` | Builtin | Get property | [Plists](functions/plists.md) |
| `GREATERP` | Builtin | Greater than | [Predicates](functions/predicates.md) |

---

## I

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `IMPLODE` | Builtin | Char list to symbol | [Strings](functions/strings.md) |
| `INDEX` | Builtin | String character access | [Strings](functions/strings.md) |
| `INTERN` | Builtin | Intern string as symbol | [Strings](functions/strings.md) |

---

## K

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `KEYS` | Builtin | Hash table keys | [Hash Tables](functions/hash_tables.md) |

---

## L

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `LAST` | Builtin | Last cons cell | [Lists](functions/lists.md) |
| `LEFTSHIFT` | Builtin | Bit shift | [Bitwise](functions/bitwise.md) |
| `LENGTH` | Stdlib | List length | [Standard Library](standard_library.md) |
| `LESSP` | Builtin | Less than | [Predicates](functions/predicates.md) |
| `LIST` | Builtin | Create list | [Lists](functions/lists.md) |
| `LISTP` | Stdlib | Test if list | [Predicates](functions/predicates.md) |
| `LOAD-FILE` | Builtin | Load Lisp file | [I/O](functions/io.md) |
| `LOGAND` | Builtin | Bitwise AND | [Bitwise](functions/bitwise.md) |
| `LOGNOT` | Builtin | Bitwise NOT | [Bitwise](functions/bitwise.md) |
| `LOGOR` | Builtin | Bitwise OR | [Bitwise](functions/bitwise.md) |
| `LOGXOR` | Builtin | Bitwise XOR | [Bitwise](functions/bitwise.md) |

---

## M

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `MACROEXPAND` | Builtin | Expand macro | [Meta](functions/meta.md) |
| `MACROP` | Builtin | Test if macro | [Predicates](functions/predicates.md) |
| `MAKE-HASH-TABLE` | Builtin | Create hash table | [Hash Tables](functions/hash_tables.md) |
| `MAKNAM` | Builtin | Same as IMPLODE | [Strings](functions/strings.md) |
| `MAPCAR` | Builtin | Map over elements | [Lists](functions/lists.md) |
| `MAPLIST` | Builtin | Map over tails | [Lists](functions/lists.md) |
| `MAX` | Stdlib | Maximum | [Standard Library](standard_library.md) |
| `MEMBER` | Stdlib | Find in list | [Standard Library](standard_library.md) |
| `MIN` | Stdlib | Minimum | [Standard Library](standard_library.md) |
| `MINUS` | Builtin | Subtraction | [Arithmetic](functions/arithmetic.md) |
| `MINUSP` | Stdlib | Test if negative | [Standard Library](standard_library.md) |
| `MOD` | Builtin | Modulo | [Arithmetic](functions/arithmetic.md) |

---

## N

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `NOT` | Builtin | Logical not | [Predicates](functions/predicates.md) |
| `NTH` | Builtin | Nth element | [Lists](functions/lists.md) |
| `NTHCDR` | Builtin | Nth cdr | [Lists](functions/lists.md) |
| `NULL` | Stdlib | Test if nil | [Predicates](functions/predicates.md) |
| `NUMBERP` | Builtin | Test if number | [Predicates](functions/predicates.md) |

---

## O

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `ODDP` | Builtin | Test if odd | [Predicates](functions/predicates.md) |
| `ONEP` | Stdlib | Test if one | [Standard Library](standard_library.md) |

---

## P

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `PAIRLIS` | Stdlib | Make alist | [Standard Library](standard_library.md) |
| `PLIST` | Builtin | Get property list | [Plists](functions/plists.md) |
| `PLUS` | Builtin | Addition | [Arithmetic](functions/arithmetic.md) |
| `PLUSP` | Builtin | Test if positive | [Predicates](functions/predicates.md) |
| `PRIN1` | Builtin | Print readable | [I/O](functions/io.md) |
| `PRINC` | Builtin | Print for humans | [I/O](functions/io.md) |
| `PRINT` | Builtin | Print | [I/O](functions/io.md) |
| `PUT` | Builtin | Set property (alias) | [Plists](functions/plists.md) |
| `PUTP` | Builtin | Set property | [Plists](functions/plists.md) |

---

## Q

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `QUOTIENT` | Builtin | Division | [Arithmetic](functions/arithmetic.md) |

---

## R

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `RANDOM` | Builtin | Random number | [Arithmetic](functions/arithmetic.md) |
| `READ` | Builtin | Read input | [I/O](functions/io.md) |
| `REMAINDER` | Builtin | Remainder | [Arithmetic](functions/arithmetic.md) |
| `REMPROP` | Builtin | Remove property | [Plists](functions/plists.md) |
| `REVERSE` | Stdlib | Reverse list | [Standard Library](standard_library.md) |
| `ROT` | Builtin | Rotate bits | [Bitwise](functions/bitwise.md) |
| `RPLACA` | Builtin | Replace car | [Lists](functions/lists.md) |
| `RPLACD` | Builtin | Replace cdr | [Lists](functions/lists.md) |

---

## S

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `SET-BANG` | Builtin | Hash table set | [Hash Tables](functions/hash_tables.md) |
| `SET-FLAG` | Builtin | Set a flag | [Plists](functions/plists.md) |
| `STRINGP` | Builtin | Test if string | [Predicates](functions/predicates.md) |
| `SUB1` | Builtin | Subtract one | [Arithmetic](functions/arithmetic.md) |
| `SUBST` | Builtin | Substitute in tree | [Lists](functions/lists.md) |
| `SYMBOLP` | Builtin | Test if symbol | [Predicates](functions/predicates.md) |

---

## T

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `TERPRI` | Builtin | Print newline | [I/O](functions/io.md) |
| `TIMES` | Builtin | Multiplication | [Arithmetic](functions/arithmetic.md) |

---

## Z

| Function | Type | Description | Reference |
|----------|------|-------------|-----------|
| `ZEROP` | Builtin | Test if zero | [Predicates](functions/predicates.md) |

---

## Count by Type

| Type | Count |
|------|-------|
| Builtin Functions | ~75 |
| Standard Library | ~15 |
| CxR Functions | ~30 |

---

## See Also

- [Special Forms Index](appendix_special_forms_index.md)
- [Standard Library](standard_library.md)
