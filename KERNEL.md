# KERNEL.md — the Lamedh language specification

This document is the specification referenced by issue #452: it defines the
observable semantics of Lamedh precisely enough that two independently
written hosts can agree on what a program means, and it defines the minimal
primitive surface a host must expose to run the shared `lib/*.lisp` stdlib
corpus unmodified. As of this writing three hosts exist or are emerging —

- the Rust reference implementation (`src/`, `cli/`)
- a standalone Common Lisp / SBCL port (`sbcl/`, PR #449)
- a from-scratch x86-64 native compiler (`lamedh-asm/`, PR #450)

## Part I — Scope, status, and method

**The Rust reference implementation is the semantics yardstick.** Where this
document states a rule of observable behavior, that rule is the Rust
reference's actual, verified behavior — not a plausible-sounding generic
Lisp convention, and not this document's own invention. Every normative rule
below was extracted by reading the Rust source directly (`src/reader.rs`,
`src/printer.rs`, `src/lib.rs`, `src/environment.rs`, `src/evaluator/*.rs`)
and, where a claim mattered enough to get wrong, verified against the exact
code a second time. This matters because Lamedh has a few genuinely
surprising rules — surprising enough that a competent implementer guessing
from "it's a Lisp" would get them wrong — and those are exactly the rules a
spec exists to pin down. §IV's EQ rule for cons cells is the sharpest
example; see there.

**A small, closed set of declared axes is where hosts are allowed to
differ**, listed in full in Part XII. Outside that list, a host's observable
behavior must match this document — which is to say, must match the Rust
reference — exactly. The list exists so that SBCL's native bignums and
`lamedh-asm`'s tag-cancellation wraparound arithmetic don't get flagged as
spec violations for a choice that is genuinely a host's to make; it is
short and explicit specifically so that "conformant divergence" and "actual
bug" stay distinguishable, which a document that just says "hosts may vary
reasonable things" cannot do.

**This is a Lisp 1.5 dialect with modern extensions, not a reimplementation
of Common Lisp or Scheme.** Where a rule below looks unusual next to CL or
Scheme habit (fixed-arity-plus-rest lambda lists with no `&optional`/`&key`;
a condition system with no type hierarchy; `EQ` false on every cons pair),
that is not an oversight this spec is working around — it is what the
language is, checked directly against the one implementation that has run
the entire `lib/*.lisp` corpus in production use.

## Part II — Lexical grammar (the reader)

**Whitespace and comments.** `ws ::= (space | line-comment | block-comment)*`.
A line comment runs from `;` to end of line. A block comment runs from `#|`
to a matching `|#` and **nests**: an inner `#|` increments a depth counter
that an inner `|#` decrements, so `#| a #| b |# c |#` is one comment, not a
premature close. An unterminated block comment or string is a hard parse
failure, not a silent truncation. A leading `#!` line (shebang) is stripped
before parsing begins, not part of the general comment grammar.

**Symbols.** Two constituent grammars, tried in this order against each
atom-shaped token:
- A *general* symbol: first character `alpha | & | $ | ?`; every character
  after that drawn from `alphanumeric | - * ? ! + = < > : _` (any number,
  including zero more). `-` is a legal non-initial character (so `FOO-BAR`
  is one symbol), and `:` is legal non-initially (module-qualified
  `MODULE:SYMBOL` reads as one symbol) but a *leading* `:` instead triggers
  the keyword-symbol grammar below.
- An *operator* symbol: one or more characters from `+ - * / = < > ! ~`,
  covering tokens like `+`, `>=`, `/=` that don't start with a letter.
  Operator symbols are **not** case-folded (they have no case), but every
  other symbol class is.
- `:foo` (a *keyword symbol*: `:` then a general-symbol-shaped tail) and
  `*foo*` (an *earmuff symbol*: `*` then a general-symbol-shaped tail then
  `*`) are each their own reader production, but **both produce an ordinary
  `Symbol`** — there is no distinct keyword or special-variable runtime
  type. The reader-level distinction exists only to accept `:` and a second
  `*` inside what would otherwise be illegal symbol characters; nothing
  downstream treats a keyword symbol differently from any other symbol
  except by convention.
- Digits alone are never ambiguous with a symbol: the numeric-literal
  productions are tried first, and every symbol production requires a
  non-digit leading character (a letter, `&`, `$`, `?`, `:`, `*`, or an
  operator character), so a bare digit string can only ever be a number.
- **Case folding is unconditional**: every symbol (general, keyword,
  earmuff) is uppercased at intern time, regardless of surrounding context.
  There is no case-sensitivity mode and no escaped-symbol syntax (no
  `|foo bar|`) to opt out of it.
- `T` (after uppercasing) reads as the ordinary interned symbol `T`. `NIL`
  (after uppercasing) does **not** read as a symbol at all — it reads as
  the same `Nil` value that `()` reads as. `NIL` is not "a symbol that
  happens to be treated as false"; it is literally the empty list, exactly
  as `()` is, with no symbol object backing it. See Part IV.

**Integer literals.** Four forms, none combinable with another in the same
token:
- Decimal: optional leading `-`, then one or more digits. Leading zeros are
  legal. There is no leading `+`; a token starting with `+` reads as the
  operator symbol `+` unless it's immediately followed by a decimal literal
  with no space, in which case it's still just the symbol (a bare `+` sign
  on a numeric literal is not part of this grammar at all).
- Octal, `Q` suffix (case-sensitive, uppercase only): `-? digit+ "Q"`.
- Hexadecimal, `H`/`h` suffix: `-? digit hex-digit* [Hh]`, and it must
  *start* with a decimal digit `0`-`9` — `FFh` is not a valid hex literal
  under this grammar (it would need a leading `0`: `0FFh`).
- Radix-prefixed, CL style: `# [xXbBoO] -? digit-in-that-radix+`. There is
  no `#d` decimal prefix (decimal already has its own unprefixed form).
- An integer literal that overflows a 64-bit signed integer degrades
  silently to a float if it still parses as one — it does not error and
  does not become a bignum at the reader level on any host, since no host
  is required to have reader-level bignums (see Part XII on numeric
  precision).

**Float literals.** `-? digit+ "." digit+ (("e"|"E") ("+"|"-")? digit+)?`
— both the integer part and the fractional part require at least one
digit. **`.5` and `5.` are both illegal** under this grammar; there is no
production that accepts either. A conformant reader must reject them the
same way (as a parse failure or as some other token, per Part XII's grammar
axis — see there), not silently accept CL-style bare-decimal-point floats.

**String literals**, delimited by `"`. Recognized escapes: `\n \t \r \\ \"
\0`. Any other backslash-prefixed character is **not an error and is not
stripped** — it is passed through to the string's contents literally as
the two characters `\` and that character. An unterminated string is a
hard parse failure.

**Character literals**, `'x'`. Recognized escapes: `\n \t \r \\ \' \0`. Any
other backslash-prefixed character decodes to that character alone, with
the backslash silently dropped — this is the opposite convention from
strings, and both conventions are normative, not a discrepancy to fix. A
character is a code point in `0..=255`; there is no character type wider
than a byte. Empty `''` is not a valid character literal.

**Record literals**, `#S(TypeName field...)` (also spelled `#s`). The head
of the inner list must be a symbol naming the record's brand; the rest must
be a proper (non-dotted) list of field values in declaration order. This is
purely structural at read time — no type registry is consulted, so reading
a `#S(...)` form never fails for referring to an unknown type.

**Quote-family reader macros**, each desugaring to a two-element list:
`'`*e* → `(QUOTE e)`; `` ` ``*e* → `(QUASIQUOTE e)`; `,`*e* → `(UNQUOTE
e)`; `,@`*e* → `(UNQUOTE-SPLICING e)`; `#'`*e* → `(FUNCTION e)`.

**What is grammar-minimal versus library-extensible.** Decimal integers,
symbols (both classes), strings, proper and dotted lists, `NIL`/`T`, and
the quote family are the load-bearing minimum — `lib/*.lisp` cannot be
written without them. Octal/hex-suffix and radix-prefix integers, floats,
block comments, character literals, keyword/earmuff symbol sugar, and
`#S(...)` records are each a layered extension: a host may implement them
however it likes, or defer them, as long as it does not change what they
mean once implemented (Part XII does not grant latitude on *meaning*, only
on *whether a from-scratch host has gotten around to parsing them yet*).

## Part III — Printed representation

A conformant printer must satisfy `(read (print x)) ≡ x` (under `EQUAL`,
Part IV) for every value built from primitive data, with these exact rules:

- `Nil` prints as `()`. There is no code path that ever prints the string
  `NIL` for this value.
- `T` is an ordinary symbol and prints as its stored (already-uppercase)
  name — nothing special-cased.
- A symbol prints as its stored name, verbatim, ignoring any property list.
- A `Number` prints as a plain decimal integer — never with a radix marker,
  regardless of how it was read.
- A `Float` prints via the host's default float-to-string conversion, with
  one required post-processing rule: if that conversion did not already
  produce a string containing `.`, `e`, `E`, `inf`, or `NaN`, a literal
  `.0` is appended. This is why a whole-number float like `3.0` prints as
  `"3.0"`, not `"3"` — the rule exists specifically to keep the printed
  form re-readable as a float rather than an integer. **`inf` and `NaN`
  printed forms are not currently re-readable** by Part II's float grammar
  (which requires digits on both sides of a literal `.`); this is a known
  round-trip gap in the reference implementation, not a rule to conform to
  — a host is free to close it, but is not required to reproduce it. The
  `e`/`E` checks are presently dead code on the reference implementation:
  Rust's default `f64`-to-string conversion never produces scientific
  notation, so a very large or very small float prints as a long run of
  plain decimal digits (still syntactically readable by Part II's grammar,
  just unwieldy) rather than as `1e300`-style notation. A host is free to
  use scientific notation for extreme magnitudes instead, as long as Part
  II's float grammar (extended to accept it) can read it back.
- A `Char` prints as `'x'`, escaping `\n \t \r \\ \' \0` exactly as the
  reader's character-literal grammar expects, and printing every other
  byte raw.
- A `String` prints with `"`, escaping `\" \\ \n \t \r \0` — a rule that
  happens to be symmetric with the reader's own string-escape set. Any
  other character, including other control characters, prints raw and
  unescaped.
- A proper list prints as `(a b c)`; an improper (dotted) list prints as
  `(a . b)` for its final non-nil, non-cons cdr. **Printing a circular
  list is not currently guarded against** — the reference printer recurses
  structurally on `cdr` with no cycle detection and will exhaust the stack
  on a genuine cycle. This is a known gap, not a spec requirement; a host
  may detect and error on cycles instead, and doing so is *not* a
  conformance violation even though it's observably different output for
  a program that builds a circular list and prints it (such a program is
  already outside anything Part IV's equality/identity rules make
  well-defined for it to rely on).
- `#S(TypeName field...)` prints fields in declaration order, matching the
  reader's field-collection order exactly, so record values round-trip
  whenever every field value is itself self-representing.

## Part IV — Data model: types, equality, and truth

**The primitive types** are: `Nil` (the empty list, doubling as boolean
false), symbols, fixnums, floats, characters, strings, and cons cells —
plus a longer tail of compound and host-facing types (hash tables, arrays,
environments, closures, records, first-class conditions, and opaque
handles for ports/network/processes) whose equality rules differ enough
from each other that they get their own treatment below rather than being
folded into "compound types compare the obvious way." Part V covers
numeric detail; this section covers identity, equality, and truth for all
of them.

**`NIL` is not a symbol.** `()` and the token `NIL` are the same value, a
distinct `Nil` type with no symbol behind it — not "the symbol NIL, which
happens to be self-evaluating and falsy," which is how some Lisps describe
it. There is no distinguished `True` type; `T` is an ordinary interned
symbol used by convention as *a* truthy value, not *the* truthy value.

**Truth is exactly "not `Nil`".** `IF` and every other conditional
dispatches on one predicate: a value is false if and only if it is `Nil`;
every other value, including `0`, the empty string, and any symbol
(including a symbol literally read from the text `NIL`'s quoted form, if
such a thing could even arise — it can't, since `NIL` never parses as a
symbol at all) is true.

**`EQ` is defined per-type, and one of its rules is currently unsettled —
flagged here as an open defect rather than frozen as normative, precisely
because it is exactly the kind of surprising rule a spec exists to catch
before three hosts each guess differently:**

> **The reference implementation currently makes `EQ` unconditionally
> `NIL` whenever either argument is a cons cell — including comparing a
> cons cell against itself.** `(let ((x (cons 1 2))) (eq x x))` returns
> `NIL`. The code (`src/evaluator/builtins_core.rs`, `BuiltinFunc::Eq`)
> justifies this with a comment citing the Lisp 1.5 manual's position that
> `EQ` is defined only on atoms — but the 1.5 manual describing `EQ` as
> guaranteed only on atoms is not the same claim as "must be `false` on
> every non-atom pair," and real Lisp 1.5 implementations, and every
> Lisp since, have used `EQ` on lists as pointer-identity comparison in
> practice: `(eq x x)` for the same actual cons cell is true everywhere
> else this operator exists. Hard-coding it to `NIL` here is a stronger
> and more surprising restriction than the manual actually requires, and
> was surprising even to this project's own maintainer on first
> encountering it. **This document does not require a conformant host to
> reproduce this behavior.** It is filed as a defect against the
> reference implementation (issue #454) rather than settled here as
> intended semantics; until that issue resolves, treat `EQ` on cons cells
> as **undefined behavior a portable program must not rely on either way**
> — neither on it being `NIL`, nor on it being pointer identity — and
> watch the linked issue for the outcome that will eventually replace this
> paragraph with a real rule.

For every other type, `EQ` is defined as follows, and — because none of
these types carry any notion of identity separate from their value in the
reference implementation (fixnums, floats, and characters are plain scalar
payloads with no boxing; strings are plain owned values, not
reference-counted) — `EQ` and `EQUAL` **necessarily coincide** for all of
them:

- **Fixnum, float, character, string**: value equality. Two freshly
  computed, entirely unshared values of any of these types that happen to
  hold the same value **are** `EQ`. `(eq 100000 100000)` on two
  independently computed fixnums is `T`. `(eq "ab" (string-append "a"
  "b"))` is `T`. A host built on a language where these types *are*
  natively boxed/interned (or not) must still make `EQ` behave this way
  observably — by using value comparison for them regardless of what its
  own host language's native `eq` would do.
- **Float, specifically**: `NaN` is `EQ` to `NaN`. This is **not** IEEE 754
  equality (which says `NaN ≠ NaN`); it is a value-equality rule with an
  explicit `NaN`-is-`NaN` carve-out, and a host must reproduce the
  carve-out, not delegate to its host language's native float comparison
  unmodified.
- **Symbol**: identity. Because symbols are interned (Part II), two reads
  of the same name always denote the same symbol object, so value equality
  and identity equality coincide for symbols too in practice — but the
  *mechanism* a host uses to implement `EQ` on symbols should be identity
  comparison of the interned object, not name-string comparison, so that a
  symbol's mutable property list (which `EQ` never inspects, but other
  operations do) stays attached to one object.
- **`Nil`**: always `EQ` to `Nil`.

**`EQUAL` is deep structural equality**, defined recursively: two atoms are
`EQUAL` exactly when they are `EQ` (note this means, given the rule above,
that **two structurally identical cons cells are never `EQUAL` by falling
through to `EQ` at the top level — `EQUAL` recurses *through* conses and
only calls `EQ` at their leaves**, comparing `car` against `car` and `cdr`
against `cdr` recursively rather than ever asking whether the outer cons
values are `EQ` to each other). Two conses are `EQUAL` exactly when their
cars are (recursively) `EQUAL` and their cdrs are (recursively) `EQUAL`.
**`EQUAL` does not perform numeric contagion**: `5` and `5.0` are never
`EQUAL` to each other — a fixnum and a float are different types, `EQ`
between them is false by the type-mismatch case, and `EQUAL` inherits that
at the leaf. A host must not "helpfully" make `(equal 5 5.0)` true.

**Compound types not listed above — hash tables, arrays, environments,
closures, records, first-class error/condition values, and any host-native
handle (a port, a network or process handle) — are still atoms under
`EQ`'s cons-exclusion rule (they are simply not cons cells), and each has
its own equality rule rather than one uniform "compound values are never
equal" fallback. A host must reproduce which rule applies to which type,
not assume they all behave like cons cells or all behave like fixnums:**

- **Hash tables, arrays, environments, and any opaque host handle (ports,
  network handles, process handles) compare by identity** — two of these
  are `EQ`/`EQUAL` exactly when they are the same underlying object, never
  merely when they hold equal contents. A host must not make two
  separately constructed, content-identical hash tables or arrays
  `EQUAL`; that is observably different from the reference.
- **Records (`#S(...)` values, `DEFRECORD` instances) compare
  structurally** — by type name and field values, recursively — not by
  identity. Two independently constructed records of the same type with
  equal fields **are** `EQUAL` (and `EQ`, under the cons-exclusion rule
  above, since a record is not a cons cell). This is the opposite rule
  from hash tables and arrays, deliberately: a host must not "fix" this
  into identity comparison on the theory that records are compound
  mutable-ish structures like hash tables — they are specified to behave
  as value types for equality purposes.
- **A first-class condition/error value compares structurally** on its
  message and data fields, the same value-type treatment as records.
- **Closures (lambdas) compare structurally on their parameter list, rest
  parameter, and body, but by identity on their captured environment** —
  two closures are equal only if they'd behave identically *and* close
  over the literal same environment object, not merely an
  environment with equal-looking bindings. Fexprs, macros, and `VAU`
  values follow the same shape (structural on their defining parts,
  identity on the closure environment).

## Part V — The numeric tower

**Contagion.** Any float operand, in any position, promotes an entire
arithmetic or numeric-comparison call to float — this is a call-wide
predicate ("does any argument have float type"), not a first-argument-wins
or pairwise-promotion rule. A character operand is automatically coerced
to its integer code point in every arithmetic operator (`+ - * /`) and in
numeric comparison (`< > =`); this coercion is unconditional, not something
a program opts into.

**Division and remainder — two operators, two different sign
conventions, both required exactly as specified:**

- `/` on two fixnums **truncates toward zero** (C/Rust-style integer
  division) and never silently returns a float. Division by zero is an
  error (Part VIII), not an infinity — this differs from the float-operand
  case of `/`, where dividing by `0.0` yields IEEE infinity or `NaN` rather
  than erroring, because the float path never checks for zero at all.
- `REMAINDER` uses **truncated remainder**: the result's sign follows the
  *dividend*, matching what `/`'s truncation implies (`(remainder -7 2)`
  is `-1`).
