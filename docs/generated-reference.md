# Lamedh Reference Manual

Auto-generated from Lisp documentation database.

---

## Categories

- BITWISE - Bitwise operations
- HASH-TABLES - Hash tables
- PLISTS - Property lists
- META - Metaprogramming
- ERRORS - Error handling
- IO - Input/Output
- SPECIAL-FORMS - Special forms and macros
- STRINGS - String operations
- LISTS - List manipulation
- PREDICATES - Type and value predicates
- ARITHMETIC - Numeric operations

---

# BITWISE Functions

Bitwise operations

---

### LOGOR

**Type:** `FUNCTION`

**Syntax:** `(logor integer...)`

Bitwise OR of all arguments.

**Examples:**
```lisp
(LOGOR 5 3)  ; => 7
```

**See also:** LOGAND, LOGXOR, LOGNOT

---

### LOGAND

**Type:** `FUNCTION`

**Syntax:** `(logand integer...)`

Bitwise AND of all arguments.

**Examples:**
```lisp
(LOGAND 5 3)  ; => 1
```

**See also:** LOGOR, LOGXOR, LOGNOT

---

### LOGXOR

**Type:** `FUNCTION`

**Syntax:** `(logxor integer...)`

Bitwise XOR of all arguments.

**Examples:**
```lisp
(LOGXOR 5 3)  ; => 6
```

**See also:** LOGOR, LOGAND, LOGNOT

---

### LOGNOT

**Type:** `FUNCTION`

**Syntax:** `(lognot integer)`

Bitwise complement (NOT).

**Examples:**
```lisp
(LOGNOT 0)  ; => -1
```

**See also:** LOGOR, LOGAND, LOGXOR

---

### LEFTSHIFT

**Type:** `FUNCTION`

**Syntax:** `(leftshift n count)`

Shifts bits left (positive count) or right (negative count).

**Examples:**
```lisp
(LEFTSHIFT 1 3)  ; => 8
(LEFTSHIFT 8 -2)  ; => 2
```

**See also:** ASH, LOGOR, LOGAND

---

# HASH-TABLES Functions

Hash tables

---

### MAKE-HASH-TABLE

**Type:** `FUNCTION`

**Syntax:** `(make-hash-table)`

Creates and returns a new empty hash table.

**See also:** GET, SET-BANG, KEYS

---

### GET

**Type:** `FUNCTION`

**Syntax:** `(get hash-table key)`

Retrieves the value for key in hash-table.

**See also:** SET-BANG, KEYS, MAKE-HASH-TABLE

---

### SET-BANG

**Type:** `FUNCTION`

**Syntax:** `(set-bang hash-table key value)`

Sets the value for key in hash-table.

**See also:** GET, DELETE-KEY, MAKE-HASH-TABLE

---

### KEYS

**Type:** `FUNCTION`

**Syntax:** `(keys hash-table)`

Returns a list of all keys in hash-table.

**See also:** GET, SET-BANG, MAKE-HASH-TABLE

---

# PLISTS Functions

Property lists

---

### GETP

**Type:** `FUNCTION`

**Syntax:** `(getp symbol indicator)`

Retrieves a property from a symbol's property list.

**See also:** PUTP, REMPROP, PLIST

---

### PUTP

**Type:** `FUNCTION`

**Syntax:** `(putp symbol indicator value)`

Sets a property on a symbol's property list.

**See also:** GETP, REMPROP, PLIST

---

### PLIST

**Type:** `FUNCTION`

**Syntax:** `(plist symbol)`

Returns the entire property list of a symbol.

**See also:** GETP, PUTP

---

### DOCUMENTATION

**Type:** `FUNCTION`

**Syntax:** `(documentation symbol)`

Returns the docstring for a symbol.

**See also:** GETP, HELP

---

# META Functions

Metaprogramming

---

### EVAL

**Type:** `FUNCTION`

**Syntax:** `(eval expression)`

Evaluates an expression.

**Examples:**
```lisp
(EVAL (QUOTE (+ 1 2)))  ; => 3
```

**See also:** APPLY, FUNCALL, QUOTE

---

### APPLY

