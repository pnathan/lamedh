# KERNEL.md — the Lamedh kernel surface

This document is the conformance specification referenced by issue #452. It
is host-agnostic by design: it belongs to the Lamedh language, not to any one
implementation. As of this writing three hosts exist or are emerging —

- the Rust reference implementation (`src/`, `cli/`)
- a standalone Common Lisp / SBCL port (`sbcl/`, PR #449)
- a from-scratch x86-64 native compiler (`lamedh-asm/`, PR #450)

— and each has independently drawn its own line between "native kernel" and
"the `lib/*.lisp` stdlib loaded on top." This document draws that line once,
for all of them.

**Conformance criterion.** A host is Lamedh-conformant if and only if it
implements *at least* the surface enumerated below, with nothing on the
list reimplemented with different observable semantics, and the shared
`lib/*.lisp` corpus loads and runs unmodified on top of it, producing
identical observable behavior for any program that does not reach for a
host-specific escape hatch. A host may implement more than this surface
natively (for performance, or because its host language already supplies
it) without losing conformance, as long as the extra native surface is not
required by `lib/*.lisp` and does not change the observable behavior of
anything that is. This document *is* the spec; it does not summarize a
decision made elsewhere.

Until `tests/kernel-conformance/` (see Suggested next step) exists,
"the shared `lib/*.lisp` corpus" means every file currently listed in
`STDLIB` in `src/lib.rs`, run in that order — not an unspecified or
host-chosen subset. A host that cannot yet load all of it is not
conformant; it is on a documented path to conformance, and should say so
plainly (as `sbcl/README.md` and `lamedh-asm/README.md` already do for
their own gaps) rather than claim partial coverage as conformance.

A primitive belongs on this list only if `lib/*.lisp` cannot be written
without it, or without something structurally equivalent. Everything a host
implements beyond this surface for its own convenience or performance is
free to vary, and is not a conformance requirement on any other host.

## 1. Representation and memory

Required:

- **Cons cells** with `CONS`/`CAR`/`CDR`, defined identity semantics under
  `EQ`, and a distinguished empty list value (`NIL`/`()`).
- **Symbols**, interned so that two reads of the same name are `EQ`.
- **Fixnum arithmetic** (`+ - * /` at minimum) over a signed integer of
  host-defined width. A host must document its own overflow behavior
  (wraparound, a trap, promotion to bignum, or a settable flag) — this
  document does not mandate one, because the three current hosts already
  disagree (see Divergences, below) and none of `lib/*.lisp` depends on a
  specific choice.
- **Floats, strings, and chars**, plus the primitive operations over them
  that `lib/*.lisp` calls natively rather than defining itself: arithmetic
  comparison and the four operators (`+ - * /`) from above extended to
  floats; string length, character access, and concatenation; character
  code conversion. This document does not freeze the exact builtin names —
  those are free to vary in spelling across hosts — but a host is missing
  part of §1, not merely offering a convenience, if any of these
  operations require dropping into `lib/*.lisp`-unreachable host code to
  express. Enumerating the precise builtin table per category is tracked
  as open work in Suggested next step; until it lands, treat "whatever
  `lib/*.lisp` calls without defining" as the operational definition of
  this bullet.
- **Equality and truthiness**: `EQ` (identity, per the symbol/cons
  semantics above) and `EQUAL` (structural equality over the types in this
  section) as two distinct, primitive predicates — `lib/*.lisp` depends on
  the distinction, not just on one generic "equal". `NIL` is the only false
  value; everything else, including `()`'s own alias, is true.
- **A global-environment mutation primitive** — `SET`/`DEFINE`-shaped —
  and **symbol property lists**, since `lib/00-core.lisp` and the module
  system are unwritable without both.
- **Hash tables and arrays** as primitive, mutable data structures (not
  merely derivable from cons cells), since `lib/16-*` / `lib/17-*` use them
  natively rather than defining them from lower primitives.

A destructive mutation primitive on cons cells is explicitly *not*
required. The Rust reference's `RPLACA`/`RPLACD` are non-destructive (each
returns a new cell — see Divergences); `lib/*.lisp` must not depend on
in-place cons mutation, so a host may offer a destructive `RPLACA` (as a
straightforward mapping onto its host language, the way the SBCL port's
native CL could) without losing conformance, and a host that offers none
at all still conforms. Allocation and GC policy are a host's own business;
that consing *works*, with the identity/equality semantics above, is what
conformance requires.

## 2. Control

Required, as exactly one primitive each:

- **One non-local-exit primitive** — a `CATCH`/`THROW`-shaped escape.
  `BLOCK`/`RETURN-FROM`, conditions, and `HANDLER-CASE` must be expressible
  as library code in terms of it (a host may still implement them natively
  for performance, as the Rust reference does — see Divergences — but a
  conformant host is not *required* to, and a from-scratch host may build
  them entirely in `lib/*.lisp` from `CATCH`/`THROW` alone).
- **One dynamic-binding primitive** — shallow or deep, host's choice — from
  which `DEFDYNAMIC`/`DEFVAR` sugar is library code. A host whose native
  language already has correct dynamic/special variables (e.g. the SBCL
  port reusing `PROGV`/`SYMBOL-VALUE`) satisfies this by construction; it
  need not reimplement one.
- **One error-signalling primitive** distinct from ordinary `THROW` — a way
  for a native operation (a type error, division by zero, an unbound
  variable) to raise a first-class condition value that library code can
  intercept. `HANDLER-CASE`/conditions being library-expressible (above)
  presupposes this: something must originate the condition value a
  `CATCH` eventually catches. A host may fold this into its non-local-exit
  primitive (signal is throw-with-a-payload) rather than adding a second
  mechanism.
- **Closures**, with lexical capture and application. A host may restrict
  *how* capture is implemented internally (copy-by-value at closure
  creation, one level of free-variable scan, etc. — see `lamedh-asm`'s v0
  limits in Divergences) as long as the *observable* semantics `lib/*.lisp`
  relies on — a closure sees the bindings lexically in scope at its
  creation point — hold.

## 3. Reflection

Required:

- **A macro/fexpr/`vau`-shaped expansion hook** sufficient to define
  `DEFMACRO` in terms of it. This is the single highest-leverage primitive
  on this list: `COND`/`AND`/`OR`/`LET`/`LET*`, the CL-compat layer, and
  iteration constructs all become library code once it exists, as they
  already are in `lib/08-vau.lisp` and `lib/21-cl-compat.lisp`. A host is
  free to implement `DEFMACRO`/`DEFEXPR` natively for performance (as the
  Rust reference currently does), but conformance only requires the lower
  primitive plus the derived forms working — not that the derivation
  itself be visible in Lisp.
- **An `EVAL` (or `compile-and-run`) hook** callable from Lisp code, so
  library code can hand the host freshly-consed forms at runtime. This is
  required for a portable form of the HM type checker (#451), the rulebook
  optimizer (`lib/11-optimizer-vau.lisp`, `lib/24-rules.lisp`), and typed
  protocols (`lib/29-protocols.lisp`) to run unmodified on every host.

Static typing is not an optional flourish this document can leave for
later: #451's argument is that the HM checker has no host dependency once
this hook exists, and a shared type checker is exactly how a major Lamedh
program — one large enough that a human can no longer hold its whole
call graph in mind — stays maintainable across the three (or four, or
more) hosts this document exists to keep in agreement. A host that
implements every primitive above but cannot run the portable checker has
not delivered a platform serious programs can be written against; §3's
`EVAL` hook is required precisely so no host gets to treat static typing
as someone else's problem.

## 4. Capability-gated I/O

Required: read, write, and syscall-adjacent operations (filesystem, shell,
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
primitive (see the SBCL port's gap in Divergences) does not conform to this
section, regardless of what `lib/22-guard.lisp` layers on top.

## 5. Reader/printer baseline

A conformant host's reader must natively parse, at minimum: integers,
symbols, lists (including dotted pairs), strings, and `'`/`` ` ``/`,`/`,@`
quote-family sugar. Everything else — radix literals (`#x`/`#o`/`#b`/`H`/`Q`
suffixes), block comments (`#|...|#`), and structured literal syntax such as
`#S(...)` records — is library-extended surface a host may parse natively
for convenience, but is not required to; a from-scratch reader with only the
minimum above, plus a library-level extension mechanism, still conforms.

A conformant host's printer must round-trip, at minimum, every value type
required by §1: symbols, numbers (fixnum, float), chars, strings (with
escapes), `NIL`, and proper/dotted cons lists — sufficient that
`(read (print x))` recovers a value `EQUAL` to `x` for any value built from
those primitives. A host may print additional opaque or structured types
however it chooses; there is no round-trip requirement on values (closures,
hash tables, ports, environments) that are not themselves primitive data.

## Divergences observed in the three current hosts

This section is not part of the conformance requirement; it is the audit
trail the Suggested next step in #452 asked for, kept attached to the spec
so future hosts and future revisions of this document can see where the
existing three already disagree or fall short.

**Rust reference** (`src/`) — implements more than this document requires,
by design (the JIT and performance-sensitive paths need it internally; §1's
closing paragraph is explicit that this bounds what library code may
assume portable, not what a host's own implementation may use to get
there):
- Fixnums (`i64`) wrap on overflow and set an `OVERFLOW` flag rather than
  trapping or promoting to bignum.
- `RPLACA`/`RPLACD` exist natively but are non-destructive (return a new
  cell) — true in-place mutation is not part of the native surface at all.
- `CATCH`/`THROW`, `BLOCK`/`RETURN-FROM`, and `HANDLER-CASE` are all native
  special forms rather than `HANDLER-CASE`/`BLOCK` being derived in Lisp
  from `CATCH`/`THROW` — a candidate for pushing down into `lib/`, per
  §2's allowance that this is optional.
- `DEFEXPR`/`DEFMACRO` are native rather than derived from `VAU`+`EVAL` in
  Lisp — likewise optional under §3, and a candidate for trimming.
- The reader natively parses radix literals and `#S(...)` record syntax
  that §5 classifies as library-extendable, not kernel-required.

**SBCL port** (`sbcl/`, PR #449) — a genuine conformance gap, not a
performance choice:
- Capabilities are tracked (`*capability-mask*`, `WITH-CAPABILITIES`) but
  **not enforced** — every native I/O primitive in `sbcl/src/io.lisp`
  (file, shell, process, TCP/UDP) remains callable regardless of the mask.
  This fails §4 as written; closing it is a prerequisite for calling this
  port conformant, not a nice-to-have.
- Integers are host-native CL bignums, not wraparound i64 — an accepted
  divergence under §1's "host must document its own overflow behavior,"
  documented candidly in `sbcl/README.md`.
- Dynamic binding is satisfied by construction (reuses CL `PROGV`), and
  `CATCH`/`THROW`/`BLOCK`/`HANDLER-CASE` are each mapped directly onto CL's
  own equivalents rather than derived from one primitive — permitted under
  §2, but worth noting this port has never actually exercised "build
  `BLOCK` from `CATCH`/`THROW` alone" the way a from-scratch host would
  need to.
- No portable HM type checker runs on this port yet (#451's subject); the
  honest unverified-axiom surface (`DECLARE-TYPE!`/`SEE-TYPE`) stands in
  for it rather than faking verification, which is the right interim
  choice over silently reporting programs as checked when they are not.
  It is not a kernel-primitive nonconformance — the `EVAL` hook §3
  requires is present — but per §3's note above, it is a gap this document
  treats as a priority to close, not an indefinitely deferrable nice-to-
  have: a host without a working portable checker is not yet a platform
  a major Lamedh program should be written against.

**lamedh-asm** (PR #450) — pre-conformant by its own README; a v0 prototype
that has not yet reached most of this surface, not a host that disagrees
with it:
- Confirmed, exactly as #452 states: `cons`/`car`/`cdr` are implemented and
  used internally by the reader/compiler, but compiled Lamedh `LAMBDA`
  bodies cannot call them — `(CONS 1 2)` in user code compiles as a call to
  an unbound global, not a primitive operation. §1 is not yet met for user
  code.
- No non-local-exit primitive, no dynamic-binding primitive: §2 is not met.
- No macro/fexpr/`vau` hook and no runtime `EVAL`: §3 is not met. The
  single-pass compiler's special-form dispatch is a fixed table
  (`QUOTE`/`IF`/`DEFINE`/`LAMBDA`/five binops); anything else is compiled
  as an application.
- No capability gating exists because no I/O is reachable from compiled
  code at all yet: §4 is vacuously unmet rather than violated.
- The reader parses only signed integers, symbols, lists, and `'quote`
  sugar — no strings, floats, or dotted pairs — short of even the minimum
  in §5. The printer round-trips only fixnums.
- Fixnum overflow silently wraps (tag-cancellation arithmetic, no overflow
  check emitted) — consistent with, not a violation of, §1's latitude on
  overflow behavior.

None of this makes `lamedh-asm` a conformance failure today: it is a v0
prototype whose own README lists every one of these as a scope limit, not a
silent trap. It is listed here so that closing each gap can be checked
directly against this document rather than against the moving target of
what the Rust reference happens to do.

## Status and suggested next step

The audit above answers the "compare the three hosts side by side" step
#452 asked for before freezing wording. Open work this first version
deliberately leaves for a follow-up rather than blocking on:

- Enumerate the exact arithmetic/string/hash-table/array builtin table
  per §1, rather than the "whatever `lib/*.lisp` calls" operational
  placeholder above.
- Audit the Rust reference's `NET-*`/`OS-*` capability names for actual
  enforcement at the call site, matching the depth already done for
  `SHELL`/`READ-FS`/etc. and for the SBCL port's gap.
- Reconcile the SBCL port's capability-enforcement gap and give
  `lamedh-asm` a tracked path through §1–§5.
- Get the portable HM type checker (#451) actually running, unmodified,
  on every host that has the §3 `EVAL` hook. This is prioritized above the
  other items in this list: static typing is not optional scaffolding, it
  is how a Lamedh program large enough to need multiple hosts stays
  maintainable on any of them.
- Scope a `tests/kernel-conformance/` corpus, per #452's own risk list, so
  this document does not decay the moment one host's convenience wins out
  over the line drawn here, and so "the shared `lib/*.lisp` corpus" above
  has an executable definition instead of a pointer to `src/lib.rs`.
