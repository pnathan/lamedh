# 2. Language Basics

This chapter covers Lamedh's reader syntax and core evaluation model: how
text becomes data, how data gets evaluated, and the small set of forms you'll
use in almost every program. Every example below was run against the
`lamedh` binary; `; =>` comments show actual output.

## 2.1 Atoms

An atom is anything that isn't a cons cell: numbers, strings, characters,
symbols, keywords, and `nil`/`()`.

### Integers

Plain decimal integers are 64-bit signed values. Lamedh also reads several
non-decimal notations: `#x`/`#b`/`#o` are modern radix prefixes, `177Q` is
Lisp 1.5's own octal suffix, and `0FFh` is an assembly-style hex suffix
(digit-leading, so names like `ch` stay symbols):

```lisp
123        ; => 123
(- 456)    ; => -456
#x1F       ; => 31
#b101      ; => 5
#o17       ; => 15
177Q       ; => 127
0FFh       ; => 255
```

### Floats, strings, characters

```lisp
3.14       ; => 3.14
```

Strings support the usual backslash escapes (`\n \t \r \\ \"`) and the
printer re-escapes on the way back out:

```lisp
"hi\n"          ; => "hi\n"
"tab\there"     ; => "tab\there"
```

A character literal is one character between single quotes, with the same
escapes as strings (`\n \t \r \\ \' \0`). `char-code` gets its byte value:

```lisp
'a'             ; => 'a'
(char-code '\n')  ; => 10
```

### Symbols and keywords

Symbols are uppercased and interned when read, so `foo`, `FOO`, and `Foo` all
name the same symbol — and interning makes `eq` on symbols a cheap, exact
pointer comparison:

```lisp
(eq 'foo 'FOO)  ; => T
```

A symbol starting with `:` is a keyword. Keywords are self-evaluating — no
quote needed:

```lisp
:foo             ; => :FOO
(eq :foo :foo)   ; => T
```

Keywords are commonly used as tags, in `case`/`typecase` clauses, and in
plist-style argument sections.

### Comments

`;` runs to end of line. `#|` `|#` is a block comment, and block comments
nest, so you can comment out a region that already contains one:

```lisp
(+ 1 #| block comment |# 2)   ; => 3
```

## 2.2 Lists and Pairs

A list is a chain of cons cells ending in `nil` (printed `()`). When the
second element of a cons isn't itself a list, the printer shows a dotted
pair:

```lisp
(cons 1 2)      ; => (1 . 2)
'(1 . 2)        ; => (1 . 2)
(list 1 2 3)    ; => (1 2 3)
'(a b . c)      ; => (A B . C)
```

Notice the symbols came back uppercased — the reader normalized `a b c` to
`A B C` on the way in.

### Quote, quasiquote, unquote

`'x` is shorthand for `(quote x)` — it returns `x` unevaluated:

```lisp
(quote foo)  ; => FOO
'foo         ; => FOO
```

