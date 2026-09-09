# lamedh-asm

A parallel implementation of Lamedh — compiled directly to x86-64 machine
code, hand-assembled, with no Rust, no libc, no C ABI, and no dependency
on the Rust interpreter at `../src`. It is a separate execution engine
for the same surface language, not a backend bolted onto the existing
one.

Freestanding: `_start` is the process entry point, every host interaction
is a bare Linux `syscall` instruction, and the only external tools in the
build are `nasm` (assembler) and `ld` (linker) — see `Makefile`.

## Why this exists, and what "fast" means here

The brief was to beat C at both intra- and inter-function optimization,
using whatever the ISA allows and a JIT would need — including code that
rewrites itself while it runs. Two design choices follow from that:

1. **Compile, don't interpret.** Every top-level form and every `LAMBDA`
   becomes a real native function the first time it is read, not a tree
   walked at every call. `compiler.asm` is a single-pass recursive
   compiler: the recursion structure over the s-expression *is* the
   code-generation strategy. There is no bytecode and no separate IR.

2. **Self-modifying code is a first-class mechanism, not a hazard to
   avoid.** The code heap (`heap.asm`) is mapped RWX for the process
   lifetime, and a call site can rewrite its own machine bytes at
   runtime. This is what powers the inline-cached calls in
   `compile_call` (see below) — a genuinely faster calling convention
   than a fixed indirect call, achieved specifically because target
   addresses are decided *after* the fact and the running program is
   trusted to patch its own instruction stream, something a Harvard
   architecture (or a W^X-hardened OS) forbids by construction.

Whether this actually beats `gcc -O3`/`clang -O3` on a given benchmark is
an open, checkable question, not yet gated by a benchmark corpus in this
tree (see Roadmap). What's implemented so far is real, tested compiled
execution with the mechanisms (tagged unboxed fixnums, compile-time
lexical addressing, closure conversion, self-patching call sites, tail
frame-reuse groundwork) that make the claim contestable at all.

## Value representation

One 64-bit word per value; the low 2 bits are a tag (`src/tags.inc`):

| tag  | meaning   | payload |
|------|-----------|---------|
| `00` | fixnum    | bits 63:2, a signed 62-bit integer |
| `01` | cons      | bits 63:2 (shifted) address a 16-byte `[car\|cdr]` cell |
| `10` | heapobj   | address of a header word naming the object's kind (symbol, closure, string, float, array) |
| `11` | immediate | `NIL`, `TRUE`, `FALSE`, `UNBOUND`, `EOF` |

Fixnum arithmetic runs directly on the tagged (shifted-left-by-2)
representation: `ADD`/`SUB` need no untag/retag at all, since the tag
bits cancel; only `IMUL` needs a post-shift correction. Cons cells,
symbols, and closures are bump-allocated on a single `mmap`'d data heap
with **no garbage collector** (see Roadmap) — sized generously for the
programs this stage targets.

## What's compiled (this stage)

- `QUOTE`, `IF` (compile-time backpatched branches), `DEFINE` (a global's
  value cell lives at a fixed heap address decided at intern time, so a
  reference or store compiles to one absolute-address load/store — no
  runtime name resolution, ever).
- Binary `+ - * < =` operating on unboxed tagged fixnums, plus `MOD`
  and `REMAINDER`.
