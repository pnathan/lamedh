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

`(def symbol value)`

Binds the `symbol` to the `value` in the current environment.

Example: `(def x 10)` binds `x` to the value `10`.

### `lambda`

`(lambda (param1 param2 ...) body)`

Creates an anonymous function. When called, it binds the arguments to the parameters and evaluates the `body`.

Example: `((lambda (x y) (+ x y)) 10 20)` returns `30`.

### `defun`

`(defun name (param1 param2 ...) body)`

A convenience macro for defining a named function. It is equivalent to `(def name (lambda (params...) body))`.

Example: `(defun add (x y) (+ x y))` defines a function `add` that takes two arguments and returns their sum.

### `let`

`(let ((var1 val1) (var2 val2) ...) body)`

Creates a new lexical scope with variables `var1`, `var2`, etc. bound to the values `val1`, `val2`, etc. and then evaluates the `body` in that scope.

Example: `(let ((x 10) (y 20)) (+ x y))` returns `30`.

### `quasiquote`, `unquote`

Quasiquote (backtick, `` ` ``) is similar to `quote`, but it allows you to selectively evaluate parts of the quoted expression with `unquote` (comma, `,`).

Example:
`(def x 10)`
`` `(a b ,x)`` returns `(a b 10)`.

### `defexpr`

`(defexpr name (param) body)`

Defines a function-like form, an f-expression or "fexpr", where the arguments are not evaluated before being passed to the function. The `param` is a single symbol that will be bound to the list of unevaluated arguments.

Example:
`(defexpr my-if (args) (if (eval (car args)) (eval (cadr args)) (eval (caddr args))))`
`(my-if (> 1 0) "yes" "no")` returns `"yes"`.

### `defmacro`

`(defmacro name (param1 param2 ...) body)`

Defines a macro. A macro is a function that is called at read time, and its return value is then evaluated in place of the macro call. This allows you to transform code before it is evaluated.

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

-   `car` `(car list)`: Returns the first element of a `list`. Example: `(car '(1 2 3))` returns `1`.
-   `cdr` `(cdr list)`: Returns the rest of a `list`. Example: `(cdr '(1 2 3))` returns `(2 3)`.
-   `cons` `(cons element list)`: Creates a new cons cell with `element` as the car and `list` as the cdr. Example: `(cons 1 '(2 3))` returns `(1 2 3)`.
-   `atom` `(atom object)`: Returns `t` if `object` is an atom (not a cons cell), `nil` otherwise. Example: `(atom 1)` returns `t`, `(atom '(1 2))` returns `nil`.

### String Functions

-   `concat` `(concat &rest strings)`: Concatenates all `strings`. Example: `(concat "a" "b" "c")` returns `"abc"`.
-   `index` `(index string n)`: Returns the character at index `n` of the `string`. Example: `(index "hello" 1)` returns `"e"`.

### Logical Functions

-   `eq` `(eq obj1 obj2)`: Returns `t` if `obj1` and `obj2` are the same object, `nil` otherwise. Example: `(eq 'a 'a)` returns `t`, `(eq 'a 'b)` returns `nil`.
-   `=` `(= num1 num2)`: Returns `t` if `num1` and `num2` are numerically equal, `nil` otherwise. Example: `(= 1 1)` returns `t`.
-   `not` `(not object)`: Returns `t` if `object` is `nil`, `nil` otherwise. Example: `(not nil)` returns `t`.

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

### Symbol Functions

- `get-p` `(get-p symbol property-name)`: Retrieves the value of a property from a symbol's property list.
- `put-p` `(put-p symbol property-name value)`: Sets a property on a symbol's property list.

Example:
```lisp
(put-p 'my-symbol "version" 1)
(get-p 'my-symbol "version") ; returns 1
```

### Miscellaneous Functions

-   `eval` `(eval expression)`: Evaluates the `expression`. Example: `(eval '(+ 1 2))` returns `3`.
-   `print` `(print &rest objects)`: Prints the `objects` to the console. Example: `(print "hello" "world")` prints `helloworld`.
-   `current-environment` `(current-environment)`: Returns a hash table containing the current environment's bindings.