Backquote (`` ` ``) builds a mostly-literal list, letting you splice in
computed values with `,` (unquote) and `,@` (unquote-splicing):

```lisp
(let ((x 5)) `(a b ,x))              ; => (A B 5)
(let ((xs '(2 3))) `(1 ,@xs 4))      ; => (1 2 3 4)
```

Quasiquote is the standard way to write code-generating macros: quote the
skeleton, unquote the parts that vary.

## 2.3 Everything Is an Expression

There's no statement/expression split in Lamedh. `if`, `let`, `progn` — all
of it evaluates to a value, usable anywhere an expression is expected.

### `def` and `setq`

`def` introduces a new binding; `setq` assigns to an existing binding,
walking up the lexical environment chain to find it. `def` returns the
symbol name; `setq` returns the assigned value:

```lisp
(def x 10)                  ; => X
(progn (setq y 20) y)       ; => 20
```

In practice you'll rarely call `def` directly for functions — that's what
`defun` is for:

```lisp
(progn
  (defun square (x) (* x x))
  (square 7))
; => 49
```

(As of the one-door compile policy, `defun` also quietly attempts a typed
native compile of the function in the background; that's covered later in
the manual and doesn't change observable behavior.)

### `let` and `let*`

`let` binds a set of names in parallel — no init expression can see another's
binding. `let*` binds sequentially, so later bindings can refer to earlier
ones:

```lisp
(let ((x 1) (y 2)) (+ x y))          ; => 3
(let* ((x 1) (y (+ x 1))) y)         ; => 2
```

### Lexical closures

Lambdas close over their defining environment. Each call to a function that
returns a lambda creates a fresh, independent binding:

```lisp
(progn
  (defun make-counter ()
    (let ((n 0))
      (lambda () (setq n (+ n 1)) n)))
  (let ((c (make-counter)))
    (list (funcall c) (funcall c) (funcall c))))
; => (1 2 3)
```

Each call to `make-counter` produces a counter with its own private `n` —
they don't share state.

### `progn` and `prog2`

`progn` evaluates a sequence of forms for effect and returns the value of the
last one. `prog2` returns the value of the *second* form instead — handy for
running a setup form, capturing a result, then running cleanup:

```lisp
(progn 1 2 3)  ; => 3
(prog2 1 2 3)  ; => 2
```

## 2.4 Truthiness

`()` (equivalently `nil`) is the only false value. `T` is the canonical true
value returned by predicates, but any non-`()` value is truthy in a
conditional — including `0` and `""`:

```lisp
()                       ; => ()
T                        ; => T
(if 0 'true 'false)      ; => TRUE
(if "" 'true 'false)     ; => TRUE
```

Don't reach for `0` or `""` as a false sentinel — only `()` works.

## 2.5 Control Flow

### `if` and `cond`

`if` takes a test, a then-branch, and an optional else-branch. `cond` chains
test/body clauses, running the body of the first truthy test; a `t` clause is
a catch-all default:

```lisp
(if t 1 2)                                   ; => 1
(if () 1 2)                                  ; => 2
(cond ((eq 1 2) 'a) ((eq 1 1) 'b) (t 'c))    ; => B
```

### `when` and `unless`

`when` runs its body (implicit `progn`) only if the test is truthy; `unless`
is the inverse:

```lisp
(when t 1 2 3)      ; => 3
(when () 1 2 3)     ; => ()
(unless () 1 2 3)   ; => 3
```

### `and` / `or`

Both are variadic and short-circuiting. `and` returns the last value if all
arguments are truthy, or `()` at the first falsy one; called with no
arguments it returns `T`. `or` returns the first truthy value, or `()` if all
are falsy (and if called with none):

```lisp
(and 1 2 3)       ; => 3
(and 1 () 3)      ; => ()
(and)             ; => T
(or () () 3)      ; => 3
(or)              ; => ()
```

### `while` and `for`

`while` loops as long as its test is truthy, evaluating the body each pass:

```lisp
(let ((n 0))
  (while (< n 3) (setq n (+ n 1)))
  n)
; => 3
```

`for` is a fast integer-counted loop: `(for (var start end [step]) body...)`.
`var` runs from `start` to `end` inclusive (default step `1`; a negative step
counts down). `for` always returns `()`:

```lisp
(for (i 0 3) (print i))
```
```
0
1
2
3
; => ()
```

Unlike a `let`-based recursive loop, `for` reuses a single environment frame
across iterations instead of allocating one per pass — see the doc comment
on `eval_for` in `src/evaluator/special_forms.rs` for the performance
rationale.

## 2.6 Predicates and Equality

`eq` is identity comparison — for symbols and small immediates it's exact and
cheap, but two structurally identical lists built separately are not `eq`.
`equal` compares structurally, recursing through conses:

```lisp
(eq '(1 2) '(1 2))     ; => ()
(equal '(1 2) '(1 2))  ; => T
(eq 'a 'a)             ; => T
```

Symbols are always `eq` to themselves because they're interned; use `equal`
for lists, strings, and anything else you want to compare by content.

A sampling of the predicate library (`lib/04-predicates.lisp` and the Rust
builtins registered in `src/environment.rs`):

```lisp
(null ())          ; => T
(atom 5)           ; => T
(consp '(1 2))     ; => T
(numberp 5)        ; => T
(stringp "hi")     ; => T
(symbolp 'a)       ; => T
(zerop 0)          ; => T
(evenp 4)          ; => T
(oddp 4)           ; => ()
(floatp 3.14)      ; => T
(not ())           ; => T
```

## 2.7 Working with Lists

The CXR family (`lib/02-cxr.lisp`) generates every 2-, 3-, and 4-level
`car`/`cdr` composition (`caar` through `cddddr`) from a small macro:

```lisp
(car '(1 2 3))    ; => 1
(cdr '(1 2 3))    ; => (2 3)
(cadr '(1 2 3))   ; => 2
(caddr '(1 2 3))  ; => 3
```

Core list operations (`lib/01-list.lisp`), plus `nth` and `assoc` which are
Rust builtins:

```lisp
(append '(1 2) '(3) '(4))                 ; => (1 2 3 4)  (variadic, 0.3)
(reverse '(1 2 3))                        ; => (3 2 1)
(length '(1 2 3))                         ; => 3
(member 2 '(1 2 3))                       ; => (2 3)
(nth 1 '(a b c))                          ; => B
(assoc 'b '((a . 1) (b . 2)))             ; => (B . 2)
```

`mapcar` zips any number of lists (0.3), stopping at the shortest:
`(mapcar #'+ '(1 2 3) '(10 20 30))` is `(11 22 33)`. `gcd`/`lcm` and the
bitwise trio `logand`/`logior`/`logxor` are variadic folds too (`logior`
is 0.3's name for the old `logor`).

The functional toolkit (`lib/13-functional.lisp`) follows a consistent
function-first argument order — `(mapcar function list)`, not the reverse —
so calls nest cleanly:

```lisp
(mapcar (lambda (x) (* x x)) '(1 2 3))    ; => (1 4 9)
(mapc (lambda (x) x) '(1 2 3))            ; => (1 2 3)
(filter #'evenp '(1 2 3 4 5 6))           ; => (2 4 6)
(reduce #'+ '(1 2 3 4))                   ; => 10
```

`mapc` is for side effects; it returns the original list unchanged. `reduce`
with no explicit init seeds from the first element; pass an extra argument
to fold from a given starting value instead.

The same file supplies the everyday list toolkit: `take`/`drop`, `zip`/
`unzip`, `iota`/`range`, `partition`, `group-by`, `flatten`,
`remove-duplicates` — and, new in 0.3, `enumerate` (index/element pairs),
`frequencies` (an `(element . count)` alist), and `sort-by`:

```lisp
(enumerate '(a b c))                      ; => ((0 A) (1 B) (2 C))
(frequencies '(a b a a))                  ; => ((A . 3) (B . 1))
(sort-by '((3 x) (1 y) (2 z)) #'car)      ; => ((1 Y) (2 Z) (3 X))
```

`sort-by` is collection-first like `sort` and takes an optional third
argument to compare the extracted keys with something other than `#'<` —
`(sort-by strings #'string-length #'>)` sorts longest-first.

`#'` reads as `(function f)`, and `funcall`/`apply` invoke a function value
directly:

```lisp
(funcall #'+ 1 2)     ; => 3
(apply #'+ '(1 2 3))  ; => 6
```

## 2.8 Recursion and Tail Calls

Lamedh optimizes tail calls: a self-call (or mutual call) in tail position
reuses the current stack frame instead of growing the native call stack. A
naive recursive loop of a million iterations would blow the stack in most
languages without this; here it just runs:

```lisp
(progn
  (defun count-to (n acc)
    (if (> acc n) acc (count-to n (1+ acc))))
  (count-to 1000000 0))
; => 1000001
```

If you rewrite the recursive call so it's no longer in tail position — say,
wrapping it in `(1+ (helper ...))` instead of calling it last — you lose this
guarantee and consume a native stack frame per call. For that reason, entry
points that run arbitrary user Lisp (the CLI, the test harness) run on a
large spawned stack via `with_large_stack`, but well-written tail-recursive
loops don't need to rely on that headroom at all.

## 2.9 Math

The basic arithmetic operators are variadic:

```lisp
(+ 1 2 3)    ; => 6
(* 2 3 4)    ; => 24
(/ 10 3)     ; => 3
```

`mod` is Euclidean: the remainder always comes out non-negative for these
cases (`0 <= |remainder| < |divisor|`, following the divisor's sign
convention rather than the dividend's):

```lisp
(mod -7 3)   ; => 2
(mod 7 -3)   ; => 1
```

`1+` and `1-` are the increment/decrement shorthands:

```lisp
(1+ 5)   ; => 6
(1- 5)   ; => 4
```

Comparisons `<`, `>`, `=`, `<=`, `>=` are variadic monotone chains, like
`+` and `*` (0.3 regularity) — `(< a b c)` means a < b and b < c. `max`
and `min` are variadic too:

```lisp
(< 1 2 3)        ; => T
(< 1 3 2)        ; => ()
(= 4 4 4)        ; => T
(<= 1 1 2)       ; => T
(max 1 5 3)      ; => 5
(min 1 5 3)      ; => 1
```

Floats mix with integers in arithmetic; `floatp` distinguishes the two
representations (section 2.6).

### Integer overflow

Integers are 64-bit and wrap on overflow rather than promoting to a bignum.
At the top level, an overflowing operation prints a warning and sets the
`OVERFLOW` flag, which you check and clear from Lisp with `flag-set-p` and
`clear-flag` — there is no `OVERFLOW` *variable*:

```lisp
(+ 9223372036854775807 1)
```
```
warning: integer overflow — a result wrapped around (check (flag-set-p 'overflow); reset with (clear-flag 'overflow))
; => -9223372036854775808
```

```lisp
(progn (+ 9223372036854775807 1) (flag-set-p 'overflow))    ; => T
(progn (clear-flag 'overflow) (flag-set-p 'overflow))       ; => ()
```

## 2.10 What's Next

This chapter covered syntax, evaluation, and the everyday functions you'll
reach for constantly. The next chapter goes deeper into functions themselves:
`defun*` and type inference, macros vs. fexprs vs. `vau` operatives, and
`&rest`/optional-argument handling.
