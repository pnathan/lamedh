# Pending and omitted conformance topics

Topics left out of `conformance/cases/` on purpose. Each entry says why.

## Symbol case and escapes (issue #523)

The owner is reverting this area, and the expected behavior is in flux.

- Case folding of symbol names (`KERNEL.md` Part II, "Case folding is
  unconditional"). Includes `'foo` reading as `FOO`, and `T`/`NIL` folding.
- Escaped symbols `|...|` (Part II, "Escaped"; Part III symbol printing rules
  for `|a b|`, `|foo|`, `|12|`, `|NIL|`, `||`).
- `INTERN` case behavior (Part II, Part XI).
- `EXPLODE` and `IMPLODE`.
- Caveat: about 15 cases write lowercase symbols in SOURCE and expect uppercase
  output (`'yes` style quoting, `'(a . b)` → `(A . B)`, `'FFh` → `FFH`). No case
  tests folding itself. The reader uppercases these names implicitly, so a host
  that changes folding under #523 will see those cases move.

## Nondeterministic or host-variant

- `GENSYM` names. Only distinctness is safe to test. The corpus has no gensym
  case at present.
- Hash-table key order from `KEYS` (Part IV, "unspecified order").
- Time, random numbers, the filesystem, the network, shells, processes, and
  environment variables. Capability gating is tested only on the denial side
  (no standing grant in a fresh environment).
- Printed text of opaque values such as `<lambda>`, `<builtin>`, `#<error ...>`
  (Part III, "the exact text is not portable").
- Error message text (Part VIII). Only `ERROR` is asserted.
- Part XII axes (bignum arithmetic, GC, host representation). A host may
  differ there by declaration.

## In flux or unspecified

- Mixed fixnum/float comparison: exact versus `f64` (Part V, #516 and #553).
  The corpus asserts only the `f64` rule that `KERNEL.md` states today.
- Typed subset: `DEFUN-TYPED`, `DEFUN*`, `DEFSTRUCT-TYPED`, `DECLARE-TYPED`,
  `JIT-OPTIMIZE`, `CHECK-TYPE` (Part VII, "not further specified here").
- Standard library and library-level forms (`DEFUN`, `WHEN`, `DOLIST`,
  `DEFRECORD`, `RESTART-CASE`, the CL-compat layer). Part XI says these are
  library code, and the full builtin-name inventory is open work (Part XIII).
- Macro-expansion caching and fuel step counts (Part VI, Part X, #460). Exact
  step counts are not part of the contract.

## JIT and compiled tiers

- The typed JIT and any compiled tier. Cases run through the interpreter path
  only. Tier parity needs its own corpus.

## Not yet covered (gaps, not decisions)

- Part IX positive grants and `WITH-CAPABILITIES` narrowing of a granted set.
  Needs a host that grants a capability before evaluation.
- `(kernel-fuel-set!)` and `(with-fuel)` exact remaining counts.
- `HASH-CODE` cross-type invariants beyond the equal-implies-equal-code case.
- Records (`DEFRECORD`, `#S` values with declared brands) beyond the reader
  and printer cases.
- Stream and port primitives (Part III opaque handles, Part IX I/O).