- `MOD` uses **floored (Euclidean) remainder**: the result's sign follows
  the *divisor*, and is always non-negative when the divisor is positive
  (`(mod -7 2)` is `1`, not `-1`). This is a genuinely different operator
  from `REMAINDER`, not two names for the same thing, and a host must
  implement both conventions distinctly rather than aliasing one to the
  other.
- The one integer-overflow-representable division case (dividend equal to
  the minimum representable fixnum, divisor `-1`) is a numeric-precision
  concern, not a sign-convention concern — see the overflow axis in Part
  XII. Note as an observed reference-implementation quirk, **not** a rule to
  reproduce: `MOD`'s handling of this same edge case silently substitutes
  `0` without raising the overflow signal that `/` and `REMAINDER` raise
  for the equivalent case. A host is free to make `MOD` raise the same
  overflow signal `/`/`REMAINDER` do here instead of silently returning
  `0`; that would be a correctness improvement over the yardstick, not a
  divergence from it, since nothing in `lib/*.lisp` exercises this exact
  edge case today.

**Comparison across types.** `<`, `>`, and `=` accept any mix of fixnum,
float, and character operands (applying the contagion and character-coercion
rules above) freely. Comparing a string against a number, or a number
against any non-numeric type, is an error (Part VIII), never a silent
coercion and never a permissive `NIL`-returning "just not equal, I guess."

