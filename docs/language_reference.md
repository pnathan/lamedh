# Litthp Lisp Language Reference

## Data Types

Litthp Lisp supports the following data types:

### Numbers

Numbers are 64-bit signed integers.

Example: `1`, `-42`, `1000`

### Strings

Strings are sequences of characters. They are enclosed in double quotes.

Example: `"hello, world"`, `"this is a string"`

### Symbols

Symbols are identifiers. They are used to name variables and functions.

Example: `x`, `my-function`, `+`

### Lists

Lists are ordered collections of elements. They are created using parentheses and are implemented as linked lists of "cons cells". A cons cell is a pair of values, a `car` (the value) and a `cdr` (the rest of the list). A list is terminated by `nil`.

Example: `(1 2 3)`, `(a b c)`, `()` (the empty list, or `nil`)

### Nil

`nil` represents both the empty list and the boolean false value. It can be written as `()` or `nil`.

## Basic Syntax and Evaluation

Litthp Lisp uses S-expressions (Symbolic Expressions) for both code and data. An S-expression is either an atom (like a number, string, or symbol) or a list of S-expressions.

### Evaluation Rules

1.  **Numbers, Strings, and `nil`** evaluate to themselves.
2.  **Symbols** are treated as variables, and they evaluate to the value they are bound to in the current environment.
3.  **Lists** are treated as function calls. The first element of the list is the function, and the rest of the elements are the arguments. The arguments are evaluated before the function is called.

### Quoting

To prevent an S-expression from being evaluated, you can use the `quote` special form. The single quote character (`'`) is a shorthand for `quote`.

Example:

-   `(+ 1 2)` evaluates to `3`.
-   `'(+ 1 2)` evaluates to the list `(+ 1 2)`.
-   `(quote (+ 1 2))` is equivalent to `'(+ 1 2)`.

## Special Forms

Special forms are expressions that do not follow the standard evaluation rule of evaluating all arguments before calling the function.

### `quote`

`(quote expression)`

Prevents the `expression` from being evaluated and returns it as is. Can be abbreviated with a single quote (`'`).

Example: `(quote (+ 1 2))` or `'(+ 1 2)` returns the list `(+ 1 2)`.

### `if`

`(if condition then-expression else-expression)`

Evaluates `condition`. If the result is not `nil`, it evaluates and returns `then-expression`. Otherwise, it evaluates and returns `else-expression`.

Example: `(if (> 3 2) "yes" "no")` returns `"yes"`.

### `def`

`(def symbol value &optional docstring)`

Binds the `symbol` to the `value` in the current environment. If the optional `docstring` is provided, it is attached to the `symbol`.

Example: `(def x 10)` binds `x` to the value `10`.
Example: `(def y 20 "This is the value of y")` binds `y` to `20` and sets its docstring.

### `lambda`

`(lambda (param1 param2 ...) &rest body)`

Creates an anonymous function. When called, it binds the arguments to the parameters and evaluates the `body` expressions. If there are multiple body expressions, they are implicitly wrapped in a `progn`.

Example: `((lambda (x y) (+ x y)) 10 20)` returns `30`.
Example: `((lambda () (print "hello") "world"))` prints "hello" and returns `"world"`.

### `defun`

`(defun name (param1 param2 ...) &optional docstring &rest body)`

A convenience macro for defining a named function. It supports an optional docstring and multiple expressions in the function body.

Example: `(defun add (x y) (+ x y))` defines a function `add` that takes two arguments and returns their sum.
Example:
```lisp
(defun my-fun (x)
  "This is a docstring."
  (print x)
  (* x x))
```

### `let`

`(let ((var1 val1) (var2 val2) ...) body)`

Creates a new lexical scope with variables `var1`, `var2`, etc. bound to the values `val1`, `val2`, etc. and then evaluates the `body` in that scope.

Example: `(let ((x 10) (y 20)) (+ x y))` returns `30`.

### `progn`

`(progn &rest expressions)`

Evaluates a sequence of `expressions` from left to right and returns the value of the last expression.

Example: `(progn (print "hello") (+ 1 2))` prints "hello" and returns `3`.

### `quasiquote`, `unquote`

