# The typed-island front end

Status: implemented. `lib/46-hm-check.lisp` (sections 7b and 10) and
`lib/47-typed-island.lisp`; tests in `tests/test_typed_island.rs` and
`tests/lisp/87-typed-island.lisp`. Builds on `docs/typed-checker-design.md`
(checkability versus compileability) and `docs/typed-region-design.md`
(the type system as the gate between execution tiers, the freeze). Issue
#451 ported the checker; this document is the compiler-facing half.

## 1. The claim

Three kernels run Lamedh: the Rust reference host with its Cranelift back
end (`src/jit/`), the SBCL port (`sbcl/`), and the x86-64 native compiler
(`lamedh-asm/`). Only the first types anything. Every one of them, to
compile a function natively, needs the same three facts:

1. the function is monomorphic over the compileable sub-lattice
   `int64 | float64 | bool | char | (array T) | struct`;
2. every call in its body resolves to a function with a known monomorphic
   signature;
3. its body is applicative: no operative survives, no literal outside the
   unboxed scalars, no free global.

Those facts are decided by type inference, and type inference has no host
dependency. So the decision is made once, in Lamedh, and every kernel
receives the result: a **manifest** of functions, each with a pinned
signature and a frozen body. A kernel then owns lowering only. On the Rust
host the manifest is consumed through `declare-typed` and `defun-typed`,
and the host's own verdict is read back for every member. The two gates
validate each other on every hand-off.

## 2. The gate (`lib/46-hm-check.lisp`, section 7b)

The native elaborator (`src/jit/elaboration.rs`) has two modes. Under
`checking: true` it is the permissive checker issue #451 ported: gradual
`any` at the operative frontier, lists, pairs, strings, declared schemes,
protocols, derived callee schemes. Under `checking: false` it is the
kernel's admission test, followed by `Infer::resolve` on every signature
type. Section 7b ports that second mode. It differs from checking exactly
where `elaboration.rs` tests `self.checking`:

| Construct | Checking mode | Codegen mode |
|---|---|---|
| string, char, `nil`, other literal | `string`, `char`, `(list a)`, `any` | `typed core: unsupported literal` |
| free symbol | `any` | `unbound variable` |
| `if` condition, `not` operand | any type | must be `bool` |
| `and`/`or` | `any` | every operand `bool`, result `bool` |
| `+ - * / mod`, comparisons | operand types unify; known non-numerics rejected | operand kind **resolved eagerly at every fold step** |
| `setq` `while` `for` | ordinary call path | native rules (local slots; `int64` statement) |
| `sqrt floor ceiling truncate round sin cos tan exp float` | declared schemes | native rules over `float64` |
| `logand logior logxor`, constant `ash`, `abs`, binary `min`/`max` | variadic schemes | native rules over resolved kinds |
| `array-add! array-sub! array-mul! array-sum array-dot` | ordinary call path | native rules over resolved element types |
| `cons car cdr list null record-* append concat quote cond variant-case when unless` | native rules | ordinary call path: `call to unknown function` |
| a call | host registry, protocol, declared scheme, derived scheme, `any` | run registry, host registry, `funcall`/`apply` as `any`, else `call to unknown function` |

Two of those rows carry the whole fidelity argument. Eager resolution:
`(defun sq (x) (* x x))` is blocked with `` `*`: cannot infer operand type``
even though nothing contradicts `int64`, because the native path chooses
`iadd` versus `fadd` at that node and has nothing to choose with. The
closed call rule: a callee is known or the function is not compileable;
there is no gradual frontier in codegen. A gate that improved on either
would admit functions the kernel refuses.

`hm-resolve` mirrors `Infer::resolve` message for message. `hm-compile-verdict`
is the portable `explain-compile`: `(COMPILEABLE (-> (T...) R))`,
`(BLOCKED "reason")` or `(DYNAMIC "reason")`. `hm-compile-group` runs a set
of functions in one state with every member registered before any body is
elaborated, pinned members under their pin, the rest under a provisional
arrow. That is the portable form of the kernel protocol *declare every
member, then define each*.

## 3. The island (`lib/47-typed-island.lisp`)

```
names
  | island-source    the live plain lambda; or, for a name the kernel reports
  |                  TYPED, the source the kernel compiled (plist, written by
  |                  the act that installed the membrane)
  | island-freeze    global macros expanded to a fixpoint, once
  | typed-island     discover in one state, verify in a clean one
  v
((members . ((NAME SIG LAMBDA ARROW) ...)) (rejected . ((NAME . "why") ...)))
  | island-optimize  optimize-form on every body, re-gated under the island's
  |                  own signatures; a changed type is a regression, reported
  | island-install!  declare-typed + defun-typed, verdict read back
  v
((NAME AGREE sig) | (NAME DISAGREE island-sig kernel-sig) | (NAME KERNEL-REJECTED "msg") ...)
```