**Overflow** is where hosts are allowed to diverge, and how: see Part XII.
Within whatever range a host's numeric model can represent exactly,
arithmetic must match this section's rules exactly and must match the
Rust reference's results bit-for-bit for every value that also fits in a
64-bit signed integer, since that is the range the shared corpus is
written and tested against.

## Part VI — Evaluation model

**Evaluation order is strictly left to right**, uniformly: a function
call evaluates its operator, then its operands in left-to-right order,
each exactly once. `IF` evaluates its test, then evaluates *only* the
selected branch — the untaken branch is not touched at all, not even to
the extent of checking its shape.

**`LET` binds in parallel; `LET*` binds sequentially.** In `(let ((a
init-a) (b init-b)) ...)`, every `init-*` form is evaluated in the
*outer* environment, before any new binding is visible — `init-b` cannot
see `a`. In `(let* ((a init-a) (b init-b)) ...)`, each `init-*` form is
evaluated after the previous binding has already been installed, so
`init-b` *does* see `a`. This is the standard Lisp distinction, stated
here because it is exactly the kind of rule a spec must pin down rather
than leave to "the usual convention."

**Tail calls: a specific, closed list of positions get proper, unbounded
tail-call elimination — nowhere else is guaranteed, and one common way of
calling a function does *not* qualify.** The guaranteed tail positions are:
the last form of a `LAMBDA` body; both branches of `IF`; the last form of
each `COND` clause; the last form of a `PROGN`; the body of `LET`/`LET*`;
and the body of a `VAU`/fexpr/macro expansion. A call in any of these
positions to another function, however deeply the chain of such calls
goes, must not grow the host's native call stack — this is a hard
guarantee a portable program may rely on for writing loops as tail
recursion. **Function-call *arguments* are never tail positions** (evaluate
them, then call — the call itself is not in tail position relative to its
own argument evaluation). **Calling a function via `FUNCALL`, `APPLY`, or
a host embedding API is explicitly *not* guaranteed to get this
treatment** — a portable program must not write a loop that depends on
`(funcall self ...)` in tail position looping forever without growing the
stack; a host may optimize that case, but is not required to, and the
reference implementation itself does not.