Quasiquote (backtick, `` ` ``) is similar to `quote`, but it allows you to selectively evaluate parts of the quoted expression with `unquote` (comma, `,`).

Example:
`(def x 10)`
`` `(a b ,x)`` returns `(a b 10)`.

### `defexpr`

`(defexpr name (param) &optional docstring &rest body)`

Defines a function-like form, an f-expression or "fexpr", where the arguments are not evaluated before being passed to the function. The `param` is a single symbol that will be bound to the list of unevaluated arguments. It supports an optional docstring.

Example:
`(defexpr my-if (args) "A custom if-like fexpr." (if (eval (car args)) (eval (cadr args)) (eval (caddr args))))`
`(my-if (> 1 0) "yes" "no")` returns `"yes"`.

### `defmacro`

`(defmacro name (param1 param2 ... &rest rest-params) &optional docstring &rest body)`

Defines a macro. A macro is a function that is called at read time, and its return value is then evaluated in place of the macro call. This allows you to transform code before it is evaluated. Macros support `&rest` parameters and an optional docstring.

Example:
`(defmacro my-unless (condition body) `(if (not ,condition) ,body nil))`
`(my-unless (= 1 2) "one is not two")` expands to `(if (not (= 1 2)) "one is not two" nil)` and returns `"one is not two"`.

## Built-in Functions

### Arithmetic Functions

-   `+` `( + &rest numbers)`: Returns the sum of all `numbers`.
-   `-` `(- &rest numbers)`: With one argument, returns its negation. With multiple arguments, subtracts the rest from the first.
-   `*` `(* &rest numbers)`: Returns the product of all `numbers`.
-   `/` `(/ number1 number2)`: Returns the result of dividing `number1` by `number2`. Example: `(/ 10 2)` returns `5`.

### List Functions

#### Basic List Operations

-   `car` `(car list)`: Returns the first element of a `list`. Example: `(car '(1 2 3))` returns `1`.
-   `cdr` `(cdr list)`: Returns the rest of a `list`. Example: `(cdr '(1 2 3))` returns `(2 3)`.
-   `cons` `(cons element list)`: Creates a new cons cell with `element` as the car and `list` as the cdr. Example: `(cons 1 '(2 3))` returns `(1 2 3)`.
-   `atom` `(atom object)`: Returns `t` if `object` is an atom (not a cons cell), `nil` otherwise. Example: `(atom 1)` returns `t`, `(atom '(1 2))` returns `nil`.

#### Advanced List Processing

-   `subst` `(subst new old tree)`: Substitutes all occurrences of `old` with `new` in `tree`. Example: `(subst 'x 'a '(a b a c))` returns `(x b x c)`.
-   `assoc` `(assoc key alist)`: Searches an association list for a pair whose car is `key`. Returns the pair if found, `nil` otherwise. Example: `(assoc 'b '((a 1) (b 2) (c 3)))` returns `(b 2)`.
-   `maplist` `(maplist list function)`: Applies `function` to successive sublists of `list` and returns a list of results. Example: `(maplist '(1 2 3) (lambda (x) (car x)))` returns `(1 2 3)`.
-   `mapcar` `(mapcar list function)`: Applies `function` to each element of `list` and returns a list of results. Example: `(mapcar '(1 2 3) (lambda (x) (* x 2)))` returns `(2 4 6)`.

#### Destructive List Operations

-   `rplaca` `(rplaca cons-cell new-car)`: Returns a new cons cell with the car replaced by `new-car`. Example: `(rplaca '(1 . 2) 3)` returns `(3 . 2)`.
-   `rplacd` `(rplacd cons-cell new-cdr)`: Returns a new cons cell with the cdr replaced by `new-cdr`. Example: `(rplacd '(1 . 2) 3)` returns `(1 . 3)`.

### String Functions

-   `concat` `(concat &rest strings)`: Concatenates all `strings`. Example: `(concat "a" "b" "c")` returns `"abc"`.
-   `index` `(index string n)`: Returns the character at index `n` of the `string`. Example: `(index "hello" 1)` returns `"e"`.

### Logical Functions

-   `eq` `(eq obj1 obj2)`: Returns `t` if `obj1` and `obj2` are the same object, `nil` otherwise. Example: `(eq 'a 'a)` returns `t`, `(eq 'a 'b)` returns `nil`.
-   `=` `(= num1 num2)`: Returns `t` if `num1` and `num2` are numerically equal, `nil` otherwise. Example: `(= 1 1)` returns `t`.
-   `not` `(not object)`: Returns `t` if `object` is `nil`, `nil` otherwise. Example: `(not nil)` returns `t`.

### Type Predicates

-   `atom` `(atom object)`: Returns `t` if `object` is an atom (not a cons cell), `nil` otherwise.
-   `stringp` `(stringp object)`: Returns `t` if `object` is a string, `nil` otherwise.
-   `numberp` `(numberp object)`: Returns `t` if `object` is a number (integer or float), `nil` otherwise.
-   `fixp` `(fixp object)`: Returns `t` if `object` is a fixed-point integer, `nil` otherwise. Example: `(fixp 42)` returns `t`.
-   `floatp` `(floatp object)`: Returns `t` if `object` is a floating-point number, `nil` otherwise. Example: `(floatp 3.14)` returns `t`.

### Hash Table Functions

-   `make-hash-table` `(make-hash-table)`: Creates and returns a new hash table.
-   `get` `(get hash-table key)`: Returns the value for `key` in `hash-table`.
-   `set` `(set hash-table key value)`: Sets the value for `key` in `hash-table` to `value`.
-   `delete-key` `(delete-key hash-table key)`: Removes the key-value pair for `key` from `hash-table`.
-   `keys` `(keys hash-table)`: Returns a list of all keys in `hash-table`.

Example:
```lisp
(def my-table (make-hash-table))
(set my-table "name" "John")
(get my-table "name") ; returns "John"
(keys my-table) ; returns ("name")
```

### Property List Functions

Symbol property lists allow you to attach arbitrary key-value pairs to symbols.

- `get-p` `(get-p symbol property-name)`: Retrieves the value of a property from a symbol's property list.
- `put-p` `(put-p symbol property-name value)`: Sets a property on a symbol's property list.
- `remprop` `(remprop symbol property-name)`: Removes a property from a symbol's property list. Returns `t` if successful, `nil` if the property didn't exist.
- `deflist` `(deflist pairs indicator)`: Defines properties for multiple symbols at once. `pairs` is a list of `((symbol value) ...)` pairs.
- `documentation` `(documentation symbol)`: Retrieves the docstring for a `symbol`.

Example:
```lisp
(put-p 'my-symbol "version" 1)
(get-p 'my-symbol "version") ; returns 1
(remprop 'my-symbol "version") ; returns T
(deflist '((foo "a") (bar "b")) "type")
(defun my-fun () "My docstring." nil)
(documentation 'my-fun) ; returns "My docstring."
```

### I/O Functions

-   `read` `(read)`: Reads an s-expression from standard input and returns it. Example: If the user types `(+ 1 2)`, `read` returns the list `(+ 1 2)`.
-   `prin1` `(prin1 object)`: Prints `object` in a readable form (with escape characters for strings) and returns it. Does not print a newline.
-   `princ` `(princ object)`: Prints `object` without escape characters (strings print without quotes). Does not print a newline.
-   `terpri` `(terpri)`: Prints a newline and returns `nil`.
-   `load-file` `(load-file filename)`: Loads and evaluates a Lisp file. The `filename` must be a string. Returns `t` on success. See the [File I/O documentation](file_io.md) for details.

Example:
```lisp
(prin1 "hello")  ; prints: "hello" and returns "hello"
(princ "hello")  ; prints: hello and returns "hello"
(terpri)         ; prints a newline
(load-file "myfile.lisp")  ; loads and evaluates myfile.lisp, returns T
```

### Error Handling

-   `error` `(error message)`: Raises an error with the given `message`. The message can be a string or any other object.
-   `errorset` `(errorset form)`: Evaluates `form` in an error-catching context. If evaluation succeeds, returns a list containing the result. If an error occurs, returns `nil`.

Example:
```lisp
(errorset '(+ 1 2))        ; returns (3)
(errorset '(/ 1 0))        ; returns NIL (division by zero)
(errorset '(error "oops")) ; returns NIL
```

### Bitwise Operations

All bitwise operations work on integers.

-   `logor` `(logor &rest numbers)`: Bitwise OR of all `numbers`. Example: `(logor 5 3)` returns `7`.
-   `logand` `(logand &rest numbers)`: Bitwise AND of all `numbers`. Example: `(logand 5 3)` returns `1`.
-   `logxor` `(logxor &rest numbers)`: Bitwise XOR of all `numbers`. Example: `(logxor 5 3)` returns `6`.
-   `leftshift` `(leftshift number shift)`: Shifts `number` left by `shift` bits. If `shift` is negative, shifts right instead. Example: `(leftshift 1 3)` returns `8`, `(leftshift 8 -3)` returns `1`.

### Miscellaneous Functions

-   `eval` `(eval expression)`: Evaluates the `expression`. Example: `(eval '(+ 1 2))` returns `3`.
-   `print` `(print &rest objects)`: Prints the `objects` to the console. Example: `(print "hello" "world")` prints `helloworld`.
-   `current-environment` `(current-environment)`: Returns a hash table containing the current environment's bindings.
