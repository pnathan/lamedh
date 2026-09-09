# KERNEL.md — the Lamedh language specification

This document defines the observable semantics of Lamedh precisely enough
that two independently written hosts agree on what a program means, and it
defines the minimal primitive surface a host must expose to run the shared
`lib/*.lisp` stdlib corpus unmodified. Three hosts exist or are emerging —

- the Rust reference implementation (`src/`, `cli/`)
- a standalone Common Lisp / SBCL port (`sbcl/`, PR #449)
- a from-scratch x86-64 native compiler (`lamedh-asm/`, PR #450)

## Part I — Scope, status, and method

**The Rust reference implementation is the semantics yardstick.** Every
rule of observable behavior stated in this document is the Rust
reference's actual behavior, verified directly against its source
(`src/reader.rs`, `src/printer.rs`, `src/lib.rs`, `src/environment.rs`,
`src/evaluator/*.rs`) — not a plausible-sounding generic Lisp convention,
and not an invention of this document. Where a rule cites a function or
file, that citation is the place in the reference where the rule is
implemented. Lamedh has a few genuinely surprising rules — surprising
enough that a competent implementer guessing from "it's a Lisp" gets them
wrong — and those are exactly the rules a spec exists to pin down. Part
IV's `EQ` rule for cons cells (pointer identity, resolved by issue #454
after a period as a hard-coded-`NIL` defect) is a sharp example of the
kind of rule this document exists to catch and pin down precisely.

**A small, closed set of declared axes is where hosts are allowed to
differ**, listed in full in Part XII. Outside that list, a host's observable
behavior must match this document — which is to say, must match the Rust
reference — exactly. The list exists so that SBCL's native bignums and
`lamedh-asm`'s tag-cancellation wraparound arithmetic do not get flagged as
spec violations for a choice that is genuinely a host's to make; it is
short and explicit specifically so that "conformant divergence" and "actual
bug" stay distinguishable, which a document that just says "hosts may vary
reasonable things" cannot do.

**Error message text is never part of the portable surface** (Part VIII).
Wherever this document says an operation "is an error", it means the
operation signals a `HANDLER-CASE`-catchable condition (Part VIII) rather
than returning a value, crashing, or continuing silently; the wording of
the message is the reference's own and a host may choose its own.

**This is a Lisp 1.5 dialect with modern extensions, not a reimplementation
of Common Lisp or Scheme.** Where a rule below looks unusual next to CL or
Scheme habit (fixed-arity-plus-rest lambda lists with no `&optional`/`&key`;
a condition system with no type hierarchy; a two-armed-only `IF`), that is
not an oversight — it is what the language is, and it is the language
that the entire `lib/*.lisp` corpus already runs in production against.

## Part II — Lexical grammar (the reader)

The reader (`src/reader.rs`) is a recursive-descent parser over UTF-8
source text. It is **not token-based**: at each position it skips
whitespace and comments, then tries a fixed, ordered list of productions
and commits to the first one that matches, consuming exactly what that
production matched. Except where a production is stated below to carry a
boundary guard, **nothing requires the character after a matched
production to be a delimiter** — the next production simply starts there.
Several concrete consequences of this rule are listed under each
production; a conforming reader must reproduce them.

**Source text and whitespace.** Source is UTF-8. `ws ::= (space |
line-comment | block-comment)*`, where *space* is exactly one of the four
characters space (U+0020), horizontal tab, carriage return, line feed. Form
feed, vertical tab, and every non-ASCII whitespace character are **not**
whitespace and are a parse error wherever they appear outside a string
(`(a<FF>b)` fails to read).

- A *line comment* is `;` followed by **one or more** characters other
  than LF or CR, ending at (not consuming) the next LF or CR. A `;`
  immediately followed by a line break or by end of input does not match
  the comment production and is a **parse error** in the reference
  (`parse_comment` requires at least one comment character). See Part XII,
  axis 6.
- A *block comment* runs from `#|` to the matching `|#` and **nests**: an
  inner `#|` increments a depth counter that an inner `|#` decrements, so
  `#| a #| b |# c |#` is one comment. Block comments are not recognized
  inside strings. An unterminated block comment is a hard parse failure.
- A *shebang line*: if the **first two bytes of a source text** are `#!`,
  everything up to (not including) the first LF is discarded before
  parsing begins (`strip_shebang`). This applies to any text handed to the
  reader's whole-text entry points (`read`, `read_all`, file loading), not
  only to files, and only at offset 0.

**Dispatch order.** After skipping `ws`, the productions are tried in this
order (`parse_expr`); the first match wins:

1. *atom*: `1+`/`1-` literal symbols, then numeric literals (float,
   radix-prefixed, hex-suffixed, octal-suffixed, decimal — in that order),
   then earmuff symbol, plus-earmuff symbol, keyword symbol, general
   symbol, operator symbol;
2. string literal;
3. `#S(` record literal;
4. `(` list;
5. `'x'` character literal;
6. `'` quote, `` ` `` quasiquote, `,@` unquote-splicing, `,` unquote,
   `#'` function shorthand.

If none matches, the text is a parse error at that position. In particular
`.` never begins a form (it is only the dotted-pair marker inside a list),
`)` outside a list is an error, and `#` followed by anything other than
`|`, `S`, `s`, `'`, `x`, `X`, `b`, `B`, `o`, `O` is an error. Parse errors
carry a 1-based line and column; the message text is not portable.

**Symbols.** Four productions, all producing an ordinary interned
`Symbol`. Constituent classes are **ASCII only**: *letter* is `A`–`Z` /
`a`–`z`, *digit* is `0`–`9`; a non-ASCII character such as `é` is not a
constituent of anything and is a parse error outside strings.

- *`1+`/`1-`*: the two-character sequences `1+` and `1-` read as the
  symbols `1+` and `1-`. This is a prefix match with no boundary guard
  and it is tried **before** numbers, so `1+x` reads as `1+` then `X`,
  and `1-5` reads as `1-` then `5`. No other digit-leading symbol exists.
- *Earmuff*: `* letter (letter | digit | -)* *`. Tried before the keyword
  and general productions. The tail admits **only** letters, digits, and
  `-`: `*foo*` and `*a-b1*` are earmuff symbols, but `*foo?*` is not —
  it reads as the operator symbol `*` followed by the general symbol
  `FOO?*`, and `*a_b*` reads as `*` then `A_B*`.
- *Plus-earmuff*: `+ letter (letter | digit | -)* +`, e.g.
  `+NUMERIC-PRECISION-MODEL+`, `+HOST-TRAITS+` (issue #463; the
  Common-Lisp-style constant naming convention). Tried before the keyword
  and general productions, mirroring *Earmuff* exactly with `+` in place
  of `*`. The tail admits only letters, digits, and `-`, same as
  *Earmuff*; a bare `+` or `+` not closed by a matching trailing `+`
  (`+foo`, `+`, `++`) is unaffected and falls through to the general or
  operator productions as before.
- *Keyword*: `: (letter | & | $) (letter | digit | - * ? ! + = < > _)*`.
  The tail excludes `:` and may not begin with `?`: `:foo:bar` reads as
  two symbols `:FOO` `:BAR`, and `:?x` is a parse error. A keyword's
  stored name includes the leading colon (`:FOO`).
- *General*: `(letter | & | $ | ?) (letter | digit | - * ? ! + = < > : _)*`.
  `-` is legal non-initially (`FOO-BAR` is one symbol); `:` is legal
  non-initially (`MODULE:SYMBOL` is one symbol; `A:` is one symbol). `.`,
  `\`, `|`, `/`, `~`, `#`, `,`, `'`, `` ` ``, `"`, and parentheses are
  never constituents: `a.b` reads as `A`, `.`, `B` (a dotted pair inside a
  list, an error at top level), `a\b` and `|a b|` are parse errors,
  `foo(bar)` reads as `FOO` then `(BAR)`.
- *Operator*: one or more characters from `+ - * / = < > ! ~`, e.g. `+`,
  `>=`, `/=`, `->`, `<=>`, `!~`. A lone `-` or `+` followed by a space is
  an operator symbol; `-5` is a number (numbers are tried first); `+5`
  reads as the symbol `+` followed by the number `5` (there is no `+`
  sign in the numeric grammar); `-foo` reads as `-` then `FOO`.
- **Case folding is unconditional**: every earmuff, keyword, and general
  symbol is uppercased before interning; `foo`, `Foo`, `FOO` are the same
  symbol. Operator symbols contain no letters and are unaffected. There is
  no case-sensitivity mode and no escaped-symbol syntax.
- `T` (after uppercasing) reads as the ordinary interned symbol `T`.
  `NIL` (after uppercasing) does **not** read as a symbol — it reads as
  the same `Nil` value that `()` reads as. This special-casing happens
  only in the general production: `:nil` is the keyword symbol `:NIL`.
  See Part IV.
- **Keyword symbols are not "just a convention"**: the evaluator treats
  any symbol whose name begins with `:` as *self-evaluating* (evaluating
  `:foo` yields `:FOO` without a variable lookup), and every binding and
  assignment form (`LET`, `LET*`, `LAMBDA` parameters, `SETQ`, `DEF`,
  `DEFDYNAMIC`, `PROG`, `FOR`) rejects a keyword — and the symbol `T` — as
  a target with an error (`check_bindable`, `src/evaluator/core.rs`).
  Otherwise a keyword is an ordinary symbol: `EQ` by identity, usable as a
  hash key, printable, with a property list.

**Integer literals.** A `Number` is a 64-bit two's-complement signed
integer (Part V). Five productions, tried in the order given under
*Dispatch order*:

- *Decimal*: `-? digit+`. No `+` sign, leading zeros allowed. **No
  boundary guard**: `12abc` reads as `12` then `ABC`; `5.` reads as `5`
  followed by a dotted-pair marker. If the digits do not fit in `i64`, the
  token instead reads as a `Float` (`9223372036854775808` reads as the
  float `9223372036854775808.0`; `-9223372036854775808` fits and is a
  `Number`). It never errors and never becomes a bignum on any host
  (Part XII, axis 1, is about arithmetic results, not literals).
- *Octal, `Q` suffix*: `-? digit+ Q` (uppercase `Q` only). No boundary
  guard: `17Qx` is `15` then `X`. If a digit is not octal (`8Q`) or the
  value overflows `i64`, the production fails and the reader falls
  through to the decimal production (`8Q` reads as `8` then the symbol
  `Q`; an overflowing `777…7Q` reads as a float then `Q`). Portable
  programs must not write such tokens.
- *Hexadecimal, `H`/`h` suffix*: `-? digit hex-digit* [Hh]`, hex digits
  in either case. The digit run **must start with a decimal digit**: `FFh`
  is the symbol `FFH`; write `0FFh`. **Boundary guard**: if the character
  after the suffix is alphanumeric or `-`, the production fails
  (`ffhello` is a symbol; `1Ahx` falls through to read `1` then `AHX`).
  Overflow fails the production and falls through as for octal.
- *Radix-prefixed*: `# [xXbBoO] -? digit-in-radix+`. **Boundary guard**:
  an alphanumeric or `-` immediately after the digits fails the whole
  production, and because no other production accepts `#x`, the result is
  a parse error (`#b102` and `#xFG` are parse errors, not `#b10` `2`).
  Overflow is likewise a parse error. There is no `#d` prefix.
- *`i64::MIN`* can be written in decimal (`-9223372036854775808`) but not
  via any suffixed or prefixed form (those parse the magnitude first).

**Float literals.** `-? digit+ "." digit+ (("e"|"E") ("+"|"-")? digit+)?`.
Both the integer part and the fractional part require at least one digit,
and the exponent is legal **only after a fractional part**: `1.5`,
`-2.0e-3`, `1.0E10` are floats; `.5`, `5.`, and `1e5` are **not**. `1e5`
reads as the number `1` followed by the symbol `E5`. Inside a list, `.`
is the dotted-pair marker, so `(a .5)` reads as `(A . 5)`, `(5. a)` reads
as `(5 . A)`, and `(.5)` is a parse error. At top level a stray `.` is a
parse error. No boundary guard: `1.5x` reads as `1.5` then `X`. A literal
whose magnitude exceeds `f64` range reads as the corresponding infinity
(`1.0e999` is `+inf`), never an error.

**String literals**, delimited by `"`, may span lines (a raw LF inside a
string is part of the string). Recognized escapes: `\n` `\t` `\r` `\\`
`\"` `\0`. Any other backslash-prefixed character is **not an error and is
not stripped** — both characters are kept literally (`"a\qb"` is the
four-character string `a\qb`). A backslash as the last character of the
input, or an unterminated string, is a hard parse failure. A string's
contents are a sequence of Unicode scalar values (stored as UTF-8);
string-indexing primitives count code points, not bytes (Part IV).

**Character literals**, `'x'`: a `'` followed by exactly one character (or
one escape) and a closing `'`. Recognized escapes: `\n` `\t` `\r` `\\`
`\'` `\0`. Any other backslash-prefixed character decodes to that
character alone, with the backslash dropped (`'\q'` is `'q'`) — the
opposite convention from strings, and both are normative. A `Char` is a
code point in `0..=255`: a character above U+00FF (`'€'`) does not match
this production and the text then falls through to the quote production,
which fails on `€` — a parse error. Empty `''` is not a character
literal. Because this production is tried before quote, `'a'` is the
character `a` while `'a` followed by a delimiter is `(QUOTE A)`; `'(1)`
is `(QUOTE (1))` because `(` is followed by `1`, not `'`, but `'('` is the
character `(`.

**Record literals**, `#S(TypeName field...)` (also `#s`). No whitespace is
permitted between `#S` and `(`. The head of the inner list must be a
symbol (read and uppercased by the general production — it names the
record's brand); the rest must be a proper list of field values in
declaration order. A dotted tail, a non-symbol head, or an empty `#S()`
is a hard parse failure. Reading is purely structural — no type registry
is consulted, so a literal for an undeclared brand still reads.

**Lists.** `( ws (expr ws)* ( "." ws expr ws )? ")"`. `()` reads as `Nil`.
The dotted tail requires at least one preceding element (`( . x)` is an
error) and must be the last thing before `)` (`(a . b c)` is an error).
`(a . ())` and `(a . nil)` both read as the proper list `(A)`. Once `(`
has been consumed, a missing `)` is a hard failure (not a backtrack).

**Quote-family reader macros**, each producing a two-element list whose
head is an interned symbol: `'`*e* → `(QUOTE e)`; `` ` ``*e* →
`(QUASIQUOTE e)`; `,`*e* → `(UNQUOTE e)`; `,@`*e* → `(UNQUOTE-SPLICING e)`;
`#'`*e* → `(FUNCTION e)`. Whitespace and comments **are** permitted
between the macro character(s) and *e* (`' a` is `(QUOTE A)`).

**Nesting depth is bounded.** A reader must reject input nested more
deeply than some finite limit with an ordinary parse error rather than
exhausting its native stack. The reference's default limit is 512 levels
(`DEFAULT_READER_DEPTH`) and a stdlib-loaded environment raises it to
50,000; the exact number is a host detail, not part of the language.

**Every production in this grammar is required, not layered.** Decimal
integers, symbols (all four productions), strings, proper and dotted
lists, `NIL`/`T`, and the quote family are what a bare `lib/*.lisp`-running
host cannot do without; octal/hex-suffix and radix-prefix integers,
floats, block comments, character literals, and `#S(...)` records are no
longer optional latitude either — a conformant host implements all of
them, with exactly the meaning stated above. There is no
partial-conformance status for a host still missing one of these: it is
not yet conformant, full stop, and says so in its own documentation
(Part XIII) rather than being spec-permitted to treat the gap as
conformant-with-an-excuse.

## Part III — Printed representation

The printer (`src/printer.rs`, `print`) produces `PRIN1`-style readable
text: `(read (print x))` is `EQUAL` (Part IV) to `x` for every value built
from `Nil`, symbols, fixnums, finite floats, characters, strings, cons
cells, and records whose fields are themselves such values. The exact
rules:

- `Nil` prints as `()`. No code path ever prints the text `NIL` for this
  value.
- A symbol prints as its stored (uppercased) name, verbatim, ignoring its
  property list. `T` prints as `T`; a keyword prints with its colon
  (`:FOO`).
- A `Number` prints as a plain decimal integer with a leading `-` when
  negative — never with a radix marker, regardless of how it was read.
- A `Float` prints via the host's shortest-round-trip decimal conversion
  (Rust's `f64::to_string`), with one required post-processing rule: if
  the resulting text contains none of `.`, `e`, `E`, `inf`, `NaN`, a
  literal `.0` is appended. So `3.0` prints as `3.0`, `-0.0` prints as
  `-0.0`, `1.0e10` prints as `10000000000.0`, and `0.0000001` prints as
  `0.0000001`. Rust's conversion never produces scientific notation, so
  the `e`/`E` checks never fire on the reference: an extreme magnitude
  prints as a long run of plain digits (`1.0e30` prints as
  `1000000000000000000000000000000.0`), which Part II's grammar reads
  back. A host may instead emit scientific notation for extreme
  magnitudes only if its reader accepts what it emits (Part II's float
  grammar already does, provided a fractional part is present). Positive
  and negative infinity print as `inf` and `-inf`; a NaN prints as `NaN`.
  **These three forms are not readable** by Part II's grammar (they read
  as symbols); this is a round-trip gap in the reference, not a rule to
  conform to — a host may close it, and is not required to reproduce it.
- A `Char` prints as `'x'`, escaping `\n` `\t` `\r` `\\` `\'` `\0` exactly
  as the reader expects. Every other byte prints raw as the Unicode scalar
  value with the same numeric value (so byte 200 prints as `'È'`, encoded
  in UTF-8), including control characters other than the six escaped ones
  (byte 7 prints as a raw BEL between quotes). This is consistent with the
  reader, which accepts any scalar value `<= 255` between quotes.
- A `String` prints between `"`, escaping exactly `\"` `\\` `\n` `\t`
  `\r` `\0` — the same set the reader decodes. Every other character,
  including other control characters and all non-ASCII, prints raw.
- A proper list prints as `(a b c)`; an improper list prints its final
  non-`Nil` cdr after ` . `: `(A B . C)`. Printing recurses structurally
  on the cdr with **no cycle detection**; since cons cells are immutable
  (Part XII, axis 2) no program can construct a cycle, so this is not
  reachable from Lisp code.
- `#S(TYPENAME f1 f2 ...)` prints the brand followed by each field printed
  by these same rules, in declaration order — the exact inverse of the
  reader's record production.
- **Opaque values print as non-readable tags**, and a host must print
  *something* non-readable for them (the exact text is not portable):
  `<builtin>`, `<lambda>`, `<fexpr>`, `<macro>`, `<vau>`, `<native>`,
  `<hash-table>`, `<array:N>` (N the length), `<typed-array:int64:N>` /
  `<typed-array:float64:N>`, `<environment>`, and for a condition value
  `#<error "message">` or `#<error "message" data>` (message rendered as
  a debug-escaped string, data printed by these rules). Ports, network
  handles, and process handles print as `#<port:...>`, `#<net:...>`,
  `#<process ...>`.

## Part IV — Data model: types, equality, and truth

**The primitive types** are: `Nil` (the empty list, doubling as boolean
false), symbols, fixnums (`Number`), floats, characters (`Char`), strings,
and cons cells — plus compound and host-facing types (hash tables, arrays,
typed arrays, environments, closures and the other callables, records,
first-class conditions, and opaque handles for ports/network/processes)
whose equality rules differ from each other and are stated individually
below. Part V covers numeric detail; this section covers identity,
equality, truth, and the exact contract of each collection type.

**`NIL` is not a symbol.** `()` and the token `NIL` are the same value, a
distinct `Nil` type with no symbol behind it. `(symbolp nil)` is `NIL`;
`(atom nil)` and `(null nil)` are `T`; `(car nil)` and `(cdr nil)` are
`NIL` (not errors); `CAR`/`CDR` of any other non-cons is an error. There
is no distinguished `True` type; `T` is an ordinary interned symbol, bound
to itself in the global environment and unrebindable (Part II), used by
convention as *a* truthy value, not *the* truthy value.

**Truth is exactly "not `Nil`".** `IF`, `COND`, `AND`, `OR`, `WHILE`, and
`NOT` dispatch on one predicate (`is_truthy`): a value is false if and
only if it is `Nil`; every other value — `0`, `0.0`, the empty string, an
empty hash table, any symbol — is true.

**`Char` and `Number` are distinct types.** `'a'` is a `Char` with code
97; `97` is a `Number`. `(charp 'a')` is `T` and `(fixp 'a')` is `NIL`.
They are never `EQ` or `EQUAL` to each other, though arithmetic and
numeric comparison coerce a `Char` to its code (Part V): `(eq 'a' 97)` is
`NIL`, `(= 'a' 97)` is `T`.

**Strings are sequences of Unicode scalar values; characters are bytes.**
A string may contain any scalar value (`"héllo"` has length 5); every
string primitive that takes or returns an index counts code points
(`string-length*`, `index`, `substring`). A `Char` is always in
`0..=255`; `(make-char n)` errors outside that range; `(char-code c)`
returns a `Char`'s code, or for a non-empty string the full code point of
its **first** character (which may exceed 255; the empty string is an
error); `(code-char n)` returns a **one-character string**, not a `Char`.

**`EQ` on cons cells is pointer/identity comparison of the underlying
allocation** (resolved by issue #454; formerly a hard-coded, unconditional
`NIL` whenever either argument was a cons cell — including comparing a
cons cell against itself — justified by a misreading of the Lisp 1.5
manual's silence on non-atoms as a mandate that `EQ` on non-atoms be
`false`; that was a stronger and more surprising restriction than the
manual actually requires, and diverged from every Lisp since, where `EQ`
on lists is pointer identity and `(eq x x)` for the same actual cons cell
is true). **This is now a MUST-match rule, not host latitude:**
`(let ((x (cons 1 2))) (eq x x))` is `T`; `(eq (cons 1 2) (cons 1 2))` is
`NIL` for two separately allocated, even structurally-identical, cons
cells; aliasing an existing cons cell through a second binding, or
reaching it via a different path (e.g. `(cdr (cons 0 x))` when `x` is a
cons), is `T` because it is the same underlying allocation. In the Rust
reference (`src/evaluator/builtins_core.rs`, `BuiltinFunc::Eq`) this
compares the `car` and `cdr` fields' `Shared`/`Rc` pointers with
`Shared::ptr_eq`, not the general structural `PartialEq for LispVal`
relation described below — `cons` always allocates fresh `car`/`cdr`
cells, and cloning a `LispVal::Cons` only bumps their refcounts, so two
`Shared` pointers are equal exactly when they denote the same allocation.
A host on a different representation (e.g. a single boxed pair cell) must
reproduce the same observable rule: pointer identity of the cons cell
itself, not of its contents.

For every other pair of values, `EQ` is the relation implemented by
`PartialEq for LispVal` (`src/lib.rs`), which is also the relation used
by `EQUAL` at the leaves (for atoms; `EQUAL` never asks whether two
conses are `EQ` to each other — see below), by `CATCH` tag matching
(Part VI), and by hash table keys (below). Two values of **different
types are never `EQ`**. Within one type:

- **Fixnum, float, character, string**: value equality. Two freshly
  computed, unshared values holding the same value **are** `EQ`:
  `(eq 100000 100000)` is `T`; `(eq "ab" (concat "a" "b"))` is `T`. A host
  whose native representation boxes or interns these differently must
  still compare them by value.
- **Float, specifically**: IEEE `==` **plus** an explicit carve-out that
  `NaN` is `EQ` to `NaN` (`lisp_float_eq`). Consequently `(eq 0.0 -0.0)`
  is `T` (IEEE says they are equal) and `(eq (/ 0.0 0.0) (/ 0.0 0.0))` is
  `T` (the carve-out). A host must reproduce both, not delegate to its
  language's native float comparison unmodified.
- **Symbol**: identity of the interned symbol object. Because the reader
  interns, two reads of the same name in the same symbol table denote the
  same object. Note that `(make-environment)` with no arguments creates a
  **separate symbol table** (below), so a symbol *interned or read inside
  that environment* is a different object from the same-named symbol
  outside it — `(eq 'foo (eval '(read-from-string "foo") e))` is `NIL` for
  such an `e`, while `(eq 'foo (eval ''foo e))` is `T` because the quoted
  form carries the outer symbol object into `e`.
- **`Nil`**: always `EQ` to `Nil`.
- **Builtin**: two references to the same primitive operation are `EQ`
  (`(eq car (function car))` is `T`); a host-registered native function
  compares by identity.
- **Closures (`LAMBDA`), macros, fexprs, and `VAU` operatives**: equal
  when their parameter lists (including the rest parameter, if any) and
  bodies are `EQUAL` as data **and** their captured environments are the
  identical environment object. This means `(eq (lambda (x) x)
  (lambda (x) x))` is `T` when both are constructed in the same
  environment, and `NIL` when constructed in different call frames.
- **Hash tables, arrays, typed arrays, environments, and every opaque
  host handle** (ports, network handles, process handles): identity.
  Two separately constructed, content-identical tables or arrays are
  never `EQ` or `EQUAL`; `(let ((h (make-hash-table))) (eq h h))` is `T`.
- **Records** (`#S(...)` values, `DEFRECORD` instances): structural — same
  brand name **and** fields pairwise `EQ`-or-`EQUAL` (the recursive
  relation). `(eq #S(p 1) #S(p 1))` is `T`; `(equal #S(p 1) #S(q 1))` is
  `NIL`. Records are value types for equality even though they are
  compound; a host must not compare them by identity.
- **Condition values** (`make-error`): structural on message string and
  data payload, like records.

**`EQUAL` is deep structural equality**, and in the reference it is
library code (`lib/04-predicates.lisp`): `(equal a b)` is `(eq a b)` when
`a` is an atom (anything that is not a cons), `NIL` when `a` is a cons and
`b` is not, and otherwise the conjunction of `(equal (car a) (car b))` and
`(equal (cdr a) (cdr b))`. It never asks whether two conses are `EQ` to
each other, so two structurally identical but separately allocated lists
are `EQUAL` even though (since issue #454) they are not `EQ` — the two
relations coincide only when the same cons cell is compared to itself.
`EQUAL` on any pair of non-cons values is
therefore exactly `EQ` on them, including identity for hash tables and
arrays and structure for records. **`EQUAL` performs no numeric
contagion**: `(equal 5 5.0)` is `NIL`, `(equal 'a' 97)` is `NIL`. A host
must not "helpfully" make them true.

**Hash tables, arrays, typed arrays, and environments each have a
specific, closed contract**, stated here so that a program which runs on
the reference does not break on a host whose native map or vector type
has slightly different key-equality or element-type rules:

- **Hash table.** `(make-hash-table)` takes **no arguments**; there is no
  `:test` and no way to choose a key-equality policy. Keys and values are
  unconstrained (any type, including hash tables, arrays, closures, and
  `Nil` as a key), and one table may mix key types. Key equality is the
  recursive `EQ`/`EQUAL` relation above (the reference stores keys in a
  `HashMap<LispVal, LispVal>` whose `Eq` is `PartialEq for LispVal` and
  whose `Hash` is consistent with it): a fresh `(cons 1 2)` finds a key
  stored as `(cons 1 2)`; `1` and `1.0` are **distinct** keys; `0.0` and
  `-0.0` are the **same** key; `NaN` finds `NaN`; a lambda constructed
  with the same text in the same environment finds the stored lambda;
  content-identical arrays are distinct keys. A host whose native map
  derives hashing independently of equality must make the two agree on
  every one of these points. Operations: `(sethash table key value)`
  inserts or replaces and returns `T`; `(gethash table key)` returns the
  value, or **`NIL` when the key is absent — there is no second return
  value and a stored `NIL` is indistinguishable from absence**; `(remhash
  table key)` removes and returns `T` whether or not the key was present;
  `(keys table)` returns a list of the keys in **unspecified order**. The
  table argument is always first; passing a non-table is an error.
- **Array.** `(array n)` creates a fixed-length vector of `n` slots, each
  `Nil`; `n` must be a non-negative fixnum (a float, a negative number, or
  a non-number is an error). The length never changes: there is no
  resize, push, or pop primitive for this type, and a host must not add
  one under these names. Slots hold independently typed values of any
  type; `(store arr i v)` overwrites slot `i` and **returns `v`**;
  `(fetch arr i)` returns slot `i`; `(array-length* arr)` returns `n`. An
  index must be a non-negative fixnum (`-1` and `1.0` are errors) and
  must satisfy `i < n` (an out-of-range index is an error, never a
  wraparound or a silent `NIL`). An array may contain itself. The
  reference refuses `n > 16,777,216` (2^24) with an error rather than
  attempting the allocation; the specific ceiling is a reference detail,
  but *some* finite, catchable-error ceiling is required, in the same
  spirit as Part X's fuel and Part VI's recursion bound.
- **Typed array.** `(typed-array n elem-type)` — `n` as for `array`, and
  `elem-type` must be exactly the symbol `INT64` or `FLOAT64` (case
  folded by the reader, so `'int64` works); any other symbol or a
  non-symbol is an error. Slots are zero-initialized (`0` or `0.0`).
  `fetch`, `store`, and `array-length*` accept typed arrays with the same
  index rules as `array`. Storage is type-checked per slot by a rule
  **narrower than Part V's arithmetic coercion**: an `INT64` array
  accepts only a `Number` (a `Float` or a `Char` is an error — `Char` is
  **not** coerced to its code here); a `FLOAT64` array accepts a `Float`
  (stored as-is, including `NaN`) or a `Number` (converted to the nearest
  `f64`), and rejects a `Char`. Reading always yields the declared type
  (`Number` for `INT64`, `Float` for `FLOAT64`). Typed arrays compare by
  identity.
- **Environment.** `(the-environment)` returns the current lexical
  environment as a first-class value; `(make-environment)` with no
  arguments returns a **fresh root environment holding only native
  builtins and `T`**: none of `lib/*.lisp` is present, **no capability is
  enabled** (regardless of the caller's grants), and it has its own
  symbol table and its own dynamic-variable registry.
  `(make-environment parent)` returns an ordinary lexical child of
  `parent` that sees every binding of `parent` and **shares** `parent`'s
  capability grant, symbol table, and dynamic registry. Two or more
  arguments, or a non-environment argument, is an error. `(eval form
  env)` evaluates `form` in `env`. A host that makes the zero-argument
  form return a stdlib-loaded or capability-carrying environment has
  implemented a different, more permissive primitive.

## Part V — The numeric tower

There are exactly two numeric types: `Number` (a 64-bit two's-complement
signed integer, the fixnum) and `Float` (IEEE 754 binary64). There are no
bignums, ratios, or complex numbers in the reference (Part XII, axis 1,
lets a host add arbitrary precision *above* the fixnum range).

**Contagion.** In `+`, `-`, `*`, `/` and in the comparisons `<`, `>`,
`=`, if **any** operand is a `Float`, every operand is converted to
`f64` and the operation is performed in floating point — a call-wide
predicate, not first-argument-wins or pairwise promotion. A `Char`
operand is unconditionally coerced to its code point (as a fixnum in the
integer path, as an `f64` in the float path) in these same operators.
Any other operand type (string, symbol, `Nil`, list) is an error.

**The four arithmetic operators** (`apply_math_op`,
`src/evaluator/builtins_core.rs`):

- `(+ a...)` is variadic; `(+)` is `0`. `(* a...)` is variadic; `(*)` is
  `1`. `(- a)` negates; `(- a b...)` subtracts each later operand from
  the first; `(-)` is an error. `(/ a b)` takes **exactly two**
  operands; one or three is an error. Integer-path results are fixnums;
  float-path results are floats (`(+ 1 2.0)` is `3.0`, `(* 2 'a')` is
  `194`).
- Fixnum `/` **truncates toward zero** (`(/ 7 2)` is `3`, `(/ -7 2)` is
  `-3`) and never returns a float. Fixnum division by zero is an error.
  Float `/` never checks for zero: `(/ 1.0 0)` is `inf`, `(/ 0.0 0.0)` is
  `NaN`, `(/ -1.0 0.0)` is `-inf`.
- **Overflow** on the integer path wraps modulo 2^64 (two's complement)
  **and sets the global `OVERFLOW` flag**, observable from Lisp with
  `(flag-set-p 'OVERFLOW)` and cleared with `(clear-flag 'OVERFLOW)` or
  `(clear-all-flags)`: `(+ 9223372036854775807 1)` returns
  `-9223372036854775808` and sets the flag. The same wrap-and-flag rule
  applies to `(- i64::MIN)`, `*`, `(/ i64::MIN -1)`, `(remainder i64::MIN
  -1)`, `gcd`, and `lcm`. This is the fixed-width model of Part XII,
  axis 1; a host on the arbitrary-precision model returns the exact
  result and never sets the flag.

**Remainder and modulus — two operators, two sign conventions, and both
accept only fixnums** (`REMAINDER`: `builtins_core.rs`; `MOD`:
`builtins_extra.rs`). A `Float` or a `Char` operand to either is an error
— Part V's contagion and char coercion do **not** apply here, and a
zero divisor is an error in both:

- `(remainder a b)` is the truncated remainder, sign following the
  *dividend*: `(remainder -7 2)` is `-1`, `(remainder 7 -2)` is `1`.
- `(mod a b)` is the Euclidean remainder, always in `0 <= r < |b|`:
  `(mod -7 2)` is `1`, `(mod 7 -2)` is `1`.
- On the one overflowing input, `(mod i64::MIN -1)`, the reference
  returns `0` **without** setting `OVERFLOW`, whereas `/` and `REMAINDER`
  on `(i64::MIN, -1)` set the flag. See Part XII, axis 4.

**Comparison** (`<`/`LESSP`, `>`/`GREATERP`, `=`): each takes **two or
more** operands (one is an error) and is a monotone chain — `(< a b c)`
is `T` exactly when `a < b` and `b < c`; `(= a b c)` when all are
numerically equal. Operands may mix fixnum, float, and character freely:
two fixnums (or two characters) compare exactly as integers; any operand
pair involving a float is compared as `f64` (so a fixnum above 2^53 may
compare equal to a float it is not equal to mathematically); `(= 5 5.0)`
is `T`; `(< 1 'a' 200)` is `T`. Any non-numeric operand (a string, a
symbol, `Nil`) is an error, never a permissive `NIL`. Equality of floats
under `=` is exact IEEE `==` (so `(= NaN NaN)` is `NIL` — unlike `EQ`,
Part IV).

**Other numeric primitives the corpus relies on**, with their exact
result types: `(float x)` converts a fixnum or char to `Float` (identity
on a float). `(floor x)`, `(ceiling x)`, `(round x)`, `(truncate x)`
accept any numeric and return a **fixnum**; `round` rounds half **away
from zero** (`(round 2.5)` is `3`, `(round -2.5)` is `-3`, `(round 0.5)`
is `1`); a result outside fixnum range saturates to `i64::MIN`/`i64::MAX`
and a `NaN` argument yields `0`. `(expt base exp)`: fixnum base with
non-negative fixnum exponent yields a fixnum and **errors on overflow**
(no wrap, no flag — `(expt 2 70)` is an error); a negative fixnum
exponent yields a float; any float operand yields a float. `sqrt`, `sin`,
`cos`, `tan`, `exp`, `log` accept any numeric and return a float.
`(gcd ...)` and `(lcm ...)` are variadic over fixnums with `(gcd)` = `0`,
`(lcm)` = `1`. `(plusp x)` and `(minusp x)` accept a fixnum or a float;
`(zerop x)` accepts **only a fixnum** — `(zerop 0.0)` is an error — and
all three error on a non-number.

**Within 64-bit signed range, every arithmetic result must match the
Rust reference bit-for-bit** — including float results, which are
ordinary IEEE binary64 operations in round-to-nearest-even. Overflow
beyond that range is the one place hosts may diverge (Part XII, axis 1).

## Part VI — Evaluation model

**Evaluating a value** (`eval_step`, `src/evaluator/special_forms.rs`):
`Nil` evaluates to `Nil`; a keyword symbol evaluates to itself; any other
symbol is a variable reference resolved per the rules below (an unbound
symbol is an error); every non-symbol atom — number, float, char, string,
and every compound or opaque value that is not a cons — is
self-evaluating. A cons is a *form*: if its head is a symbol carrying a
special-form tag, the form is dispatched to that special form; otherwise
it is an application.

**Special forms are recognized by symbol identity, not by binding.** The
tag is attached to the interned symbol at intern time
(`SymbolTable::intern`, `src/environment.rs`), so a form whose head is
one of the names in Part VII is *always* that special form — a local or
global binding of that name is never consulted in operator position.
Conversely the names are not reserved as variables: `(let ((if 3)) if)`
is `3`. Special forms are not first-class: evaluating the bare symbol
`IF` is an ordinary (usually unbound) variable reference, and a special
form cannot be passed to `FUNCALL`/`APPLY`.

**Application evaluates the operator first, then the operands left to
right, each exactly once.** The operator position is evaluated by the
ordinary rule above (there is no separate function namespace: a symbol in
operator position resolves to its variable value). If the operator value
is a macro, fexpr, or `VAU` operative, the operands are **not** evaluated
(see below). Otherwise the operand list must be a proper list (a dotted
operand list is an error) and each operand is evaluated in order; the
resulting value is then applied. Applying anything other than a closure,
builtin, or native function is an error ("not a function"), as is an
arity mismatch.

**`IF` takes exactly three operands**: `(if test then else)`. A
two-operand `(if test then)` or a four-operand form is an **error**, not
an implicit `NIL` else — `WHEN`/`UNLESS` (library macros in
`lib/12-control.lisp`) exist for the one-armed case. The test is evaluated
(non-tail); then exactly the selected branch is evaluated, in tail
position; the other branch is never touched.

**`COND`**: `(cond clause...)`. Each clause must be a non-empty proper
list `(test body...)`; an atom clause or an empty `()` clause is an error
(detected only when reached). Tests are evaluated in order until one is
truthy; that clause's body forms are then evaluated in order and the
**last body form is in tail position**. A clause `(test)` with no body
returns the test's value. If no test is truthy, `COND` returns `NIL`.

**`PROGN`**: `(progn)` is `NIL`; otherwise the forms are evaluated in
order and the last is in tail position.

**`AND` and `OR` are kernel special forms in the reference** (Part VII).
`(and)` is `T`; `(and f...)` evaluates left to right and returns `NIL`
at the first `NIL`, else the value of the last form. `(or)` is `NIL`;
`(or f...)` returns the first truthy value, else `NIL`. **No position in
`AND`/`OR` is a tail position.**

**`QUOTE`** takes exactly one operand and returns it unevaluated.
**`QUASIQUOTE`** takes exactly one template and rebuilds it
(`quasiquote_eval`, `src/evaluator/quasiquote.rs`): an atom is returned
as-is; a cons `(UNQUOTE e)` is replaced by the value of `e`; a list
element `(UNQUOTE-SPLICING e)` is replaced by the elements of the value
of `e`, which must be a proper list (else an error), followed by the
processed rest of the list — this works in any element position,
including before a dotted tail; any other cons is rebuilt from its
processed car and cdr. **There is no nesting-level tracking**: an inner
`` ` `` is an ordinary symbol `QUASIQUOTE` in the template and inner
`,` forms are still evaluated at the *outer* level — `` `(a `(b ,(+ 1 2))) ``
yields `(A (QUASIQUOTE (B 3)))`. An `UNQUOTE` with other than one operand
is an error; an ill-formed `UNQUOTE-SPLICING` is treated as ordinary data.

**`LET` binds in parallel; `LET*` binds sequentially.** Both take a
binding list followed by **at least one** body form (a bare `(let ())` is
an error; `(let () 1)` is `1`). Every binding must be a two-element list
`(name init)` — `(x)` or a bare `x` is an error — and `name` must be a
non-keyword symbol other than `T` (`NIL` is not a symbol, so `(let ((nil
1)) ...)` is an error). In `LET`, every `init` is evaluated in the
**outer** environment, in order, before the body sees any new binding.
In `LET*`, one new frame is created and each `init` is evaluated with all
earlier bindings of the same `LET*` already installed. Multiple body
forms are implicitly wrapped in `PROGN`; the body is in tail position.

**Tail calls: a specific, closed list of positions receive proper,
unbounded tail-call elimination** (the trampoline in
`run_trampoline_inner`, `src/evaluator/functions.rs`): the last form of a
`LAMBDA` body; both branches of `IF`; the last body form of the selected
`COND` clause; the last form of `PROGN`; the (implicitly `PROGN`-wrapped)
body of `LET` and `LET*`; the body of a `VAU` operative or fexpr; and the
expansion of a macro. A call in any of these positions to another
function, however long the chain, must not grow the host's native stack:
`(defun tl (n) (if (= n 0) 'done (tl (- n 1))))` runs for ten million
iterations. **Every other position is a non-tail position**, and a
portable program must not rely on tail behavior there — in particular the
body of `CATCH`, `BLOCK`, `UNWIND-PROTECT`, `HANDLER-CASE`, `AND`, `OR`,
`WITH-FUEL`, `WITH-CAPABILITIES`, `PROG`, `WHILE`, `FOR`, and every
function-call *argument*. Nor does a call through `FUNCALL`, `APPLY`, or
a host embedding API receive tail treatment: the reference evaluates the
callee's body with a fresh non-tail `eval`, so `(defun f (n) (if (= n 0)
'done (funcall 'f (- n 1))))` exhausts the recursion bound for large `n`
while the direct call does not.

**Non-tail evaluation depth is bounded, and exceeding the bound is a
catchable error**, not a native stack fault (`DepthGuard`,
`src/evaluator/core.rs`; default `10_000` nested `eval` frames,
host-adjustable). The bound counts `eval` entries, so one Lisp-level
non-tail call may cost more than one frame (a `FUNCALL` costs two: the
`funcall` application and the callee's body). A host must size its native
stack so that this bound, not the stack, is what a program hits first;
the reference runs the evaluator on a 512 MiB thread for exactly this
reason.

**`LAMBDA` parameter lists are fixed-arity, plus at most one rest
parameter**, spelled either `(a b &REST r)` or with a dotted tail `(a b .
r)` — never both. `&REST` must be followed by exactly one symbol; any
other `&`-prefixed name (`&OPTIONAL`, `&KEY`, ...) anywhere in the list
is an error **at definition time**; parameter names must be non-keyword
symbols other than `T`. A body of zero forms is legal and evaluates to
`NIL`; multiple forms are wrapped in `PROGN`. Calling a lambda with `n`
fixed parameters and no rest parameter requires exactly `n` arguments;
with a rest parameter, at least `n` — the surplus is collected into a
fresh proper list (possibly `NIL`) bound to the rest parameter. A closure
captures the environment in which the `LAMBDA` form was evaluated; each
call creates one fresh frame whose parent is that captured environment
(lexical scoping — the caller's environment is never consulted for
lexical lookup). `(function f)` / `#'f` returns the value of `f` if it is
a callable (else an error); `#'(lambda ...)` is the same as the `LAMBDA`
form.

**Variable resolution** (`Environment::resolve`): if the symbol is
dynamic (below), read the symbol's single global value cell. Otherwise
walk the lexical frame chain from the current frame outward; the root
frame's storage *is* the symbol's value cell. An unbound symbol is an
error.

**`SETQ`**: `(setq var1 val1 var2 val2 ...)` — an odd number of operands
is an error; each `var` must be a non-keyword symbol other than `T`; each
`val` is evaluated (left to right, each assignment taking effect before
the next `val` is evaluated) and the last value is returned. Each
assignment resolves its target as follows (`Environment::update_sym`):
1. If the symbol is dynamic, write its global value cell. Read together
   with the dynamic-variables paragraph, this is coherent: once a symbol
   is dynamic, `LET`, lambda parameters, and every other binding form
   install a *dynamic* binding for it (saving and later restoring the
   same cell) rather than a frame slot, so `(let ((x 1)) (setq x 2) x)`
   is `2` whether or not `x` is dynamic.
2. Otherwise walk the lexical chain outward and overwrite the first frame
   in which the symbol is already bound (the root frame counts as bound
   when the symbol's value cell holds a value).
3. If no frame binds it, **create** a new binding in the frame where the
   `SETQ` was evaluated (or in the global cell if that is the root): `(let
   ((x 1)) (setq y 2) y)` is `2` and `y` is unbound afterwards. `SETQ` of
   an unknown name is never an error.

**Dynamic variables use shallow binding, and declaring a symbol dynamic
is global, retroactive, and irreversible.** `(defdynamic name init
[docstring])` — also spelled `(defvar ...)`, the same special form —
requires an `init` form (a one-operand `(defdynamic *x*)` is an error),
marks the symbol dynamic in the environment's shared registry, then
evaluates `init` and stores it in the symbol's global value cell, and
returns the symbol. (The reference prints a warning to stderr when the
name lacks `*earmuffs*`; that is not normative.) From that moment:
- resolution of that symbol, in **every** frame and every already-created
  closure, reads the global cell (`resolve` checks the flag on each
  reference, not at binding time): `(defun f (x) (lambda () x))`, `(setq g
  (f 10))`, `(defdynamic x 99)`, `(funcall g)` yields `99`;
- every binding form (`LET`, `LET*`, lambda and macro parameters, `PROG`
  variables, `FOR`) binds it by saving the cell's current contents,
  installing the new value, and **restoring the saved contents when the
  form exits by any path** — normal return, error, `THROW`,
  `RETURN-FROM`, `RETURN`/`GO`, or fuel exhaustion — including bindings
  accumulated across a chain of tail calls, which are restored in LIFO
  order when the chain finally returns: `(defdynamic *x* 1) (catch 'c
  (let ((*x* 2)) (throw 'c 0))) *x*` is `1`;
- `SETQ` writes the cell (rule 1 above), so an assignment inside a
  dynamic binding is visible to callers until the binding exits and is
  then undone by the restore;
- there is no undeclare. This matches CL's `(proclaim '(special x))`
  and a host must reproduce it exactly; scoping the declaration to a
  file or module, or letting live closures keep their lexical slots, is
  a conformance failure.
Symbols in the registry are shared by an environment and all its lexical
children (and copied by a world fork); `(make-environment)` with no
arguments starts with an empty registry (Part IV).

**`VAU`, fexprs, and macros are three distinct operative mechanisms:**
- **`(vau (ops env) body...)`** — the parameter list must be exactly two
  symbols. On application, a fresh child of the operative's *definition*
  environment binds `ops` to the **entire unevaluated operand list** and
  `env` to the **caller's environment** as a first-class value; the body
  (implicitly `PROGN`-wrapped) runs in tail position. `((vau (o e) (list o
  (eval (car o) e))) (+ 1 2))` is `(((+ 1 2)) 3)`. `$VAU` is an alias.
  Applied via `APPLY`, the (already evaluated) argument list is what `ops`
  receives. Anonymous `(vau ...)` and `defvau` (library) both produce
  ordinary first-class values.
- **A fexpr** — `(defexpr name (params...) body)` or the anonymous
  `(fexpr (params...) body...)` — takes a proper parameter list with no
  rest parameter (a dotted tail is an error). A fexpr with **exactly one
  parameter** binds it to the whole unevaluated operand list; a fexpr
  with `n != 1` parameters requires exactly `n` operands and binds them
  one-to-one, unevaluated. The body runs in a fresh child of the
  definition environment, in tail position, with no access to the
  caller's environment except through values it is handed.
- **A macro** — `(defmacro name (params...) body)` or the anonymous
  `(macro (params...) body...)` — has a lambda-style parameter list
  (fixed parameters plus optional `&REST`/dotted rest) checked with the
  same arity rules as lambdas. At **every** call the body runs in a fresh
  child of the definition environment with the unevaluated operands
  bound, producing an *expansion*; the expansion is then evaluated in the
  **caller's** environment, in tail position. **Expansion is never
  cached**: each call re-runs the macro body. `(macroexpand form)`
  performs one expansion step without evaluating the result.
- `DEFEXPR` and `DEFMACRO` take three or four operands (`name params
  [docstring] body`); the body is a single form. They bind `name` in the
  current environment (globally, at top level) and return the symbol.

**Three non-local exit mechanisms exist, all dynamic-extent, and an
unmatched exit of any of them is not a condition — it propagates to the
top level and halts the unit of execution** (they are distinct `LispError`
variants — `Throw`, `ReturnFrom`, `Return`, `Go` — that `HANDLER-CASE`
and `ERRORSET` deliberately let pass, Part VIII):
- **`(catch tag-form body...)`** evaluates `tag-form` (so tags are
  values), then the body forms in order (**not** tail positions),
  returning the last value or `NIL` for an empty body. **`(throw tag-form
  value-form)`** evaluates both and unwinds to the innermost dynamically
  enclosing `CATCH` whose tag is **`EQUAL`-equivalent** to the thrown tag
  — the recursive `PartialEq` relation of Part IV, so a list tag `'(a b)`
  matches a freshly built `(list 'a 'b)` and a string tag matches an
  equal string — yielding `value`. Non-matching `CATCH` frames re-raise
  the throw outward.
- **`(block name body...)`** with an **unevaluated** symbol `name`;
  **`(return-from name [value])`** (value defaults to `NIL`) unwinds to
  the innermost dynamically enclosing `BLOCK` with the same name —
  dynamic, not lexical: a function called from inside the block may
  `RETURN-FROM` it. Body forms are not tail positions.
- **`(prog (vars...) item...)`** binds each var to `NIL`, then executes
  the items in order, treating a bare symbol item as a label. `(go
  label)` jumps to a label in the innermost dynamically enclosing
  `PROG` (an unknown label is an error at the `PROG`); `(return value)`
  exits the innermost `PROG` with `value`; falling off the end yields
  `NIL`. Items are not tail positions.

**`(unwind-protect body-form cleanup...)` is a kernel special form**:
`body-form` (exactly one form, non-tail) is evaluated; then **every
cleanup form is evaluated unconditionally** — after a normal return, an
error, a fuel exhaustion, or any non-local exit passing through — and the
body's outcome (value or propagating error/exit) is then delivered. **An
error raised by a cleanup form is discarded**, and does not replace the
body's outcome or stop the remaining cleanup forms. A cleanup form's
value is never returned.

**`WHILE` and `FOR` are kernel special forms in the reference**: `(while
test body...)` re-evaluates `test` before each pass and returns `NIL`;
`(for (var start end [step]) body...)` evaluates `start`, `end`, and
`step` once (fixnums; a zero step is an error), iterates `var` from
`start` to `end` **inclusive** in one reused frame, and returns `NIL`.
Both may be derived from `COND`/`LET`/tail calls on a host (Part XII,
axis 3) provided the observable behavior — including the single reused
`FOR` frame, which every closure created in the body shares — matches.

## Part VII — Special forms reference

The reference attaches a special-form tag to exactly these symbol names
(`SymbolTable::intern`, `src/environment.rs`); every one of them is
dispatched by identity in operator position as Part VI describes:

- **Core evaluation**: `QUOTE`, `QUASIQUOTE`, `IF`, `COND`, `AND`, `OR`,
  `PROGN`, `LET`, `LET*`, `SETQ`, `LAMBDA`, `FUNCTION`, `LABEL`.
- **Definition**: `DEF` (`(def name value [docstring])`, binds in the
  current frame), `DEFINE` (Lisp 1.5 `((name value)...)` list form),
  `DEFDYNAMIC`/`DEFVAR`, `DEFEXPR`, `DEFMACRO`, `MACRO`, `FEXPR`, `VAU`/`$VAU`.
- **Control**: `CATCH`, `THROW`, `BLOCK`, `RETURN-FROM`, `PROG`, `RETURN`,
  `GO`, `WHILE`, `FOR`, `UNWIND-PROTECT`, `HANDLER-CASE`.
- **Fencing**: `WITH-FUEL`, `WITH-CAPABILITIES` (Parts IX–X).
- **Typed subset** (reference-specific, not required for the `lib/*.lisp`
  kernel surface and not further specified here): `DEFUN-TYPED`,
  `DEFUN*`, `DEFSTRUCT-TYPED`, `DECLARE-TYPED`, `JIT-OPTIMIZE`,
  `CHECK-TYPE`.

The forms given exact semantics in Parts VI, VIII, IX and X are the ones
a host must reproduce exactly, whether it implements each as a true
kernel primitive or derives it from a smaller set — Part XII, axis 3,
names which are eligible for derivation. Every other named construct the
corpus uses — `DEFUN` (a macro in `lib/00-core.lisp`), `WHEN`, `UNLESS`,
`DOLIST`, `DOTIMES` (`lib/12-control.lisp`), `IGNORE-ERRORS`,
`RESTART-CASE`, `HANDLER-BIND` (`lib/16-conditions.lisp`), `DEFVAU`, the
CL-compat layer — is library code built from this list plus the
primitives of Parts II–VI; a host that gets this list right and loads
`lib/*.lisp` unmodified gets those forms for free.

## Part VIII — The condition system

**A condition is a value with exactly two fields, and there is no
taxonomy.** `(make-error message [data])` builds a first-class condition
value: `message` is a string (any other value is converted with the Part
III printer, so `(make-error 'sym)` has message `"SYM"`); `data` is any
single value, `Nil` if omitted; further arguments are ignored. `(error-p
v)`, `(error-message c)`, `(error-data c)` inspect it. There is no type
tag, no class hierarchy, and no `DEFINE-CONDITION`-style extension at
either the native or the library level.

**Signalling.** `(error)` signals a condition with message `"Error"` and
`Nil` data; `(error c)` where `c` is a condition value re-signals it
unchanged; `(error message [data] ...)` signals `(make-error message
data)` (only the first extra argument is data). A native failure inside a
primitive or special form (unbound variable, wrong argument count, type
error, division by zero, index out of range, recursion bound, fuel
exhaustion, capability denial, ...) is raised as a *native error* carrying
only a message; the moment a handler observes it, it is converted to a
condition value with that message and `Nil` data, so **handlers see one
uniform two-field value** regardless of origin.

**`HANDLER-CASE` catches every condition unconditionally.**
`(handler-case expr (head (var) handler-body...))` takes exactly two
operands: the protected form and **one** clause. The clause's `head`
symbol is **not inspected** (`error` by convention; any symbol works);
its second element is a list whose first symbol, if present, is bound to
the condition. If `expr` returns normally its value is returned. If a
condition or native error is raised anywhere in `expr`'s dynamic extent,
control unwinds to the `HANDLER-CASE`, a fresh child environment binds
`var` to the condition value, and the handler body forms are evaluated in
order (non-tail), the last value being the result. There is no second
clause and no type specifier: "catch this kind but not that kind" is not
an operation the language provides, and a program distinguishes causes by
inspecting the message or data by convention. **This is a scoping fact
about the language, not a gap left open**: `lib/16-conditions.lisp`'s
`RESTART-CASE`/`HANDLER-BIND`/`INVOKE-RESTART` layer is built entirely on
`HANDLER-CASE`, `CATCH`/`THROW`, dynamic variables, and `ERRORSET`, and
adds no typing.

**`ERRORSET` is a kernel primitive (a builtin function, not a special
form).** `(errorset form [ignored])` evaluates the *value* `form` (so
callers quote it: `(errorset '(car 5))`) in the current environment and
returns a one-element list `(value)` on success, or `NIL` if a condition
or native error was raised — the wrapper list makes a successful `NIL`
distinguishable from failure. `IGNORE-ERRORS` is a library macro over it.

**Non-local exits are not conditions and are invisible to both
`HANDLER-CASE` and `ERRORSET`.** A `THROW`, `RETURN-FROM`, `RETURN`, or
`GO` passing through a `HANDLER-CASE` or `ERRORSET` is not intercepted;
if no matching `CATCH`/`BLOCK`/`PROG` exists on the dynamic chain it
propagates to the top level and terminates the unit of execution
(`(handler-case (throw 'foo 1) (error (e) 'caught))` is not caught). A
portable program must ensure every target is reachable, because there is
no safety net. `UNWIND-PROTECT` cleanups do run as such an exit passes.

**The exact message text a native error produces is not part of the
portable surface**, even though the reference's condition value carries
one: messages vary in capitalization and detail from one primitive to the
next, and at least one (calling a non-callable value: `"Not a function:
Number(5)"`) embeds a Rust debug rendering rather than a Lamedh-printed
value. A conformant host must signal *a* condition — of the same
two-field form — for the same class of native failure (unbound variable,
calling a non-callable value, wrong number of arguments, non-numeric
argument to a numeric operator, division by zero, index out of range,
typed-array type mismatch, recursion bound exceeded, fuel exhausted,
capability denied), but is not required to reproduce the wording, and a
program that pattern-matches exact text relies on something this
specification does not guarantee.

## Part IX — Capability-gated I/O

Read, write, and syscall-adjacent operations (filesystem, shell, process
and environment access, networking, stdin) sit behind a capability system
a host enforces **at the primitive call site** — not merely as
bookkeeping.

**Names and enforcement.** The reference's capability names are exactly:
`READ-FS`, `CREATE-FS`, `TEMP-FS`, `SHELL`, `IO`, `NET-DNS`,
`NET-CONNECT`, `NET-LISTEN`, `OS-ENV`, `OS-ENV-WRITE`, `OS-PROCESS`,
`OS-SIGNAL`. Each has a gate function of one uniform form
(`require_read_fs` and siblings, `src/evaluator/builtins_core.rs`) that a
gated primitive calls before acting; a name is compared exactly after
uppercasing. Resource-acquiring network and process primitives call their
gate (`net-resolve` → `NET-DNS`; `tcp-connect*`, `udp-connect`,
`udp-send-to` → `NET-CONNECT`; `tcp-listen*`, `udp-bind` → `NET-LISTEN`;
environment reads → `OS-ENV`, writes → `OS-ENV-WRITE`; spawning →
`OS-PROCESS`; signalling → `OS-SIGNAL`), while operations on an
already-acquired handle (accept, read, write, close) are not re-gated —
acquisition is the gate. This document specifies the *form* of the
primitive — consult a named capability set before acting — and the
reference's names are the recommended default; adopting them verbatim is
not itself the conformance requirement, *enforcing something* at the call
site is. A host whose capability names are inert labels queried by no
primitive does not conform, regardless of what `lib/22-guard.lisp` layers
on top.

**A gated operation attempted without permission signals an ordinary,
`HANDLER-CASE`-catchable condition of the same two-field form as any other
native error (Part VIII)** — not a panic, a process abort, or a silent
no-op. The reference's gates distinguish in the message, as a convenience
and not a requirement, between "never granted" and "granted but attenuated
by an enclosing fence".

**The grant has two layers, and a conformant host must reproduce both:**
- A **standing grant**, made by host embedding code (`enable_feature`) or
  the CLI's `--capability` flag against an environment, persisting for the
  environment's lifetime. **There is no Lisp-callable way to add to it**:
  the only Lisp-facing primitives are queries — `(feature-enabled-p name)`
  (symbol or string; `T` when the name is granted *and* not masked by the
  current fence) and `(capability-mask-allows-p name)`. The grant lives in
  state shared by an environment and every lexical child of it, including
  `(make-environment parent)`, `LET` frames, and closures — none of which
  can narrow or widen it privately. `(make-environment)` with no
  arguments has an **empty** grant. A forked world (the reference's
  `fork_world`) receives an independent **copy** of the grant at fork
  time; later changes to either world do not affect the other.
- A **dynamic-extent attenuation mask**, per thread, installed only by
  `WITH-CAPABILITIES`, which can only narrow. An operation proceeds only
  when the standing grant permits it *and* the mask (if any) permits it.

**`(with-capabilities list-form body...)` is a special form.**
`list-form` is **evaluated** and must yield a proper list of symbols
(strings, non-symbols, or a dotted list are errors) — so the idiom is
`(with-capabilities '(READ-FS) ...)`; a bare `(with-capabilities (READ-FS)
...)` evaluates `(READ-FS)` as a call. The new mask is the requested names
when no mask is active, otherwise the **intersection** with the enclosing
mask: `(with-capabilities '(READ-FS) (with-capabilities '(SHELL) ...))`
leaves nothing allowed, no matter what the inner fence asks for. Body
forms are evaluated in order (non-tail), stopping at the first error;
the value is the last form's or `NIL` for an empty body. On exit by any
path — completion, error, non-local exit — the previous mask is restored
exactly. There is nothing to debit: a mask has no "amount spent". The mask
follows the *call*, not the lexical fence: helpers called from inside the
fence, and code `EVAL`ed inside it, are masked; a closure created inside
the fence but called outside runs with the caller's authority. There is
deliberately no primitive that installs a wider mask.

## Part X — Step-budget fencing (fuel)

A sandboxed host needs a second axis of defense beyond capabilities: a
ceiling on *how much computation* untrusted code can do even when it
touches no I/O — an infinite loop in pure arithmetic is still a denial of
service. Lamedh calls this budget **fuel**, and it is a kernel mechanism,
not a library convenience, because a library-level counter is defeated by
code that never calls it.

**One per-thread step counter, decremented once per evaluation step,
checked before the step runs** (`charge_kernel_fuel`,
`src/evaluator/core.rs`, called at the top of every trampoline
iteration). "One step" is one iteration of the evaluator's dispatch loop:
every `eval` entry (each non-tail sub-evaluation — a test, an argument, a
`LET` init) and every tail step (an `IF` branch taken, a `PROGN` last
form, a tail call) each cost exactly one unit, in both the tree-walking
path and any compiled path a host has. A tail-recursive loop that never
grows the stack is therefore still metered. The counter has two states:
*unarmed* (no limit; the default) and *armed with `n` remaining*. When
armed and `n > 0`, a step decrements it; when armed and `n == 0`, the
step **does not run**: the counter is set back to *unarmed* and a native
error ("fuel exhausted") is raised. A host that meters only its slow path
and lets compiled code run unmetered has not implemented this section;
the reference refuses to JIT-compile while a budget is armed for exactly
this reason.

**Exhaustion is an ordinary `HANDLER-CASE`-catchable condition**, unlike an
unmatched `THROW` (Part VIII), specifically so that cleanup code —
`UNWIND-PROTECT` cleanup forms, a `HANDLER-CASE` handler, the fence's own
budget restore — can run instead of being re-killed on its own first
step. That is why exhaustion **disarms** the counter as it signals: with
the counter stuck at zero, every cleanup step would re-signal forever.

**The consequence, tracked as issue #457:** because the counter stays
disarmed until the fence that armed it exits, **guest code that catches
the exhaustion condition with a `HANDLER-CASE` positioned inside the very
fence that exhausted runs unmetered from that point until the fence
exits** — and a handler that loops and never returns runs indefinitely.
`(with-fuel 100 (handler-case (loop-forever) (error (e) (kernel-fuel-
remaining))))` returns `NIL` (unarmed) rather than a small number. The
window is bounded to that one fence: an enclosing fence re-arms on the
inner fence's exit, debiting the inner fence's whole budget as spent. What
conformance requires is narrower than "reproduce this disarm mechanism":
a host **must** let ordinary cleanup code run after exhaustion — some
disarming or grace mechanism is required, not merely permitted — but a
host is free to choose a narrower mechanism (for instance a small fixed
cleanup allowance) that closes the catch-and-reloop window; doing so is an
improvement over the yardstick, not a divergence from it, as long as
ordinary cleanup still runs. Once #457 is fixed in the reference this
paragraph will be tightened to forbid the unconditional bypass.

**`(with-fuel budget-form body...)` is a special form, and nested fences
attenuate**, following the exact algorithm of `SpecialForm::WithFuel`
(`src/evaluator/special_forms.rs`):
1. Evaluate `budget-form` (this evaluation is charged to the *enclosing*
   budget, if any). The value must be a non-negative fixnum; a negative
   number, a float, or a non-number is an error, and a missing budget is
   an error.
2. Let `prev` be the enclosing remaining budget (or *unarmed*). Arm the
   counter with `armed = min(budget, prev)` if `prev` is a number, else
   `budget`. **A nested fence can never grant itself more than its
   enclosing fence has left**, whatever number it asks for: `(with-fuel
   100 (with-fuel 100000 (kernel-fuel-remaining)))` reports about `96`.
   Increment the fence-depth counter.
3. Evaluate the body forms in order (non-tail), stopping at the first
   error or non-local exit. An empty body yields `NIL`. `(with-fuel 0
   form)` exhausts on the first step.
4. On exit **by any path**, decrement the fence depth; let `now` be the
   counter's remaining value, or `0` if exhaustion disarmed it; compute
   `spent = armed - now` (saturating); and restore the counter to `prev -
   spent` (saturating) if `prev` was a number, or to *unarmed* if it was
   unarmed. Fuel spent inside a nested fence is therefore never free from
   the outer fence's point of view, and this debit happens on every exit
   path, the same guarantee dynamic-variable restore has in Part VI.
5. Deliver the body's value or propagate its error/exit.

The fuel counter and the capability mask are **per-thread state, not
per-environment state**: they follow the running computation, not the
environment a closure was defined in.

**Fuel is queryable and settable from Lisp, and the setter is gated by
fence position, not by Part IX's named capabilities.**
`(kernel-fuel-remaining)` returns the remaining count, or `NIL` when
unarmed. `(kernel-fuel-set! n-or-nil)` arms the counter with `n` (a
non-negative fixnum) or disarms it with `NIL`, and returns the previous
state (a count or `NIL`). **Outside any fence** it is unrestricted — this
is how a host driver arms the first budget. **Inside a fence with an
armed counter**, a request to set a value greater than the current
remaining count, or to disarm, is an **error** (not a silent no-op, not a
clamped success); lowering is permitted. This asymmetry makes "widen your
own sandbox" impossible while leaving the host's driver free to set one
up.

**Fuel is orthogonal to capabilities and to Part VI's recursion bound; a
host must implement all three.** A program can exhaust the recursion bound
with abundant fuel (deep non-tail recursion that is short in steps), and
exhaust fuel while never approaching the recursion bound (a shallow
unbounded loop). Capabilities gate *what* untrusted code can touch; the
recursion bound gates *how deep* it can nest; fuel gates *how much total
work* it can do. A host with the first two but not fuel cannot safely run
untrusted Lamedh code at all.

## Part XI — The kernel primitive inventory

A host must provide, as either a true native primitive or something that
produces identical observable behavior when derived from a smaller native
set (Part XII says which forms have that latitude):

- **Representation**: `CONS`/`CAR`/`CDR` with the identity and equality
  rules of Part IV, including `(car nil)` = `(cdr nil)` = `NIL`; interned
  symbols with the reader's interning and case rules, `INTERN` (which
  uppercases), `GENSYM` (fresh uninterned symbols, never `EQ` to anything
  else), and symbol property lists (`GETP`/`PUTP`); the numeric types and
  operations of Part V; strings and characters per Parts II–IV; hash
  tables, arrays, typed arrays, and environments as primitive mutable
  structures with the exact contracts Part IV states
  (`lib/15-sets-hash.lisp`, `lib/17-arrays.lisp` use them natively); a
  global-binding primitive (`SET`, `DEF`, `DEFINE`) — `lib/00-core.lisp`
  and the module system are unwritable without property lists and global
  definition. Cons cells are immutable — no destructive mutation primitive
  exists or may exist; see Part XII, axis 2.
- **Control**: the special forms and evaluation-order guarantees of Parts
  VI–VII, including the exact tail-call position list, the three
  non-local-exit mechanisms, `UNWIND-PROTECT`, one dynamic-binding
  mechanism, and one error-signalling mechanism producing the two-field
  condition value of Part VIII together with `HANDLER-CASE` and
  `ERRORSET`.
- **Reflection**: `VAU` (or an equivalent operative hook) per Part VI,
  sufficient to define `DEFMACRO` in terms of it; `EVAL` taking a form and
  an optional environment; `THE-ENVIRONMENT` and `MAKE-ENVIRONMENT`;
  `READ-FROM-STRING` and the printer as `PRIN1-TO-STRING`/
  `PRINC-TO-STRING`. This is required for a portable form of the HM type
  checker (#451), the rulebook optimizer (`lib/11-optimizer-vau.lisp`,
  `lib/24-rules.lisp`), and typed protocols (`lib/29-protocols.lisp`) to
  run unmodified on every host. Static typing is not an optional flourish
  this document can leave for later: #451's argument is that the HM
  checker has no host dependency once this hook exists, and a shared type
  checker is exactly how a Lamedh program large enough that a human can no
  longer hold its whole call graph in mind stays maintainable across every
  host this specification exists to keep in agreement. A host that
  implements every other primitive in this document but cannot run the
  portable checker has not delivered a platform serious programs can be
  written against.
- **Capability-gated I/O**: Part IX.
- **Step-budget fencing (fuel)**: Part X — a native step counter charged
  on every evaluation step, the `WITH-FUEL` fence with attenuation-only
  nesting, `KERNEL-FUEL-REMAINING`, and a `KERNEL-FUEL-SET!` that is
  unrestricted outside any fence and strictly narrow-only inside one.
- **Reader/printer**: Parts II–III, with the extension/minimal split Part
  II states.
- **The exact set of builtin names `lib/*.lisp` invokes**, spelled and
  cased exactly as the corpus calls them. This is *not* a free-to-vary
  convenience: a stdlib file that calls `(CONS ...)` will not load on a
  host that only provides the same operation under a different name.
  Enumerating that exact table per operator category is tracked as open
  work in Part XIII; until it lands, "every name `lib/*.lisp` actually
  calls, spelled exactly as it calls them" is the operational definition
  of this bullet, and a host cannot claim conformance against a subset of
  it chosen for its own convenience.

## Part XII — Declared axes of host variation

This is the complete list of places a conformant host may produce
observably different results from the Rust reference, and exactly what
latitude each axis grants. Nothing outside this list is a free variable —
if a rule in Parts II–XI doesn't appear here, it is not optional.

1. **Numeric precision beyond 64-bit signed integer range.** A host must
   declare one of two models and be internally consistent about it:
   **fixed-width wraparound** (matching the Rust reference bit-for-bit:
   overflowing arithmetic wraps modulo 2⁶⁴ into the signed range and the
   host makes the `OVERFLOW` signal observable, as Part V describes) or
   **arbitrary precision** (the host returns the exact mathematical
   result and never wraps; the overflow-signal concept does not apply and
   `(flag-set-p 'OVERFLOW)` may be permanently `NIL`). Within 64-bit
   signed range, both models must agree with each other and with the Rust
   reference exactly — this axis only has teeth once a computation's true
   result leaves that range. Integer *literals* are outside this axis:
   Part II's overflow-to-float rule applies on every host. **Which model a
   host picked must itself be introspectable, not only documented**: a
   host binds a global constant, named `+NUMERIC-PRECISION-MODEL+`, to the
   symbol `WRAPAROUND-64` or `ARBITRARY-PRECISION` accordingly, using the
   global-environment mutation primitive Part XI already requires — no
   new kernel primitive is needed to satisfy this. This introspectability
   requirement is not itself an axis: every host must expose its choice
   this way, whichever of the two models it picked; only the underlying
   choice of model varies. Implemented on the Rust reference (issue #463):
   `Environment::new_with_builtins` (`src/environment.rs`, and so every
   environment built on it — `with_stdlib`, `with_prelude`, sandboxed
   environments alike) binds `+NUMERIC-PRECISION-MODEL+` to `WRAPAROUND-64`,
   matching `src/evaluator/builtins_core.rs`'s `+`/`-`/`*`//`
   (`checked_*` arithmetic over `i64` falling back to `wrapping_*` and
   setting `OVERFLOW`, never promoting to arbitrary precision). It is also
   the first entry of `+HOST-TRAITS+`, a single collective registry — an
   alist of `(AXIS-NAME . CHOSEN-VALUE)` pairs, `((NUMERIC-PRECISION-MODEL
   . WRAPAROUND-64))` on the reference today — that later axes needing the
   same introspectable-choice treatment are expected to add entries to,
   rather than each minting its own top-level constant. Reading either
   global requires the reader to parse `+earmuff+`-style tokens
   (`parse_plus_earmuff_symbol` in `src/reader.rs`, added alongside this
   change; previously only `*earmuff*` was supported and a leading `+` was
   consumed as the bare `+` operator symbol). Other hosts (SBCL: #455,
   `lamedh-asm`: #456) still need to bind their own choice the same way.
   Issue #459 (reader-level `#+`/`#-` feature-conditional dispatch) had not
   landed when this was implemented; `+HOST-TRAITS+` stands alone for now,
   but is a plausible registry to share with #459's feature list if and
   when that lands, per the discussion on issue #463.
2. **Destructive cons mutation is not an axis: cons cells must be
   immutable on every host.** `RPLACA` and `RPLACD` return a **new** cons
   cell sharing the untouched half of the original
   (`src/evaluator/builtins_extra.rs`, `BuiltinFunc::Rplaca`/`Rplacd`);
   the original is never modified. This is required, not incidental: the
   reference shares cons children structurally between parser output,
   closure bodies, quasiquote templates, and macro inputs, so in-place
   mutation would silently alter every value sharing the sub-structure;
   and it is what makes circular lists impossible, which Part III's
   printer (no cycle detection) and Part IV's equality rules (defined only
   for finite structure) rely on. A host built on a language whose native
   `RPLACA` mutates (CL, for the SBCL port) must not expose that
   behavior: `RPLACA`/`RPLACD` must be the same non-destructive,
   new-cell operation on every host, or be omitted, and no primitive may
   mutate an existing cons cell's car or cdr in place.
3. **Whether `BLOCK`/`RETURN-FROM`, `PROG`/`GO`/`RETURN`, `AND`, `OR`,
   `WHILE`, `FOR`, `UNWIND-PROTECT`, `HANDLER-CASE`, and
   `DEFMACRO`/`DEFEXPR` are true native primitives or are derived** from
   `CATCH`/`THROW`, `COND`/`LET`/tail calls, dynamic variables, and
   `VAU`+`EVAL` respectively. The reference makes all of these native, for
   performance; Parts VI–VIII specify their observable behavior precisely
   enough that a from-scratch host may instead build any of them as
   library code on top of the smaller primitive set in Part XI, and the
   result conforms as long as the observable behavior — including which
   positions are and are not tail positions, and the invisibility of
   non-local exits to `HANDLER-CASE` — matches.
4. **`MOD`'s overflow edge case** (Part V): a host may either reproduce
   the reference's silent `0` on `(mod i64::MIN -1)` without setting
   `OVERFLOW`, or set the same flag `/` and `REMAINDER` set for that
   input. Both conform; the latter is a correctness improvement, since no
   corpus code depends on the former.
5. **Native surface beyond this document.** A host may implement more
   than Part XI requires natively, for performance or because its host
   language already supplies it (the Rust reference's JIT and
   performance-sensitive paths are themselves full of this) — as long as
   the extra native surface is not required by `lib/*.lisp` and does not
   change the observable behavior of anything that is.
6. **The empty line comment** (Part II): the reference rejects a `;`
   immediately followed by a line break or end of input as a parse error.
   A host may instead treat it as an empty comment. Because no text that
   loads on the reference contains one, this leniency cannot change the
   meaning of any conforming program; it only accepts text the reference
   rejects.

## Part XIII — Status and open work

This document is a specification, not an audit report: it states required
and permitted behavior, and deliberately does not carry a running account
of which host currently falls short of which rule. Per-host conformance
gaps are tracked as ordinary issues against the host in question —
currently #455 (the SBCL port) and #456 (`lamedh-asm`) — and closed there
as the host's own work, not maintained as prose here that would drift the
moment either issue's status changes. A rule in Parts II–XII that a host
doesn't yet meet is that host's issue tracker's business; this document
only needs to be right about what the rule *is*.

What remains open in the specification itself, tracked explicitly rather
than smoothed over:

- **Enumerate the exact builtin-name table** `lib/*.lisp` calls, per
  category (arithmetic, string, hash-table, array, and beyond), closing
  Part XI's last bullet. This is the largest remaining mechanical task and
  the one most directly checkable by a script rather than by writing more
  prose.
- **Audit every network and process primitive against its gate.** Part
  IX records the reference's rule — resource acquisition is gated, use of
  an acquired handle is not — from the gate functions and the acquiring
  primitives that call them; a per-primitive check that no acquiring
  operation in `src/evaluator/builtins_net.rs` and `builtins_os.rs` skips
  its gate is the remaining yardstick-consistency item.
- **Get the portable HM type checker (#451) actually running**,
  unmodified, on every host that has the Part XI `EVAL` hook —
  prioritized above the other items in this list, per Part XI's own
  statement of why.
- **Scope a `tests/kernel-conformance/` corpus**, per #452's own risk
  list, so this document does not decay the moment one host's convenience
  wins out over the line drawn here, and so conformance against this
  specification is something a script can check rather than something
  only an audit essay can argue for. The concrete examples given inline in
  Parts II–X (reader consequences, arithmetic results, fence arithmetic)
  are the natural seed of that corpus.
- **Prove the kernel primitive set is actually sufficient for serious
  library code, not merely for what already happens to be native**
  (issue #458): implement a well-typed, high-speed hash table from
  scratch in Lamedh — over `typed-array` buckets and a hash function
  written in Lamedh, type-checked via the portable HM checker (#451) or
  `lib/29-protocols.lisp` — and benchmark it against the native
  `HashTable` builtin Part IV specifies. A language whose fast, typed
  collections are all native, with only a slow or untyped escape hatch
  available for anything else, has not delivered on Part XI's reflection
  requirements no matter how precisely their observable behavior is
  pinned down here.
- **Fix the kernel-fuel catch-and-reloop bypass** (issue #457): guest
  code that catches the fuel-exhausted condition inside its own fence and
  never returns from the handler gets an unconditional fuel bypass for
  the rest of that fence, not bounded cleanup grace — a defect in the
  reference, not a tolerable quirk. Once fixed, Part X's framing of this
  as permitted-but-not-required host behavior is to be tightened to
  disallow unconditional bypass outright.
- **Add reader-level feature-conditional dispatch** (issue #459). The
  reader has no `#+`/`#-` (Common-Lisp `*features*`-style) syntax, or any
  other mechanism, by which a single shared `lib/*.lisp` file can branch
  on which host or host capability it is running under; the only `#`
  dispatches are those listed in Part II. Nothing implementable exists yet,
  so Part II does not describe one; when a design lands in the reference,
  Part II gains its grammar and Part XII its declared axis (which host
  feature names exist). Part XII axis 1's `+HOST-TRAITS+` registry (issue
  #463, implemented) is a plausible registry to share this data with,
  since both are "what does this host claim to support," queried at read
  time by this issue versus at run time by #463 — not designed together
  since #463 landed first and #459 had not landed, but not precluded
  either.
- **Cache macro expansion per call site** (issue #460): today every macro
  call re-runs the macro body from scratch on every invocation, including
  every iteration of a compiled loop, because macro dispatch shares one
  code path with `VAU`/fexpr dispatch, which genuinely must run fresh
  every call. Since a macro's expansion is otherwise a function of its
  definition and the literal call-site operand forms, caching it once
  per call site is sound and matches how every other Lisp treats
  `DEFMACRO`; `VAU` and fexprs are unaffected and stay uncached by
  design. Once implemented, Part VI gains the cache's existence and its
  one-time-global-observation semantics as a normative rule, and Part X
  gains a note that fuel step counts may differ between a cache hit and
  a cache miss.
- **Fix two binder-identity gaps `VAU` and `APPLY` leave open** (issues
  #461, #462): `VAU` construction and application do not guard dynamic-
  variable parameters the way macros and fexprs do (#461), and `VAU`
  construction plus `APPLY`-on-fexpr/`VAU` do not use the canonical
  binder id that #285/#287 established for gensym'd or foreign-table
  parameter names (#462) — a regression of that fix's coverage, not a
  new class of bug. Both are reference-implementation defects to fix,
  not host latitude Part XII grants.