Non-tail evaluation depth (ordinary recursion that is not eliminated by
the rule above) is bounded, and exceeding the bound is a catchable error
condition (Part VIII), not an unrecoverable native stack fault, though a
host is free to implement the bound generously (the reference
implementation runs the whole evaluator on an oversized stack precisely
so this bound, not the underlying native stack, is what a program
actually hits first).

**`LAMBDA` parameter lists are fixed-arity, plus at most one rest
parameter, spelled either as `&REST name` or as a dotted tail `(a b .
name)` — never both in the same list.** There is no `&optional` and no
`&key`; this is not an oversight, it is the whole grammar. Calling a
lambda with the wrong number of arguments is an error (Part VIII), as is
calling a value that is not callable at all in operator position; neither
is a native crash.

**`SETQ` resolves its target with a specific precedence, and creates a
new binding rather than erroring when the target is unbound:**
1. If the symbol has ever been declared dynamic (see the dynamic-variables
   paragraph below), `SETQ` writes the symbol's single global dynamic
   cell unconditionally. Read together with that paragraph, this is not
   "`SETQ` overrides an existing lexical shadow": once a symbol is
   declared dynamic, `LET`, lambda parameters, and every other binding
   form stop creating an ordinary lexical frame slot for that name at all
   — they install a new *dynamic* binding instead (the same shallow-binding
   mechanism the paragraph below describes), exactly as declaring a
   variable `special` does in Common Lisp. So there is no separate lexical
   binding for `SETQ`'s rule to skip past; there is only ever the one
   dynamic cell, and every binding form and every `SETQ` for that name
   agree on addressing it. `(let ((x 1)) (setq x 2) x)` evaluates to `2`
   whether or not `x` is dynamic — if `x` is dynamic, the `LET` installed
   a fresh dynamic binding holding `1` before `SETQ` changed it to `2`; if
   not, ordinary lexical rules apply and give the same answer by the usual
   route.

   **A sharper, genuinely surprising consequence, confirmed directly
   against the resolution code rather than assumed: declaring a symbol
   dynamic is retroactive and global, with no way back.** Variable lookup
   (`Environment::resolve`) checks the symbol's `is_dynamic` flag fresh on
   *every* reference, not once at the binding site that created a frame
   for it. So the moment any code, anywhere in a running program, declares
   `x` dynamic, every existing lexical frame slot named `x` — however long
   it has been live, in however many already-executing closures, created
   long before the declaration ran — becomes permanently unreachable for
   both reads and `SETQ`: every subsequent reference to `x`, from any of
   those closures, redirects to the one global dynamic cell instead, as
   if the declaration had been in effect from the start. Because symbols
   are interned globally, this is not scoped to a file or a module: one
   `(defdynamic x)` anywhere makes *every* `x` in the entire running
   program dynamic, retroactively, with no corresponding "undeclare"
   operation to undo it. This matches how `(proclaim '(special x))`
   behaves in Common Lisp (also global, also effectively one-way in
   practice) — a CL programmer will find this familiar; a programmer
   coming from Scheme or from Lisp 1.5 itself, where variables are
   lexical by default and nothing is named globally-dynamic after the
   fact, will not. A host must reproduce this exact retroactive, global,
   one-way behavior — it is not latitude Part XII grants, and getting it
   wrong (e.g. by scoping a dynamic declaration to a module, or by having
   already-live closures keep their lexical slots) is a conformance
   failure, not a reasonable interpretation.
