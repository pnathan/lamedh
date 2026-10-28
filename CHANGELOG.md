# v0.4.1 — 2026-10-23

## **Breaking:** `min`/`max` follow IEEE 754-2019 for floats; int literals are not coerced (#530, #616)

- **Untyped calls** (the interpreter): `min`/`max` are `a -> a -> a` over one
  numeric type. With any float argument the result is a float, so
  `(max 0.5 1)` is `1.0` (was `1`). All-fixnum calls are unchanged.
- **Floats** follow IEEE 754-2019 `maximum`/`minimum`: a NaN argument makes the
  result NaN, and `-0.0` orders below `+0.0`. So `(max -0.0 0.0)` and
  `(max 0.0 -0.0)` are both `0.0`, and `(min 0.0 -0.0)` and `(min -0.0 0.0)`
  are both `-0.0`. Previously the result could depend on argument order and
  could drop a NaN.
- **Typed code**: the operands of `min`/`max` must share one type. An integer
  *literal* beside a `float64` operand is a type error, not a coercion:
  `(defun-typed (f float64) ((x float64)) (max x 1))` is rejected in the native
  elaborator and the portable checker alike, as on `main` before #530. Write
  `1.0`. Compiled float64 `min`/`max` implement the same IEEE rule as the
  interpreter, so the two agree on NaN and signed zeros.
- Unchanged: `+`, `-`, `*`, `/`, `quotient` and the comparisons still coerce an
  integer literal beside a `float64` operand (literal-only; an int64 variable
  or expression beside a float64 is still a type error). Ported to the SBCL
  library and `lamedh-asm`.

## Twelve reader, evaluator, OS and stdlib defects (#617)

Found by a bug bash and confirmed by two independent verifiers; each has a
regression test.

- **Reader:** an empty `;` comment no longer raises a parse error; a discarded
  `#+`/`#-` form that ends a list no longer makes the list unreadable; a
  `#S(...)` literal at the depth limit returns the depth error instead of
  underflowing.
- **`make-temp-file`, `make-temp-directory`:** a prefix containing a path
  separator, NUL or `..` is an error. Under `TEMP-FS` alone these could create
  paths outside the temp directory. **Code that passed such a prefix now
  errors.**
- **Dynamic parameters:** a function with duplicate dynamic parameter names
  restores the saved values in reverse order, so the original global comes back.
- **Compiled `COND`:** a fallback to the interpreter evaluates only the
  remaining clauses; earlier test forms no longer run twice.
- **`isqrt`** is exact above 2^53. **`code-char`** rejects code points at or
  above 2^32 instead of truncating them.
- **`os-env-set*`, `os-env-unset*`** return `:INVALID-ARGUMENT` for names or
  values that `std::env` rejects (empty or containing `=` or NUL), where they
  panicked. **`os-signal*`** rejects pids at or below 0 or above `i32::MAX`
  before the policy check.
- **Optimizer:** constant folding of `/` declines `i64::MIN / -1`.
- **`max`, `min`** reduce in constant stack depth (superseded by the IEEE
  rewrite above, which keeps that property).

## Typed-call depth cap is configurable; lower defaults on `wasm32` (#619, #625)

The non-tail call cap for typed (compiled) code was a hard-coded 50,000,
sized for the 512 MiB `with_large_stack` thread, and `set_eval_depth_limit`
did not affect it. A host with a small stack (a browser, via `wasm32`)
overflowed its stack long before 50,000 and trapped, which kills the WASM
instance. The cap is now a per-thread setting:

- `lamedh::set_typed_call_depth_limit(n)` and `lamedh::typed_call_depth_limit()`
  set and read it. The value is sampled when a top-level typed call starts, so
  the native call path reads a plain field. A typed `fib 35` showed no
  measurable slowdown (noisy run on a busy machine).
- `DEFAULT_TYPED_CALL_DEPTH_LIMIT` is 50,000 natively, so native behavior is
  unchanged.
- On `wasm32` the defaults are lower: 400 typed calls and, for the tree-walker,
  `DEFAULT_EVAL_DEPTH_LIMIT` of 2,000 instead of 10,000. Measured under node,
  deep recursion now returns a recursion-limit error and the instance keeps
  working. See `docs/embedding.md`.

## `EQ` on cons cells is pointer identity (#454)

`EQ` returned `NIL` whenever either argument was a cons cell, so `(eq x x)` was
false for a list. It now compares the cells' identity: `(let ((x (list 1))) (eq x x))`
is `T`, and two separately built cells are still not `EQ`. `EQUAL` and every
other `EQ` case are unchanged. KERNEL.md Part IV states the rule.

## Reader: `#\c` characters, `#(...)` arrays, `#+`/`#-`, `+NAME+` symbols (#526, #527, #459, #463)