**Type:** `FUNCTION`

**Syntax:** `(apply function args)`

Applies function to a list of arguments.

**Examples:**
```lisp
(APPLY (QUOTE +) (QUOTE (1 2 3)))  ; => 6
```

**See also:** EVAL, FUNCALL, MAPCAR

---

### FUNCALL

**Type:** `FUNCTION`

**Syntax:** `(funcall function arg...)`

Calls function with the given arguments.

**Examples:**
```lisp
(FUNCALL (QUOTE +) 1 2 3)  ; => 6
```

**See also:** APPLY, EVAL

---

### HELP

**Type:** `FUNCTION`

**Syntax:** `(help) or (help 'symbol) or (help :categories)`

Interactive help system. Use (help) for overview, (help 'symbol) for specific help.

**See also:** DOCUMENTATION, APROPOS

---

### DOCUMENTATION

**Type:** `FUNCTION`

**Syntax:** `(documentation symbol)`

Returns the docstring for a symbol.

**See also:** GETP, HELP

---

# ERRORS Functions

Error handling

---

### ERROR

**Type:** `FUNCTION`

**Syntax:** `(error message)`

Raises an error with the given message.

**See also:** ERRORSET

---

### ERRORSET

**Type:** `FUNCTION`

**Syntax:** `(errorset form)`

Evaluates form, catching errors. Returns (result) on success, NIL on error.

**Examples:**
```lisp
(ERRORSET (QUOTE (+ 1 2)))  ; => (3)
(ERRORSET (QUOTE (/ 1 0)))  ; => ()
```

**See also:** ERROR

---

# IO Functions

Input/Output

---

### PRINT

**Type:** `FUNCTION`

**Syntax:** `(print object...)`

Prints objects to standard output.

**Returns:** NIL

**See also:** PRIN1, PRINC, TERPRI

---

### PRIN1

**Type:** `FUNCTION`

**Syntax:** `(prin1 object)`

Prints object in readable form (strings with quotes).

**Returns:** The object printed

**Examples:**
```lisp
(PRIN1 HELLO)  ; => HELLO
```

**See also:** PRINC, PRINT

---

### PRINC

**Type:** `FUNCTION`

**Syntax:** `(princ object)`

Prints object without escaping (strings without quotes).

**Returns:** The object printed

**See also:** PRIN1, PRINT

---

### TERPRI

**Type:** `FUNCTION`

**Syntax:** `(terpri)`

Prints a newline character.

**Returns:** NIL

**See also:** PRINT, PRINC

---

### READ

**Type:** `FUNCTION`

**Syntax:** `(read)`

Reads one S-expression from standard input.

**Returns:** Parsed S-expression

**See also:** EVAL, LOAD-FILE

---

### LOAD-FILE

**Type:** `FUNCTION`

**Syntax:** `(load-file filename)`

Loads and evaluates a Lisp source file.

**Arguments:**
- `FILENAME` - String path to file

**Returns:** T on success

**See also:** READ, EVAL

---

# SPECIAL-FORMS Functions

Special forms and macros

---

### QUOTE

**Type:** `SPECIAL-FORM`

**Syntax:** `(quote expression) or 'expression`

Prevents evaluation and returns expression as data.

**Examples:**
```lisp
(QUOTE (+ 1 2))  ; => (+ 1 2)
(QUOTE FOO)  ; => FOO
```

**See also:** QUASIQUOTE, EVAL

---

### IF

**Type:** `SPECIAL-FORM`

**Syntax:** `(if condition then-form else-form)`

Evaluates condition; if non-NIL, evaluates then-form, otherwise else-form.

**Examples:**
```lisp
(IF T "yes" "no")  ; => "yes"
(IF () "yes" "no")  ; => "no"
```

**See also:** COND, AND, OR

---

### COND

**Type:** `SPECIAL-FORM`

**Syntax:** `(cond (test form...)...)`

Multi-way conditional. Evaluates tests until one is true, then evaluates its forms.

**Examples:**
```lisp
(COND ((= 1 2) "a") (T "b"))  ; => "b"
```

**See also:** IF, AND, OR

---

### AND

**Type:** `SPECIAL-FORM`

**Syntax:** `(and form...)`

Short-circuit AND. Returns first NIL or last value.

**Examples:**
```lisp
(AND T T T)  ; => T
(AND T () T)  ; => ()
(AND 1 2 3)  ; => 3
```

**See also:** OR, NOT, IF

---

### OR

**Type:** `SPECIAL-FORM`

**Syntax:** `(or form...)`

Short-circuit OR. Returns first non-NIL value or NIL.

**Examples:**
```lisp
(OR () () T)  ; => T
(OR 1 2 3)  ; => 1
```

**See also:** AND, NOT, IF

---

### DEF

**Type:** `SPECIAL-FORM`

**Syntax:** `(def symbol value &optional docstring)`

Binds symbol to value in the current environment.

**Examples:**
```lisp
(DEF X 42)  ; => X
```

**See also:** SETQ, LET, DEFUN

---

### SETQ

**Type:** `SPECIAL-FORM`

**Syntax:** `(setq symbol value)`

Assigns a new value to an existing variable.

**See also:** DEF, LET

---

### LET

**Type:** `SPECIAL-FORM`

**Syntax:** `(let ((var val)...) body...)`

Creates local variable bindings for the duration of body.

**Examples:**
```lisp
(LET ((X 1) (Y 2)) (+ X Y))  ; => 3
```

**See also:** DEF, LAMBDA, PROG

---

### LAMBDA

**Type:** `SPECIAL-FORM`

**Syntax:** `(lambda (params...) body...)`

Creates an anonymous function (closure).

**Examples:**
```lisp
((LAMBDA (X) (* X X)) 5)  ; => 25
```

**See also:** DEFUN, FUNCTION, APPLY

---

### DEFUN

**Type:** `MACRO`

**Syntax:** `(defun name (params...) &optional docstring body...)`

Defines a named function with optional docstring.

**See also:** LAMBDA, DEF, DEFMACRO

---

### DEFMACRO

**Type:** `SPECIAL-FORM`

**Syntax:** `(defmacro name (params...) body...)`

Defines a macro that transforms code before evaluation.

**See also:** DEFUN, DEFEXPR, MACROEXPAND

---

### PROGN

**Type:** `SPECIAL-FORM`

**Syntax:** `(progn form...)`

Evaluates forms in sequence, returns last value.

**Examples:**
```lisp
(PROGN (+ 1 2) (* 3 4))  ; => 12
```

**See also:** PROG, LET

---

### PROG

**Type:** `SPECIAL-FORM`

**Syntax:** `(prog (vars...) statements...)`

Imperative block with local variables and labels for GO/RETURN.

**See also:** GO, RETURN, PROGN, LET

---

# STRINGS Functions

String operations

---

### CONCAT

**Type:** `FUNCTION`

**Syntax:** `(concat string...)`

Concatenates all string arguments.

**Examples:**
```lisp
(CONCAT "Hello" " " "World")  ; => "Hello World"
```

**See also:** INDEX, EXPLODE

---

### INDEX

**Type:** `FUNCTION`

**Syntax:** `(index string n)`

Returns the character at position n (0-indexed) as a string.

**Examples:**
```lisp
(INDEX "hello" 0)  ; => "h"
(INDEX "hello" 4)  ; => "o"
```

**See also:** CONCAT, EXPLODE

---

### EXPLODE

**Type:** `FUNCTION`

**Syntax:** `(explode atom)`

Converts an atom to a list of single-character symbols.

**Examples:**
```lisp
(EXPLODE (QUOTE HELLO))  ; => (H E L L O)
```

**See also:** IMPLODE, INTERN

---

### IMPLODE

**Type:** `FUNCTION`

**Syntax:** `(implode char-list)`

Converts a list of character symbols to an interned symbol.

**Examples:**
```lisp
(IMPLODE (QUOTE (H E L L O)))  ; => HELLO
```

**See also:** EXPLODE, INTERN, GENSYM

---

### GENSYM

**Type:** `FUNCTION`

**Syntax:** `(gensym)`

Generates a unique uninterned symbol.

**Returns:** Unique symbol like G0001

**See also:** INTERN, IMPLODE

---

### INTERN

**Type:** `FUNCTION`

**Syntax:** `(intern string)`

Interns a string as a symbol in the global symbol table.

**Examples:**
```lisp
(INTERN "HELLO")  ; => HELLO
```

**See also:** IMPLODE, GENSYM

---

# LISTS Functions

List manipulation

---

### CAR

**Type:** `FUNCTION`

**Syntax:** `(car list)`

Returns the first element of a list (the car of a cons cell).

**Arguments:**
- `LIST` - A cons cell or NIL

**Returns:** First element, or NIL for empty list

**Examples:**
```lisp
(CAR (QUOTE (A B C)))  ; => A
(CAR ())  ; => ()
```

**See also:** CDR, CONS, CADR, CADDR

---

### CDR

**Type:** `FUNCTION`

**Syntax:** `(cdr list)`

Returns the rest of a list (the cdr of a cons cell).

**Arguments:**
- `LIST` - A cons cell or NIL

**Returns:** Rest of list, or NIL

**Examples:**
```lisp
(CDR (QUOTE (A B C)))  ; => (B C)
(CDR (QUOTE (A)))  ; => ()
```

**See also:** CAR, CONS, CDDR

---

### CONS

**Type:** `FUNCTION`

**Syntax:** `(cons car cdr)`

Creates a new cons cell with the given car and cdr.

**Arguments:**
- `CAR` - First element
- `CDR` - Rest (usually a list)

**Returns:** New cons cell

**Examples:**
```lisp
(CONS (QUOTE A) (QUOTE (B C)))  ; => (A B C)
(CONS (QUOTE A) (QUOTE B))  ; => (A . B)
```

**See also:** CAR, CDR, LIST

---

### LIST

**Type:** `FUNCTION`

**Syntax:** `(list item...)`

Creates a list from its arguments.

**Examples:**
```lisp
(LIST 1 2 3)  ; => (1 2 3)
(LIST)  ; => ()
```

**See also:** CONS, APPEND

---

### APPEND

**Type:** `FUNCTION`

**Syntax:** `(append list1 list2)`

Concatenates two lists.

**Examples:**
```lisp
(APPEND (QUOTE (A B)) (QUOTE (C D)))  ; => (A B C D)
```

**See also:** CONS, LIST, REVERSE

---

### REVERSE

**Type:** `FUNCTION`

**Syntax:** `(reverse list)`

Returns a list with elements in reverse order.

**Examples:**
```lisp
(REVERSE (QUOTE (A B C)))  ; => (C B A)
```

**See also:** APPEND

---

### LENGTH

**Type:** `FUNCTION`

**Syntax:** `(length list)`

Returns the number of elements in a list.

**Examples:**
```lisp
(LENGTH (QUOTE (A B C)))  ; => 3
(LENGTH ())  ; => 0
```

**See also:** NULL

---

### NTH

**Type:** `FUNCTION`

**Syntax:** `(nth n list)`

Returns the nth element of a list (0-indexed).

**Examples:**
```lisp
(NTH 0 (QUOTE (A B C)))  ; => A
(NTH 2 (QUOTE (A B C)))  ; => C
```

**See also:** NTHCDR, CAR, CADR

---

### LAST

**Type:** `FUNCTION`

**Syntax:** `(last list)`

Returns the last cons cell of a list.

**Examples:**
```lisp
(LAST (QUOTE (A B C)))  ; => (C)
```

**See also:** CAR, CDR, NTH

---

### MEMBER

**Type:** `FUNCTION`

**Syntax:** `(member item list)`

Searches for item in list using EQUAL. Returns tail starting at match.

**Examples:**
```lisp
(MEMBER (QUOTE B) (QUOTE (A B C)))  ; => (B C)
(MEMBER (QUOTE X) (QUOTE (A B C)))  ; => ()
```

**See also:** ASSOC, EQUAL

---

### ASSOC

**Type:** `FUNCTION`

**Syntax:** `(assoc key alist)`

Searches an association list for a pair with matching key.

**Examples:**
```lisp
(ASSOC (QUOTE B) (QUOTE ((A . 1) (B . 2))))  ; => (B . 2)
```

**See also:** MEMBER, PAIRLIS

---

### MAPCAR

**Type:** `FUNCTION`

**Syntax:** `(mapcar list function)`

Applies function to each element of list, returns list of results.

**Examples:**
```lisp
(MAPCAR (QUOTE (1 2 3)) (LAMBDA (X) (* X 2)))  ; => (2 4 6)
```

**See also:** MAPLIST, APPLY

---

### MAPLIST

**Type:** `FUNCTION`

**Syntax:** `(maplist list function)`

Applies function to successive tails of list.

**Examples:**
```lisp
(MAPLIST (QUOTE (A B C)) (LAMBDA (X) (LENGTH X)))  ; => (3 2 1)
```

**See also:** MAPCAR

---

### SUBST

**Type:** `FUNCTION`

**Syntax:** `(subst new old tree)`

Replaces all occurrences of old with new in tree.

**Examples:**
```lisp
(SUBST (QUOTE X) (QUOTE A) (QUOTE (A B A)))  ; => (X B X)
```

---

# PREDICATES Functions

Type and value predicates

---

### ZEROP

**Type:** `FUNCTION`

**Syntax:** `(zerop n)`

Returns T if n is zero.

**Examples:**
```lisp
(ZEROP 0)  ; => T
(ZEROP 1)  ; => ()
```

**See also:** PLUSP, MINUSP, ONEP

---

### PLUSP

**Type:** `FUNCTION`

**Syntax:** `(plusp n)`

Returns T if n is positive (greater than zero).

**Examples:**
```lisp
(PLUSP 1)  ; => T
(PLUSP 0)  ; => ()
```

**See also:** MINUSP, ZEROP

---

### MINUSP

**Type:** `FUNCTION`

**Syntax:** `(minusp n)`

Returns T if n is negative (less than zero).

**Examples:**
```lisp
(MINUSP -1)  ; => T
(MINUSP 0)  ; => ()
```

**See also:** PLUSP, ZEROP, ABS

---

### EVENP

**Type:** `FUNCTION`

**Syntax:** `(evenp n)`

Returns T if n is an even integer.

**Examples:**
```lisp
(EVENP 2)  ; => T
(EVENP 3)  ; => ()
```

**See also:** ODDP

---

### ODDP

**Type:** `FUNCTION`

**Syntax:** `(oddp n)`

Returns T if n is an odd integer.

**Examples:**
```lisp
(ODDP 3)  ; => T
(ODDP 2)  ; => ()
```

**See also:** EVENP

---

### <

**Type:** `FUNCTION`

**Syntax:** `(< a b)`

Returns T if a is less than b.

**Examples:**
```lisp
(< 1 2)  ; => T
(< 2 1)  ; => ()
```

**See also:** >, =, LESSP, GREATERP

---

### >

**Type:** `FUNCTION`

**Syntax:** `(> a b)`

Returns T if a is greater than b.

**Examples:**
```lisp
(> 2 1)  ; => T
(> 1 2)  ; => ()
```

**See also:** <, =, LESSP, GREATERP

---

### =

**Type:** `FUNCTION`

**Syntax:** `(= a b)`

Returns T if a and b are numerically equal.

**Examples:**
```lisp
(= 1 1)  ; => T
(= 1 1)  ; => T
(= 1 2)  ; => ()
```

**See also:** EQ, EQUAL

---

### ATOM

**Type:** `FUNCTION`

**Syntax:** `(atom x)`

Returns T if x is not a cons cell (i.e., is an atom).

**Examples:**
```lisp
(ATOM (QUOTE A))  ; => T
(ATOM 42)  ; => T
(ATOM (QUOTE (A B)))  ; => ()
```

**See also:** CONSP, LISTP, SYMBOLP

---

### SYMBOLP

**Type:** `FUNCTION`

**Syntax:** `(symbolp x)`

Returns T if x is a symbol.

**Examples:**
```lisp
(SYMBOLP (QUOTE FOO))  ; => T
(SYMBOLP ())  ; => T
(SYMBOLP 42)  ; => ()
```

**See also:** ATOM, NUMBERP, STRINGP

---

### NUMBERP

**Type:** `FUNCTION`

**Syntax:** `(numberp x)`

Returns T if x is a number (integer or float).

**Examples:**
```lisp
(NUMBERP 42)  ; => T
(NUMBERP 3.14)  ; => T
(NUMBERP (QUOTE A))  ; => ()
```

**See also:** FIXP, FLOATP

---

### FIXP

**Type:** `FUNCTION`

**Syntax:** `(fixp x)`

Returns T if x is a fixed-point (integer) number.

**Examples:**
```lisp
(FIXP 42)  ; => T
(FIXP 3.14)  ; => ()
```

**See also:** FLOATP, NUMBERP

---

### FLOATP

**Type:** `FUNCTION`

**Syntax:** `(floatp x)`

Returns T if x is a floating-point number.

**Examples:**
```lisp
(FLOATP 3.14)  ; => T
(FLOATP 42)  ; => ()
```

**See also:** FIXP, NUMBERP

---

### STRINGP

**Type:** `FUNCTION`

**Syntax:** `(stringp x)`

Returns T if x is a string.

**Examples:**
```lisp
(STRINGP "hello")  ; => T
(STRINGP (QUOTE HELLO))  ; => ()
```

**See also:** SYMBOLP, ATOM

---

### CONSP

**Type:** `FUNCTION`

**Syntax:** `(consp x)`

Returns T if x is a cons cell.

**Examples:**
```lisp
(CONSP (QUOTE (A B)))  ; => T
(CONSP ())  ; => ()
```

**See also:** ATOM, LISTP, NULL

---

### LISTP

**Type:** `FUNCTION`

**Syntax:** `(listp x)`

Returns T if x is a list (cons or NIL).

**Examples:**
```lisp
(LISTP (QUOTE (A B)))  ; => T
(LISTP ())  ; => T
(LISTP (QUOTE A))  ; => ()
```

**See also:** CONSP, NULL, ATOM

---

### NULL

**Type:** `FUNCTION`

**Syntax:** `(null x)`

Returns T if x is NIL.

**Examples:**
```lisp
(NULL ())  ; => T
(NULL (QUOTE ()))  ; => T
(NULL (QUOTE (A)))  ; => ()
```

**See also:** NOT, LISTP

---

### NOT

**Type:** `FUNCTION`

**Syntax:** `(not x)`

Returns T if x is NIL, NIL otherwise.

**Examples:**
```lisp
(NOT ())  ; => T
(NOT T)  ; => ()
```

**See also:** NULL, AND, OR

---

### EQ

**Type:** `FUNCTION`

**Syntax:** `(eq a b)`

Returns T if a and b are the same object (identity test).

**Examples:**
```lisp
(EQ (QUOTE A) (QUOTE A))  ; => T
(EQ (QUOTE (1)) (QUOTE (1)))  ; => ()
```

**See also:** EQUAL, =

---

### EQUAL

**Type:** `FUNCTION`

**Syntax:** `(equal a b)`

Returns T if a and b are structurally equivalent (recursive comparison).

**Examples:**
```lisp
(EQUAL (QUOTE (A B)) (QUOTE (A B)))  ; => T
(EQUAL "hi" "hi")  ; => T
```

**See also:** EQ, =

---

### FUNCTIONP

**Type:** `FUNCTION`

**Syntax:** `(functionp x)`

Returns T if x is a function (lambda, fexpr, or builtin).

**Examples:**
```lisp
(FUNCTIONP (LAMBDA (X) X))  ; => T
```

**See also:** MACROP, BOUNDP

---

### BOUNDP

**Type:** `FUNCTION`

**Syntax:** `(boundp symbol)`

Returns T if symbol has a value binding.

**Examples:**
```lisp
(BOUNDP (QUOTE CAR))  ; => T
```

**See also:** SYMBOLP

---

# ARITHMETIC Functions

Numeric operations

---

### +

**Type:** `FUNCTION`

**Syntax:** `(+ number...)`

Returns the sum of all arguments. With no arguments, returns 0.

**Arguments:**
- `NUMBERS` - Zero or more numbers to add

**Returns:** Sum of arguments (float if any argument is float)

**Examples:**
```lisp
(+ 1 2 3)  ; => 6
(+ 1.5 2.5)  ; => 4
(+)  ; => 0
```

**See also:** -, *, /

---

### -

**Type:** `FUNCTION`

**Syntax:** `(- number) or (- number number...)`

With one argument, returns negation. With multiple, subtracts rest from first.

**Arguments:**
- `NUMBER` - One or more numbers

**Returns:** Difference or negation

**Examples:**
```lisp
(- 5)  ; => -5
(- 10 3)  ; => 7
(- 10 3 2)  ; => 5
```

**See also:** +, *, /

---

### *

**Type:** `FUNCTION`

**Syntax:** `(* number...)`

Returns the product of all arguments. With no arguments, returns 1.

**Arguments:**
- `NUMBERS` - Zero or more numbers to multiply

**Returns:** Product of arguments

**Examples:**
```lisp
(* 2 3 4)  ; => 24
(*)  ; => 1
```

**See also:** +, -, /, EXPT

---

### /

**Type:** `FUNCTION`

**Syntax:** `(/ dividend divisor)`

Returns the quotient of two numbers. Integer division truncates toward zero.

**Arguments:**
- `DIVIDEND` - Number to divide
- `DIVISOR` - Number to divide by (non-zero)

**Returns:** Quotient

**Examples:**
```lisp
(/ 10 2)  ; => 5
(/ 10 3)  ; => 3
(/ 10 3)  ; => 3.333333
```

**See also:** REMAINDER, MOD, *, -

---

### REMAINDER

**Type:** `FUNCTION`

**Syntax:** `(remainder dividend divisor)`

Returns the remainder of integer division.

**Examples:**
```lisp
(REMAINDER 10 3)  ; => 1
(REMAINDER -10 3)  ; => -1
```

**See also:** MOD, /

---

### MOD

**Type:** `FUNCTION`

**Syntax:** `(mod x y)`

Returns x modulo y. Result has same sign as divisor.

**Examples:**
```lisp
(MOD 10 3)  ; => 1
(MOD -10 3)  ; => 2
```

**See also:** REMAINDER, /

---

### EXPT

**Type:** `FUNCTION`

**Syntax:** `(expt base power)`

Returns base raised to the power.

**Examples:**
```lisp
(EXPT 2 10)  ; => 1024
(EXPT 3 3)  ; => 27
```

**See also:** *, /

---

### ADD1

**Type:** `FUNCTION`

**Syntax:** `(add1 n)`

Returns n + 1. Same as (1+ n).

**Examples:**
```lisp
(ADD1 5)  ; => 6
```

**See also:** SUB1, +, -

---

### SUB1

**Type:** `FUNCTION`

**Syntax:** `(sub1 n)`

Returns n - 1. Same as (1- n).

**Examples:**
```lisp
(SUB1 5)  ; => 4
```

**See also:** ADD1, +, -

---

### ABS

**Type:** `FUNCTION`

**Syntax:** `(abs n)`

Returns the absolute value of n.

**Examples:**
```lisp
(ABS 5)  ; => 5
(ABS -5)  ; => 5
```

**See also:** MINUSP

---

### MAX

**Type:** `FUNCTION`

**Syntax:** `(max number...)`

Returns the largest of its arguments.

**Examples:**
```lisp
(MAX 1 5 3)  ; => 5
(MAX -1 -5)  ; => -1
```

**See also:** MIN

---

### MIN

**Type:** `FUNCTION`

**Syntax:** `(min number...)`

Returns the smallest of its arguments.

**Examples:**
```lisp
(MIN 1 5 3)  ; => 1
```

**See also:** MAX

---

### RANDOM

**Type:** `FUNCTION`

**Syntax:** `(random n)`

Returns a random integer from 0 (inclusive) to n (exclusive).

**Examples:**
```lisp
(RANDOM 10)  ; => "0-9 randomly"
```

---

---
*Generated by Lamedh documentation system*
()