2. Otherwise, `SETQ` walks the lexical environment chain outward from the
   call site and updates the first frame where the symbol is already
   bound.
3. If no frame has it bound anywhere in the chain, `SETQ` does **not**
   error — it creates a new binding local to the calling environment's
   own frame (or, if the calling environment is the global environment,
   sets the symbol's global value cell). A program that `SETQ`s a name it
   never `LET`- or parameter-bound gets a fresh local variable, silently,
   by design.

**`VAU`, fexprs, and macros are three distinct reflection mechanisms with
different binding shapes and different expansion timing, and the
difference in timing matters for what a portable program can assume:**
- **`VAU`** takes exactly two parameters: one bound to the call's entire
  unevaluated operand list, and one bound to a first-class value denoting
  the calling environment (not specially privileged — an ordinary
  environment value a `VAU` body can pass around and `EVAL` in). Its body
  runs once per call, in a fresh child of the `VAU`'s own closure
  environment, with no separate expansion step — there is nothing to
  cache, because there is no expansion phase distinct from execution.
- **A fexpr** (`DEFEXPR`) takes a fixed-arity parameter list (no rest
  parameter) bound one-to-one to the call's unevaluated operand forms (or,
  for a single-parameter fexpr, the whole operand list at once); its body
  likewise runs once per call in a child of its closure environment, with
  no separate expansion phase.
- **A macro** (`DEFMACRO`) is genuinely different: at every call, its body
  runs first, against the unevaluated operand forms, to produce an
  *expanded form*; that expanded form is then evaluated in the *caller's*
  environment. **This expansion is not cached anywhere** — a macro is
  re-expanded from scratch on every single call, not compiled once and
  reused. A portable program must not assume a macro's expansion happens
  only once per call site; each call re-runs the macro's own body as
  ordinary Lisp code.

**`CATCH`/`THROW` and `BLOCK`/`RETURN-FROM` are dynamic-extent, non-lexical
escapes, and an unmatched one is not a catchable Lisp-level condition —
it terminates evaluation.** `CATCH`'s tag is evaluated (so tags are
dynamic values, not lexical names) and compared against a thrown tag with
`EQ`... more precisely, with the value-equality rules of Part IV applied
to whatever type the tag happens to be; `BLOCK`/`RETURN-FROM` instead key
on a name taken directly from an unevaluated symbol at each site. Both
walk outward dynamically through however many enclosing frames of the
same construct exist at the moment of the throw/return, re-propagating
past any that don't match. **A `THROW` or `RETURN-FROM` with no matching
`CATCH`/`BLOCK` anywhere on the current dynamic call chain is not
something `HANDLER-CASE` can intercept** (see Part VIII) — it propagates
all the way to the top level and halts whatever unit of execution is
running. A portable program must ensure every `CATCH`/`BLOCK` it uses is
actually reachable from every place that might `THROW`/`RETURN-FROM` to
it, because there is no safety net.

**Dynamic variables use shallow binding**: a dynamically-declared symbol
has exactly one live value cell, shared globally; entering a dynamic
binding form saves that cell's current contents and installs a new value,
and *leaving* that form — by any means, including a `THROW` or
`RETURN-FROM` that unwinds straight through it — restores the saved value
before control passes further out. This restore-on-every-exit-path
guarantee, including non-local exits and including exits that unwind
through several accumulated dynamic bindings made across a chain of tail
calls, is load-bearing: `lib/*.lisp`'s condition-handling layer
(`lib/16-conditions.lisp`) is written assuming dynamic bindings always
unwind correctly no matter how control leaves their scope.