**Freeze.** The kernel's codegen path never expands a macro; a `when` in a
body is an unknown call to it. The front end expands every global macro to
its fixpoint before typing, and the frozen residue enters the manifest.
This is the phase separation of `docs/typed-region-design.md` §4: a
`defmacro` after the freeze reaches nothing, because the residue no longer
mentions the macro. `quote` and `quasiquote` are data and are not entered.

**Discovery.** `hm-compile-group` over the whole set, then again, then
again, in the same state, until the set of members that compile stops
growing. Passes matter because of eager resolution: `(defun addp (a b) (+ a
b))` blocks alone, a caller `(addp x 1.5)` elaborated in the same state
pins `a` and `b`, and on the next pass `addp` compiles. A member that fails
leaves its partial bindings behind on purpose. The compiling set grows
monotonically, so discovery ends within one pass per member.

**Verification.** The discovered members are re-run from a fresh state with
the discovered signatures as pins, and the set is shrunk until one round
passes with no failure. Only that round's signatures enter the manifest.
Two properties follow. *Consistency*: no rejected member's bindings shaped
any signature. *Closure*: a member that called a rejected peer meets `call
to unknown function` in the clean round and is dropped, and its callers
after it, each with that reason.

**Why a group.** The kernel's own `jit-optimize` compiles one definition at
a time, in definition order. Mutual recursion and caller-pinned helpers
never compile natively that way. Handed as a group with signatures declared
first, they do; `declare-typed` exists for exactly this. The island is the
largest such group, and the manifest is what `declare-typed` and
`defun-typed` need.

**Optimizer validation.** `island-optimize` runs `optimize-form` (the
compiler pipeline hook: Lisp passes, rulebook, frame collapse, constant
folding) over every member and re-gates the group under the island's own
signatures as pins. A member whose optimized body resolves to the same
signature takes it. One whose optimized body no longer compiles under that
signature (which is how a changed type appears under a pin), or on which
the optimizer signalled, keeps its original body and is listed under
`regressions`. The member set and every signature are invariant by
construction. The optimizer is validated on every run, not trusted.

**Hand-off.** `island-forms` is the manifest as data: every member's
`declare-typed` form, then every member's `defun-typed` form, for a kernel
consumed offline. `island-install!` evaluates them in the caller's
environment and reads `see-type` back for each member. `AGREE` on every
member is the portable front end and the native kernel agreeing on that
island. Note that `defun-typed` rebinds a member to the kernel's typed
entry with no dynamic fallback: a call outside the signature becomes a
membrane error.

## 4. Honesty

- A name without a visible source is rejected as such. The plist source is
  read in one situation only: the kernel reports the name `TYPED` and the
  plist entry was written by the act that installed that binding
  (`jit-optimize`, `defun*`, `defun-typed`). Type annotations in that source
  are dropped; the gate re-derives them.
- A blocked member carries the kernel's own blocker wording.
- `island-install!` reports what the kernel said. It never reports `AGREE`
  from the island's side alone.
- The freeze walker does not know binding forms. A local variable named
  like a global macro is expanded at its use sites; such a body types
  wrongly and is rejected by the gate, never mistyped into the island.

## 5. Validation

`tests/test_typed_island.rs` holds both gates to each other:

- On every function the standard library defines, the portable gate and
  `explain-compile` agree. For a name the kernel has `TYPED`, the portable
  gate reproduces the kernel's signature: from the source alone when the
  kernel inferred it, under the kernel's signature as pins when the author
  annotated it. For every other name, the kernel admits it iff the portable
  gate does.
- The island of the whole standard library, handed to the kernel, comes
  back `AGREE` on every member, and contains every function the kernel had
  already compiled from a visible source.
- Mutual recursion, caller-pinned helpers, closure under calls, freezing,
  and a deliberately type-changing optimizer are each pinned by a test.

The suite passes under `--no-default-features` as well: the closure tier
reports `TYPED ... INTERPRETED`, and agreement compares signatures, not
tiers.

## 6. What the other kernels receive

The SBCL port and `lamedh-asm` load `lib/*.lisp` unmodified, so both load
this front end. Neither has a typed registry today, so on them
`island-source` sees only live lambdas, `hm-host-arrow` sees nothing, and
`island-install!` reports `KERNEL-REJECTED` for every member. What they can
consume is `island-forms`: a manifest of `declare-typed` and `defun-typed`
forms with every type pinned and every body frozen, needing no inference
on the receiving side. That is the interface: the front end decides, a
kernel lowers.