- `#\a`, `#\(`, `#\Space`, `#\Newline`, `#\Tab` read as the existing `Char`
  value, the same as `'a'` (#526). `--fmt` and the incomplete-input detector
  skip them.
- `#(e1 e2 ...)` reads as an array literal. Elements are not evaluated, and the
  array is built once at read time (#527).
- `#+feature form` and `#-feature form` are CL-style reader conditionals. A
  feature expression is a symbol (case-folded, leading `:` optional) or
  `(and ...)`, `(or ...)`, `(not e)`. A form guarded by an absent feature must
  still parse, but yields no value at the read site. Each environment carries a
  reader-feature set, seeded with `RUST`, `READER-FLOATS`, `READER-RADIX` and
  `READER-BLOCK-COMMENTS`; it is separate from sandbox capabilities. Embedders
  use `Environment::enable_reader_feature`, `disable_reader_feature`,
  `reader_feature_enabled` and `reader_features_list` (#459).
- A leading-`+` symbol such as `+NUMERIC-PRECISION-MODEL+` reads as one token.
  Before, the `+` operator consumed the first character and stranded the rest
  (#463).

## Number tokens must end at a delimiter (#503)

A token that starts like a number but is not a complete one was split into a
number and a symbol: `123abc` read as `123 ABC`, `2.0.3` as the dotted pair
`2.0 . 3`, `1-2` as `1 -2`. It is now a parse error naming the token. `1+` and
`1-` alone remain symbols. Also:

- `1e5` (exponent-only float) reads as `100000.0`; before it read as `1 E5`.
- Octal accepts a lower-case `q` suffix: `177q` is 127.
- `Q`, `H` and `#x` literals are parsed with the sign attached, so
  `-8000000000000000H` is `i64::MIN`.
- An out-of-range or invalid-digit `Q`/`H`/radix literal (`8Q`, `#xFG`,
  `8000000000000000H`) is a parse error. It no longer falls through to the
  decimal parser.

## Arrays print their contents (#527)

`(print (list->array '(1 2 3)))` used to print `<array:3>`. It prints `#(1 2 3)`, and
the reader accepts that back. Arrays longer than 100 elements print the first
100 followed by an unreadable `#<...N more>` marker, so an abridged array never
reads back as a shorter one; channel and spawn serialization use the new
`printer::print_unabridged`, and `printer::ARRAY_PRINT_LIMIT` is 100. An array
that contains itself prints `#<circular-array>` at the back-reference. Typed
arrays print as `#<typed-array:int64 1 2 3>`, which is not readable.

## Printing a long list is linear (#509)

List printing rebuilt the tail string at every cons and recursed once per
spine cell: 400,000 elements took 41 s and 10^6 did not finish. The printer now
appends into one buffer and walks the spine in a loop. Output is byte-identical.
(The 41 s figure is from the commit message, not re-measured.)

## Two-operand `if` is an error inside function bodies (#506)

KERNEL.md specifies `IF` with exactly three operands. The tree-walker enforced
it, but the closure compiler used for function bodies accepted `(if c t)` with
an implicit `NIL` else, so the same form errored at top level and returned a
value inside a `defun`. Every path now signals `if takes exactly three
arguments`. Both optimizers (`optimizer.rs`, `lib/11-optimizer-vau.lisp`) fold
only the three-operand form; `opt-pass-if` no longer completes `(if c t)` to
`(if c t nil)`. Use `when`. **Existing code that relied on the implicit else
now errors.**

## Fuel exhaustion cannot be caught by the fence it exhausts (#457, #502)

- Exhausting the kernel fuel budget disarmed the counter, signalled an ordinary
  condition, and let guest code wrap `HANDLER-CASE` around it inside the same
  `WITH-FUEL` fence. A handler that never returned got unlimited fuel. Exhaustion
  now signals `LispError::FuelExhausted`, a control-flow signal that guest
  `HANDLER-CASE`/`ERRORSET` do not intercept. Only the owning `WITH-FUEL` form
  converts it back to a catchable error when it leaves the fence. A fixed,
  non-renewable grace allowance of 256 steps lets `UNWIND-PROTECT` cleanup and
  the fence's own bookkeeping run. Each fence gets its own allowance.
  KERNEL.md Part X/XIII moves from permitted to MUST (#457).
- Typed (`defun-typed`, compiled `defun*`) loops escaped `with-fuel`, `--fuel`
  and `--mcp`. When fuel is armed a typed call now runs on the reference
  interpreter, which charges one step per `while`/`for` iteration, self tail
  call and function entry. Armed fuel therefore costs the native speedup (#502).

## Evaluator fixes: dynamic `LET`, `T`/keyword binders, long `,@` splices (#237)

- `LET` evaluates every init before installing any dynamic binding. With
  `*X*` = 1 and `*Y*` = 10, `(let ((*x* *y*) (*y* *x*)) (list *x* *y*))` gave
  `(10 10)`; it is `(10 1)`, as over lexical names. A failing init leaves no
  dynamic binding behind.
- `DEFINE`, `DEFMACRO`, `DEFEXPR` and `LABEL` reject the constant `T` and
  keywords as names, as `DEF`, `SETQ`, `LET` and lambda lists already did.
  `(defmacro t ...)` used to overwrite the global `T`. **Code that bound `T` or
  a keyword with these forms now errors.**
- Splicing a very long list with `,@` (`` `(0 ,@big) `` over hundreds of
  thousands of elements) completes; it overflowed the native stack and aborted
  the process. Splice order, dotted tails and error messages are unchanged.

## `dotimes` binds a fresh variable per iteration (#528)

`for` reuses one counter slot, so closures made in a `dotimes` body all saw the
last value: `(2 2 2)`. Each closure now sees its own iteration's value:
`(0 1 2)`, as with `dolist`. `dotimes` rebinds only when the macro-expanded
body can build a closure, so a closure-free body keeps the plain `for` and its
native compilation.

## `VAU` parameter binding fixes (#461, #462)

- A `VAU` whose parameter name is a dynamic variable (`DEFDYNAMIC`/`DEFVAR`)
  saw the global value instead of the operand list or calling environment. It
  now guards the parameter like macros and fexprs do, and restores the prior
  value on exit (#461).
- `VAU` construction and `APPLY` on a fexpr or `VAU` bound the parameter name
  by re-interning it, so a gensym or foreign-table symbol could bind an id the
  body never resolved. Both paths use the canonical binder id (#462).

## Macro expansion is cached per compiled call site (#460)

A macro call inside a `defun`/`lambda` body expands once, on the first call
through that site, and the compiled expansion is reused. **Behavior change:** a
macro body that reads global or dynamic state observes it once per call site,
not once per call, as in Common Lisp. Redefining the macro invalidates the site.
Fexprs and `VAU` operatives still run fresh every call. The tree-walker,
`macroexpand` and `APPLY` on a macro are not cached. `fork_world` resets the
cache so no prototype-world cells cross the fork. See Rust embedding API for the
`LispVal::Macro` type change this required.

## Integer/float comparison is exact (#516)

`=`, `<`, `>`, `<=` and `>=` on a mix of integer and float rounded the integer
through `f64`, so integers above 2^53 compared equal to neighbouring floats:
`(= 9007199254740993 9007199254740992.0)` was `T`. The comparison is now exact,
and it is `()` (like Common Lisp). KERNEL.md Part V states the exact rule (it
had specified `f64`, #553), and the SBCL port follows it; lamedh-asm still
compares as `f64` (#639). Float-with-float and integer-with-integer
comparisons are unchanged, and NaN is unordered as before. `<=` and `>=` over
more than two arguments are `<` or `=` per pair, so a NaN makes them false.

In typed code, an integer literal beside a `float64` operand elaborates as a
float constant only when `|n| <= 2^53`. A larger literal stays `int64` and the
mixed comparison is a type error, in the native elaborator and the portable
checker alike. **Typed code comparing a `float64` with an integer literal above
2^53 now fails to compile.**

## Arithmetic edge cases (#514, #518, #507)

- `(ash x n)`: a shift of `i64::MIN` saturated incorrectly (the negation
  wrapped, and panicked in debug builds). `(ash 1 MIN)` is 0 and `(ash -1 MIN)`
  is -1. A left shift sets `OVERFLOW` exactly when bits are lost: `(ash 0 100)`
  no longer flags and `(ash 3 63)` does. Compiled constant-shift `ash` follows
  the same rule (#514).
- `(abs -0.0)` returns `0.0`; `-0.0` is not `minusp`, so it came back unchanged
  in both the interpreter and the typed tier. The integer path, including the
  wrapping `abs` of `i64::MIN`, is unchanged (#518).
- `(expt 2.0 4294967296)` returned `1.0`: the `i64` exponent was truncated to
  `i32`. `jit::float_powi` uses `powi` when the exponent fits in `i32` and
  `|x|.powf(e)` with the sign restored from the exponent's parity otherwise. The
  evaluator and the typed JIT share it (#507).

## Errors (#520, #521)

- Non-callable values in "Not a function" and in introspection/`for` type
  errors printed with Rust `Debug` (`Symbol("COMPILED-P")`, `Number(5)`). They
  print with the Lisp printer. A quoted symbol that names a function now says
  `(pass the function it names with #'NAME)`; a macro, fexpr or `VAU` says it
  cannot be applied (#521).
- The recursion-limit hint named `set_eval_depth_limit`, a Rust-only API. It now
  names `lamedh --max-depth N`. Lisp gains `(eval-depth-limit)` and
  `(set-eval-depth-limit! n)`; the setter can lower and restore the limit but
  never raise it past the host's ceiling. Toplevel backtraces collapse runs of
  identical frames: `F (×9998) ← MAIN` instead of `F ← F ← …`. `LAST-BACKTRACE`
  is unchanged (#520).

## Host-trait globals (#463)

`+HOST-TRAITS+` is an alist of `(AXIS . VALUE)` pairs (KERNEL.md Part XII), bound
in every environment. `+NUMERIC-PRECISION-MODEL+` is bound directly. This host
reports `WRAPAROUND-64`. Portable code reads the model instead of probing
`OVERFLOW`.

## `hash-code` builtin (#474)

`(hash-code v)` returns a fixnum hash that agrees with `EQUAL` (`EQUAL` values
have equal codes). It covers identity-compared values (arrays, hash tables,
environments). The underlying `Hash` impl now also hashes closures, macros,
fexprs and `VAU` operatives through their captured environment's pointer; they
were unhashable by value before. Rust: `lamedh::hash_code`.


## New forms: `labels`, `eql`, `type-of`, `make-list`, `string-length`, `parse-integer :junk-allowed`, `ematch` (#526, #529)

- `labels` (`lib/12-control.lisp`) binds local functions that recurse and
  mutually recurse. It expands to `let` + `setq` over lambdas.
- `eql` (an alias of `eq`: numbers compare by type and value, characters by
  value), `type-of`, and `make-list` (`:initial-element`) are in
  `lib/21-cl-compat.lisp`. `eql`, `type-of` and `string-length` have declared
  schemes in `lib/28-types.lisp`.
- `string-length` aliases `string-length*`.
- `(parse-integer s :junk-allowed t)` parses the leading `[sign]digits`:
  `(parse-integer "12x" :junk-allowed t)` is 12.
- `ematch` takes `match`'s clauses and signals an error whose `ERROR-DATA` is
  `(MATCH-FAILURE datum)` when no clause matches; `match-failure-p` recognizes
  it. `match` keeps its `NIL` fall-through. The `match` help now says that bare
  symbols are literals and only `?`-prefixed symbols bind (#529).

## Unicode-aware case conversion and character classes (#519)

`char-upcase`, `char-downcase`, `string-upcase`, `string-downcase`, `alpha-p`,
`alphanumeric-p`, `char-upper-p` and `char-lower-p` were ASCII range checks.
They sit on new kernel primitives backed by Rust's Unicode tables:
`string-upcase*`/`string-downcase*` (full case mapping: `"straße"` becomes
`"STRASSE"`) and `char-alphabetic-p*`/`char-numeric-p*`/`char-uppercase-p*`/
`char-lowercase-p*`. `char-upcase`/`char-downcase` keep their one-character
contract through the one-to-one mapping, so `ß` is returned unchanged. ASCII
behavior is identical. `digit-p` and `whitespace-p` stay ASCII; the reader's
symbol case folding is unchanged.

## Constant-stack and linear-time list and string functions (#504, #510)

- `mapc` returned `LIST` after its recursive call, so `mapc`, `dolist` (which
  expanded to it) and `alist->hash`/`clrhash` used one eval frame per element
  and failed at the 10,000-frame limit. `mapc` now walks with a tail-recursive
  helper; `mapcan` and `mapcon` collect in reverse and `append`/`nconc` right to
  left, with the same call order and concatenations as the nested form;
  `dolist` expands to a `while` loop over a gensym tail and binds `VAR` freshly
  with `let` (#504).
- `string->list`, `string-split`, `string-join` and `remove-duplicates` were
  quadratic. The first three wrap new one-pass kernel primitives
  (`string->list*`, `string-split*`, `string-join*`) and keep the documented edge
  cases (empty delimiter, single-element join). `remove-duplicates` uses an
  `EQUAL`-keyed hash set for numbers, strings and symbols and a member scan for
  other elements (#510).

## `time` measures native execution (#511)

`time` called `step-count`, which arms kernel fuel; armed fuel skips the native
fast path of auto-compiled functions, so `time` clocked the interpreter. `time`
now evaluates without arming fuel and prints `(TIME-MS ms)`. Inside an
already-armed fence it still reports steps (`(TIME-MS ms STEPS n)`). `step-count`
is unchanged. There is a new `time` help entry.

## `lib/45-hashtable.lisp`: a hash table in pure Lamedh (#458)

An optional `hashtable` module: open addressing with indirection. A
`typed-array` of `int64` buckets is the probe index; parallel general arrays
hold keys and values. It grows by doubling before the live-plus-tombstone load
reaches 0.7. API: `make-lht`, `lht-p`, `lht-get`, `lht-has-key-p`, `lht-put!`,
`lht-remove!`, `lht-count`, `lht-keys`, `lht-each`, `lht->alist`, `alist->lht`,
`lht-clear!`. `make-map`, `map-p` and the `map-*` macros pick the native
`hash-table` or an LHT per host. Key equality follows `EQUAL`: `0.0` and `-0.0`
hash alike and `NaN` finds `NaN`. It uses only primitives KERNEL.md Part XI
requires, so it runs on hosts without the native `hash-table`. Tests:
`tests/lisp/72-lamedh-hashtable.lisp`.

## Stdlib fixes: `regex:find` START, `url:parse` empty authority, `~,Nf` on large values

- `(regex:find re s start)` searches the whole string from `START`, so `^` and
  `\b` see the text before it. It sliced `S` at `START`, so `(regex:find
  (regex:compile "^h") "xhh" 1)` matched at 1 and `\bfoo` matched inside
  `"afoo"`, unlike `regex:find-all`. Offsets are still relative to `S`.
- `url:parse` returns `HOST` `""` for an empty authority (`file:///etc/passwd`)
  and still `NIL` when there is no `//`. `url:build` of that result now
  round-trips; it returned `file:/etc/passwd`.
- `(format nil "~,2f" n)` no longer wraps or saturates: `123456789012345678`
  printed `-61010651724749834.76` and `1e17` printed `92233720368547758.07`.
  `(format nil "~,20f" 0.5)` raised "exponentiation overflow". Integers, floats
  whose scaled value exceeds `i64`, and digit counts above 18 take a string
  path. In-range output is unchanged.

## Help text corrections (#508, #517)

Documentation only, no behavior change. `rplaca`/`rplacd` help claimed Lisp 1.5
destructive mutation; the builtins return a fresh cons and leave the argument
unmodified (restoring mutation is tracked in #553). `mod` help claimed a
divisor-signed result; `mod` is Euclidean (`0 <= r < |y|`), so `(mod 7 -2)` is 1.
The manual, `docs/functions/arithmetic.md` (its `(mod 10 -3) => -2` example was
wrong), `docs/generated-reference.md` and `llms.txt` are corrected, and tests
evaluate each documented example.


## Un-annotated `defun`/`defun*` compile under the checker's monomorphic scheme (#512)

Codegen resolves operand types eagerly, so `fib`'s `(+ (fib ...) (fib ...))`
saw the recursive call's unsolved return type and reported "`+`: cannot infer
operand type" although the checker had inferred `(-> (INT64) INT64)`. When the
checker's scheme is monomorphic and compileable it now seeds codegen's
signature as pins. The portable gate (`hm-compile-lambda`, `hm-compile-group`,
`island-discover`) seeds the same pins. Checking-only builtins (`CAR`, `NULL`,
`VARIANT-CASE`, ...) now block with "builtin `X` is not supported in compiled
code" instead of "call to unknown function", natively and portably. Naive
recursive `defun`s now compile natively, so two guards were added: after the
typed call cap, native frames refuse every call once an error is pending (a
`defun-typed` `fib 100000` was OOM-killed from exponential work), and the
membrane skips native retries in a fallback taken because native failed.

## More forms compile: `quotient`, `remainder`/`rem`, `let*`; int literals beside `float64` (#522, #530, #513)

- `quotient` is an alias of `/` (same arity, int64 and float64, same flags).
  `remainder`/`rem` compile to a truncated `BinOp::Rem`, int64-only like the
  evaluator: division by zero raises `Division by zero`, and `MIN rem -1` is 0
  and sets `OVERFLOW`. `let*` desugars to nested single-binding `let` and
  accepts only the `(name init)` pair shape (#522).
- An integer literal beside a `float64` operand in `+ - * /` or a numeric
  comparison elaborates as the float constant: `(+ x 1)` compiles with `x`
  `float64`. Literals only; an `int64` variable is never coerced. N-ary `-` and
  `mod` do not coerce, and neither do `min`/`max` (see the `min`/`max` entry
  above). The portable checker also coerces for `quotient` (#530).
- A `cond`/`when`/`unless`/`case` that ends a `let`, `let*` or `progn` whose
  value is discarded now desugars in statement mode, so it compiles instead of
  failing with "`if` branches disagree" (#513).

## Compiled loops return `NIL` in value position (#524)

A compiled `for`/`while`/`dotimes` evaluated to int64 `0` where the
interpreter returns `NIL`. The elaborator types the loop value `bool` (the word
is still 0, which native code carries as `false`), so a tail-position loop
returns `NIL`. Using the value as a number fails to unify and the function
stays interpreted. Statement-position loops are unchanged.

## Checker: `t`/`nil` functions type as `bool` (#505)

`t` elaborated as the gradual `any`, so a `defun*` returning `t`/`nil` left its
return variable free and generalized to `forall a`; an arithmetic caller was
reported `CHECKED` and failed at runtime. In checking mode, in both the native
and the portable checker: `t` is `bool`; a branch ending in bare `nil` that
meets `bool` is `bool`; an `any` body forces the return to `any`; an `if` with
an `any` branch joins to `any`.

## Typed JIT: redefinition and float negation fixes

- Redefining a typed function with the same arity but different parameter or
  return types left every compiled caller on the old ABI: with `inl` redefined
  from `int64` to `float64`, a caller `(* (inl x) 2)` multiplied the float
  result word as an integer. Callers cannot be re-elaborated, so each direct
  caller's body is now dropped (the policy of `docs/typed-jit-design.md`
  section 3.5). A strict call reports the stale caller and the callee that
  changed; an auto-typed `defun` falls back to its dynamic closure; redefining
  the caller revives it. Functions that had inlined a dropped caller are
  rebuilt.
- A `defun-typed` redefinition that the checker rejects keeps the previous
  signature. It used to install the new signature over the old, still-running
  body, so `see-type` reported the rejected signature and calls misread the
  result word.
- `(- x)` on `float64` is IEEE negation: `(- 0.0)` is `-0.0` in compiled code,
  as in the evaluator. It compiled to `+0.0`.

## Native-tier correctness fixes (#500, #501, #429)

- The auto-typed membrane treated every native `Err` as "arguments did not fit"
  and re-ran the dynamic closure, so a body that had stored into a zero-copy
  typed array and then errored ran its side effects twice, and a
  recursion-limit error could become a silent wrong answer. `Jit::call_entry_aliased`
  returns `JitEntry::Declined` for rejections before the body runs and
  `Entered` once it has; only `Declined` falls back (#500).
- `elab_min_max_compiled` and `elab_array_alloc` reserved temp slots after
  elaborating their arguments, so a nested temp user overwrote the outer
  argument: `(max b (min x 9))` with `b=5, x=1` returned 1, and a Container
  With Most Water kernel returned 8 instead of 49. Slots are reserved first
  (#501).
- Float-touching functions did not compile natively at all (the bitcast carried
  invalid flags), and a callee's pending cross-function tail call could replace
  the caller's own result. Both are fixed through `jit_resolve_pending_tail`.
  Native codegen failures now print the full verifier detail to stderr instead
  of being swallowed (#428, #429).

## Elementwise array ops are vectorized (#525)

`array-div!`, `array-scale!`, `array-fma!` and `array-neg!` lower to the same
2-lane Cranelift SIMD loop plus scalar tail as `array-add!`/`-sub!`/`-mul!`,
replacing the `jit_array_op` trampoline. `array-fma!` uses Cranelift `fma`
(fused, one rounding), so every tier stays bit-identical.

## Native code speed (#434, #435, #442)

Observable as speed only; results are unchanged. Figures are the ones reported
in the commit messages and were not re-measured for this release.

- The Cranelift optimizer is on. `JITBuilder::new` defaults to `opt_level=none`,
  so no optimization pass had run on any native edition. Whole-loop SDF kernel:
  3.3x to 2.8x of Rust; typed-array dot: 3.9x to 3.0x (#434).
- Native locals are cranelift-frontend SSA `Variable`s instead of stack slots,
  and overflow-flag stores are cold. Whole-loop kernels reach 2.0x Rust (#435).
- `Jit::call_by_id` gives all-scalar signatures a stack word buffer and no
  write-back machinery. `jit_call` 215 ns to 92 ns; a pinned `JitFnHandle`
  41.5 ns (Rust floor 2.2 ns) (#442). See Rust embedding API.

## Rust embedding API

- **Breaking:** `LispVal::Macro` holds `Shared<Macro>`, not `Box<Macro>` (#460,
  7a32b81). `Shared` is `Rc` by default and `Arc` with the `arc-val` feature.
  Before: `LispVal::Macro(Box::new(m))`. After: `LispVal::Macro(Shared::new(m))`
  (`lamedh::Shared`). Matching `LispVal::Macro(m)` still reaches fields through
  deref (`m.params`, `m.body`). Code that moved the macro out of the box
  (`let mac: Macro = *boxed;`) must clone instead: `(*m).clone()` (`Macro` is
  `Clone`). `PartialEq` stays structural, so `EQUAL` on macros is unchanged;
  identity comparison is now available as `Shared::ptr_eq`. `Code::Call` also
  gained an `expansion: SharedCell<Option<CachedExpansion>>` field; hosts that
  construct or exhaustively destructure `Code::Call` must add it.
- **Breaking:** `LispError` gains `FuelExhausted` (#457). An exhaustive `match`
  on `LispError` needs a new arm. A host that arms fuel directly, outside a Lisp
  `WITH-FUEL` fence, sees `FuelExhausted` (displayed `Error: fuel exhausted
  (kernel step budget)`), not a `Generic` error; only the owning `WITH-FUEL`
  form converts it.
- **Breaking:** new variants on public enums break exhaustive matches.
  `BuiltinFunc`: `HashCode`, `EvalDepthLimit`, `SetEvalDepthLimit`,
  `ArrayDivBang`, `ArrayScaleBang`, `ArrayFmaBang`, `ArrayNegBang`,
  `StringUpcase`, `StringDowncase`, `CharAlphabetic`, `CharNumeric`,
  `CharUppercase`, `CharLowercase`, `StringToList`, `StringSplit`, `StringJoin`.
  `jit::Ty::Boxed`, `jit::Value::Boxed(LispVal)`, `BinOp::Rem`, and new `Core`
  variants (`FBinary`, `ArrayOp`, `ArraySum`, `ArrayDot`, `BoxedOp`).
- `lamedh::call_function(name, &[LispVal], &env)` calls a Lisp function with
  already-evaluated arguments: no reader, no printer, TCO driven to completion
  (#423). `lamedh::fn_handle(name, &env)` returns a `FnHandle`; `FnHandle::call`
  re-resolves the name on every call, so redefinition is picked up, and
  creation fails at once for an unbound name. Macros, fexprs and `VAU` are
  rejected. About 480-530 ns per trivial call against about 2000 ns for
  `eval_str`.
- `lamedh::native_entry(name, &env)` (feature `jit`) extracts a
  `jit::entry::NativeFnHandle` for a native leaf `defun*` with an
  `int64`/`float64`/`bool` signature and calls the machine code directly, about
  11 ns per call from any thread (#424). Rejected: non-scalar signatures and
  non-leaf bodies (the error names the callees). `NativeFnHandle` is
  `Send + Sync`, pins one native edition (`generation()`, `params()`, `ret()`;
  a redefinition does not redirect it), and offers `call_f1`, `call_f3`,
  `call_i1`, `call_i2` and `call_words`. Raw entries are unmetered and do not
  set `OVERFLOW`/division-by-zero/index conditions in the Lisp world;
  `last_error()` reports them per thread. Native edition storage moved from
  `Rc` to `Arc`. Also `Environment::jit_native_entry` and
  `Environment::jit_generation`.
- `Environment::jit_fn_handle(name)` and `Environment::jit_call_handle(&handle, args)`
  pin a typed function by name with a cached id validated by a registry
  definitions generation, so steady-state calls skip the hash (#427).
  `Jit::defs_generation`, `Jit::call_by_id` and `Jit::FAST_ARGS` back it.
- `jit::JitEntry`, `Environment::jit_call_entry_aliased`,
  `Environment::jit_call_with_array_writeback_aliased` and `jit::array_alias_map`
  expose the aliasing-aware membrane paths (#400, #500). `jit::FUEL_EXHAUSTED`,
  `jit::last_call_arena_peak_words` and `jit::float_powi` are new re-exports.
- Reader features: `Environment::enable_reader_feature`, `disable_reader_feature`,
  `reader_feature_enabled`, `reader_features_list` (#459).
- `lamedh::hash_code(&LispVal) -> i64` backs `(hash-code v)` (#474).

## CLI

- `lamedh --max-depth N` sets the interpreted recursion limit, in nested eval
  frames (default 10000, `N >= 1`). It is also the ceiling for Lisp's
  `(set-eval-depth-limit! n)`. The interpreter runs on a 512 MiB stack, so a
  limit far above the default can exhaust it and abort the process. Compiled
  code has its own non-tail call limit (#520).
- The `OVERFLOW` warning text now also mentions integer literals that exceeded
  `i64` (#515; the reader change itself is in the existing section).
- `--fuel` and `--mcp` bound compiled typed functions (see Fuel exhaustion). New
  test: `cli/tests/mcp_cli.rs::typed_function_runaway_loop_terminates_with_fuel_error`
  (#502).

## Ports and tooling

- **KERNEL.md** (#452, #453) defines the primitive surface every host must
  provide so the shared `lib/*.lisp` corpus loads unmodified, and audits the
  Rust reference, the SBCL port and lamedh-asm against it. Parts II (grammar),
  IV (equality), V (numerics), VI/X (macros, fuel), XI (hash-code) and XII/XIII
  (host traits, reader features) were updated by the entries above.
- **SBCL port, fuel fences** (#455): `WITH-FUEL` debits the enclosing budget by
  what the inner fence spent, so a nested fence can no longer restore an
  exhausted outer budget. `KERNEL-FUEL-SET!` inside a fence only narrows;
  widening or disarming signals an error, as in the Rust reference.
- **Conformance suite** (`conformance/`, `tests/test_conformance.rs`, #621, #622):
  143 implementation-neutral cases, `(case ID SPEC-REF SOURCE EXPECT)`, derived
  from KERNEL.md by Part. The Rust runner gives each case a fresh environment and
  fails any case over 1 s. Cases left out for now are listed in
  `conformance/PENDING.md`. The SBCL and lamedh-asm ports do not run it yet.
- **SBCL port** (`sbcl/`, #449, #485): a standalone Common Lisp implementation
  with its own reader and evaluator that loads the reference `lib/*.lisp` as
  byte copies. Conformance work since: `/` takes two arguments and no CL ratios
  leak (#536); `mod` is Euclidean (#538); `zerop` rejects floats and `if` needs
  three operands (#539); mixed int/float comparison is pairwise through `f64`
  (#539); IEEE results for division by zero, `log 0`, `sqrt -1` (#534); `()`
  prints as `()` and `print` frames as the reference (#537); a catchable
  recursion limit of 10,000 eval frames instead of a stack crash (#532);
  scripts evaluate form by form so a late parse error keeps earlier output
  (#541); a quoted symbol resolves as a function in `funcall`/`apply`/HOFs
  (#533); `uiop:quit` exit code (#535); `rplaca`/`rplacd`, `make-array`,
  `typed-array`, `array-sum`/`array-dot`, `compiled-p`, `defun*` (#540); a
  faster evaluator, memoized lambda compilation and a stdlib fasl cache, with
  `LAMEDH_SBCL_NO_CACHE` to bypass it (#542); `#(...)` arrays print and read as
  in Rust (#607).
- **lamedh-asm** (`lamedh-asm/`): `apply` passes every argument instead of
  truncating at 32 (#545); `*` wrap sets `OVERFLOW` (#546); `PRIN1`/`*-TO-STRING`
  capture buffer grows (#547); data-heap allocator splits and coalesces free runs
  (#548); constant-stack prelude list functions and linear `make-string` (#549);
  out-of-range integer literals read as floats, negative `expt`, `ash` overflow
  (#550); `+ - * < =` type-check with float contagion and catchable errors
  (#543); `apply`/`funcall` on a non-function signals a catchable error (#544);
  prelude basics (`length`, `member`, `dolist`, arrays, `setf`/`push`/`incf`,
  `flet`/`labels`) (#552); dotted pairs, float print rounding, exponents,
  escapes, `if` arity, variadic `#'+`, `iota/1` (#551); floats print as the
  shortest round-trip decimal (#551); free garbage found while the ZCT is full
  in the same collection (#602); `#(...)` arrays (#607).
- **lamedh-asm float reader** (#614): decimal float literals convert to double
  correctly rounded (round half to even, denormals, overflow to infinity)
  through an exact big-decimal scaling. The old fast path was off in the last
  place for long significands, large exponents and `f64::MAX`. New test
  `088_float_read_rounding` (1168 literals).
- **CI:** new `.github/workflows/sbcl.yml` and `lamedh-asm.yml`; `rust.yml`
  changed. Port suites run on `lib/**` changes (#603).
- **Benchmarks:** embedder call-ladder (`benchmarks/embedder`, #427, #433), Lua
  5.4 and LuaJIT comparison (#443), tri-implementation harness over Rust, SBCL
  and asm (#531). `benchmarks/embedder/rust` is excluded from the workspace.
- `examples/vau_and_fexpr_examples.lisp`: four `defvau` operatives and four
  `defexpr` fexprs (#470).

## Typed JIT: tail loops reclaim their per-iteration arena buffers (#212, partial)

An allocating tail loop (Tier 1 self-call or Tier 2a mutual/general tail
call) used to root every array/struct buffer it allocated in the call arena
until the top-level call returned: O(iterations) memory under O(1) stack. Now,
when a function's parameters **and** return type are all `int64`/`float64`/
`bool`/`char`, each self-tail edge (native, closure and interpreter editions)
and each cross-function tail hop between two such functions frees every buffer
rooted since the iteration began. Such a function cannot carry a buffer across
the edge, nor store a fresh one into an older buffer, so nothing freed is
reachable. Loops with a compound or `boxed` parameter/return keep the old
behavior (the next iteration may legitimately use the previous buffer); they
are not covered and #212 stays open for them (needs escape/region analysis,
`docs/typed-region-design.md`). `jit::last_call_arena_peak_words()` exposes the
per-call arena high-water mark for tests.

## Oversized decimal literals set `OVERFLOW` (#515)

A decimal integer literal outside `i64` (`9223372036854775808`) still reads
as a float (KERNEL.md Part II), but the silent precision loss is now
observable: the reader sets the global `OVERFLOW` flag, so the CLI prints its
overflow warning and `(flag-set-p 'OVERFLOW)` is true. Explicit floats,
in-range integers, and forms skipped by `#+`/`#-` set nothing. Q/H/radix
overflow remains a parse error. The CLI warning text now also mentions
literals.

## `cond`/`when`/`unless`/`case` compile natively (#404)

These forms were checker-only placeholders in the typed elaborator, so any
typed body using them stayed interpreted. In codegen they now desugar to
nested `if` (mirrored in the portable gate, `46-hm-check.lisp`). A missed
branch is NIL, which native code carries as `false`:

- In a position whose value is discarded (a non-final body form, or any
  form of a `while`/`for` body) every branch compiles, whatever its type.
- In value position the form compiles when every branch is `bool`;
  otherwise the function stays interpreted, as before.
- `case` compiles for integer keys only (`EQUAL` on int64 is `=`). A
  `t`/`otherwise` default is supported.

The checker also stops reporting a false TYPE-ERROR for `case`: it used to
elaborate clause selectors such as `(1 10)` as calls.

## `dotimes` reaches the compiled tier (#403)

A `defun` whose body uses `dotimes` now compiles natively. The typed
elaborator desugars `(dotimes (var count [result]) body...)` to the same
`let` + `for` (+ result `let`) the macro in `lib/12-control.lisp` expands
to, and the portable codegen gate in `46-hm-check.lisp` mirrors it.
Previously `dotimes` was seen as an unknown call and the function stayed
at the checked tier. `setq`/`while`/`for` over local slots already
compiled.

## Compiled `abs`/`min`/`max` evaluate each argument once; `min`/`max` compile at any arity (#397)

The typed JIT compiles `abs`/`min`/`max` by desugaring them to `if` plus a
comparison. That desugaring cloned the argument's code into the test and
both branches, so `(abs (f x))` evaluated `(f x)` up to three times, and a
side effect in the argument ran more than once. Each argument is now
evaluated once into a temp slot. `min`/`max` also compile at any arity of
at least one, not only with 2 arguments. They fold from the right exactly
as `lib/05-math.lisp` does, so ties between signed zeros resolve the same
way as in the interpreter. The portable codegen gate (`46-hm-check.lisp`)
accepts the same arities.

## `log` and float `expt` compile natively (#398)

`(log x)` (natural log, `float64`) and `expt` now compile in the typed JIT
for float64^float64, float64^int64 and int64^float64. The result is always
`float64`. `log` goes through the existing unary libm trampoline, and
`expt` through a new binary one (`jit_ftrans2`). Each operand combination
calls the same Rust `f64` method as the evaluator's `EXPT` (`powf`, or
`powi(n as i32)`), so compiled and interpreted results are bit-identical.

Not compiled, so these stay interpreted:
- `(expt int int)`: the evaluator returns an integer (a float for a
  negative exponent) and signals on overflow.
- Two-argument `(log x base)`.

The portable codegen gate (`46-hm-check.lisp`) mirrors these rules.

## Clearer error for a `make-array` array passed to a typed call (#399)

`(make-array n)` fills its slots with NIL. Passing such an array to a typed
`(array int64)`, `(array float64)` or `(array char)` parameter used to fail
with the bare message `expected int64 argument, got ()`. The error now
gives the index of the slot, says it is an uninitialized `(make-array n)`
slot, and names the fix: build the array with `(typed-array n 'int64)`,
which is zero-filled, or store every element before the call. Any other
wrong element also reports its index now.

## More elementwise array ops: `array-div!`, `array-scale!`, `array-fma!`, `array-neg!`, and allocating `array-add`/`array-sub`/`array-mul` (#394, #393)

These extend the `array-add!` family. Like the existing ops, each one
writes into `out`, runs over the shortest array operand, and wraps int64
arithmetic:

- `(array-div! out a b)`: `a[i] / b[i]`. **float64 only.** An int64
  division would need its own divide-by-zero behaviour, so the tree-walker
  rejects int64 elements and typed code with int64 arrays stays
  interpreted.
- `(array-scale! out a s)`: `a[i] * s`, where `s` is a scalar of the
  element type.
- `(array-fma! out a b c)`: `a[i] * b[i] + c[i]`. For float64 this is
  fused (`mul_add`, a single rounding).
- `(array-neg! out a)`: `-a[i]`. Float64 negation gives `-0.0` for `0.0`.
- `(array-add a b)`, `(array-sub a b)`, `(array-mul a b)`: return a fresh
  array of `min(len a, len b)` elements. They are defined in
  `lib/17-arrays.lisp`; typed code compiles them to an allocation followed
  by the SIMD `!` op.

In typed code the four new `!` ops compile to a new `Core::ArrayOp` node.
All tiers agree bit for bit with one scalar reference implementation
(`runtime.rs::array_op`). Native code lowers the four ops to Cranelift SIMD
loops (#525), as `add!`/`sub!`/`mul!` already were. The portable codegen gate
(`46-hm-check.lisp`) mirrors all seven forms.

## Same plain array passed twice to a typed function now aliases (#400)

Passing one `LispVal::Array` as two typed parameters used to copy it into
two separate arena buffers and write both back in argument order (last
writer wins), so `(store a 0 111) (fetch b 0)` returned the old value. The
membrane now detects argument identity and gives every occurrence of the
same top-level array ONE buffer, written back once — typed results match
the interpreter on every entry path (`defun-typed`, auto-typed `defun`/
`defun*`, `Jit::call_lisp`), with or without the Cranelift backend. Arrays
nested inside struct or array arguments are still copied independently.

## `array-sum`/`array-dot` over float64, with Fortran `SUM` semantics (#392)

`(array-sum a)` and `(array-dot a b)` now accept `(array float64)` (result
`float64`) as well as `(array int64)` in the typed JIT, the portable
checker's codegen gate (`46-hm-check.lisp`), and the tree-walker (which
also takes typed arrays, and promotes a general array to float64 when any
element is a float). `array-dot` requires both arrays to have the same
element type in typed code.

The element type is never guessed. If nothing else in the function fixes
it (for example an unannotated parameter), the reduction does not
elaborate, so the function stays interpreted, as it already did for `+`
and the elementwise `array-add!` family. Previously an unconstrained
`array-sum`/`array-dot` operand was silently unified with `(array int64)`.

**Contract**, aligned with Fortran's `SUM` intrinsic: for float64 the
result is a processor-dependent approximation of the mathematical sum and
the **order of additions is unspecified** — implementations may reassociate
to optimize, and programs must not depend on a particular rounding order.
int64 stays wrapping (associative, so exact in any order). An empty float
sum is `+0.0`; NaN and infinities propagate as in IEEE addition.

Implementation (not a guarantee): every tier uses one shape — 8 strided
lanes, combined `((l0+l2)+(l4+l6)) + ((l1+l3)+(l5+l7))`, then the `n mod 8`
tail left to right; `dot` rounds each product (no FMA). The native backend
lowers both kinds to a loop over four 2-lane vector accumulators
(`I64X2`/`F64X2`) plus a scalar tail loop, so tiers currently agree
bit-for-bit. Tests check the contract with a `n·eps·Σ|x|` tolerance against
a sequential sum and assert tier agreement only as an implementation test.

## jit: `Ty::Boxed`, an opaque compileable handle to any `LispVal` (#476)

**Breaking for Rust embedders:** `jit::Ty::Boxed` and `jit::Value::Boxed(LispVal)` are new variants of public enums, and `Core` gained variants, so an exhaustive `match` on them needs new arms. None of these enums is `#[non_exhaustive]`.

A new compileable type, surface name `boxed`, lets typed code carry an
arbitrary, un-narrowed `LispVal` as a native `u64` word — a 1-based index
into a per-call root table on `Ctx`, not a raw pointer, so it can never
dangle across the table's own growth and needs no lifetime management
beyond the call that produced it (handles never enter a `LispVal`, so they
cannot outlive their call or be observed by `fork_world`). Boxing a
`Shared`-backed value (array, cons, symbol, hash table, …) clones the
handle, not the contents, so a `store` through it mutates the caller's own
object with no write-back path required — for the inline scalar variants
(number, float, char, nil) it is an ordinary value copy, sound because
they're immutable.

Five intrinsics are the only way to look inside a handle, all routed
through one shared `boxed_op` so the interpreter, closure, and native tiers
can never diverge: `BoxedEqual`, `BoxedHash` (via #474's `hash-code`), and
a general-array trio `BoxedAref`/`BoxedAset`/`BoxedLen` that error-record
(never panic) on a non-array receiver or an out-of-range index, matching
the tree-walker's own `FETCH`/`STORE`/`ARRAY-LENGTH*` wording. A general
(non-typed) array now crosses the typed/untyped boundary as one handle
word instead of being copied element-by-element, which is what makes an
`O(1)`-per-call boundary possible for functions that only touch a few
elements of a large array.

Arithmetic, comparison (`Cmp`), CAR/CDR and other compiled introspection,
cons allocation, narrowing, and a boxed→`int64` coercion are all
deliberately out of scope for v1; `boxed` is never inferred, only written
explicitly in a `defun-typed`/`declare-typed` signature. See
`docs/typed-jit-design.md` §0.5.0 for the full design writeup.

## stdlib: the LHT probe loop compiles (the #476 payoff for #458/#472)

`lib/45-hashtable.lisp`'s per-step hot path, `LHT-PROBE`, is now a
`defun-typed` function with the signature
`((array int64) boxed int64 boxed int64 int64 int64) -> int64`: BUCKETS
crosses as a zero-copy typed array, KEYS and KEY as `boxed` handles, and the
result is one packed int64 (hit = the bucket, miss = `(- -1 bucket)`, full =
below `-cap`) instead of a `(STATUS BUCKET PAYLOAD)` list consed per step.
`LHT-INSERT-EMPTY!` (the rehash loop), `LHT-MIX64` and `LHT-HASH` (pure
int64 once `HASH-CODE` has run; `HASH-CODE` on a `boxed` operand is the
intrinsic) are typed the same way, and `LHT-INDEX` moved from `defun*` to
`defun-typed`, which also removes the `; defun* LHT-INDEX ...` line every
process printed to stderr at startup. `LHT-FIND`/`-GET`/`-PUT!`/`-REMOVE!`
stay interpreted: they read mixed-type slots out of the table record, which
needs the boxed-to-int64 coercion that is a v1 non-goal. They decode the
packed result once per operation.

Measured with `benchmarks/hashtable/bench.lisp` (release, 3000 interned-symbol
keys, three runs each): insert 108-112 to 74-76 us/op, lookup 27-29 to 17
us/op; against the native `HASH-TABLE`, 50x to 37x on insert and 13x to 8x on
lookup. The remaining gap is per-operation interpreted work (`LHT-FIND`'s
frame, the record accessors, two membrane crossings), not the probe loop.

## tooling: the gauntlet's exit status, and clippy's blind spot

`scripts/gauntlet.sh` exits non-zero when any of its four verdict markers is
missing; the verdict file stays the record. Clippy, in the gauntlet, in CI's
lint job and in the documented pre-commit command, now runs with
`--features fuzz`, so `tests/brutal_correctness.rs` is linted. That file
compiles under no other gate, and the first such run found three lints in it.

## The portable type checker and the typed-island front end (#451)

`lib/46-hm-check.lisp` ports the reference host's Hindley-Milner checker
to Lamedh: the type vocabulary, unification with row polymorphism and
nominal subsumption, generalization, the declaration registry and the
elaborator over surface syntax, in both of the native elaborator's modes.
Checking mode drives `condense-verdict` and the condensation honesty
machinery on every host. Codegen mode is the compileable-type gate —
`Infer::resolve` and the `checking: false` rules, eager operand
resolution and the closed call rule included — exposed as
`hm-compile-verdict` and `hm-compile-group`.

`lib/47-typed-island.lisp` is the front end on that gate. `typed-island`
turns a list of names into a manifest: the largest group that compiles
together, each member with a monomorphic signature and a frozen
(macro-expanded) body, closed under calls, discovered in one shared
inference state and verified in a clean one so no rejected member shapes
a signature. `island-optimize` re-gates optimized bodies and reports a
type-changing optimization rather than taking it. `island-forms` renders
the manifest as `declare-typed` + `defun-typed` forms; `island-install!`
hands it to the host kernel and reads the kernel's verdict back per
member, guarded by default so a call outside a signature keeps the
dynamic definition's answer (`'strict` binds the kernel's entry). Author
annotations enter the group as pins. On the Rust host the portable gate and `explain-compile` agree on
every standard-library function, and the stdlib island installs with the
kernel agreeing on every member (`tests/test_typed_island.rs`). See
`docs/typed-island-frontend.md`.

# v0.4.0 — 2026-07-16

The 0.4.0 arc: more of the typed JIT's surface compiles to native code,
error messages teach the language back to whoever hits them, and the CLI
grew a matching set of agent-facing tools (`--check`, `--fmt`/`--fmt-check`,
`--test`, `--mcp`, `--fuel`) plus a cloud-gated ship gauntlet.

## JIT: native loops, TypedArray membrane, inlining, stack allocation,
## array-of-records (#412)

`Core::Assign`/`Core::While`/`Core::For` join the typed JIT IR — `setq` on
local slots, native loops with branch-on-zero tests, and bounded iteration
with overflow-checked, silently-breaking increment (matching the
tree-walker contract) — executed identically across the Core interpreter,
closure compiler, and Cranelift backend (bit-for-bit differential tests).
Stacked on top, in the same arc:

- **May-mutate write-back analysis**: read-only array parameters skip the
  O(n) copy-back, eliminating overhead for read-heavy typed functions.
- **Core-level inlining**: small (<30 nodes), non-recursive, already-
  defined callees are spliced into the caller's Core body at compile time,
  with dependent-invalidation tracking so redefining an inlinee
  recompiles every inliner.
- **`LispVal::TypedArray`**: a flat `u64` buffer with a declared element
  type (`int64`/`float64`); a zero-copy membrane passes the raw buffer
  pointer straight into a matching typed function instead of copying, so
  mutations are shared in place. New builtins `typed-array`/`typed-array-p`.
- **Stack allocation**: escape analysis stack-allocates constant-size,
  non-escaping `Core::ArrayNew`/`StructNew` results via a Cranelift
  `StackSlot` instead of the arena.
- **Contiguous array-of-records**: `(array (struct ...))` with all-scalar
  fields stores elements inline with stride-based addressing
  (`ArrayNewStride`/`ArraySetStride`/`InlineFieldGet`/`InlineFieldSet`);
  the elaborator fuses accessor/constructor calls on array elements into
  fused loads/stores when a whole-tree safety analysis proves every use
  is safe, falling back to the general layout on any escape.

Measured (release, best-of-N): Levenshtein 10k `6673 ms → 74 ms` (~90x,
faster than SBCL on the same machine); a realistic 10-rep workload
`48 ms → 22 ms` (2.2x).

## Teaching errors: did-you-mean + Common-Lisp-ism guidance (#413)

Lamedh has ~zero LLM training-data presence, so error messages double as
in-context documentation. Unbound-symbol / undefined-function errors now
carry two mechanisms (`src/teaching_errors.rs`): a Levenshtein did-you-mean
list over *actually bound* symbols (up to 3 suggestions), and a small
verified table of well-known Common Lisp forms Lamedh deliberately lacks
(`LOOP`, `DEFSTRUCT`, `DEFCLASS`, `DEFMETHOD`, `DEFGENERIC`, `DEFCONSTANT`,
`MULTIPLE-VALUE-BIND`, `VALUES`, `WITH-OPEN-FILE`), each paired with the
real replacement — CL-ism guidance takes precedence when both would fire.

## Loud type inference: `signature` / `compiled-p` / `why-not-typed` (#414)

`defun*` silently fell back to a plain lambda on inference failure, with no
way to ask why from Lisp. Three new introspection builtins make the typed
JIT's decisions observable: `(signature 'fn)` (the inferred type as a
readable sexpr, or `nil`), `(compiled-p 'fn)` (`NATIVE`, `CLOSURE`, or
`nil` — the tier that will actually run), and `(why-not-typed 'fn)` (the
concrete inference-failure reason for a `defun*` that fell back).

## Cloud gauntlet CI + clippy `-D warnings` everywhere (#415)

`rust.yml` now mirrors `scripts/gauntlet.sh` as four parallel jobs
(DEFAULT release, `--no-default-features`, `--features fuzz`, fmt+clippy)
with per-job caches, so PRs merge-gate in Actions instead of on a laptop.
Clippy warnings are errors in the gauntlet script, the documented lint
command, and the CI lint job alike — one policy, three places.

## `lamedh --check`: static verification without execution (#416)

Parses and lints `.lisp` files without running them: parse errors,
unbound-operator calls (with teaching-errors did-you-mean/CL-ism
guidance), and provable arity mismatches on file-defined functions and
plain stdlib lambdas. Zero-false-positive contract verified over the
whole `lib/`, `examples/`, and `benchmarks/` corpus. Exit 0 clean / 1
warnings / 2 parse-or-read failure; `--error-format=sexpr` for
machine-readable findings. Docs: `docs/check.md`.

## `lamedh --fmt`/`--fmt-check` and `lamedh --test` (#417)

A conservative canonical formatter (`src/fmt.rs`) that only rewrites
indentation and incidental whitespace — never token reflow, never
string/char/comment content — validated for idempotence and
meaning-preservation over the whole `lib/` corpus. `lamedh --test` is a
machine-readable runner over `deftest`-registered tests: human `FAIL name:
message` lines plus a `test result: N passed; M failed` summary, or
`--error-format=sexpr` findings mirroring `--check`'s schema.

## `lamedh --mcp` and `lamedh --fuel` (#418)

`--mcp` runs Lamedh as a Model Context Protocol server over stdio
(newline-delimited JSON-RPC 2.0) against one persistent interpreter:
`eval` (fuel-fenced), `check`, `doc`, `apropos`, `run-tests` (fresh
scratch world), `introspect`. Sandboxed by default (all capabilities off)
since it evaluates untrusted, agent-generated code; `--capability` grants
specific ones back. `--fuel N` arms the existing `WITH-FUEL` kernel step
budget per top-level execution unit (script run / `-s` expression / REPL
line), so runaway generated code terminates instead of hanging; arming
fuel disables the native JIT for the metered unit. Docs: `docs/mcp.md`.

## `llms.txt`: the generated LLM reference

A single dense file (~70 KB) for models and agents to read once and write
competent Lamedh immediately: syntax/semantic gotchas, a Common-Lisp-
divergence cheat sheet, the whole `HELP-DB` standard library as one line
per function (signature + description, generated live from the built
interpreter via `lib/97-doc-renderer.lisp`'s `render-llms-index`), and ten
worked examples (records/row polymorphism, protocols, pattern matching,
sum types, conditions/restarts, the typed JIT, modules, regex, testing,
`--mcp`) each verified with `lamedh --check` and an actual run.
`scripts/generate-llms-txt.sh` (wired into `scripts/generate-docs.sh`)
regenerates it from `docs/llms-txt-template.md` plus the live index and
`src/lib.rs`'s module table, so it can never hand-drift from the
interpreter; `tests/test_llms_txt.rs` guards the 100 KB budget. Checked in
at the repo root and mirrored at `docs/llms.txt` for GitHub Pages, linked
from `docs/llms.md`.

# v0.3.0 — unreleased

## Fuzz battery off the default path (dev tooling)

`tests/brutal_correctness.rs` — the randomized JIT differential/metamorphic
fuzz battery (nine batches, tens of thousands of random programs, minutes in
debug) — is now gated behind a non-default `fuzz` cargo feature. Plain
`cargo test` compiles it to nothing and skips it entirely; the ship gauntlet
(`scripts/gauntlet.sh`) and CI run `--features fuzz` in **release**, where the
same battery is seconds. `BRUTAL=1` still scales every batch to its deep
sweep. This takes the single biggest tax off day-to-day and agent iteration
with no loss of ship-gate coverage.

## perf(startup): with_stdlib world fork + module-rewrite kernel builtin

Two attacks on stdlib-loading cost, one per consumer shape:

**Many environments per process** (test suites, per-request sandboxes):
`Environment::with_stdlib()`/`with_prelude()` now build a private
per-thread prototype world once and serve every call as a **deep-copy
fork** (`Environment::fork_world`): a memoized graph copy that
duplicates the symbol table (same names/ids, fresh `Rc` cells), every
global value cell, environment frame, closure (including pre-compiled
`Code` bodies), hash table, array, plist, and the typed registry —
preserving sharing and `EQ` identity isomorphically, sharing only
immutable/host-owned values (builtins, `Native`/`Extension` handles,
routing tables, typed declaration-plane definitions, closure-tier
compiled editions). Forked worlds share no mutable cell, so isolation
is exactly that of independent full loads; a world holding live host
handles (ports/sockets/children/channels) refuses to fork and the
caller falls back to a real load. Measured: a fork costs ~65 ms
release / ~130 ms debug vs ~740 ms / ~5.6 s for a full load, and
`cargo test --test test_examples` (61 fresh environments on one
large-stack thread) drops accordingly. `tests/test_helpers.rs`'s
`env_with_stdlib` now rides `with_stdlib()` instead of re-reading
`lib/` from disk. New `tests/test_world_fork.rs` pins the contract:
isolation (defs, redefinitions, plists, registries, capability flags,
dynamic vars, typed definitions), symbol identity, late binding, full
vocabulary, fresh/fork agreement, and fork-is-cheap.

**One environment per process** (the CLI): cold start was dominated by
the module system's rename walk — `$MODULE-REWRITE` did an O(renames)
`ASSOC` per AST node over every `WITH-MODULE` body (loading
`40-http.lisp` alone cost ~390 ms). The walk is now the `SEXPR-RENAME`
kernel builtin (hash-table lookup per symbol, quote/quasiquote
untouched, exact same semantics) plus a single merged
`module.functions` registry write instead of a quadratic per-name
append; `with-module` keeps the same surface. New
`Environment::with_stdlib_fresh()`/`with_prelude_fresh()` run the
loader directly with no prototype retention; the CLI uses
`with_stdlib_fresh()` (it builds exactly one world). Measured:
`40-http.lisp` 393 ms → ~118 ms, full embedded load ~985 ms → ~710 ms
on the same machine.

## Gauntlet speed (dev tooling)

`scripts/gauntlet.sh` runs the authoritative verification in **release**
(the `test_examples` and `brutal_correctness` suites are execution-bound —
release runs them several times faster than debug, repaying the one-time
compile) and writes an explicit verdict file the caller gates ship on.
`brutal_correctness`'s default smoke sweep dropped 4_000 → 1_200 random
programs (`BRUTAL=1` still runs the 50_000-program deep sweep;
`BRUTAL_PROGRAMS` overrides either default). No behavior change to the
interpreter — verification only.

condensation

Everything from the v0.2.0 tag to here. The headline: **one record
definition form** over one type story — records, sums, HM generics,
guards, processes, patterns, modules, and the checker meeting in one
language. Sections below, roughly newest first.

## jit: `abs`/`min`/`max` compile to native (comparison-select desugar)

`abs`, and the 2-argument `min`/`max`, now compile in a `defun-typed` body
for `int64` and `float64`. They desugar to already-compilable Core —
`(abs x)` → `(if (< x 0) (- x) x)`, `(max a b)` → `(if (> a b) a b)`,
`(min a b)` → `(if (< a b) a b)` — exactly mirroring their `lib/05-math.lisp`
definitions. This is **comparison-select, not the FP `fmin`/`fmax`/`fabs`
instructions**, so it matches the evaluator's `-0.0`/NaN behaviour bit-for-bit
(e.g. `(abs -0.0)` stays `-0.0`; `fabs` would give `+0.0`). No new kernel
machinery — purely an elaboration change. Previously `abs`/`min`/`max` on a
`float64` did not compile at all (the Lisp `abs` compares against integer `0`,
and `min`/`max` are variadic `&rest` functions). Variadic `min`/`max` (other
than 2 args) stay interpreted. Fuzzer-verified.

## jit: SIMD-vectorized integer array reductions (`array-sum`/`array-dot`)

The reduction half of the array-op family the elementwise ops
(`array-add!`/`-sub!`/`-mul!`) promised: `(array-sum a)` (wrapping sum of
every element) and `(array-dot a b)` (wrapping sum over `min(len a, len b)`
of `a[i]*b[i]`, each product wrapping too), both `(array int64)` **only** —
float reduction reorders rounding and needs a reassociation policy this
family does not attempt. In a `defun-typed` body both compile to a 2-lane
`I64X2` vector accumulator loop plus a horizontal `extractlane`/`iadd`
reduction of the two lanes and a scalar tail for an odd final element.
Soundness rests on one fact: **wrapping int64 addition is associative**
(`(a+b)+c ≡ a+(b+c) mod 2^64`), so a pairwise/multi-lane vector reduction is
**bit-identical** to a sequential left-fold — verified across all three
tiers (native Cranelift, Core interpreter, tracing interpreter), including
MAX+MAX wraparound landing in both the vectorized body and the scalar tail.
New `Core::ArraySum`/`Core::ArrayDot` nodes; codegen-only elaboration,
int64-only by construction (no float overload exists to fall back to).

## jit: SIMD-vectorized elementwise array ops (`array-add!`/`-sub!`/`-mul!`)

A first array-op family for optimizing programmers: `(array-add! out a b)`,
`(array-sub! out a b)`, `(array-mul! out a b)` — elementwise, out-param
(writes into a preallocated `out`, returns it), over `(array int64)` /
`(array float64)`, iterating the minimum of the three lengths. In a
`defun-typed` body they compile to a **SIMD loop**: a 2-lane `I64X2`/`F64X2`
vector body (which Cranelift lowers to SSE on x86-64 and NEON on aarch64)
plus a scalar tail for an odd final element. Semantics are **wrapping** and
flag-free — a bulk op sets no per-element OVERFLOW; a vector `iadd` is
two's-complement wraparound by construction. Because SIMD lanes are
independent (no reduction/reassociation), results are bit-identical to a
scalar loop for both int and float — verified across all three tiers
(native Cranelift, Core interpreter, tracing interpreter). Allocating/sugar
variants and reductions (`array-sum`/`array-dot`) come later. New
`Core::ArrayMap2` node; codegen-only elaboration. (Built via a Sonnet
subagent; supervisor-verified + gauntlet-gated.)

## jit: bitwise ops compile to native (`logand`/`logior`/`logxor`/`ash`)

The int64 bitwise operators now compile. `logand`/`logior`/`logxor` are
variadic and fold to nested native `band`/`bor`/`bxor` (empty call → the
op's identity, `-1`/`0`/`0`, like the evaluator). `ash` compiles when its
shift is a **compile-time constant in `-63..=63`**: a positive shift folds
to a left shift, a negative to an arithmetic right shift by its magnitude,
`0` is the identity. A runtime or out-of-range shift stays interpreted —
the evaluator's `ash` masks / overflows / sign-fills past 63, edge cases the
constant path sidesteps rather than mis-lowering. This lets checksum-style
code — `(logxor (ash crc -8) table-entry)`, `(logand (ash n -8) 255)` —
reach `TIER . COMPILED`, bit-identical to the interpreter. New `BinOp`
variants; codegen-only so checking keeps the declared/variadic schemes.
Soundness-gated by the differential fuzzer.

## io/jit: `(float x)` int→float conversion, compiled

New `float` builtin: int→float conversion (identity on a float) — the
explicit numeric crossing the strict typed island needs, since `+`/`-`/`*`/
`/` deliberately don't coerce int and float (issue #324). It compiles to
native via a new `Core::IntToFloat` node (`fcvt_from_sint`; `i64 as f64` in
the Core interpreter). This unblocks the common `fract` idiom
`(- x (float (floor x)))` — `floor` returns `int64`, so the conversion is
what lets it subtract from a `float64`. A value-noise hash
`(let ((v (* (sin …) k))) (- v (float (floor v))))` now reaches
`TIER . COMPILED`, bit-identical to the interpreter. Soundness-gated by the
differential fuzzer.

## jit: `sin`/`cos`/`tan`/`exp`/`round` compile to native (libm trampoline)

The transcendental float intrinsics `sin`, `cos`, `tan`, `exp`, and the
half-away-from-zero `round` now compile too, completing the unary float set.
They have no direct Cranelift instruction, so the native backend calls a new
`jit_ftrans` trampoline — which is a one-line wrapper over the same
`FUnOp::apply_word` the Core interpreter uses, so the compiled and
interpreted results are identical by construction (`(sin 1.0)` →
`0.8414709848078965` either way; `(round 2.5)` → `3` and `(round -2.5)` →
`-3`, i.e. `f64::round`'s half-away rounding, not Cranelift `nearest`'s
ties-to-even). A typed body can now compile real float math end to end. The
trampoline is `jit`-only; the dependency-light checker/Core-interpreter path
computes the same values without it. Soundness-gated by the differential
fuzzer.

## jit: `sqrt`/`floor`/`ceiling`/`truncate` compile to native

The typed JIT now lowers the unary float intrinsics `sqrt`, `floor`,
`ceiling`, and `truncate` to native code — a new `Core::FUnary` node wired
through all three executors (the Cranelift backend, the Core interpreter, and
the disassembler). `sqrt` uses the `fsqrt` instruction and yields `float64`;
`floor`/`ceiling`/`truncate` use the `floor`/`ceil`/`trunc` instructions
followed by a **saturating** `fcvt_to_sint_sat`, which matches Rust's
saturating `f64 as i64` that the evaluator uses — so `(floor -2.3)` → `-3`
and `(sqrt 2.0)` → `1.4142135623730951` are bit-identical whether
interpreted or compiled (a single `FUnOp::apply_word` is the shared source of
truth). A typed body doing real float math like
`(sqrt (+ (* a a) (* b b)))` now reaches `TIER . COMPILED`.

The lowering is codegen-only: in checking mode these keep their permissive
declared schemes from `lib/28-types.lisp` (argument `any`); the compiled path
requires a concrete `float64` argument or the function stays interpreted. The
transcendentals (`sin`/`cos`/`exp`) and half-away-from-zero `round` need a
libm trampoline and land separately. Soundness-gated by the differential
interpreter-vs-JIT fuzzer.

## jit: `let` and `progn` compile to native

The typed JIT now lowers ordinary `let` and `progn` to native code, not just
the explicit `let-typed` spelling. Both were previously gated to the checker
only (they typechecked but stalled a `defun-typed`/one-door `defun` body at
the CHECKED tier), even though their elaborators already emit real codegen
core — `elab_let` a `Core::Let`, `elab_body` a `Core::Seq` — the very nodes
`let-typed` compiles. A typed body built from `let`, `progn`, `if`,
arithmetic, comparisons, and array `fetch`/`store` now reaches `TIER
. COMPILED` and runs as native code (e.g. a recursive array fill compiles and
executes identically to the interpreter).

The boundary is principled: `cond`/`when`/`quote` (and the list/pair forms)
stay checker-only, because their elaborators emit only a placeholder
`Core::LitI(0)` for typing purposes — compiling them would miscompile to
literal `0`, so they correctly refuse the native tier and keep the whole
function interpreted. Verified sound by the differential fuzzer
(interpreter-vs-JIT) across the stdlib, since one-door `defun` now
auto-compiles any function whose body uses `let`/`progn`.

## test: split the examples monolith; adopt cargo-nextest as an optional runner (#58)

`tests/test_examples.rs` ran all 51 classics programs inside a single
`#[test]` looping over `examples/`, making it one ~200s serial tall pole.
It is now **one test per example** (a `example_tests!` manifest expanding
to a `#[test]` each, plus an `examples_manifest_is_complete` guard that
fails loudly if a program is added/removed without updating the manifest).
Each program builds its own stdlib environment — cheap since the world-fork
(#370) — so the harness runs them in parallel: the example suite's tall
pole drops from the sum of all 51 to the single slowest program (~4× on the
binary in isolation). This speeds any binary-serializing runner, including
the release `scripts/gauntlet.sh` ship gate.

Also adds `.config/nextest.toml` and documents **`cargo nextest run`** as an
optional faster local runner (process-per-test, full cross-binary
parallelism; it inherits the 128 MiB `RUST_MIN_STACK` from
`.cargo/config.toml`, so stdlib-building tests pass under it). nextest is
for fast local iteration — the authoritative ship gate is still the release
gauntlet (nextest does not run doctests). Measured on this machine (debug,
full default suite): `cargo test` 806 s → `cargo nextest run` ~651 s; the
monolith split is what closes most of that gap for `cargo test` itself.

## modules: help subsystem registered; #56 namespacing complete

`doc-renderer` and `help-system` gain metadata-only `defmodule`
declarations, completing issue #56: **every optional stdlib library is now
a registered module.** The help surface stays flat — `help`/`apropos` are
REPL UX and `register-doc`/`register-category` are the documentation API
called across the whole codebase — so this is registration, not
qualification.

With this, the #56 retrofit is done. Two libraries were fully **qualified**
because their public names are genuinely tool-shaped: `shell` (`shell:sh`,
…) and the `match` search/rewrite tools (`match:sgrep`, `match:rewrite`,
…). Every other optional — the language-defining forms (variants,
protocols, guard, condensation's records, types), the ergonomically-flat
DSLs (testing, instrument, lisp15), the pipeline-hook/kernel-pinned
surfaces (rules, optimizer-vau, call-graph), and the help subsystem — is a
**metadata-only registered module** whose names stay exactly where they
were. Introspect any of them with `(module-p 'name)`,
`(module-exports 'name)`, `(module-requires 'name)`. The prerequisite
load-order reorder (module system ahead of the optionals) is the
foundation this all rests on.

## modules: sgrep/rewrite tools move under the `match` namespace (#56)

The structural search-and-rewrite tools in `lib/23-match.lisp` are now
qualified under the `match` module: `match:sgrep`, `match:sgrep-fn`,
`match:sgrep-source`, `match:sgrep-file`, `match:rewrite`. The matcher
**engine stays flat** — `pat-match`, `match`, `destructuring-bind`,
`instantiate`, `match-fail-p` are unchanged: `match`/`destructuring-bind`
are language forms, and `pat-match`/`instantiate` are the deconstruct/
reconstruct engine primitives other libraries build on directly (e.g.
`lib/24-rules.lisp` uses flat `pat-match`/`instantiate` and is unaffected).
Only the clearly tool-shaped `#171` search/rewrite surfaces are namespaced.
`instantiate` was moved up beside the engine so the tool functions form one
`with-module` block; the flat engine names they call are not module-local,
so `with-module` leaves those calls untouched. **Behavior change**: call
the tools qualified (`match:sgrep …`) or `(import match)`.

## modules: register the flat optional libraries as modules (#56)

Eleven optional libraries whose names are deliberately kept flat —
`lisp15`, `testing`, `optimizer-vau`, `call-graph`, `guard`, `rules`,
`variants`, `instrument`, `types`, `protocols`, `help-data` — now each
carry a `(defmodule NAME (:export ...))` declaration for introspection
and registration. This is **metadata only**: there is no `with-module`
body rewrite and nothing is imported, so every name stays exactly where
it was (`deftest`, `defvariant`, `defprotocol`, `trace`, `attrib` … are
all still flat) and no caller changes. The gain is uniform module
introspection: `(module-p 'testing)` → `T`, `(module-exports 'variants)`,
`(module-requires 'guard)`, alongside the fully-qualified modules
(`shell`, and the #253 modules). These libraries stay flat by design —
they are language-defining forms (variants, protocols, guard) or
ergonomically-flat DSLs (the `testing` assertions, the `instrument`
debug verbs, the Lisp 1.5 compat names) — so registration, not
qualification, is the right treatment. `types` and `help-data` are data
modules (type-table axioms / help database) with empty export sets.

## modules: shell helpers move under the `shell` namespace (#56)

`lib/07-shell.lisp` is now the `shell` module (`defmodule shell`, built on
the reordered load sequence from the previous entry). Its five helpers are
qualified: `shell:sh`, `shell:shell-exit-code`, `shell:shell-stdout`,
`shell:shell-stderr`, `shell:shell-ok-p` — call them qualified or bind them
flat with `(import shell)`. The module declares `(:requires SHELL)` for
capability introspection (`(module-requires 'shell)`). The bare kernel
`shell` builtin (the `(exit-code stdout stderr)` primitive) is unchanged and
stays flat — only the Lisp helper layer is namespaced. **Behavior change**:
`(sh ...)` and the flat accessors are no longer bound by default; use
`shell:sh` or `(import shell)`. Guard's `capabilities-needed` still infers
`SHELL` through `shell:sh` (it walks the call graph into the kernel
primitive). First of the #56 tool-library retrofits.

## internal: stdlib load order — module system ahead of the optionals (#56 prep)

Reordered the embedded `STDLIB_SOURCES` load sequence so the module
system (`20-condensation.lisp` then `27-modules.lisp`) loads immediately
after the Prelude, ahead of every optional library. Previously the
pre-0.3 optionals (`07-shell`, `10-testing`, `11-optimizer-vau`,
`19-call-graph`, `22-guard` … `26-instrument`) loaded *before*
`27-modules.lisp`, so none of them could be wrapped in
`DEFMODULE`/`WITH-MODULE` at their own load position — the blocker for
the #56 namespacing retrofit. Only the load ORDER in `src/lib.rs`
changed; filenames keep their historical numbers, the optionals are
reordered by their real dependency edges (optimizer-vau → rules,
call-graph → guard, types → protocols), and `PRELUDE_SOURCES`
(`with_prelude`) is untouched. No user-visible behavior change: every
name still resolves identically; this only makes the module system
available to the libraries that will be retrofitted onto it.

## io: `read-file-section` decodes strictly — no more silent lossy text coercion (#359)

`read-file-section` decoded its bytes with `from_utf8_lossy`, silently
substituting U+FFFD for any non-UTF-8 byte — exactly the implicit lossy
text coercion epic #253's design rules out (text must cross the byte
boundary explicitly). It now decodes **strictly**: invalid UTF-8 signals a
descriptive error naming the offending byte offset, mirroring
`text:utf8->string`. Two explicit opt-ins replace the old silent behavior:
`read-file-section-lossy` (U+FFFD substitution, like
`text:utf8->string-lossy`) and `read-file-section-bytes` (the raw bytes as
an `Array<Char>`, to cross the text boundary yourself via the `text:` /
codec substrate). Both are READ-FS-gated like their sibling. `read-file`
(whole-file) already errored on invalid UTF-8 via `read_to_string`, and
`read-file-byte` already returned a raw integer — neither was ever lossy,
so neither changed. **Behavior change**: code that relied on
`read-file-section` silently tolerating non-UTF-8 input must now call
`read-file-section-lossy` (same result) or `-bytes` (raw). Tests:
`tests/test_read_file_section.rs`.

## stdlib: tail-recursive Prelude list/string builders — no more eval-frame overflow on ordinary-sized input (#361)

`string->list`, `filter` (the LIST fallback under the protocol), `take`,
`take-while`, `butlast`, `zip`, `pairlis`, `copy` (and its actual dispatch
target for lists, `copy-list*`), `remove-duplicates`,
`string-join`, `string-repeat` (and `make-string`, which calls it),
`string-capitalize`'s word-walk, and `string-split` recursed
non-tail-recursively — each built its result via `(cons x (recurse ...))`
(or the CONCAT equivalent for the string builders) with the recursive call
*inside* the cons, not in tail position. That is genuinely non-tail: it
consumed one native eval frame per input element, hitting the ~10000-frame
recursion guard on inputs of only a few thousand elements/characters —
`(string->list <4kb string>)` or `(filter #'evenp <5000-element list>)`
errored outright. Found during #257 (codecs); its workaround was local to
the codec modules, so every flat-Prelude caller stayed exposed.

Each is now a tail-recursive accumulator (+ `reverse`/`reverse-aux` at the
end) or, for the string builders, a direct accumulating concatenation —
the evaluator's TCO keeps native stack usage O(1) regardless of input
size. Purely an internal rewrite: arguments, return values, and edge
cases (empty input, `n <= 0`, `n` past the end, dotted tails) are
unchanged; new `$`-prefixed accumulator helpers are internal. Declared/
inferred type schemes for every touched public function are unchanged
(verified via `see-type` before/after).

## runtime(net): TLS port wrapping behind an off-by-default cargo feature (#365)

A new optional embedded module, `TLS` (`lib/43-tls.lisp`), wrapping a
connected TCP `Port` (#258) as a TLS client or server — the seam #258's own
design predicted. Backed by `rustls` + `webpki-roots` + `rustls-pemfile`
(owner ruling, #364/#365: rustls over native-tls, so nothing here links
system OpenSSL/SChannel/SecureTransport), all three behind a new **`net-tls`
cargo feature that is NOT in the default feature set** — the default build's
behavior and dependency tree are byte-identical to before this change; only
`cargo build --features net-tls` (or `cargo test --features net-tls`) pulls
in a TLS crate at all. The crypto provider is `ring`, not `aws-lc-rs`: no
cmake/nasm build-tool requirement.

A TLS stream is an ordinary `PORTS` `Port` (two new `PortState` variants,
themselves gated by `net-tls`): every `PORTS` read/write/close operation and
every `TCP` out-of-band operation (`shutdown!`, `set-read-timeout!`/
`set-write-timeout!`, `local-addr`/`peer-addr`) keeps working on it
unchanged. `tls:wrap-client`/`tls:wrap-server` take ownership of the
underlying TCP connection out of the plaintext `Port` you pass in (it
becomes `CLOSED` the instant it is wrapped — never simultaneously reachable
as both plaintext and TLS); `tls:connect`/`tls:connect-insecure!` are
`tcp:connect` + wrap sugar for the common case.

Verification is on by default (default root store: Mozilla's set, via
`webpki-roots`, plus any `:extra-roots` you supply as PEM — a String path,
`READ-FS`-gated, or `Array<Char>` bytes — this is also how a test harness
trusts a throwaway self-signed CA) and checks the certificate against
`:hostname`, also used for SNI. `:alpn` offers protocol names;
`tls:alpn-protocol` reads back what was negotiated. `tls:peer-certificates`/
`tls:peer-certificate-summary` expose the peer's raw DER chain and a
structural summary (count, leaf length, leaf bytes) — no X.509 parser is
part of this dependency ruling, so there are no parsed subject/issuer/expiry
fields, by design. `tls:sni-hostname` reads the SNI a client offered,
server-side. Server cert/key are supplied the same way as `:extra-roots`
(PEM path or bytes); a handshake timeout is the underlying TCP port's own
read/write timeout, set before (or after) wrapping.

**No insecure bypass without an explicitly named API, and a host opt-in.**
`tls:connect-insecure!`/`tls:wrap-client-insecure!` are the only way to skip
certificate verification — there is no keyword flag on `tls:connect` that
silently does it — and even they signal a structured `:policy-denied` error
unless the *host* embedding this interpreter separately calls the new
`Environment::set_allow_insecure_tls` (Rust-only, default `false`, modeled
on `set_net_policy`'s "Lisp cannot install/inspect this" shape). Lisp code
alone can never disable verification, no matter what it calls. Key material
is never exposed to any printer path.

**Feature-off behavior, not feature-off absence.** Every `TLS-*` kernel
builtin is registered unconditionally, so `(require 'tls)` and every
`tls:*` name always resolve regardless of how the crate was built — with
`net-tls` compiled out, every one of them except `tls:available-p` (which
reports the compiled-in state) signals a structured `:category
:tls-unavailable` error instead of doing any work, rather than an
unbound-variable error.

`HTTP` (#259, `lib/40-http.lisp`) gains `https://` client support when
`net-tls` is compiled in: `$http-check-scheme!` — the single scheme
checkpoint the plaintext-only design deliberately isolated for exactly this
— now permits `https://` (initial request or redirect `Location` alike)
when `(tls:available-p)`, connecting via `tcp:connect` then `tls:connect`
before speaking HTTP/1.1 over the resulting port; `http:request`/`get`/
`post` gained an `:extra-roots` keyword forwarded to `tls:connect`. With
`net-tls` compiled out, `https://` keeps its previous structured
`:https-unsupported` error (now naming the `net-tls` feature rather than
this now-closed issue). Redirects still never silently cross schemes.
Server-side `https://` (wrapping an accepted connection before `serve`/
`serve-one!` parses it) is out of scope here — `tls:wrap-server` already
composes directly with `tcp:accept` for a caller that wants one without
going through `HTTP`'s own accept loop.

Tests: `tests/test_tls.rs` (`#[cfg(feature = "net-tls")]`-gated,
loopback-only) — a throwaway self-signed CA + "localhost" leaf certificate
generated fresh per test via the `rcgen` **dev-dependency** (never a
build/normal dependency, so it does not affect `net-tls`'s "off by default"
story for embedders); client/server round trip both directions of the wrap,
negotiated ALPN, peer-certificate summary, verification failure against an
untrusted CA with the default root store, `connect-insecure!` denied without
the host opt-in and working with it, and `https://` end to end through
`http:request`. Feature-off behavior is covered in the normal suites instead
(`tests/test_net.rs`, `tests/test_http.rs`), gated
`#[cfg(not(feature = "net-tls"))]` so they run in the default and
`--no-default-features` builds and stay correct once `net-tls` is on.
Manual: `docs/manual/13-networking.md` §13.8; embedding: `docs/embedding.md`.

## runtime(os): typed Linux/POSIX interface, opaque owned handles, no raw syscalls (#260)

Two new optional embedded modules over a new `OS-*`/`OS-LINUX-*` Rust
kernel substrate (`src/evaluator/builtins_os.rs`, zero new crate
dependencies — `std::env`/`std::process`/`std::fs`/`std::time` only,
plus one hard-coded `kill(2)` FFI declaration, see below): `OS`
(`lib/41-os.lisp`, portable) and `OS-LINUX` (`lib/42-os-linux.lisp`,
typed Linux-specific facilities), per the epic's "portable module + a
Linux-only module, unsupported operations signal structured errors
rather than binding opaquely" ruling.

**No raw syscall numbers, no bare file descriptors/PIDs as authority.**
A spawned process is `LispVal::OsChild` — an opaque handle (compares by
identity, deterministic `wait!`/`try-wait!` reap, Drop backstop) never
reconstructible from a printed integer; its stdio pipes (when
requested) are ordinary #255 `PORTS` ports, reusing the existing
`wrap_reader`/`wrap_writer` embedder API rather than adding new port
representations. Signals go only through a fixed typed-name table
(`os:signal!`, `:term`/`:kill`/`:hup`/...) — never a raw signal number.

`OS`: process identity/environment (`args`, `executable-path`, `cwd`,
`chdir!`, `env-get`/`env-list`/`env-set!`/`env-unset!`, `pid`, `ppid`,
`hostname`), time (`now`, `monotonic-nanos`, `elapsed-seconds`,
`sleep`/`sleep-seconds`), randomness (`make-prng`/`prng-next` — a pure,
explicitly-seeded SplitMix64 generator — and `random-bytes`, OS-backed
secure entropy from `/dev/urandom`; the pre-existing global `random`
convenience function is untouched, not replaced), and process
spawn/control (`spawn` — explicit argv with **no shell interpolation**,
explicit/inherited environment, per-stream `:inherit`/`:null`/`:pipe`
stdio, returning a `(:handle :stdin :stdout :stderr)` alist —
`process-wait!`/`process-try-wait!`/`process-kill!`/`process-terminate!`/
`process-alive-p`/`process-id`, and `signal!` for PIDs not held as an
owned handle). `OS-LINUX`: `stat`/`lstat` (typed alist — size, mode,
uid/gid, nlink, ino/dev, mtime/atime/ctime, is-dir/is-file/is-symlink;
never a raw C struct) and `readlink`.

**Capabilities** (all off by default, same `require_*`/fence-attenuation
machinery as `READ-FS`/`NET-CONNECT`): `OS-ENV` (read identity/env),
`OS-ENV-WRITE` (mutate it), `OS-PROCESS` (spawn — a returned child
handle needs no further grant to `wait!`/`try-wait!`/`kill!`/
`terminate!`, the epic's "a returned handle is authority to continue"
rule), `OS-SIGNAL` (signal a PID not held as an owned handle).
`OS-LINUX-STAT*`/`OS-LINUX-READLINK*` reuse the existing `READ-FS` grant
rather than inventing a parallel one. Time/randomness are ungated (pure
or read-only-entropy, mirroring the pre-existing global `random`). A new
host policy hook, `Environment::set_os_policy`/`OsOperation`, mirrors
`set_net_policy` for spawn/signal (scope a grant to a specific
executable/argv/cwd or target PID) — checked in addition to, not
instead of, the capability.

**Structured errors** throughout (`:category` `:not-found`/
`:permission-denied`/`:already-exists`/`:invalid-argument`/`:closed`/
`:policy-denied`/`:unsupported-platform`/`:signal-failed`/`:other`),
following #258's `builtins_net.rs` pattern.

**Deferred** out of this increment (see the ticket's own "only after the
basic resource/polling model is stable" sequencing): a standalone `pipe`
primitive not tied to a spawned child, terminal/PTY settings,
eventfd/timerfd/signalfd/epoll, Unix-domain sockets, and openat-style
directory-relative operations.

**`kill(2)` FFI**: `SIGTERM`/arbitrary-PID `SIGNAL!` need a signal number
Rust std has no safe API for; `src/lib.rs` declares one hard-coded
`unsafe extern "C" { fn kill(pid: i32, sig: i32) -> i32; }` (the same
technique `std::process::Child::kill` uses internally for `SIGKILL`) —
a single fixed, typed, internal binding, not the "arbitrary FFI"/"raw
syscall numbers" the epic rules out; Lisp only ever reaches it through
the fixed signal-name table and the typed kernel primitives.

Relation to `lib/07-shell.lisp`: `shell`/`sh` (unchanged) remain the
`/bin/sh -c` convenience escape hatch; `os:spawn` is this ticket's
principled typed layer — explicit argv, no shell interpolation,
non-blocking wait, and kill/terminate control none of `shell`'s API
exposes.

## stdlib(http): capability-gated streaming HTTP/1.1 client and server (#259)

A new optional embedded library, `HTTP` (`lib/40-http.lisp`) — an
HTTP/1.1 client and server written **entirely in Lisp** over the #258
TCP substrate and the #257 codecs (`URL` for parsing, `MIME` for
ordered/case-insensitive multi-value headers, `JSON` for
`http:collect-json`): **zero new crate dependencies, zero new Rust
kernel surface** (the ticket's "back it with a Rust HTTP crate" text was
overridden by the epic's no-new-dependency ruling; status-line/header/
chunked framing proved to be ordinary Lisp string/byte work). HTTP/2 and
HTTP/3 are out of scope. `(require 'http)` on a prelude environment;
`with_stdlib()` loads it eagerly like every optional module.

**Plaintext `http://` only**: TLS is pending the owner dependency ruling
(#365), so `https://` — given directly or arriving via a redirect
`Location` — is a structured `:CATEGORY :HTTPS-UNSUPPORTED` error naming
that issue, checked at one single scheme checkpoint where TLS later
slots in without API change; never a silent downgrade.

Client: `http:request` (+ `http:get`/`http:post`), request bodies
NIL/String/`Array<Char>`/readable port (ports stream out chunked);
response `:BODY` is an unread framing-aware body stream —
`Content-Length` exact / chunked (hex size lines, extensions, trailers,
zero-chunk terminator) / read-to-close / no body for HEAD/1xx/204/304 —
read incrementally (`http:stream-read!`/`stream-eof-p`/`stream-close!`)
or collected bounded (`http:collect-bytes`/`-string`/`-json`, 10 MiB
default cap; text is an explicit UTF-8 step). `Connection: close`
always (no pooling). Timeouts: connect / per-read / overall deadline.
Redirects: 301/302/303/307/308, hop-capped, 303→GET, 301/302 POST→GET,
307/308 method+body-preserving (unreplayable streamed bodies are a clear
error), cross-origin hops strip `Authorization`/`Cookie`/
`Proxy-Authorization`; no ambient proxy variables.

Server: `http:serve`/`http:serve-one!` over a `tcp:listen` listener;
handler gets a request alist (path/query/headers/streaming body — CL and
chunked framing both) and returns `http:respond` alists (body
NIL/String/bytes/port, `Content-Length` set automatically, port bodies
chunked). Serial keep-alive: one connection served fully before the next
accept (concurrency is #140's business); request-line/header-size/
header-count/body-size limits enforced (oversize body → `413` without
running the handler); an uncaught handler error is a generic `500` that
never leaks the condition to the peer (`:on-error` receives it
host-side); `:stop-p`/`:max-requests` for between-connection shutdown.

**No new capability**: requiring `HTTP` grants nothing — the client
rides `tcp:connect`'s `NET-CONNECT` gate, the server `tcp:listen`'s
`NET-LISTEN`, both fence-attenuated as ever (tested for both sides).

Fixed en route: `lib/27-modules.lisp` now declares its real dependency
`(require 'condensation)` — under `with_prelude()`, `(require 'modules)`
followed by any `WITH-MODULE` body (e.g. `(require 'text)`) died with
"unbound variable: CONDENSE-APPEND-NEW" because `with_stdlib()`'s eager
load order had been masking the missing declaration.

Tests: `tests/test_http.rs` — loopback-only `std::thread` peers,
OS-assigned ports; chunked/CL/204/304 framing, repeated headers,
case-insensitive lookup, truncated bodies, malformed status lines, read
timeouts, >100 KB bodies both framings (tail-recursive, stack-safe),
redirect matrix incl. loop capping and credential stripping, server
parsing/dispatch/keep-alive/413/500-sanitization, capability denial and
fence attenuation. Manual: `docs/manual/13-networking.md` §13.7.

## runtime(net): capability-gated DNS, TCP, and UDP over binary ports (#258)

Lamedh now has synchronous networking: DNS resolution, TCP (connect/bind/
listen/accept/shutdown/timeouts), and UDP (bind/send-to/receive-from),
built entirely on `std::net` — **zero new crate dependencies**. Three new
optional embedded libraries follow the epic #253 namespace ruling (a
genuinely new facility, not flat-namespace growth): `NET`
(`lib/37-net.lisp`, addresses/DNS), `TCP` (`lib/38-tcp.lisp`), and `UDP`
(`lib/39-udp.lisp`). **TLS is explicitly deferred** — see below.

Representation: a connected TCP stream is an ordinary `LispVal::Port`
(#255) — a new `PortState::TcpStream` variant — so every `PORTS` operation
(`read-byte!`/`write-bytes!`/`close!`/`with-open-port`/`port-p`/...) works
on it unchanged, and it is the seam a future TLS layer wraps without an
API change. Listeners and UDP sockets are not byte streams, so they get
their own opaque resource, a new `LispVal::NetHandle` (`NetHandleObj` in
`src/lib.rs`) — identity-compared, printed, self-evaluating, and Drop-
backstopped exactly like `Port`. Kernel substrate:
`src/evaluator/builtins_net.rs`.

Addresses: `NET:ADDRESS` is a `DEFRECORD` (family/ip/port) — first-class,
printable data; the kernel never hands a raw `std::net::SocketAddr` to
Lisp, only a `(family ip port)` triple that `NET` wraps.
`NET:ADDRESS->STRING` brackets IPv6 (`"[::1]:8080"`). `NET:RESOLVE`
resolves a host (and optional service port) to an ordered list of
addresses; `NET:LOCAL-ADDR`/`NET:PEER-ADDR` inspect a connected port or
handle.

TCP: `TCP:CONNECT` (optional timeout-ms), `TCP:LISTEN` (backlog argument
accepted for API completeness but currently advisory-only — `std::net`
has no OS backlog customization without an extra crate), `TCP:ACCEPT`
(returns `(port . peer-address)`), `TCP:SHUTDOWN!` (`:READ`/`:WRITE`/
`:BOTH`), `TCP:SET-READ-TIMEOUT!`/`SET-WRITE-TIMEOUT!`,
`TCP:CLOSE-LISTENER!` (idempotent; every subsequent `ACCEPT` errors
immediately — a blocked `ACCEPT` on another OS thread is not guaranteed
to unblock, since plain `std::net::TcpListener` has no portable
wakeup-on-close).

UDP: `UDP:BIND`, `UDP:CONNECT!` (sets a default peer), `UDP:SEND-TO`/
`UDP:SEND`, `UDP:RECEIVE-FROM` (returns `(bytes peer-address
possibly-truncated-p)` — datagram boundaries are preserved; the
truncation flag is a best-effort "received length equals the requested
buffer size" signal, since plain `std::net` exposes no `MSG_TRUNC`
without raw syscalls, out of this issue's no-new-dependency/no-ioctl
scope).

Capability model: three new ambient authorities, checked through the same
`require_*`/`cap_mask_allows` dynamic-extent-fence pattern as
`READ-FS`/`CREATE-FS`/`IO` (#320/#325) — verified directly, a networking
call inside `(with-capabilities () ...)` fails even when the host/CLI
granted the capability. `NET-DNS` gates explicit resolution
(`NET:RESOLVE`); `NET-CONNECT` gates outbound connections/sends
(`TCP:CONNECT`, `UDP:CONNECT!`, `UDP:SEND-TO`); `NET-LISTEN` gates
binding/listening (`TCP:LISTEN`, `UDP:BIND` — even an ephemeral-port UDP
socket receives from any sender once bound, so it needs the "inbound
traffic" capability like a TCP listener does). Once a resource is
acquired, continued use needs no further check (#255's "a successfully
returned handle is authority to continue" rule) — reading/writing an open
TCP port, or sending on an already-`CONNECT!`ed UDP socket, is
unrestricted.

In addition to the capability, a new Rust-only embedder hook,
`Environment::set_net_policy` (`src/environment.rs`), is consulted before
every resolve/connect/bind with the operation and the caller-supplied
host/port (not yet DNS-resolved): a scoped-grant mechanism so a broad
`NET-CONNECT` grant (e.g. for an HTTP client) does not become
unrestricted SSRF authority. `None` (the default) allows every operation
once its capability is granted; Lisp code cannot install, replace, or
inspect the policy. Errors are structured `LispVal::Error` values whose
`data` alist includes `:OPERATION`/`:CATEGORY`/`:HOST`/`:PORT`/
`:OS-ERROR`, `:CATEGORY` being one of `:TIMEOUT`/`:REFUSED`/`:RESET`/
`:DNS`/`:CLOSED`/`:POLICY-DENIED`/`:ADDR-IN-USE`/
`:ADDR-NOT-AVAILABLE`/`:OTHER` — extended onto `PORTS`' existing
`:OPERATION`/`:KIND`/`:NAME`/`:OS-ERROR` error shape for TCP-stream byte
operations too, so a timed-out `read-byte!` on a TCP port classifies the
same way a failed `TCP:CONNECT` does.

**TLS deferred.** The issue's TLS section (wrap a connected TCP port as a
TLS client/server port, certificate verification, ALPN/SNI) is out of
scope for this change: it forces a TLS-crate dependency decision that
belongs to the project owner, and this issue ships zero new dependencies.
The port-wrapping design (a TCP connection is an ordinary `Port`) is the
seam a TLS layer wraps later without changing the `PORTS`/`TCP` API.

## checker: literal-nil `if` joins stop committing to `(list a)` — the nil-on-miss guard idiom checks (#336)

The on-demand derived-scheme path (#308) was re-introducing the bias the
declared layer's honesty rule 1 exists to prevent: deriving `parse-integer`
(body `(if hit n nil)`) committed the `if` join to the nil side, producing
`(-> (a) (list b))`, and the standard guard idiom downstream —
`(let ((n (parse-integer s))) (if (numberp n) n 10))` — was then REJECTED
with "`if` branches disagree: List(Var) vs Int64". Now a literal-`nil`
branch meeting a non-list branch (ground scalar or still-free variable)
degrades the `if` to `any` (gradual, matching `when`/`unless`) instead of
erroring or pinning a free variable to "list of something"; nil-vs-list
still unifies as a list, and two non-nil branches that genuinely disagree
still error. A companion fix keeps self-recursive nil-on-miss helpers
honest: when a body's final type degrades to `any` but a recursive call
already concretized the assumed return variable mid-elaboration,
`check_callee` forces the assumption back to `any` before generalizing
(this also newly CHECKS `$require-resolve-disk`, previously a TYPE-ERROR).
`cond` joins are deliberately unchanged (see `elab_cond`'s comment; the
`cond` analogue needs a nil-on-miss-aware recursion assumption — a 0.4
direction the ticket itself names). Docs: the join rule is described in
docs/typed-checker-design.md.

## stdlib(io): format's directive set grows, plus READ-LINE, WITH-OUTPUT-TO-STRING, and s-expression file round-trip (#150)

Closes out the I/O & formatting ticket that #255's PORTS module left
partially done. `format` (`lib/18-format.lisp`) grows from `~a ~s ~d ~% ~~`
to `~a ~s ~d ~f ~x ~o ~b ~c ~% ~& ~~ ~{...~} ~^`: `~f`/`~,<n>f` fixed-point
floats, `~x`/`~o`/`~b` integer radix, `~c` bare-character rendering, `~&`
fresh-line (scoped to the current `format` call's own output — nothing in
the language tracks a destination's column across calls), `~{...~}` list
iteration with the `~a~^, `-style early-stop separator idiom named as
optional in the original ticket. **An unrecognized directive, or a
supported one written with an unsupported numeric/colon/at-sign prefix
(`~3a`, `~:d`), is now a hard error naming it, not the old silent
pass-through** — the larger directive set made a typo too easy to miss;
see `docs/cl-divergences.md`. `format`'s destination also now accepts a
`PORTS` port (writes the UTF-8 bytes to it), on top of the existing `nil`
(string) and `t` (stdout).

New on top of `PORTS` (#255), each lazily `(require 'ports)`ing on first
use so an environment that never touches I/O never pays for it: `read-line`
(`&optional port`, defaulting to the process's stdin under the `IO`
capability) and `with-output-to-string` (capture writes to a fresh
in-memory port as a string, always closing the port, even on error).
`read-sexpr-file`/`write-sexpr-file` round-trip a list of s-expressions
through a file on top of the existing whole-file `read-file`/`write-file`
builtins and the existing `read-string` reader builtin — no new Rust
kernel surface for any of this ticket. `format-build`'s control-string
walk and the new `~{...~}` iteration helper both stay tail-recursive
(stack-safe past the #361 10,000-frame trap for both a long control
string and a long iteration list).

## stdlib(data): optional JSON, URL, Base64, hex, and MIME codec modules (#257)

Five new optional embedded libraries for ordinary application and HTTP
programming, following the `TEXT`/`PORTS` namespace ruling from epic
#253: `BASE64` (`lib/32-base64.lisp`), `HEX` (`lib/33-hex.lisp`), `URL`
(`lib/34-url.lisp`), `JSON` (`lib/35-json.lisp`), and `MIME`
(`lib/36-mime.lisp`) — `(require 'name)` or `(import name)`, independently
of each other. All five are pure data transforms: **no capability is
required**, and every operation works inside a sandbox with every
capability denied. Every module is 100% Lisp — no new Rust kernel
builtins were needed; JSON parsing leans on the existing `STRING->NUMBER`
kernel primitive for exact int64-vs-float number classification and
IEEE-754-faithful float lexing, and every codec's per-character/per-byte
scan is written tail-recursively over UTF-8 byte codes (via
`TEXT:STRING->UTF8` + `ARRAY->LIST`, both native and O(n)) rather than the
Prelude's `STRING->LIST`, which recurses once per character and is not
stack-safe past a few thousand characters — a general fix for any large
flat input, not just JSON's own explicit `:MAX-DEPTH` nesting guard.

`JSON:PARSE`/`STRINGIFY`: object↔hash table (`String` keys, last-key-wins
on duplicates), array↔`Array` (not a list — the ticket's mapping, not a
free choice), `true`/`false`/`null`↔`T`/`NIL`/the keyword `:NULL` (three
mutually distinct values — `:NULL` avoids the `NIL`-is-both-false-and-
empty-list pun), integer literals in `i64` range as exact `Number`s,
literals outside that range controlled by `:ON-INTEGER-OVERFLOW` (`:ERROR`
default, or `:FLOAT` to widen instead of erroring), every other number as
`Float`. `STRINGIFY` always writes a `.` in `Float` output (even for a
whole value like `2.0`) so a `Float` never silently round-trips back as an
integer `Number`; `NaN`/infinite floats are a `STRINGIFY` error, not a
silent approximation. Strict throughout: rejects trailing garbage,
unescaped control characters, leading zeros, and unpaired `\u` surrogate
escapes, with every error carrying a line/column position;
`:MAX-DEPTH` (default 512) bounds nesting so deep input is a clean error
instead of a native stack overflow. `:PRETTY`/`:INDENT` control compact
vs. indented serialization.

`BASE64:ENCODE`/`DECODE` and `HEX:ENCODE`/`DECODE`: `Array<Char>`
bytes↔ASCII `String` (a byte is a `Char` or an integer 0-255, per the
epic's byte-array convention), every one of the 256 byte values
round-trips exactly in every position. Base64 supports `:ALPHABET`
(`:STANDARD` RFC 4648 §4 `+/`, or `:URL` RFC 4648 §5 `-_`) and `:PAD`
(default `T`) independently; Hex supports predictable-case `:CASE`
(`:LOWER` default, `:UPPER`) on encode and is case-insensitive on decode.
Both decoders are strict: invalid characters, misplaced/wrong-count
padding, and length/padding-policy mismatches are named errors.

`URL:ENCODE-PATH-SEGMENT`/`ENCODE-QUERY-COMPONENT` use different
safe-character sets (conflating path-segment and query-component
percent-encoding is a real bug class); `DECODE` is context-free (one
decoder for both, `:LOSSY` mirrors `TEXT:UTF8->STRING`/`-LOSSY`).
`URL:PARSE`/`BUILD` split/rebuild `SCHEME`/`USERINFO`/`HOST`/`PORT`
(handles a bracketed IPv6 literal)/`PATH`/`QUERY`/`FRAGMENT` (the latter
three returned raw, never auto-decoded, avoiding double-decode ambiguity)
via a small explicit state machine — no regular expressions.
`URL:PARSE-QUERY`/`BUILD-QUERY` preserve repeated keys and ordering as a
list of conses, never collapsed into a hash table.

`MIME:HEADERS-*`: a header list is `(name . value)` conses in original
order with original case preserved — deliberately not a hash table, so
`HEADERS-GET-ALL` never collapses a repeated header like `Set-Cookie`
(`HEADERS-GET` returns only the first match; `HEADERS-ADD`/`-SET`/
`-REMOVE`/`-NAMES` round out the API). `MIME:PARSE-CONTENT-TYPE`/
`BUILD-CONTENT-TYPE` handle Content-Type parameters including
quoted-string values with backslash escapes.

New manual chapter: [12. Codecs](docs/manual/12-codecs.md).

## runtime(io): binary ports and deterministic ownership for host resources (#255)

Lamedh now has a synchronous binary `Port` abstraction for byte streams —
files, in-memory byte buffers, and the process's standard streams — living
in a new optional embedded library, the `PORTS` module
(`lib/31-ports.lisp`, `(require 'ports)` or `(import ports)`), following
the namespace ruling from epic #253 and mirroring how `TEXT` (#254) wraps
its kernel primitives. Representation: a new opaque `LispVal::Port`
variant (`PortObj` in `src/lib.rs`) backed by a Rust enum over
`fs::File`/`Cursor<Vec<u8>>`/`Vec<u8>`/the standard streams/a
host-registered `Read`/`Write`, compared by identity like
`Array`/`HashTable`/`Environment`; the kernel `PORT-*` substrate
primitives live in `src/evaluator/builtins_ports.rs`.

Construction: `ports:open-input`/`open-output`/`open-append` (files),
`ports:open-input-bytes`/`open-output-bytes` + `output-contents`
(in-memory byte buffers), `ports:stdin`/`stdout`/`stderr` (the process's
standard streams). Binary operations work uniformly across every port
kind: `read-byte!`/`read-bytes!` (EOF is `NIL` from `read-byte!`, a
possibly-empty `Array<Char>` — never `NIL` — from `read-bytes!`),
`write-byte!`/`write-bytes!` (returns the count actually written, so
partial writes are observable), `flush!`, `close!` (idempotent — a
second close is a silent no-op), `open-p`/`input-p`/`output-p`/
`seekable-p`, and `position`/`seek!` on seekable ports (files and
byte-array input ports; byte-array output ports and the standard streams
are not seekable and signal a structured error on `position`/`seek!`).
`ports:position` is qualified-only — deliberately left out of the export
list so `(import ports)` never shadows the Prelude's flat
`(position item lst)` list helper.
Text wrappers (`read-line!`, `read-string!`, `write-string!`,
`read-all-bytes!`) are thin, explicit layers over `TEXT`'s UTF-8
boundary (#254) — bytes never implicitly become text. `with-open-port`
closes its port on every exit from its body — normal return, an ordinary
error, `THROW`, `RETURN-FROM`, or `GO` — via `UNWIND-PROTECT`; Rust's
ordinary `Drop` (the file closes when the last reference to the port
value is dropped) is a last-resort safety net, not the documented
contract.

Capability model unchanged from the existing filesystem builtins: opening
a file port for reading needs `READ-FS`, for writing/appending needs
`CREATE-FS`, `(ports:stdin)` needs `IO`; `stdout`/`stderr` and in-memory
ports need no capability (matching `PRINC`/`PRIN1` already writing to
stdout unconditionally). Acquisition is gated by `require_read_fs`/
`require_create_fs`/`require_io` exactly like `read-file`/`write-file`,
so `WITH-CAPABILITIES`'s dynamic-extent fence attenuation (#320/#325)
reaches port construction the same way it reaches every other host
builtin — verified directly: a port-open call inside
`(with-capabilities () ...)` fails even when the host/CLI granted the
capability. I/O errors are structured `LispVal::Error` values whose
`data` is an alist of `:operation`/`:kind`/`:name`/`:os-error`, not just
an English string.

Embedding: `LispVal::wrap_reader`/`wrap_writer` let a host hand Lisp code
an arbitrary `Read`/`Write` stream as a port (a pipe, a decompressor, a
captured buffer, ...) without ever exposing a raw file descriptor to
Lisp — see `docs/embedding.md`.

## runtime(modules): REQUIRE/PROVIDE and the Prelude/optional-library split (#256)

Lamedh now has named, dependency-aware, load-once library units —
`(require 'name)` / `(provide 'name)` — layered on the existing global
symbol table with zero new symbol-identity machinery (no Common Lisp
packages, no `pkg:symbol` reader syntax, no import renaming, no enforced
privacy; see `lib/06-require.lisp`'s header for the full design). This is
a *loading* discipline, distinct from and composable with `defmodule`'s
existing *naming* discipline (`lib/27-modules.lisp`, docs/manual/10-modules.md
§10.7).

`require` resolves a module name through a per-environment registry in
order: host-registered sources (`env.register_module(name, source)`,
no capability required), sources embedded in the binary (no capability
required), then — only under `READ-FS` — files under host-configured disk
search paths (`env.add_module_search_path(path)`; Lisp can read but never
set this list, so a host constrains disk resolution without exposing that
authority to sandboxed code). A second `require` of an already-loaded
module is a documented no-op; a module whose source errors, or which
finishes without calling `(provide 'name)`, is *not* marked loaded (partial
top-level definitions are not rolled back — this was never a transaction).
A `require` for a module already mid-load (directly or transitively) is a
hard cycle error naming the full chain. `(require-reload 'name)` is the
explicit development escape hatch; ordinary `require` never silently
re-evaluates. `provide` takes an optional exports list (metadata only);
`require` warns on an unbound declared export or a cross-module export
collision (errors instead, given `*require-strict-exports*`).
`(loaded-modules)`, `(module-state 'name)`, and `(module-info 'name)`
introspect the registry.

New `Environment::with_prelude()` loads only the stable general-purpose
vocabulary (core forms, lists, math, control flow, functional/string/
sets-hash/conditions/array helpers, `format`, `setf`/CL-compat, and
`require`/`provide` itself — see `src/lib.rs`'s crate doc for the exact
file list) — lighter and faster-starting than `Environment::with_stdlib()`,
which remains fully source- and behavior-compatible: it is now defined as
the Prelude plus every previously-unconditional optional library (shell,
Lisp 1.5 compatibility, testing, the optimizer, call-graph analysis,
condensation, guard fences, pattern matching, the rulebook, variants,
instrumentation, `defmodule` itself, the type table, protocols, the
`TEXT` module, and the help system — including 20-condensation.lisp,
closing the epic #253 acceptance criterion that it be loadable as an
embedded optional module rather than only via `-i lib/`), loaded in the
same order as before and immediately marked REQUIRE-loaded so a later
`(require 'name)` against a `with_stdlib()` environment is a correct
no-op rather than a redundant re-evaluation. Embedder API additions:
`Environment::with_prelude`, `Environment::register_module`,
`Environment::add_module_search_path`/`clear_module_search_paths`,
`lamedh::require_module`, `lamedh::loaded_modules`.

## stdlib(text): complete String API and the explicit UTF-8 Array<Char> boundary (#254)

`lib/14-strings.lisp` (the flat Prelude string surface from #147) is now
a complete String API: construction/access (`make-string`,
`string-concat`, `string-empty-p`, bounds-checked `char-at`), the full
case-sensitive comparison family (`string<`, `string>`, `string<=`,
`string>=`, `string-ne`, alongside the existing `string=`/`string-lessp`)
and a new Unicode-aware, locale-independent case-insensitive family
(`string-ci=`, `string-ci<`, `string-ci-ne`, ...), reverse search
(`string-last-index-of`), occurrence counting (`string-count`),
`string-replace-first`/`string-replace-all`, one-argument
`string-trim-left`/`string-trim-right`, `string-capitalize`, and
`string-reverse`. `string-lessp`'s pre-existing case-sensitive (not CL's
case-insensitive) meaning is unchanged and undisturbed by the new
`-ci` family. New Rust kernel primitive `string-casefold*` backs the
case-insensitive comparisons via Rust's locale-independent Unicode case
fold.

New `TEXT` module (`lib/30-text.lisp`, `(defmodule text ...)` /
`with-module`, 0.3's module story) carries the explicit, non-coercive
String <-> UTF-8 `Array<Char>` boundary: `(text:string->utf8 s)`,
`(text:utf8->string bytes)` (strict — signals a descriptive error naming
the offending byte offset on invalid UTF-8), and
`(text:utf8->string-lossy bytes)` (U+FFFD substitution). `Char` stays
exactly a `u8` byte and `Array<Char>` stays the ordinary dynamic array
already used by the #137 typed/JIT `(array char)` island — no new value
representation. New optional-library surface is namespaced per the
epic #253 ruling rather than growing the flat Prelude; existing #147
names are untouched.

## variant-case composes with the checker (#350)

The checker has a native eliminator rule for `variant-case`: the
scrutinee unifies with the clause constructors' owning variant, clause
vars bind positionally to the constructor's field types, and every
clause body joins to one result type. Before this, clauses like
`(circ (r) (* r r))` were misread as constructor applications, so every
variant-consuming `defun` carried a false `TYPE-ERROR` and was stuck on
the dynamic tier — the two flagship 0.3 features (sums and the checker)
didn't compose. Now `(defun area (x) (variant-case x (circ (r) (* r r))
(sq (s) (* s s))))` checks as `(-> (SHAPE) INT64)`, parametric variants
generalize (`(defun opt-len (o) (variant-case o (some (v) 1) (none ()
0)))` : `∀a. (option a) → int64`), wrong-variant scrutinees are rejected
at call sites, and binder-count/clause-body errors are real errors.
Exhaustiveness stays a runtime concern (the vau names missing brands).

## Docs: divergences from Common Lisp

New one-pager `docs/cl-divergences.md` — the cheat-sheet for readers
with CL reflexes, every claim probe-verified: Lisp-1 namespace, wrapping
64-bit integers (no bignums/rationals), `'a'` char literals, pure
`sort`/`rplaca`, collection-first hash accessors, the absent/replacement
table (loop→dotimes/while, CLOS→protocols, values→lists,
eql→eq/equal, …), same-word-different-behavior notes (`defvar`,
`format`'s exact directive set, `case` defaults), and the list of CL
reflexes that just work.

## Records — one definition form (breaking)

```lisp
(defrecord npc
  (tag symbol) (hp int64) (loot (list string))
  (:invariant (>= hp 0))
  (:derive equality lens))
```

- **`defrecord` is the only way to define a record**, and it does
  everything `defconcept`/`defstruct`/`defrecord` did together: branded,
  checker-denotable, NOMINAL, row-subsumable types over one runtime
  representation (StructObj), tier-dispatched — all-native fields compile,
  anything else runs dynamic with the same surface
  (`(record-compiled-p 'name)` reports the tier). Generates `make-Name`,
  `Name-p`, `Name-field`, `validate-Name`; optional `(:invariant ...)` and
  `(:derive equality|printer|lens ...)` sections; bare field names mean
  `(field any)`; the condensation trace (`condense-trace`, `edit!`,
  `deflaw`, `example`, fingerprints) works on every record.
- **Removed: `defconcept`** — use `defrecord`; a legacy
  `(:fields ((f ty)...))` section is still accepted to ease migration.
- **Removed: untyped `defstruct`** — with its keyword constructor protocol
  and `set-NAME-FIELD!` mutators. Records are values: update with
  `record-with`. `setf` accessor places still expand to
  `(set-<accessor>! obj v)` against user-defined mutators.
- **Removed: the positional record representation** (`(BRAND f1 ...)`
  lists). `defstruct-typed` remains only as internal machinery behind the
  compiled tier.
- `record-ref` / `record-with`: by-name field read and functional update
  with checker-native row rules — `(defun worth (x) (record-ref x 'value))`
  derives `(forall (a b) (-> ((record ((value a)) b)) a))` with no axioms.
- Records print as `#S(BRAND v ...)` and the reader accepts `#S(...)`
  literals: print/read round-trip (spawn and channel serialization), usable
  as source syntax.

## Sequence protocols: map and for-each

- `(map coll fn)` — kind-preserving map as a protocol (list → list,
  array → array, string → string); `(for-each coll fn)` visits for
  effect over list/array/string/hash, with the hash instance receiving
  `(fn key value)`. Collection FIRST (protocols dispatch on their first
  argument — and it matches the container convention). Extend either
  with `definstance`.
- Ruled: `filter`/`reduce` stay list-specific and fn-first (the
  `mapcar`/`every` heritage class); protocols dispatch on the first
  argument, so protocolizing them would flip their argument order.
  Convert at the edge (`array->list`, `string->list`) for other kinds.
- **Breaking**: the Lisp 1.5 appendix's `map` (apply f to successive
  TAILS, return nil) is renamed **`map-tails`** — the bare name now means
  what every modern reader expects. Documented appendix deviation.

## Stdlib staples

Gap-probe additions (each follows its class's argument order):

- `(sort-by lst keyfn [pred])` — sort by extracted key, collection first
  like `sort`; `pred` (default `#'<`) compares keys.
- `(enumerate lst [start])` — index/element pairs, `zip` shape.
- `(frequencies lst)` — `(element . count)` alist, first-seen order;
  typed `(forall (a) (-> ((list a)) (list (pair a int64))))`.
- `(string-pad-left s width [pad])` / `string-pad-right` /
  `(string-repeat s n)` — padding never truncates.

## One dispatch system (breaking)

- **Removed: `definterface`, `implements?`, `method`, `method-symbol`**
  and the whole lib/21-interfaces layer (row-aware signature unifier,
  fingerprinted claims). Two dispatch mechanisms was one too many:
  `defprotocol`/`definstance` is the survivor.
- The conformance value lives on, re-seated on protocols:
  `(implements! type protocol...)` asserts that TYPE carries a clean
  instance of every named protocol (graded INSTANCE / MISMATCH — the
  implementation's checker verdict is a type error / MISSING), and
  `implements-p` is the predicate form. A "contract" is a set of
  protocol names.
- `examples/npcs.lisp` and `examples/oo-patterns.lisp` rewritten to
  protocols; identical output, less machinery.

## HOF protocols are function-first (breaking)

- Ruled: higher-order functions follow the CL convention — FUNCTION
  FIRST. `map` and `for-each` flip to `(map fn coll)` /
  `(for-each fn coll)`; `option-then` and `result-then` flip to
  `(option-then f o)` / `(result-then f r)`, matching
  `option-map`/`result-map`. Access operations (`ref`, `put!`, `copy`,
  `sort-by`) stay collection-first, matching CL's `aref`/`elt`/`sort`.
- Protocols now declare their dispatch position:
  `(defprotocol map "..." (:dispatch 1))` dispatches on the second
  argument (kernel: `declare-protocol-dispatch!`; the checker selects
  instances by the dispatch argument's shape).
- `filter` is now a generic kind-preserving protocol — it was already
  fn-first, so nothing breaks: lists as before, plus arrays and strings
  (`(filter #'alpha-p "a1b2c")` is `"abc"`).

## Invariants are enforced at construction (breaking)

- `make-Name` now REFUSES a value the `:invariant` would fail
  (`MAKE-ACCT: invariant violated`) instead of constructing it; the
  wrapper captures the validator at definition time, so later rebinding
  `validate-Name` cannot weaken construction. `validate-Name` remains
  the judgment for the two roads that bypass the constructor door:
  `record-with` updates and `#S` reader literals — validate explicitly
  after either.

## flatten respects dotted pairs (breaking)

- `flatten` now flattens nested PROPER lists only; a dotted pair is a
  leaf: `(flatten '((1 . 2) (3 4)))` is `((1 . 2) 3 4)`. The old
  behavior recursed into every cons and silently destroyed
  alist/coordinate-shaped data (it turned the classics' game-of-life
  world into bare integers). New `proper-list-p` predicate, typed.

## Dotted parameter tails

- `(lambda (a . more) ...)`, `(defun f (a . more) ...)`, and
  `(defmacro m (test . body) ...)` now accept the classic dotted-tail
  shorthand as `&rest` (previously an inscrutable `list_to_vec` error).
  Mixing both spellings errors; fexprs keep their fixed `(args env)`
  shape with a message that says so.

## random is a PRNG now

- `random` previously returned `nanos-since-epoch mod n` on EVERY call —
  a monotonic wall-clock ramp, not a random sequence (found when the
  monte-carlo-pi example converged to 2.57). It is now SplitMix64 over
  persistent thread-local state, lazily time-seeded; new
  `(random-seed! n)` makes runs reproducible.

## The substrate star (breaking)

The monomorphic per-type implementations behind the protocols now carry
a trailing `*`, in the old Lisp tradition of marking a name as visibly
outside the normative vocabulary: `string-length*`, `array-length*`,
`hash-table-count*`, `list-length*`, `array-map*`, `copy-list*`,
`array-copy*`, `copy-hash*`, `array-fetch*`, `array-store*`. The
unstarred names are REMOVED. Write against the protocol names
(`length`, `map`, `copy`, `ref`, `put!`); call a starred form when
you've committed to the type and want the direct monomorphic call
(until 0.4 splices instances at call sites, that's the hot path).
Unstarred survivors are not substrate: converters (`array->list`,
`char->code`, ...), true type-specifics (`string-split`, `array-fill`,
...), and the separately-ruled lenient reads (`gethash`, `nth`, `elt`).

## Access protocols: ref, put!, copy

One vocabulary over the per-type access zoo, all collection first:

- `(ref coll k)` — strict read at an index/key/field: lists, arrays,
  strings, hash tables, and records (by brand, via the fallback).
  **Absence is an ERROR**, which is what lets every instance carry an
  honest result type (`(check-type (ref (list 1 2) 0))` is `int64`);
  the lenient nil-on-miss reads keep their old names (`gethash`, `nth`,
  `elt`). A known type with no instance is a static error.
- `(put! coll k v)` — write, returns `v`: arrays and hash tables (the
  mutable containers; records are values — `record-with`).
- `(copy x)` — fresh list/array/hash (**`copy-hash` was missing
  entirely and is new**), identity on immutable strings, Lisp 1.5
  structure-copy fallback for atoms.
- The type-prefixed names (`fetch`, `store`, `array-copy`, ...) remain
  as the monomorphic substrate the instances dispatch to and compile
  through. Ruled: `char-code` (strict kernel) vs `char->code` (coercing
  wrapper) are not duplicates; both stay.

## Dotted pairs in the checker

- `(cons 'k 2)` now types as `(pair symbol int64)` instead of erroring:
  a cons whose tail is a known non-list ground type takes the
  dotted-pair view, and `car`/`cdr` project known pairs. Unknown tails
  keep the list-cons view (the recursion default), so existing derived
  schemes are unchanged. The alist-cell idiom types end to end.
- New example: `examples/wordcount.lisp` — the classic word-frequency
  report, dogfooding the 0.3 staples (`frequencies`, `sort-by`,
  `enumerate`, padding, `for-each`, Option, `variant-case`). Found both
  the missing pair rule and the staples gaps.

## Typed protocols

```lisp
(defprotocol volume "loudness of a thing")
(definstance volume ((n int64)) int64 (* n 2))
(definstance volume ((s string)) int64 (string-length s))
```

- One name, many typed instances, three resolutions: the CHECKER selects
  the instance whose shape matches the first argument's inferred type and
  gives its precise scheme (a known type with no instance is a static
  error — "no `volume` instance for (list int64)"); the RUNTIME
  dispatches on the value's kind (list/string/array/hash/scalars, record
  brands, variants); the COMPILER treats each instance body as an
  ordinary defun, so eligible instances go native through the one-door
  pipeline. When every instance agrees on one ground result type, even a
  gradual call site derives it: `(defun n-items (x) (length x))` checks
  as `(forall (a) (-> (a) int64))`.
- `defprotocol` captures any prior binding as the fallback instance, so
  protocolizing a builtin is seamless. **`length` is the shipped pilot**:
  lists/strings/arrays/hash tables out of the box, and your own types
  join with one `definstance`.
- Instance implementations live under reader-unnameable hidden names —
  they cannot be shadowed or called around the dispatcher from source.

## The census, batches 2–3: one name, and the type table

- **`delete-key`/`delete-key-bang` removed; `remhash` is the one hash
  removal name** (collection first, kernel-direct).
- **`length` covers every sized collection**: lists, strings, arrays, and
  now hash tables.
- **The type table** (`lib/28-types.lisp`): verified declared schemes for
  ~45 builtins and stdlib functions — predicates
  `(forall (a) (-> (a) bool))`, integer-only predicates strict both ways,
  list functions (`member`/`filter`/`mapc`/`every`/`exists`/`notany`),
  strings and conversions (`string-upcase`, `princ-to-string : a →
  string`, `intern`, `implode`...), math with known results (`sqrt → 
  float64`, `floor → int64`). Two honesty rules exclude entries: NIL-ON-
  MISS functions never claim a result type (`nth`, `assoc`,
  `string->number` stay gradual — a declared "hit" type would let checked
  code consume a legal ()), and variadic/multi-arity functions can't carry
  fixed-arity schemes (they get kernel checker rules instead). Net effect:
  string/list/math pipelines derive full schemes with zero annotations,
  and misuse errors at the call site.

## The census, batch 1: variadicity (breaking)

The full special-form/builtin/stdlib census lives in docs/audit-0-3.md;
this batch fixes the variadicity findings.

- `append` is a variadic kernel builtin — `(append)`, `(append xs)`,
  `(append a b c ...)`, dotted final tail preserved — replacing the 2-ary
  stdlib recursion (it is also faster and checker-known now).
- `gcd`/`lcm` are variadic folds with the CL identities (`(gcd)` = 0,
  `(lcm)` = 1).
- **`logor` is renamed `logior`** — the name every Lisp reader (and
  model) expects; `logand`/`logxor` were already variadic and now the
  trio is uniform.
- `mapcar` takes N lists Common-Lisp style, zipping and stopping at the
  shortest.
- Checker-native rules for the variadic family: `append` (all `(list a)`
  → `(list a)`; dotted-tail append stays dynamic-only), `concat`
  (strings → string), `min`/`max` (numeric chain), `logand`/`logior`/
  `logxor`/`gcd`/`lcm` (int64). Side effect of the `concat` rule: the NPC
  example's greet methods left the dynamic frontier — their row schemes
  now derive fully.

## HM generics — parametric records and variants

```lisp
(defrecord (duo a b) (first a) (second b))
(defvariant (option a) (some (value a)) (none))
(defrecord (node a) (val a) (next (option (node a))))
```

- Records and variants take TYPE PARAMETERS, as proper HM type
  application: `make-duo : (forall (a b) (-> (a b) (duo a b)))`,
  instantiated freshly at every use — `(duo-first (make-duo 1 "s"))` is
  `int64`, `(unwrap-or (some "s") 0)` is a static error, `(none)`
  instantiates freely like nil. Nominal by name with pairwise argument
  unification; constructor applications absorb into their variant's
  application; SIBLING constructors of one variant unify (an `if`
  building `(some x)` / `(none)` types cleanly); parametric record
  applications subsume into rows with instantiated field types.
- **Recursion composes**: `(node a)` referencing `(option (node a))`
  gives `node-next : (forall (a) (-> ((node a)) (option (node a))))`.
- **Option and Result are parametric now** with precise helper schemes
  (`unwrap-or : (forall (a) (-> ((option a) a) a))`, monadic
  `option-then`/`result-then`, `result : (result a e)`).
- A BARE generic name in a type is sugar for the all-`any` application
  (`option` ≡ `(option any)`) — the gradual reading, and what
  pre-parametric code already meant.
- Erased at runtime: every value is the same branded StructObj; generics
  never compile (checker-only), monomorphic records keep their tiers.
- Built-in type-constructor names (`pair`, `list`, `array`, `record`,
  scalars) are REJECTED as record/variant names — they'd silently shadow
  the built-in meaning in type surfaces.

## Sum types

```lisp
(defvariant shape
  (circle (r int64))
  (rect   (w int64) (h int64)))
```

- **`defvariant`**: a closed set of branded record constructors (bare
  constructor names — `(circle 3)`) plus a checker-level union type. A
  `CIRCLE` unifies where a `SHAPE` is demanded; a constructor of another
  variant is rejected by name ("HEADS is not a constructor of variant
  SHAPE").
- **`variant-case` is exhaustive**: missing a constructor without an
  `else` clause errors, naming the missing brands.
- **`match` destructures records/constructors with `#S` patterns**:
  `(match v (#S(CIRCLE ?r) ...) ...)`, nesting and all.
- **Option and Result** are ordinary variants in the stdlib
  (`some`/`none`, `ok`/`err`) with `unwrap`/`unwrap-or`/`option-map`/
  `option-then`/`result-or`/`result-map`/`result-then`/`option-of`, and
  `try-call` bridging the condition system into Result.
- Breaking: the `some` list quantifier in `lib/13` is renamed **`exists`**
  (`(exists #'evenp xs)`); `some` is Option's constructor now.
- `record-new` accepts zero field values (nullary constructors like
  `(none)`).

## Recursive records

- Self- and mutually-referential `defrecord` field types are NOMINAL now,
  not a silent `any`: `(defrecord node (val int64) (next node))` gives
  `node-next : (-> (NODE) NODE)`. Struct unification is by brand name;
  struct-into-row expansion re-resolves the definition through the
  registry; forward references get provisional definitions (a misspelled
  field type surfaces as a phantom brand at first unification instead of
  degrading silently). The blessed terminator idiom is Option:
  `(next option)` with `(none)`.

## Checker and rows

- Row types (Rémy-style, with a gradual `Any` frontier) ported into the
  checker; typed structs subsume into record rows; `declare-type!` declared
  schemes consulted at call sites; `see-type` reports
  TYPED/CHECKED/DECLARED/TYPE-ERROR/DYNAMIC verdicts as data.
- **Derived schemes at call sites**: the checker checks unknown lambda
  callees on demand (memoized, recursion-safe), so row types flow through
  helper functions with zero annotations.
- **One door**: plain `defun` quietly attempts typed compilation; opt out
  with `(declare (no-compile))` or `declaim`. `defun-typed`/`defun*` remain
  for explicit signatures/inference.

## Stack traces

- Runtime errors carry a **backtrace of named frames**: the toplevel prints
  `Error: boom` followed by `in: INNER ← MIDDLE ← OUTER` (innermost first),
  and handlers read the frames of the error they caught via
  `(last-backtrace)`. Tail calls collapse into one frame (Scheme-style TCO
  traces — a 100k-deep tail loop is a single entry). Recording is
  pay-mostly-on-error: overhead on hot benchmarks is within noise. Direct
  toplevel errors format exactly as before. Host API:
  `lamedh::format_error_with_backtrace`.

## Dynamic-extent guard fences (breaking)

- `with-fuel` and `with-capabilities` are KERNEL SPECIAL FORMS now, with a
  thread-local capability mask and RAII save/restore. Attenuation follows
  the CALL, not the fence's lexical body: **helpers called from inside a
  fence are fenced**, eval'd code is fenced, and kernel capability checks
  consult the mask on every gated operation. There is no Lisp-callable way
  to widen either state (`kernel-fuel-set!` is narrow-only inside a
  fence; no capability-mask setter exists).
- Escaped closures follow the same law in reverse: a closure created
  under a fence but called outside runs with the caller's authority —
  the semantics `spawn` always had, now uniform.
- Under an armed fuel budget, one-door native membranes take their
  interpreted fallback (compiled internal loops never returned to the
  metered trampoline — the fuel escape is closed); `jit-optimize` returns
  `COMPILE-DISABLED-BY-GUARD` and `defun-typed` errors while armed.
- The tick-instrumentation walker, lexical seal shadows, and eval
  re-sealing hatches are deleted from `lib/22` (~150 lines); the
  gated-builtin table remains for static `capabilities-needed` manifests.
- New read-only introspection: `(capability-mask-allows-p 'CAP)` — how
  custom (module-provided) capabilities attenuate through the same mask.

## Checker honesty

- Arithmetic and comparisons reject KNOWN non-numeric operands statically
  (`(+ "a" "b")` used to check as `string`); char arithmetic and char
  comparisons stay legal (evaluator parity), variables and `any` stay
  gradual — no scheme changes. Char literals now check as `char`.

## Modules

```lisp
(defmodule geometry (:export area) (:provides FAST-MATH))
(with-module geometry
  (defun helper (x) (* x 3))
  (defun area (r) (helper (* r r))))
(geometry:area 2)   ; => 12
(import geometry)   ; binds AREA
```

- A module is a NAMING DISCIPLINE plus metadata over the flat global
  namespace: `with-module` stores definitions as `MODULE:SYMBOL` (the
  reader now accepts `:` as a non-initial symbol constituent) and
  qualifies module-local references; `import` binds a module's exports
  globally (snapshot semantics); `module-of`/`module-functions`/
  `module-exports`/`module-requires`/`module-provides` introspect.
- **Modules can provide capabilities** — conservatively: a `(:provides
  CAP)` clause registers a NEW capability name into the attenuable
  vocabulary. It is held by registration at the outermost level, gates
  only explicit `(require-capability 'CAP)` checks, attenuates through
  `with-capabilities`/`sandboxed`/`spawn` like a built-in, and can never
  grant kernel abilities (READ-FS and friends stay host-granted). The
  fence now shadows `require-capability` so the gate attenuates with it.
- `(:requires CAP...)` records a module's needs for introspection and
  manifests.

## Regularity (breaking, deliberately)

One convention where there were several; the breaks are the point.

- **Hash operations are COLLECTION FIRST, one order**: `(gethash table
  key)`, `(remhash table key)` — matching `sethash`/`fetch`/`store`/
  `getp`. The either-order type-guessing (#246) is removed.
- **Comparisons are variadic monotone chains** like `+`/`*`: `(< a b c)`,
  `(= x y z)`, `(<= ...)`, `(>= ...)`.
- **`defun` supports `&optional` and `&key`** (with defaults; later
  defaults see earlier parameters; composes with `&rest`). Expanded at the
  defun layer to a variadic lambda + `LET*` prologue, so such functions
  stay on the dynamic tier. Bare `lambda`/`defmacro` still take only
  `&rest`.
- **`(set sym val)`**: the value-level global setter (both arguments
  evaluated) — the computed-symbol twin of the quoting `cset` macro.
- **H-suffix hex literals must start with a decimal digit** (`0FFh`, not
  `FFh`) — the assembly convention — so `ch`, `each`, `deadh` are ordinary
  symbols again.
- **The LABEL variable-read hack is gone**: a list VALUE headed by the
  symbol `label` no longer auto-evaluates when read from a variable (it
  made `(list 'label ...)` data explode and reserved `label` as a field/
  parameter name everywhere). The `label` special form in operator
  position is unchanged.
- **`-s` is repeatable** on the CLI; each string evaluates in order in one
  shared environment.

## explain-compile

- `(explain-compile 'f)` reports the execution tier as data —
  `((TIER . COMPILED) (SIGNATURE ...))`, or `CHECKED` with the SCHEME and
  the CONCRETE blocker keeping it off the native tier (ambiguous operand
  types, non-storable list/record schemes, a `(declare (no-compile))`
  pin), or `DYNAMIC`/`TYPE-ERROR`. Side-effect-free (a dry-run twin of the
  codegen path — explaining never installs anything).

## Instrumentation: trace / time / step-count — and ONE fuel ruler

- `(trace 'f)` / `(untrace 'f)`: real call tracing (args in, value out,
  indented by depth) — replacing the Lisp 1.5 flag-only stubs. Natively
  compiled internals count as one call.
- `(time form)` prints wall milliseconds + kernel steps and returns the
  value; `(step-count form)` returns `(steps . value)`.
- **Breaking — one ruler**: `with-fuel N` is now denominated in KERNEL
  STEPS (one trampoline iteration each), the exact unit `step-count`
  measures: `(car (step-count form))` sizes the budget, tight to a handful
  of steps. The old unit (function entries, ×256 kernel backstop) is gone,
  along with the tick-instrumentation walker — the kernel counter is the
  single meter (the no-compile rewrites remain: in-fence definitions stay
  interpreted so native loops can't escape metering). `fuel-remaining`
  reads the kernel counter. Nested fences clamp to and spend from the
  enclosing remainder; fence setup charges the enclosing budget.
- `spawn`'s `:fuel` was already kernel steps; it now agrees with
  `with-fuel` instead of being 256× finer.
- New builtin: `(monotonic-micros)`.
- Soundness fix (found by TRACE's wrapper): a `&rest` closure created
  under an intervening `LET` read the let's slots instead of enclosing
  variables — `&rest` call frames now contribute an Opaque scope level so
  outer `LocalGet` depths count them.

## Guards, fuel, and processes

- Composable guard fences, pure Lisp: `with-fuel`, `with-capabilities`,
  `sandboxed`; guarded code is introspective; static capability manifests
  (`capabilities-needed`) computed from the call graph; a kernel
  step-budget backstop makes fuel exhaustion un-catchable-by-accident.
- `spawn` / `spawn*` / `await`: share-nothing interpreter threads whose
  capabilities are the requested set intersected with the caller's
  effective set — attenuation all the way down (#140). Channels and
  `clone-interpreter` behind the `concurrency` feature.

## Conditions

- Restarts: `restart-case`, `invoke-restart`, `find-restart`,
  `compute-restarts`, `handler-bind`, plus `use-value`/`store-value`/
  `retry`/`abort-to-restart`/`with-retry-restart`. (Documented deviation:
  handler-bind handlers run post-unwind.)

## Patterns and the rulebook

- Structural pattern language: `pat-match` (`?x`, `??segments`, guards),
  `match`, `destructuring-bind`, `sgrep`/`sgrep-source`/`sgrep-file`
  (positioned hits via `read-all-positioned`), bottom-up `rewrite`.
- The rulebook optimizer: `defrule`/`list-rules`/`apply-rules` — optimizer
  passes as pattern data feeding `optimize-form`.
- Go-style interfaces: `definterface`, `implements?`/`implements!` verify
  method sets against checker verdicts with a row-aware unifier.

## Performance

- Slot frames with routing tables: sound compile-time lexical addressing
  for lambda params and LET binders (#200 M3); unified compile/tree-walker
  trampolines; typed-JIT tail calls (self, mutual, and general); lambda
  bodies pre-compiled at definition time; per-call allocation cuts
  (SmallVec operands, symbol-id frames, precomputed special-form dispatch,
  cached symbol flags, lazy defun analysis hooks); shallow binding for
  dynamic variables; optional `arc-val` atomic refcounting feature;
  COLLAPSE-FRAMES and purity-checker optimizer passes over a DEFUN call
  graph.

## Correctness and parity (the #210 audit and friends)

- Typed tiers now match the evaluator exactly on: Euclidean `MOD`,
  `OVERFLOW`/div-by-zero flag propagation through the membrane (including
  `MIN%-1` and flag-before-error ordering), `FETCH`/`STORE` out-of-bounds
  errors, `CODE-CHAR`/`CHAR-CODE`, variadic `AND`/`OR`, strict binary
  `/`/`MOD` arity, mutated array parameters written back.
- Soundness: gensym symbols bind by their own id in lambda params and
  `SETQ`; dynamic bindings preserved across tail calls; cross-namespace
  symbol-id remapping in local frames; checked `gcd`/`lcm`; reachable JIT
  panics converted to Lisp errors; reader recursion bounded;
  `RENAME-FILE` requires `READ-FS` in addition to `CREATE-FS`.

## Reader, CLI, and docs

- Reader: 1-based positions in parse errors, nesting block comments
  `#| |#`, CL-style radix literals (`#x`/`#b`/`#o`), shebang support.
- REPL: persistent history (saved on `(exit)` too) and symbol tab
  completion.
- Self-evaluating keyword symbols; runtime error messages include the
  offending value; `cargo doc` warning-free; docs refreshed to match
  behavior; classic OO patterns example on row types (`examples/`).

# v0.2.0

efficiency.

The typed JIT release: HM-lite type inference (#135), the typed membrane
with a native Cranelift backend (#124), `defun-typed`/`defstruct-typed`,
typed regions, and the `jit` feature on by default.