## Part VII — Special forms reference

The forms named and given exact semantics in Part VI — `QUOTE`, `IF`,
`LAMBDA`, `LET`/`LET*`, `PROGN`, `COND`, `SETQ`, `VAU`, `DEFEXPR`,
`DEFMACRO`, `CATCH`/`THROW`, `BLOCK`/`RETURN-FROM`, `HANDLER-CASE` (Part
VIII), and `DEFDYNAMIC` (the dynamic-variable declaration primitive
itself — `DEFVAR` is the same primitive under an alias, not a separate
library-level form built on top of it) — are the special forms a host
must give exactly this behavior to, whether it implements each one as a
true kernel primitive or derives it from a smaller set (Part XII covers
which specific forms are eligible for that latitude, and which are not).
Every other named construct `lib/*.lisp` uses — `AND`, `OR`, `WHEN`,
`UNLESS`, `DO`, the CL-compat layer, `DEFUN` — is already, in the
reference implementation itself, ordinary library code built from this
list plus the primitives of Parts II–VI; a host that gets this list right
and loads `lib/*.lisp` unmodified gets those forms for free and does not
need its own account of their semantics here.

## Part VIII — The condition system

**There is no native taxonomy of condition types, and a portable program
cannot dispatch on one.** A condition value carries exactly two pieces of
information: a message string and a data payload (an arbitrary value,
`Nil` if unused) — nothing else. There is no type tag, no class hierarchy,
and no `DEFINE-CONDITION`-style extension mechanism anywhere in the
language, at either the native or the library level.

**`HANDLER-CASE` catches every condition unconditionally — it does not
pattern-match on a condition type, because none exists to match on.** Its
one clause form binds the condition value (message plus data) to a
variable and runs its body; there is no second clause form and no type
specifier to write, because "catch this kind of error but not that kind"
is not an operation the language provides. A program that needs to
distinguish error causes does so by inspecting the message string or the
data payload itself, by convention, not by type dispatch. **This is a
scoping fact about the language, not a gap this document is leaving open
for a future revision to close** — `lib/16-conditions.lisp`'s
`RESTART-CASE`/`HANDLER-BIND`/restart-invocation layer is built entirely
on this one untyped `HANDLER-CASE` plus `CATCH`/`THROW` plus dynamic
variables, and does not itself add typing either.

**A `CATCH`/`THROW` non-local exit and a `BLOCK`/`RETURN-FROM` non-local
exit are not conditions at all and are never visible to `HANDLER-CASE`** —
they are a structurally separate control-flow mechanism (Part VI) that
passes straight through any enclosing `HANDLER-CASE` untouched. A portable
program cannot use `HANDLER-CASE` to intercept a stray `THROW`; it must
use a matching `CATCH`.

**The exact message text a given native error produces is not part of the
portable surface**, even though the reference implementation's condition
value happens to carry one: message strings vary in capitalization and
level of detail from one native operation to the next, and at least one
(the "not a function" error for calling a non-callable value) embeds a
Rust-internal debug rendering of the offending value rather than a clean
Lamedh-printed one. A conformant host must signal *a* condition — of the
same two-field (message, data) shape — for the same class of native
failure (unbound variable, unbound function, division by zero, wrong
number of arguments, index out of range, calling a non-callable value,
non-numeric argument to a numeric operator, exceeding the non-tail
recursion bound), but is not required to reproduce the reference
implementation's exact wording, and a program that pattern-matches on
exact error text is relying on something this specification does not
guarantee.

## Part IX — Capability-gated I/O

Read, write, and syscall-adjacent operations (filesystem, shell,
process/environment, network) must sit behind a capability system a host
enforces at the primitive call site — not merely as bookkeeping.