- `PROGN`, `COND`, `AND`, `OR`, `LET`, `LET*`, `SETQ`, `HANDLER-CASE`,
  `BLOCK`/`RETURN-FROM`, `WHILE` as real special forms (Part VI/VII),
  plus `ERROR`/`ERRORSET`/`ERROR-P`/`ERROR-MESSAGE`/`ERROR-DATA`
  (Part VIII's condition system).
- `CAR`/`CDR`/`CONS`/`EQ`/`ATOM`/`NULL`, `DEFMACRO`, `CATCH`/`THROW`,
  `PRINT`/`NEWLINE`, `STRING-LENGTH`, `FD-OPEN`/`FD-CLOSE`/`FD-WRITE`/
  `FD-READ`, `FLOAT`/`F+`/`F-`/`F*`/`F/`/`F<`, and `MAKE-ARRAY`/
  `ARRAY-REF`/`ARRAY-SET`/`ARRAY-LENGTH`/`HASH-CODE` — see "The
  kernel surface" below.
- String and float literals (`"..."`, `3.14`) read as heapobjs
  (`HDR_STRING`, `HDR_FLOAT`) and are self-evaluating, exactly like a
  fixnum literal — no compiler change was needed for that part, since
  the data heap never relocates and a heapobj's absolute address bakes
  as a code immediate the same way any other literal datum does.
- `LAMBDA` with real closure conversion: a free-variable scan
  (`scan_free_vars`) decides what a nested lambda must capture *before*
  a single byte of its body is emitted; captured values are copied by
  value into the closure object at the point the `LAMBDA` form actually
  runs (not once at compile time — a closure created inside a loop over
  different captured values is a fresh heap object each time).
- Function application, two flavors:
  - **A local/free-value callee** (a variable holding a closure, or an
    immediately-invoked `LAMBDA` literal): one indirect call through the
    closure's stored code pointer.
  - **A global name** (`(SQUARE 7)` where `SQUARE` was `DEFINE`'d): a
    **self-patching inline cache**. The call site starts pointing at a
    small per-call-site trampoline that resolves the symbol's *current*
    value, rewrites the original call site's `rel32` displacement in
    place to jump straight to the resolved code, and only then transfers
    control — every subsequent call from that exact site is a plain
    direct call, no indirection, no re-resolution. This is the same
    `patch_rel32` primitive `compile_if`'s compile-time branch
    backpatching uses; compile-time and runtime self-modification are
    literally the same mechanism.

Local/free variable references compile to a fixed `[rbp+disp]` load
decided entirely at compile time (`current_scope`, a compile-time-only
lexical scope chain) — never a name lookup at runtime.

## The kernel surface: what's native versus what belongs in a library

[Issue #452](https://github.com/pnathan/lamedh/issues/452) proposes a
host-agnostic spec for the minimal primitive surface any Lamedh host
(the Rust reference, the SBCL port, this one) must provide, so that a
shared `lib/*.lisp`-style corpus can run unmodified on top of any of
them. This project is the concrete first attempt at drawing that line
for a from-scratch host, and three of its later primitives exist
specifically to test where the line falls:

- **`CAR`/`CDR`/`CONS`/`EQ`/`ATOM`/`NULL`** compile to calls into the
  same host routines the reader and compiler already use internally
  (`reader.asm`'s `car`/`cdr`/`cons`), reached from *compiled Lamedh
  code* for the first time — previously only the compiler itself could
  call them. This is the minimum needed before any list-processing
  library code is expressible in Lamedh at all on top of this kernel.
- **`DEFMACRO`** is the single highest-leverage primitive on the
  candidate list: a macro transformer is compiled exactly like a
  `LAMBDA` (`compile_defmacro` reuses `compile_lambda` directly, wrapping
  the body in a synthetic `(LAMBDA params body)` built with `cons`), and
  expanding a macro call means invoking that *already-compiled closure*
  directly from host code — `invoke_closure_host` does an ordinary
  indirect call through the closure's stored code pointer, synchronously,
  at compile time, with the call site's raw unevaluated argument forms
  as arguments (`raw_args_to_regs`). Nothing distinguishes "the compiler"
  from "compiled code" here; they are both just x86-64 machine code
  running in the same process, so a macro transformer needs no
  interpreter of its own. Whatever it returns is recursively compiled in
  its place. `COND`/`AND`/`OR`/`LET`/the CL-compat layer all become
  ordinary Lamedh source once this exists, the same way they already are
  in the reference implementation's `lib/08-vau.lisp` and
  `lib/21-cl-compat.lisp` — see `tests/cases/011_defmacro.asm` for
  `UNLESS` derived from `IF` this way, with no change to the compiler.
- **`CATCH`/`THROW`** is the other "control" candidate: a fixed-depth
  stack of installed catch points (`compiler.asm`'s `catch_stack` —
  tag, saved `rbp`/`rsp`, resume target, 32 bytes per frame). `CATCH`
  installs a frame inline and falls straight through into its body;
  `THROW` walks the stack from the top for an EQ tag match and does an
  ordinary longjmp — restore `rbp`/`rsp` from the matched frame, put the
  thrown value in `rax`, jump to its resume point — including when the
  `THROW` executes from inside a different compiled function than the
  one holding the `CATCH` (`tests/cases/012_catch_throw.asm`'s third
  case: a `THROW` inside a `DEFINE`'d function unwinds correctly back
  through an inline-cached call). `BLOCK`/`RETURN-FROM` and first-class
  conditions are ordinary library code once this exists, the same way
  they already are in the reference implementation.
- **`PRINT`/`NEWLINE`** are the first primitives that let *compiled*
  Lamedh code produce output at all — every test before them called
  `print_fixnum` from the hand-written host driver, never from within a
  compiled program (`tests/cases/013_print.asm`). They wrap the same
  `print_fixnum`/`print_newline` host routines the test harness always
  used, reached the same way `CAR`/`CDR`/`CONS` reach `car`/`cdr`/`cons`.
  `PRINT` returns its argument, the way most Lisps' `PRINT` does. This
  is deliberately the *minimum* possible I/O primitive (stdout, one
  fixnum at a time, no format string) — see Roadmap for what a real
  `FORMAT` still needs (variadic args, a general write primitive)
  before it can be library code the way `CAR`/`CDR`-based list
  processing already is. `PRINT` itself now dispatches on the
  argument's *runtime* tag (`print_value` in `strings.asm`): a string
  writes its raw bytes, anything else prints as a fixnum's decimal
  value — the compiled code `compile_print` emits is unchanged; only
  the host address it bakes moved from `print_fixnum` straight to
  `print_value`.
- **`STRING-LENGTH`** and **`FD-OPEN`/`FD-CLOSE`/`FD-WRITE`/`FD-READ`**
  round out enough of a host surface to read and write real files.
  Every fd is a plain tagged fixnum, so `STDIN`/`STDOUT`/`STDERR` (0/1/2)
  need no separate primitives at all — `(FD-WRITE 2 "oops")` already
  writes to stderr. `FD-OPEN`'s mode argument is a plain fixnum (`0`
  read, `1` write/create/truncate, `2` append/create), not a symbol:
  a bare symbol in argument position would be read as a *variable
  reference* by `compile_form`'s existing global/local lookup, not the
  symbol's own identity, and requiring the caller to quote it (`'WRITE`)
  seemed like the wrong trade for three fixnum constants. All four are
  raw one-shot Linux syscalls (`fileio.asm`) — `FD-WRITE`/`FD-READ`
  don't loop on a short write/read, and a read past EOF or a syscall
  error folds to an empty string rather than raising anything, there
  being no conditions yet to raise (same honest scope as `THROW` with
  no matching `CATCH`).
- **`FLOAT`/`F+`/`F-`/`F*`/`F/`/`F<`** are a boxed IEEE754 double
  (`HDR_FLOAT`, `floats.asm`) and its arithmetic — but, unlike every
  other primitive above, arithmetic on it is *not* inlined as target
  SSE2 instructions. `codegen.asm`'s own header is explicit that it
  hand-encodes only the 8 base GPRs; rather than extend it with XMM
  register support, each float op is an ordinary host routine (using
  XMM registers freely on the host side, invisibly to the target-code
  emitter) reached through `compile_binary_hostcall`/
  `compile_unary_hostcall` — the exact same mechanism `CAR`/`CDR`/`EQ`
  already use to reach `car`/`cdr`, just with a different address
  baked in. `FLOAT` converts a fixnum to a float (the only way to get
  one other than a reader literal); `F<` returns `IMM_TRUE`/`IMM_NIL`,
  matching `<`'s own convention. This is a real, honestly-labeled
  trade-off, not a shortcut disguised as a feature: a JIT worth the
  name would inline these; a real function call per float operation is
  the cost of not yet teaching `codegen.asm` about XMM registers at
  all (see Roadmap).
- **`MAKE-ARRAY`/`ARRAY-REF`/`ARRAY-SET`/`ARRAY-LENGTH`** are a fixed-
  length, index-addressable slot vector (`HDR_ARRAY`, `arrays.asm`) —
  and `ARRAY-SET` is the **first primitive in this kernel that mutates
  a heap value after creation**. Every value type before it either
  can't be rewritten at all (fixnums, immediates) or is captured/read
  but never mutated in place (a closure's captured values are copied by
  value at creation; a cons built by `CONS` is never `RPLACD`'d).
  Scoped as narrowly as that one capability: no grow/shrink, no bounds
  check (v0 — see limits below).
- **`HASH-CODE`/`MOD`/`REMAINDER`** round out the small extra kernel
  surface a *real* hash table needs beyond plain list processing:
  `HASH-CODE` returns a stable, always-non-negative fixnum for a
  fixnum (its own magnitude) or any heapobj (its own heap address —
  stable since nothing here relocates), letting different key
  *values* land in different buckets without needing structural
  hashing; `MOD` and `REMAINDER` are two distinct integer-division
  operators (neither previously existed — `compile_binop` only ever
  grew `+ - * < =`), matching the [KERNEL.md](https://github.com/pnathan/lamedh/pull/453)
  spec exactly: `MOD` is Euclidean (always `0 &lt;= r &lt; |b|`),
  `REMAINDER` is truncated (sign follows the dividend) — they disagree
  exactly when the operands' signs differ (`(MOD -7 2)` is `1`,
  `(REMAINDER -7 2)` is `-1`).
- **Hash tables are not a kernel primitive at all** — the concrete
  demonstration issue #452's kernel/library boundary was framed around,
  and still isn't one even with real, expected-O(1) performance.
  `HT-MAKE`/`HT-SET!`/`HT-GET`/`HT-HAS-KEY`/`HT-BUCKET-ASSOC`/
  `HT-BUCKET-REMOVE` (`tests/cases/022_hashtable_array.asm`) are six
  small `DEFINE`s: a fixed-size `MAKE-ARRAY` bucket table, each bucket
  a short `CONS`-built alist chain, `HASH-CODE`+`MOD` picking the
  bucket, `ARRAY-SET` mutating that one bucket slot on `HT-SET!`. Only
  the four primitives above are new kernel surface; the hashing/
  bucketing/chaining *policy* is all library code, exactly like the
  persistent alist version it replaces
  (`tests/cases/020_hashtable.asm`, kept as-is: a smaller, purely
  functional alternative when mutation isn't wanted). No resizing (a
  fixed 61-bucket table — a real implementation would grow it as load
  increases); no key removal; a real key comparison would need
  `EQUAL`-style structural equality to support string/list keys, not
  just `EQ`'s pointer identity — fine for the symbol/fixnum keys this
  library is tested with, a real limit for anything else.

Symbols carry a dedicated macro slot (`symtab.asm`, offset 24) distinct
from their ordinary value cell, so a name can be a macro or a function
without ambiguity; macro-hood is checked at compile time only; a
reference to a global name that isn't a macro never pays for the check
at runtime.

## KERNEL.md conformance

[KERNEL.md](https://github.com/pnathan/lamedh/pull/453) (issue #452) is
the host-agnostic Lamedh specification: the exact primitive names,
semantics, special forms, condition/capability/fuel systems, and reader/
printer rules a host must match so the shared `lib/*.lisp` corpus loads
unmodified. This project predates that document; it is being brought
into conformance incrementally, tracked honestly rather than silently:

- **Matches**: `CAR`/`CDR`/`CONS`/`EQ`/`ATOM`/`NULL` (names and basic
  semantics); `EQ` on cons cells is pointer identity, which the spec's
  own Part IV explicitly leaves undefined (issue #454) rather than
  requiring the reference's hardcoded `NIL`; `MOD`/`REMAINDER` now
  match Part V's exact Euclidean/truncated split; `PRINT` now produces
  Part III's PRIN1-style readable text for every value this kernel
  has — `NIL` as `()`, `T` as `T`, a symbol as its name, a cons
  recursively as a proper or dotted list — not just fixnums/strings/
  floats (`tests/cases/024_print_readable.asm`). `PROGN`, `COND`,
  `AND`, `OR` now exist (`tests/cases/025_progn_cond_and_or.asm`) with
  the spec's exact edge cases: `(progn)` is `NIL`, a `COND` clause
  with no body returns the test's own value, `(and)` is `T`, `(or)`
  is `NIL`. `LAMBDA` bodies are no longer single-expression-only —
  multiple forms are implicitly `PROGN`-wrapped (`compile_lambda` now
  compiles `cddr` of the whole form through `compile_progn`, not just
  `caddr` through `compile_form`). `LET` (parallel) and `LET*`
  (sequential) also exist now (`tests/cases/026_let.asm`), sharing the
  enclosing function's own stack frame rather than allocating one of
  their own — a `LET` reserves its slots with a plain `sub rsp` at
  entry and releases them with `add rsp` at exit, addressed via the
  same `[rbp+disp]` scheme as every other local, at whatever depth
  `current_frame_depth` says is next-free (tracked and restored around
  nested `LAMBDA`/`LET`/`LET*` the same way `current_scope` already
  is). `SETQ` also exists (`tests/cases/027_setq.asm`): it writes the
  first lexically bound slot found via `frame_lookup` against
  `current_scope` (a `LET`/`LET*` binding or a `LAMBDA` param/free
  slot, whichever is nearest), or the target symbol's global value
  cell — the same absolute-address store `DEFINE` itself uses — when
  it isn't lexically bound anywhere. This kernel has no dynamic-
  variable mechanism yet, so the dynamic half of Part VI's `SETQ`
  resolution rule doesn't apply, and a name that's neither lexically
  bound nor previously `DEFINE`'d just gets its (always-reserved)
  global cell written rather than a fresh binding created in the
  enclosing frame — a narrower but still useful approximation of the
  spec's own fallback rule.
- **The condition system (Part VIII) now exists**: `ERROR`/
  `HANDLER-CASE`/`ERRORSET`/`ERROR-P`/`ERROR-MESSAGE`/`ERROR-DATA`
  (`tests/cases/028_conditions.asm`), built entirely on the existing
  `CATCH`/`THROW` machinery via one shared internal tag every
  `HANDLER-CASE`/`ERRORSET` installs its catch frame with and every
  `ERROR` throws to — Part XII axis 3's own explicit license to derive
  a special form this way, not a new signaling primitive. One shared
  tag is safe because `CATCH`/`THROW`'s own "nearest matching frame
  wins" search already gives the correct nesting behavior for free: a
  `HANDLER-CASE` nested inside another catches first, and `(ERROR c)`
  re-signaling an already-a-condition `c` unchanged correctly escapes
  to the *next* enclosing one, since the catching `HANDLER-CASE`'s own
  frame is already popped by the time its handler body runs. `(ERROR)`
  with 0/1/2 arguments, and `HANDLER-CASE`'s unconditional catching (no
  typed clause) match the spec exactly. `ERRORSET` is narrower than
  the spec's own: it compiles and runs its form directly as ordinary
  Lamedh source, not "evaluates a value that is itself then run as
  code" — this kernel has no `EVAL` primitive yet to do that second
  step with (see Roadmap). No native failure (division by zero, index
  out of range, wrong arity, unbound variable) signals a condition
  yet — those still misbehave exactly as before; only explicit `ERROR`
  calls go through this system so far.
- **`BLOCK`/`RETURN-FROM`** are the same CATCH/THROW derivation
  trick again, one call site simpler than `HANDLER-CASE`: `name` is
  *unevaluated*, so it's used directly as the catch frame's tag (no
  shared tag needed here — each block's own name already is a unique
  tag), and a caught `RETURN-FROM`'s thrown value needs no rebinding,
  it just *is* the `BLOCK`'s result. Dynamic, not lexical, matching
  the spec: a function called from inside a `BLOCK` can
  `RETURN-FROM` it (`tests/cases/029_block_while.asm`, mirroring
  `CATCH`/`THROW`'s own cross-function test). **`WHILE`** is an
  ordinary backward-branch loop — nothing to derive, since a backward
  jump to an already-known target needs no forward-patching machinery
  at all, unlike everything above it.
- **Not yet conforming, tracked as ongoing work**: most of Part VII's
  special forms (`PROG`/`FOR`/`UNWIND-PROTECT`/`VAU`/`DEFDYNAMIC`/
  `QUASIQUOTE`) don't exist yet; capability gating
  (Part IX) and fuel (Part X) don't exist yet; proper tail calls
  (Part VI) aren't implemented (see v0 limits below); the hash
  table/array/float primitive *names* (`HT-*`/`ARRAY-*`/`F+` etc.)
  don't match Part XI's required exact names (`MAKE-HASH-TABLE`/
  `ARRAY`/`FETCH`/`STORE`/contagion-based `+`) —
  Part XI is explicit that this is the actual conformance bar, not a
  detail; there is no character type, no Unicode-codepoint string
  indexing, no typed arrays, no environments-as-values, no `GENSYM`/
  property lists, no `EVAL`/`READ-FROM-STRING`; the printer has no
  cycle detection (unreachable anyway — cons cells are immutable here)
  and no opaque-value tags (`<lambda>` etc.) for closures/arrays/hash
  tables yet. This list is deliberately specific so it can shrink
  honestly, item by item, rather than being replaced by a vaguer
  "in progress" note.

## v0 limits (known, not silent)

- A lambda body may have multiple forms, implicitly `PROGN`-wrapped
  (no longer a v0 limit — see "KERNEL.md conformance" above).
- A nested `LAMBDA` may only capture free variables from its
  *immediately* enclosing lambda's own frame — a variable needed from
  two levels up needs manual re-threading through the middle lambda for
  now; deeper transitive free-variable propagation isn't implemented.
- Captured variables are captured **by value** at closure-creation time,
  not as shared mutable cells — there is no `SETQ` on a captured
  variable visible to the closure that captured it (or vice versa).
- No garbage collector. No bignums, `vau`, first-class conditions, or
  dynamic variables yet.
- Arrays are fixed-size at creation (`MAKE-ARRAY` doesn't grow/shrink)
  and `ARRAY-REF`/`ARRAY-SET` do no bounds checking — an out-of-range
  index reads or writes adjacent heap memory rather than raising
  anything. `ARRAY-SET` is also the only mutation primitive in this
  kernel; nothing else (a cons, a closure's captured values) can be
  rewritten after creation.
- The array-backed hash table (`HT-MAKE` et al.) is a fixed 61-bucket
  table with no resizing, no key removal, and `EQ`-only key comparison
  (symbol/fixnum keys hash well; a string or list key would need
  `EQUAL`-style structural equality this kernel doesn't have).
- Strings are immutable byte buffers only: no `STRING-REF`,
  `STRING-APPEND`, `SUBSTRING`, or any string-building primitive yet —
  just reader literals, `STRING-LENGTH`, and `PRINT`. Escapes are
  limited to `\n`, `\t`, `\"`, `\\` (anything else after a backslash is
  copied through literally); a literal longer than the reader's 4KB
  scratch buffer is silently truncated.
- Float arithmetic (`F+`/`F-`/`F*`/`F/`/`F<`) is a real host-routine
  call per operation, not an inlined target instruction — see "The
  kernel surface" above. `PRINT` of a float is always fixed
  6-decimal-place formatting (`3.500000`), never scientific notation
  or shortest round-trip output; there is no `FLOAT<`-style family for
  `<=`/`>`/`>=`/`=` yet, and no mixed fixnum/float arithmetic (`(F+ 1
  2.0)` does not work — both operands must already be floats; use
  `FLOAT` to convert first).
- File I/O does not loop on a short `read`/`write`, and folds a
  negative syscall result (an error) to an empty string rather than
  signaling anything — there being no conditions yet to raise.
- `THROW` with no matching `CATCH` traps (`int3`) rather than raising a
  catchable condition — there being no conditions yet to raise.
- Proper tail-call frame reuse (`jmp` instead of `call`+`ret`, reusing
  the caller's stack frame) is not yet implemented for ordinary Lisp
  calls; the inline-cache trampoline's *own* internal dispatch already
  ends in a tail-jump, but the enclosing function's call site itself
  still uses `call`.
- `&REST` parameters are supported, but only when the fixed-parameter
  count is >= 3 (`(LAMBDA (A B C &REST MORE) ...)`, not `(LAMBDA (A
  &REST MORE) ...)`) — a deliberately narrow v1 that keeps every rest
  argument stack-resident, never register-spilled, sidestepping the
  register/stack boundary entirely (see the calling convention below).
- No benchmark corpus gate yet (see below).

None of these are silent traps in the sense of producing wrong answers
within their stated scope — they are things the compiler simply doesn't
attempt yet.

## Calling convention

This project owns its own convention; there is no C ABI to honor.
Compiled functions: `fn(rdi=closure_ptr, rsi=arg0, rdx=arg1, rcx=arg2,
[stack: arg3, arg4, ...]) -> rax`. The first 3 positional arguments
arrive in registers; the 4th onward arrive already pushed onto the
stack by the caller, in ascending order (`arg3` at `[rbp+16]`, `arg4`
at `[rbp+24]`, ...) — `build_param_frame` addresses them there directly,
with no copy into a local slot, since a stack-passed parameter is
already sitting exactly where it needs to be read from. Tested up to 32
arguments (`tests/cases/015_32args.asm`); nothing enforces that as a
hard ceiling, it is simply as far as this project has tested.

Argument evaluation order is right-to-left, operator position included
— `compile_call_args` recurses to the end of the argument list before
evaluating anything, which is what makes the first-3-in-registers /
rest-on-the-stack split fall out cleanly without a second pass: after
it returns, the stack (top to bottom) already reads `arg0, arg1, arg2,
arg3, ...`, exactly the pop order compile_call's two call paths use and
exactly the layout a callee's stack-passed params expect. The operator
form itself (which may be an arbitrary expression yielding a closure,
not just a bare symbol) is evaluated *last*, once every argument is
already on the stack — evaluating it first would leave the closure
value sitting underneath any stack-passed arguments, which is wrong for
the identical reason a mid-list pop would be. This is a real, visible
evaluation-order choice for anything with side effects in argument
position, the same way classic cdecl's own right-to-left evaluation is
a side effect of its stack layout rather than an accident — there is no
C ABI here dictating otherwise, so the layout gets to decide the order.

Every call site passes the actual argument count in `rax`, in addition
to the first 3 arguments in registers and any remainder on the stack
— set once at the call site and preserved through the inline-cache
trampoline's own internal scratch use, whether or not the callee cares.
A callee that ignores it pays nothing; a `&REST`-taking callee uses it
to know how many stack-passed arguments past its fixed parameters
actually exist. `&REST` is supported only when the fixed-parameter
count is >= 3 (`split_rest_params` splits `(A B C &REST MORE)` into
the fixed list `(A B C)` and the rest symbol `MORE` at `LAMBDA`-compile
time), because that restriction guarantees every rest argument is
stack-resident: the compiled prologue walks the stack-passed tail from
the last actual argument down to the fixed count, consing each onto an
accumulator (right-to-left, so the final list comes out in the
original left-to-right order), and stores the result into the `&REST`
parameter's own local slot — which always lands at `[rbp-32]`, right
after the 3 register-spilled fixed-parameter slots, precisely because
the restriction guarantees there are always exactly 3 of those.

Compiled code preserves **no** register across a call into other
compiled code — not even the base 8 GPRs used as scratch throughout
codegen. A caller (including hand-written test drivers) that needs a
value to survive a call into compiled code must keep it on the stack,
never trust a register — exactly what the compiler itself already does
for every Lisp-level intermediate value (`compile_binop` pushes operands
across nested calls rather than trusting a register to survive one).

`codegen.asm` hand-encodes every emitted instruction from a fixed set of
opcodes over the 8 base GPRs only (`rax rcx rdx rbx rsp rbp rsi rdi` —
encodings 0–7, so no REX.B/R is ever needed in emitted code).

## Build & test

```
make test      # assembles everything, runs tests/run.sh
make clean
```

No Cargo project lives here (deliberately — this directory has no
`Cargo.toml` and is invisible to the workspace at the repo root).
`nasm` and `ld` are the only required tools.

`tests/run.sh` assembles the shared core once, then for each
`tests/cases/NAME.asm` (which defines `global lamedh_main`) links it
against `boot.asm` + the core and diffs the resulting binary's stdout
and exit code against `tests/cases/NAME.expected` / `.exitcode`
(exit code defaults to 0 if the `.exitcode` file is absent).

## Roadmap

- Benchmark corpus + gate: a fixed set of numeric/looping Lamedh
  programs with hand-written C equivalents, checked into this tree, run
  under both `gcc -O3`/`clang -O3` and this compiler, wall-clock/cycle
  compared — the falsifiable form of "beats C."
- A real register allocator (linear-scan to start) instead of spilling
  every local to a fixed stack slot.
- Proper tail calls: frame-reuse `jmp` for calls in tail position.
- A copying or generational GC for the data heap.
- `&REST` params without the `nfixed>=3` restriction (would need a
  register/stack-boundary-crossing rest list, not just a stack-only
  one).
- General (not single-level) free-variable propagation through nested
  lambdas.
- Shared mutable closure cells (boxed captures) so `SETQ` on a captured
  variable is visible across closures over it.
- XMM support in `codegen.asm`, so float arithmetic can be inlined as
  target SSE2 instructions instead of a host-routine call per
  operation; mixed fixnum/float arithmetic; `FLOAT<=`/`FLOAT>`/
  `FLOAT>=`/`FLOAT=`; a real (shortest round-trip or scientific-
  notation) float printer instead of fixed 6-decimal-place formatting.
- Bignums, `vau`, dynamic variables — the rest of the Lisp 1.5 +
  extensions surface the Rust interpreter (`../src`) already
  implements. `DEFMACRO` existing means most of `lib/08-vau.lisp`'s
  derived forms and the CL-compat layer are now just a matter of
  writing them, not extending the compiler; `BLOCK`/`RETURN-FROM` are
  the next candidate for the same `CATCH`/`THROW`-derivation treatment
  `HANDLER-CASE` already got.
- A real `EVAL` primitive (form + optional environment), so `ERRORSET`
  can match the spec exactly ("evaluate a value that is itself then
  run as code") instead of just compiling its argument form directly;
  native failures (division by zero, index out of range, wrong arity,
  unbound variable) signaling catchable conditions instead of
  misbehaving.
- A resizable hash table (grow the bucket array and rehash past some
  load factor, instead of a fixed 61 buckets); `RPLACA`/`RPLACD`-style
  cons mutation now that `ARRAY-SET` has established the pattern;
  `EQUAL`-style structural equality, so the hash table (and `EQ`-only
  callers generally) can take string/list keys, not just
  pointer-identity-comparable ones.
- String mutation/building primitives (`STRING-REF`, `STRING-APPEND`,
  `SUBSTRING`) and a real `FORMAT` built on top of `PRINT`/`FD-WRITE`
  and variadic args.
- AArch64 backend (currently x86-64 Linux only).
