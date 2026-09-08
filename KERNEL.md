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
implements exactly the surface enumerated below — no more required, and
nothing on the list reimplemented divergently — and the shared `lib/*.lisp`
corpus (or an explicitly agreed portable subset of it) loads and runs
unmodified on top of it, producing identical observable behavior for any
program that does not reach for a host-specific escape hatch. This document
*is* the spec; it does not summarize a decision made elsewhere.

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
- **Floats, strings, and chars** sufficient for the reader/printer baseline
  in §5 and for the arithmetic and string operations `lib/*.lisp` calls
  natively (see each host's builtin surface for the exact list; this
  document does not enumerate individual arithmetic/string builtins, only
  the representational categories).

Explicitly *not* required: a destructive mutation primitive on cons cells.
The Rust reference's `RPLACA`/`RPLACD` are non-destructive (each returns a
new cell — see Divergences) precisely so that no host is required to support
in-place list mutation or its consequences (aliasing, circular structure).
Allocation and GC policy are a host's own business; that consing *works*,
with the identity/equality semantics above, is what conformance requires.

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
- No portable HM type checker exists yet (#451's subject); the honest
  unverified-axiom surface (`DECLARE-TYPE!`/`SEE-TYPE`) stands in for it
  rather than faking verification — this is a scoping gap, not a kernel
  nonconformance.

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

## Suggested next step

The audit above answers the "compare the three hosts side by side" step
#452 asked for before freezing wording. The remaining work is reconciling
the SBCL port's capability-enforcement gap and lamedh-asm's road to §1–§5,
and — per #452's own risk list — scoping a `tests/kernel-conformance/`
corpus so this document does not decay the moment one host's convenience
wins out over the line drawn here.