This document specifies the *shape* of the primitive — an operation that
consults a named, per-environment capability set before acting — not the
enforcement mechanism, the exact capability names, or which operations are
gated by which name. The Rust reference's current names (`READ-FS`,
`CREATE-FS`, `TEMP-FS`, `SHELL`, `IO`, plus `NET-*`/`OS-*`) are a reasonable
default for a conformant host to adopt verbatim, but adopting them is not
itself the conformance requirement; *enforcing something* at the call site
is. A host that defines the capability names as inert labels queried by no
primitive (as filed against the SBCL port in issue #455) does not conform
to this section, regardless of what `lib/22-guard.lisp` layers on top.

## Part X — Step-budget fencing (fuel)

A sandboxed host needs a second axis of defense beyond capabilities: a way
to run untrusted code with a hard ceiling on *how much computation it can
do* even when it touches no I/O at all — an infinite loop in pure
arithmetic is still a denial of service. Lamedh calls this budget **fuel**,
and it must be a genuine kernel mechanism, not a library convenience,
because a library-level step counter is trivially defeated by code that
never calls the counting function.

**The kernel maintains one step counter, decremented once per evaluation
step, checked before that step runs.** "One evaluation step" means one
iteration of the evaluator's own dispatch loop — the same unit of work
that Part VI's tail-call trampoline advances on every call, so a
tail-recursive loop that never grows the stack is still metered correctly:
each tail step still charges fuel even though it charges no additional
stack frame. Charging happens unconditionally, on every step, in both a
tree-walking evaluator and any compiled/JIT path a host has — a host that
only meters the slow path and lets compiled code run unmetered has not
implemented this section.

**Exhaustion signals a normal, `HANDLER-CASE`-catchable condition, and
that is a deliberate, load-bearing design choice with a known,
documented consequence.** Unlike a `THROW`/`RETURN-FROM` past an
unmatched target (Part VIII, which is *not* catchable), running out of
fuel produces the same two-field (message, data) condition shape as any
other native error, specifically so that surrounding `UNWIND-PROTECT`- and
`HANDLER-CASE`-based cleanup code gets a chance to run instead of being
killed off mid-cleanup by its own metering. The mechanism a host uses to
make that possible — the reference implementation disarms its own counter
at the instant it signals exhaustion, so that cleanup code evaluated while
handling the condition doesn't immediately re-trigger the same signal
before it can finish — has a real, acknowledged gap: **guest code that
directly wraps a `HANDLER-CASE` around the fuel-exhausted condition and
loops from inside the handler can keep running past its nominal budget**,
because the counter that would normally stop it has just been disarmed to
let the handler run at all. This is not a defect this document is asking
hosts to fix; it is documented reference-implementation behavior (called
out explicitly in the reference's own `--mcp` sandboxing code as a known
limitation), and a conformant host must reproduce the same shape of gap
rather than quietly closing it in a way that changes observable behavior
— though a host is free to close it as a genuine improvement over the
yardstick, the same latitude Part XII already grants for `MOD`'s overflow
quirk, as long as it does not change behavior for any program that isn't
specifically trying to evade its budget.

**A step-budget fence is a special form, `(WITH-FUEL n body...)`, and
nested fences attenuate rather than compose additively — the same rule
capabilities follow.** Entering a fence with a requested budget `n`
installs `min(n, remaining-budget-of-the-nearest-enclosing-fence)`, never
more than what the enclosing fence has left: **a nested fence can never
grant itself a larger effective budget than its enclosing fence has
remaining, no matter what number it asks for.** This is the mechanism
that makes "guest code cannot simply remove its own limit" true in the
one specific sense this document requires: code running inside a fence
that wraps itself in `(WITH-FUEL 999999999999999 ...)` gets silently
clamped to whatever the enclosing fence actually has left, not the
inflated number it asked for. (This is a *different* guarantee from the
catch-and-reloop gap two paragraphs up — clamping stops a guest from
widening its own budget; it does nothing about a guest that catches
exhaustion and loops within the budget it already had re-armed for
cleanup. Both facts are part of this section; neither substitutes for the
other.) On leaving a fence — by ordinary completion, by a caught error, or
by any non-local exit passing through it — the amount of fuel actually
spent inside the fence must be debited from the enclosing fence's own
remaining budget, so that spending inside a nested fence is not free
fuel from the outer fence's point of view; this restoration must happen
on every exit path, the same non-negotiable guarantee dynamic-variable
unwinding gets in Part VI.

**Fuel is queryable and settable from Lisp code, and the setter is the one
place this mechanism is itself capability-gated — but by fence position,
not by the ordinary named-capability system of Part IX.** A read-only
query returns the current remaining budget (or an unarmed/no-limit
indication outside any fence). A setter can arm, widen, or disarm the
budget entirely *when called from outside any fence* — this is the
mechanism a host's own embedding layer uses to arm a budget before running
untrusted code in the first place, and it is necessarily unrestricted
there, since something has to be able to set the first budget. **From
inside a fence, the same setter must refuse to set a value larger than
the fence's current remaining budget** — attempting to widen or disarm
the budget from within a fence is an error, not a silent no-op and not a
silently clamped success. This asymmetry (unrestricted outside a fence,
strictly attenuating-only inside one) is what makes "widen your own
sandbox" impossible while still leaving a host's own driver code free to
set up the sandbox in the first place.

**Fuel is orthogonal to capabilities and to the non-tail recursion depth
bound of Part VI — a host must implement all three, and none substitutes
for another.** A program can exhaust its recursion-depth bound while
holding abundant fuel (deep non-tail recursion that terminates quickly in
step count but not in stack depth), and a program can exhaust its fuel
while never approaching the recursion bound (a fast, shallow, unbounded
loop). Capabilities gate *what* untrusted code can touch; the recursion
bound gates *how deep* it can nest; fuel gates *how much total work* it
can do. A host that implements capabilities and the recursion bound but
not fuel has not built a platform that can safely run untrusted Lamedh
code at all, since an infinite pure-computation loop needs none of the
I/O capabilities gates and needn't recurse non-tail at all to burn
unbounded wall-clock time.

## Part XI — The kernel primitive inventory

A host must provide, as either a true native primitive or something that
produces identical observable behavior when derived from a smaller native
set (Part XII says which forms have that latitude):

- **Representation**: cons/car/cdr with the identity/equality rules of
  Part IV; interned symbols; the numeric types and operations of Part V;
  strings and characters per Parts II–IV; hash tables and arrays as
  primitive mutable structures (`lib/16-*`/`lib/17-*` use them natively,
  not as derived structures); a global-environment mutation primitive
  (`SET`/`DEFINE`-shaped) and symbol property lists (`lib/00-core.lisp`
  and the module system are unwritable without both). Cons cells are
  immutable — no destructive mutation primitive exists or may exist; see
  Part XII.
- **Control**: the special forms and evaluation-order guarantees of Parts
  VI–VII, including the exact tail-call position list, one non-local-exit
  mechanism, one dynamic-binding mechanism, and one error-signalling
  mechanism producing the two-field condition shape of Part VIII.
- **Reflection**: a `VAU`-or-equivalent expansion hook per Part VI,
  sufficient to define `DEFMACRO` in terms of it, and an `EVAL` (or
  `compile-and-run`) hook callable from Lisp code. This is required for a
  portable form of the HM type checker (#451), the rulebook optimizer
  (`lib/11-optimizer-vau.lisp`, `lib/24-rules.lisp`), and typed protocols
  (`lib/29-protocols.lisp`) to run unmodified on every host. Static typing
  is not an optional flourish this document can leave for later: #451's
  argument is that the HM checker has no host dependency once this hook
  exists, and a shared type checker is exactly how a Lamedh program large
  enough that a human can no longer hold its whole call graph in mind
  stays maintainable across every host this specification exists to keep
  in agreement. A host that implements every other primitive in this
  document but cannot run the portable checker has not delivered a
  platform serious programs can be written against.
- **Capability-gated I/O**: Part IX.
- **Step-budget fencing (fuel)**: Part X — a native step counter charged
  on every evaluation step, a `WITH-FUEL`-shaped fence with
  attenuation-only nesting, and a setter that only widens the budget from
  outside a fence. Orthogonal to capabilities and to the recursion-depth
  bound; a host needs all three to safely run untrusted code.
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
   overflowing arithmetic wraps modulo 2⁶⁴ and the host makes an
   overflow signal observable somehow, whether as a flag or otherwise) or
   **arbitrary precision** (the host returns the exact mathematical
   result and never wraps; the overflow-signal concept simply does not
   apply and may be permanently false/absent). Within 64-bit signed
   integer range, both models must agree with each other and with the
   Rust reference exactly — this axis only has teeth once a computation's
   true result leaves that range.
2. ~~Destructive cons mutation.~~ **This is not an axis — cons cells must
   be immutable, full stop, and this is a MUST, not a place hosts may
   differ.** An earlier revision of this document listed destructive
   `RPLACA`/`RPLACD` as a free choice, on the reasoning that `lib/*.lisp`
   never observably depends on in-place mutation. That reasoning was
   incomplete: the Rust reference's `RPLACA`/`RPLACD` return a *new* cons
   cell rather than mutating in place for a specific, load-bearing reason
   stated directly in its source comment (`src/evaluator/builtins_extra.rs`,
   `BuiltinFunc::Rplaca`/`Rplacd`) — it is "an intentional safety feature"
   that makes circular list construction *impossible*, and this document's
   own Part III already relies on that: the printer has no cycle
   detection and would exhaust the stack on a genuine cycle, and Part IV's
   equality rules are only meaningful for finite structure. A host that
   offers a genuinely destructive `RPLACA`/`RPLACD` — even one that seems
   like a "natural mapping" onto its own host language's native mutation
   (as CL's `RPLACA` would be for the SBCL port) — would let a Lamedh
   program construct a cycle that the rest of this specification does not
   define behavior for anywhere else. **`RPLACA`/`RPLACD` must be the same
   non-destructive, always-returns-a-new-cell operation on every host, or
   must be omitted entirely; a host must never expose a way to mutate an
   existing cons cell's car or cdr in place.**
3. **Whether `BLOCK`/`RETURN-FROM`, `HANDLER-CASE`, and `DEFMACRO`/`DEFEXPR`
   are true native primitives or are derived from `CATCH`/`THROW`,
   dynamic variables, and `VAU`+`EVAL` respectively.** The reference
   implementation happens to make all of these native, for performance;
   Part VI–VIII specify their observable behavior precisely enough that a
   from-scratch host may instead build every one of them as library code
   on top of the smaller primitive set in Part XI, and the result conforms
   as long as the observable behavior matches.
4. **Reader/printer extension timing** (Part II's closing paragraph): a
   host may defer implementing radix-prefixed/suffixed integer literals,
   floats, block comments, character literals, or `#S(...)` records
   without losing conformance on the primitives it has implemented, as
   long as what it *has* implemented means exactly what this document
   says. A host is not conformant merely because it has an excuse for
   what it's missing; it is on a documented path to conformance, and
   should say so (as `sbcl/README.md` and `lamedh-asm/README.md` already
   do).
5. **`MOD`'s overflow edge case** (Part V): a host may either reproduce
   the reference implementation's silent-zero behavior on the one
   representable-overflow input, or raise the same overflow signal
   `/`/`REMAINDER` raise for that input instead. Both conform; the latter
   is arguably a bug fix, not a divergence, since no corpus code depends
   on the former.
6. **Native surface beyond this document.** A host may implement more
   than Part XI requires natively, for performance or because its host
   language already supplies it (the Rust reference's JIT and
   performance-sensitive paths are themselves full of this) — as long as
   the extra native surface is not required by `lib/*.lisp` and does not
   change the observable behavior of anything that is.

## Part XIII — Status and suggested next step

This document is a specification, not an audit report: it states required
and permitted behavior, and deliberately does not carry a running account
of which host currently falls short of which rule. Per-host conformance
gaps found while writing or reviewing this document are tracked as
ordinary issues against the host in question — currently #455 (the SBCL
port) and #456 (`lamedh-asm`) — and closed there as the host's own work,
not maintained as prose here that would drift the moment either issue's
status changes. A rule in Parts II–XII that a host doesn't yet meet is
that host's issue tracker's business; this document only needs to be
right about what the rule *is*.

What remains open in the specification itself, tracked explicitly rather
than smoothed over:

- **Enumerate the exact builtin-name table** `lib/*.lisp` calls, per
  category (arithmetic, string, hash-table, array, and beyond), closing
  Part XI's last bullet. This is the largest remaining mechanical task and
  the one most directly checkable by a script rather than by writing more
  prose.
- **Confirm the Rust reference's `NET-*`/`OS-*` capability names are
  actually enforced** at the call site, matching the depth already
  established here for `SHELL`/`READ-FS`/etc. — this is about the
  yardstick's own internal consistency, not a host lagging behind it.
- **Resolve issue #454** (`EQ` on cons cells) and update Part IV from
  "undefined, open defect" to a real rule once it lands.
- **Get the portable HM type checker (#451) actually running**,
  unmodified, on every host that has the Part XI `EVAL` hook —
  prioritized above the other items in this list, per Part XI's own
  statement of why.
- **Scope a `tests/kernel-conformance/` corpus**, per #452's own risk
  list, so this document does not decay the moment one host's convenience
  wins out over the line drawn here, and so conformance against this
  specification is something a script can check rather than something
  only an audit essay can argue for.
