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
  and `REMAINDER`; `+`/`-` set an observable `OVERFLOW` flag
  (`FLAG-SET-P`/`CLEAR-FLAG`/`CLEAR-ALL-FLAGS`) on wraparound.
- `PROGN`, `COND`, `AND`, `OR`, `LET`, `LET*`, `SETQ`, `HANDLER-CASE`,
  `BLOCK`/`RETURN-FROM`, `WHILE`, `PROG`/`GO`/`RETURN` (lexical v0 scope
  — see "KERNEL.md conformance" above) as real special forms (Part
  VI/VII), plus `ERROR`/`ERRORSET`/`ERROR-P`/`ERROR-MESSAGE`/`ERROR-DATA`
  (Part VIII's condition system).
- `CAR`/`CDR`/`CONS`/`EQ`/`ATOM`/`NULL`, `DEFMACRO`, `CATCH`/`THROW`,
  `PRINT`/`NEWLINE`, `STRING-LENGTH`, `FD-OPEN`/`FD-CLOSE`/`FD-WRITE`/
  `FD-READ`, `FLOAT`/`F+`/`F-`/`F*`/`F/`/`F<`, and `ARRAY`/
  `FETCH`/`STORE`/`ARRAY-LENGTH*`/`HASH-CODE` — see "The
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
  directly from host code — `invoke_macro` does an ordinary indirect
  call through the closure's stored code pointer, synchronously, at
  compile time, with the call site's raw unevaluated operand forms as
  arguments, for *any* number of operands: the first 3 go in
  `rsi`/`rdx`/`rcx`, and any beyond that are pushed onto the real host
  stack immediately before the call, in the same order
  `compile_call_args`' own target-code convention produces, so a
  transformer with more than 3 fixed/`REST` parameters sees its
  stack-passed operands at exactly the offsets `build_param_frame`
  already expects (`tests/cases/040_macro_many_args.asm`). This
  supersedes an earlier `raw_args_to_regs`+`invoke_closure_host` pair
  that only forwarded the first 3 operand forms (silently dropping the
  rest — a real, separate limitation from the `&REST`/`nfixed`
  restriction that was lifted below) and, worse, never set the incoming
  argument count (target `rax`) at all: a `&REST`-taking transformer
  reads that count at runtime to decide which register slots hold real
  `REST` data, and reading whatever host-side garbage happened to be in
  `rax` corrupted that decision silently rather than erroring —
  `lib/prelude.lisp`'s own `DEFUN` macro (`(NAME PARAMS &REST BODY)`,
  `nfixed=2`) is exactly this shape and is what exposed it
  (`tests/cases/038_macro_rest_below3.asm`). Nothing
  distinguishes "the compiler" from "compiled code" here; they are both
  just x86-64 machine code running in the same process, so a macro
  transformer needs no interpreter of its own. Whatever it returns is
  recursively compiled in its place. `COND`/`AND`/`OR`/`LET`/the
  CL-compat layer all become ordinary Lamedh source once this exists,
  the same way they already are in the reference implementation's
  `lib/08-vau.lisp` and `lib/21-cl-compat.lisp` — see
  `tests/cases/011_defmacro.asm` for
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
- **`ARRAY`/`FETCH`/`STORE`/`ARRAY-LENGTH*`** are a fixed-length,
  index-addressable slot vector (`HDR_ARRAY`, `arrays.asm`), named to
  match [KERNEL.md](https://github.com/pnathan/lamedh/pull/453) Part
  XI/IV exactly — `ARRAY`, `FETCH`, `STORE`, and `ARRAY-LENGTH*` are the
  spec's own primitive names, not this project's earlier `MAKE-ARRAY`/
  `ARRAY-REF`/`ARRAY-SET`/`ARRAY-LENGTH` — and `STORE` is the **first
  primitive in this kernel that mutates a heap value after creation**.
  Every value type before it either can't be rewritten at all (fixnums,
  immediates) or is captured/read but never mutated in place (a
  closure's captured values are copied by value at creation; a cons
  built by `CONS` is never `RPLACD`'d). Scoped as narrowly as that one
  capability: no grow/shrink, no bounds check (v0 — see limits below;
  Part IV requires an out-of-range index to be a catchable error, not
  yet true here).
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
- **`FLAG-SET-P`/`CLEAR-FLAG`/`CLEAR-ALL-FLAGS`** make fixnum overflow
  *observable* (`overflow.asm`), the "make it observable somehow" half
  of KERNEL.md Part V/Part XII axis 1's fixed-width model that issue
  #456 flagged as entirely missing. Fixnum `+`/`-` already run directly
  on the tagged (shifted-left-by-2) representation — tag bits are zero
  and cancel, so the emitted `add`/`sub` is bit-for-bit ordinary 64-bit
  two's-complement arithmetic on a value pre-multiplied by 4, which is
  exactly the "tag-cancellation wraparound arithmetic" KERNEL.md's own
  Part I names as one of the two reasons axis 1 exists. `compile_binop`
  now emits one extra check right after a compiled `+`/`-`: `jno .skip`
  around a call to `set_overflow_flag`, reusing the hardware overflow
  flag the `add`/`sub` instruction already computed — no separate
  range check needed, and no cost at all on the non-overflowing path
  beyond one predicted-not-taken branch. `*` is deliberately not wired
  to this yet: its compiled form runs `imul` on the already-shifted
  operands and corrects with a `sar` afterward, so `imul`'s own OF
  reflects overflow of that pre-correction, extra-shifted product, not
  of the represented fixnum multiplication — reusing it would be a
  different bug wearing this feature's name
  (`tests/cases/031_overflow.asm` exercises `+` wrapping at `2^61-1`
  and `-` wrapping at `-2^61`, this representation's actual dynamic
  range, plus both clearing primitives).
- **`STRING-REF`/`STRING-APPEND`/`SUBSTRING`** round out the string
  surface beyond `STRING-LENGTH`/`PRINT` (`tests/cases/036_string_ops.asm`)
  — see "v0 limits" below for their exact scope (byte-indexed, no
  bounds check, and `STRING-REF` still returns a raw fixnum byte value
  rather than a `Char` even now that the `Char` type exists — see
  "KERNEL.md conformance" below).
- **`EVAL`/`READ-FROM-STRING`/`PRINC-TO-STRING`** are Part XI's
  reflection primitives (`tests/cases/037_eval_read_princ.asm`) — the
  single highest-leverage addition per issue #452's own text, since it
  unlocks everything above it in this list plus `ERRORSET`'s spec-exact
  behavior at once. `EVAL` is `compile_thunk` (`eval_form` in
  `compiler.asm`, next to it) plus one indirect call to the resulting
  native function — exposing the driver's own top-level mechanism to
  *compiled* Lamedh code, the same "the compiler is just more compiled
  code" idea `DEFMACRO` already rests on, one level up. `READ-FROM-
  STRING` (`read_from_string_tagged`, `reader.asm`) is the more
  delicate of the two: `reader_buf`/`reader_pos`/`reader_end` are
  single global cells, not a stack, so calling this *from currently
  running compiled code* — e.g. inside a form a file-reading driver is
  itself mid-file evaluating — must save and restore the caller's own
  reader position around its own use, exactly the way a callee-saved
  register would, or the driver's next top-level read would resume from
  the wrong place. `PRINC-TO-STRING` (`princ_to_string`, `print.asm`)
  redirects `write_buf` — the single point every leaf of `print_value`'s
  dispatch already funnels through — into an in-memory buffer instead
  of stdout for the duration of one call, needing no change to
  `print_value`/`print_string`/`print_symbol`/`print_list`/
  `print_fixnum`/`float_print` at all; nested calls (a value that
  itself prints via `PRINC-TO-STRING` while being rendered by an outer
  one) save and restore the same way `READ-FROM-STRING` does.
- **Hash tables are not a kernel primitive at all** — the concrete
  demonstration issue #452's kernel/library boundary was framed around,
  and still isn't one even with real, expected-O(1) performance.
  `HT-MAKE`/`HT-SET!`/`HT-GET`/`HT-HAS-KEY`/`HT-BUCKET-ASSOC`/
  `HT-BUCKET-REMOVE` (`tests/cases/022_hashtable_array.asm`) are six
  small `DEFINE`s: a fixed-size `ARRAY` bucket table, each bucket
  a short `CONS`-built alist chain, `HASH-CODE`+`MOD` picking the
  bucket, `STORE` mutating that one bucket slot on `HT-SET!`. Only
  the four primitives above are new kernel surface; the hashing/
  bucketing/chaining *policy* is all library code, exactly like the
  persistent alist version it replaces
  (`tests/cases/020_hashtable.asm`, kept as-is: a smaller, purely
  functional alternative when mutation isn't wanted). No resizing (a
  fixed 61-bucket table — a real implementation would grow it as load
  increases); no key removal; key comparison is `EQ`, which (see
  "KERNEL.md conformance" below) is now value equality for strings,
  floats, fixnums, characters, and symbols — string keys work correctly
  now, not just symbol/fixnum ones — but a *list*-shaped key still needs
  `EQUAL`-style structural equality this kernel doesn't have.

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
  requiring the reference's hardcoded `NIL`. `EQ` on fixnums/characters/
  symbols was always correct — those tags *are* the value, so a bare
  tagged-word compare (`compile_eq`'s original body) already gives value
  equality for free — but `EQ` on strings and floats was silently wrong
  in the same way: two independently heap-allocated `HDR_STRING`/
  `HDR_FLOAT` objects holding identical content compared unequal, since
  the compare was still just the two heap addresses. Part IV requires
  value equality there too, so `compile_eq` now delegates to a new
  `lisp_eq` host routine (`strings.asm`) via `compile_binary_hostcall`:
  a fast pointer-equal exit, then (only for two `TAG_HEAPOBJ` operands of
  the same header) a length/byte-for-byte compare for strings or a call
  to `float_eq_exact` (`floats.asm`) for floats — the latter is IEEE `==`
  plus the spec's own explicit carve-out that `NaN` is `EQ` to `NaN`, so
  `(eq 0.0 -0.0)` and `(eq (F/ 0.0 0.0) (F/ 0.0 0.0))` are both `T`
  despite plain `==` disagreeing with `EQ` on the latter
  (`tests/cases/044_eq_value_equality.asm`). A string and a float (or
  either against a fixnum) are never `EQ` regardless of content —
  different `HDR_*` tags fail before any value compare runs. This bug
  was found by, and this fix was required for, `EQUAL` (see "The
  prelude" below): `EQUAL`'s recursion bottoms out at `EQ` on atoms, so
  `examples/fizzbuzz/main.lisp`'s own self-check — which builds two
  independent lists of strings and compares them with `EQUAL` — reliably
  found unequal what should have been equal until this fix; fizzbuzz's
  full example, self-check included, now runs unmodified to `OK`/exit 0.
  `MOD`/`REMAINDER` now
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
  typed clause) match the spec exactly. `ERRORSET` now matches the
  spec exactly too, now that `EVAL` exists (see "The kernel surface"
  below): `(errorset '(car 5))`'s protected form is compiled and run to
  get a *value* (ordinarily the quoted list it evaluates to) and that
  value is itself then run as code via `eval_form`, the second step the
  spec requires and this kernel previously had no way to take.
  **`CAR`/`CDR` on an argument that is neither a cons nor `NIL` now
  signal a real, catchable condition too** — `(errorset '(car 5))`
  genuinely catches it and returns `NIL` now, instead of segfaulting on
  an out-of-bounds dereference (`tests/cases/047_car_cdr_type_errors.asm`).
  This is the first *native* (non-`ERROR`-call) failure this kernel
  signals as a condition: `car`/`cdr` (`reader.asm`) now check the tag
  before dereferencing, and on mismatch tail-call a new
  `fail_wrong_type` host routine (`native_errors.asm`) that builds an
  ordinary two-field condition — message text plus the culprit value
  itself as `data` — and throws it to the shared `handler_case_tag()`
  via a new `native_throw(tag, value)` routine, the exact same
  catch-stack search/restore/jump `compile_throw`'s own *generated*
  code performs for a Lisp-level `(THROW ...)`, just factored out once
  as an ordinary callable subroutine so native code can reach the same
  machinery instead of needing a second, parallel failure path. v0
  scope, narrower than the reference on purpose: the message text is a
  fixed string per caller ("CAR: expected a cons or NIL"), not the
  reference's own interpolated "CAR: expected a list, got 5" —
  `ERROR-DATA` still exposes the actual culprit value, just not folded
  into the message text. Division by zero, index out of range, wrong
  arity, and unbound-variable reads are still unchanged — those still
  misbehave exactly as before (division by zero and out-of-range array
  access still segfault or produce garbage; calling with the wrong
  arity is still unchecked; an unbound-variable *read* still returns
  the `IMM_UNBOUND` immediate rather than erroring) — only explicit
  `ERROR` calls and now `CAR`/`CDR`'s own type check go through this
  system so far.
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
- **Fixnum overflow is now observable** (Part V / Part XII axis 1):
  `+`/`-` set a global `OVERFLOW` flag on wraparound, queried with
  `FLAG-SET-P` and cleared with `CLEAR-FLAG`/`CLEAR-ALL-FLAGS` — see
  "The kernel surface" above. `*` does not set it yet (same section).
- **`QUASIQUOTE`/`UNQUOTE`/`UNQUOTE-SPLICING` now exist** (`file_runner_prelude`'s
  own coverage in `tests/run.sh`, `lib/prelude.lisp`), via the standard
  technique: a `DEFMACRO` (`QQ-EXPAND`) whose transformer walks the
  unevaluated template at macro-expansion time and emits `CONS`/
  `APPEND`/`QUOTE` code that rebuilds it at runtime, evaluating each
  `UNQUOTE`'d subform in the caller's own environment when that
  generated code actually runs — no new kernel primitive beyond the
  reader's three new macro characters (`` ` ``/`,`/`,@`, `reader.asm`,
  reusing the exact mechanism `'` already had). Matches KERNEL.md's own
  "no nesting-level tracking" rule exactly, verified against the
  spec's own worked example: `` `(a `(b ,(+ 1 2))) `` produces
  `(A (QUASIQUOTE (B 3)))`, since the recursive expander never treats
  an inner `` ` `` specially — it's just an ordinary symbol in the
  template — so an inner `,` is found and substituted at the *outer*
  level regardless of how many `(QUASIQUOTE ...)` conses structurally
  surround it. `UNQUOTE-SPLICING` splices correctly at any list
  position, including immediately before a dotted tail (``(1 ,@ys . 2)``).
  **`FOR` now also exists** (`file_runner_prelude`'s own coverage in
  `tests/run.sh`), derived from `LET`/`WHILE`/tail-calls exactly the
  way Part XII axis 3 explicitly licenses (the reference's own doc
  comment says as much): `(for (var start end [step]) body...)`
  evaluates `start`/`end`/`step` once into a single `LET` frame (`step`
  defaults to `1` when omitted, and a zero `step` signals an ordinary
  `ERROR` — `(handler-case (for (i 1 5 0) ...) ...)` catches it),
  `WHILE` re-tests a direction-aware inclusive-bound predicate
  (counting up continues while `var <= end`, counting down while
  `end <= var`), and the body's own trailing `SETQ` mutates `var` *in
  place* rather than rebinding it on each pass — precisely what gives
  "one reused frame every closure in the body shares" (the spec's own
  exact phrase) for free, the same mechanism `DOTIMES` already relies
  on. `GENSYM` (not a fixed internal name) for the end/step bindings
  means a nested `FOR` can't collide with an outer one's, the same
  hygiene fix `DOTIMES` itself needed. **`UNWIND-PROTECT` now also
  exists** (`tests/cases/052_unwind_protect.asm`), and needed exactly
  the real `CATCH`/`THROW`-unwind integration a previous version of
  this README named as the reason it couldn't be another simple
  derivation the way `BLOCK`/`HANDLER-CASE` were: `compile_throw` and
  `emit_throw_baked` (ERROR's own signaling path) were first
  refactored to both funnel through one shared host routine,
  `native_throw` (`native_errors.asm` — previously only native
  failures like `CAR`/`CDR`'s wrong-type check used it, while THROW/
  ERROR each hand-encoded their own separate copy of the identical
  search loop as target machine code), so there is exactly one place
  a passing throw's search can notice something. `UNWIND-PROTECT`
  installs a "marker" frame on the same catch stack (reusing the
  32-byte slot shape, but frame[8] holds the cleanup closure where an
  ordinary frame keeps its saved rbp) tagged with a fresh shared
  `unwind_protect_marker_tag()` — the same one-shared-tag trick
  `handler_case_tag` already uses. `native_throw`'s search now checks
  every frame it passes for this tag: a match fires that frame's
  cleanup closure (`invoke_thunk`, a tail-call into the closure's own
  code pointer with 0 args) and permanently shrinks `catch_stack_top`
  past it *before* the call, so a `CATCH`/`THROW` the cleanup performs
  on its own can never re-observe or re-fire the very marker it's
  nested inside — then the search continues below it. Verified
  directly: a `THROW` passing straight through an `UNWIND-PROTECT`
  that never catches it still fires the cleanup before the enclosing
  `CATCH` delivers its value; nested `UNWIND-PROTECT`s unwind in LIFO
  order; the body's own value is captured *before* cleanup runs, even
  when cleanup mutates the same variable the body just read; and a
  native failure (`CAR` on a non-cons) passing through fires the
  cleanup exactly the same way a Lisp-level `THROW` does. **v0 scope,
  narrower than the spec on purpose**: "an error raised by a cleanup
  form is discarded" is not yet true — a cleanup form's own uncaught
  condition propagates as an ordinary new `native_throw` search, which
  can end up superseding whichever throw was already being processed,
  rather than being silently swallowed so the original throw
  continues; implementing that exactly needs the cleanup invocation
  itself wrapped in a synthetic innermost catch on `handler_case_tag()`
  — a real next step, deliberately not attempted here to keep this
  change's blast radius to the primary, spec-critical guarantee
  (cleanup runs on every exit path, full stop). **Capability gating
  (Part IX) now also exists** (`src/capabilities.asm`,
  `file_runner_prelude`'s own coverage in `tests/run.sh`): a standing
  grant plus a dynamic-extent attenuation mask, matching the spec's own
  two-layer model — "per-thread" collapses to plain global state here,
  a documented simplification (this is a single-threaded freestanding
  binary with no environment-as-value yet to scope it to instead). All
  twelve capability names from the spec (`READ-FS` through
  `OS-SIGNAL`) are recognized by `FEATURE-ENABLED-P`/
  `CAPABILITY-MASK-ALLOWS-P`, but only `READ-FS`/`CREATE-FS` are
  actually *enforced* anywhere, since `FD-OPEN` (`fileio.asm`) is the
  only gated primitive this kernel has — mode 0 (read) requires
  `READ-FS`, modes 1/2 (write/append) require `CREATE-FS`, checked at
  the call site itself (not mere bookkeeping, per the spec's own
  explicit requirement) via a new `require_capability` routine that
  signals a genuine catchable condition (the same `fail_wrong_type`/
  `native_throw` machinery every other native failure here uses) on
  denial, not a crash or silent no-op. The standing grant defaults to
  *all* capabilities, matching the reference CLI's own default
  (`AGENTS.md`: "enables all capabilities by default"); there is no
  `--sandbox` flag or Lisp-callable way to narrow it yet (also matching
  spec: "There is no Lisp-callable way to add to it" — this kernel
  simply has no way to *subtract* from it yet either). `WITH-CAPABILITIES`
  is a two-line `DEFMACRO` (`lib/prelude.lisp`) over `UNWIND-PROTECT` —
  the exact "restore the previous mask on exit by any path" guarantee
  the spec requires is exactly what `UNWIND-PROTECT` (just added,
  above) already provides, so this needed no new special-form
  machinery at all, only two small primitives
  (`PUSH-CAPABILITY-MASK!`/`POP-CAPABILITY-MASK!`) that intersect-or-
  replace the mask and save/restore it. Verified directly: nested
  `WITH-CAPABILITIES` fences intersect (never union — asking for
  `SHELL` inside a `READ-FS`-only fence leaves nothing allowed); the
  mask restores exactly on normal completion *and* on a `THROW` passing
  through; and denying `FD-OPEN` inside a fence that excludes `READ-FS`
  is genuinely catchable via `HANDLER-CASE`. **v0 scope**: only symbol
  arguments are supported for capability names, not the spec's own
  "symbol or string" (a string is treated as unrecognized rather than
  case-folded and matched); an unrecognized name inside a
  `WITH-CAPABILITIES` list is silently skipped rather than signaling
  the spec's own "a non-symbol is an error"; fuel (Part X) doesn't
  exist yet; proper tail calls
  (Part VI) aren't implemented (see v0 limits below); the array
  primitive names now match Part XI/IV exactly (`ARRAY`/`FETCH`/
  `STORE`/`ARRAY-LENGTH*`, no longer `MAKE-ARRAY`/`ARRAY-REF`/
  `ARRAY-SET`/`ARRAY-LENGTH`); **the hash table now also uses Part XI's
  required exact names** — `MAKE-HASH-TABLE`/`SETHASH`/`GETHASH`/
  `REMHASH`/`KEYS`, promoted from `tests/cases/022_hashtable_array.asm`'s
  own demonstration into real `lib/prelude.lisp` library code
  (`file_runner_prelude`'s own coverage in `tests/run.sh`), with the
  exact arities and return values the spec requires: `(make-hash-table)`
  takes no arguments, `sethash`/`remhash` return `T`, `gethash` returns
  `NIL` for an absent key. No new kernel primitive needed — `ARRAY`/
  `FETCH`/`STORE`/`HASH-CODE`/`MOD` plus ordinary `CONS`/`CAR`/`CDR`/`EQ`
  build a fixed 61-bucket array of alist chains, same design as the
  test it promotes. Writing this exposed one real, previously-latent
  kernel bug: `HASH-CODE` (`hash_code_tagged`, `arrays.asm`) hashed
  strings and floats by their own heap *address*, not content — so two
  separately-built, `EQ`-equal (content-equal, per this session's
  earlier `lisp_eq` fix) strings or floats could land in *different*
  buckets, making a value genuinely stored under an equal key
  unfindable. `HASH-CODE` must agree with `EQ` for a hash table to work
  at all, so it now hashes a string's actual bytes (`fnv1a_hash`,
  `symtab.asm`, exported and reused rather than duplicated) and a
  float's raw bit pattern, normalized first so it agrees with
  `float_eq_exact`'s own rules: every `NaN` bit pattern folds to one
  canonical value (`NaN` is `EQ` to `NaN` regardless of payload bits),
  and `-0.0`'s bits fold to `+0.0`'s (the two are `EQ` per IEEE `==`) —
  every other value's sign bit stays significant, since a genuinely
  different signed number must not collapse into its negation's
  bucket. Every other heapobj (symbols, closures, arrays — pointer
  identity is the only `EQ` rule for these) still hashes by its own
  stable heap address, unaffected. **v0 scope, narrower than the spec
  on purpose**: key equality is `EQ`, not the reference's own full
  recursive `EQ`/`EQUAL` union, so a cons/array/lambda-shaped key does
  not yet find itself by structural content; no resizing (fixed 61
  buckets, same as the demonstration this promotes). **A `Char` type
  now exists** (`tests/cases/048_char_literals.asm`): a code point in
  `0..=255`, packed directly into the tagged immediate word (like
  `NIL`/`T`/`UNBOUND`/`EOF` in `tags.inc`, not heap-allocated — a
  `Char`'s enumeration index is `256+code`, completely disjoint from
  the existing `IMM_NIL..IMM_EOF` set), so `EQ` on two `Char`s is
  already correct for free via `lisp_eq`'s own fast pointer-equal path
  (two equal `Char`s are the identical tagged word), and `(eq 'a' 97)`
  is `NIL` — a `Char` and a `Number` are never `EQ` regardless of
  numeric value, matching Part IV exactly, even though `CHAR-CODE`
  deliberately bridges the two. The reader's `'x'` production
  (`chars.asm`/`reader.asm`) is tried *before* quote sugar, matching
  the spec's own disambiguation rule and its two contrasting examples
  exactly: `'a'` is the character `a`, `'a` followed by a delimiter is
  `(QUOTE A)`, `'(1)` is `(QUOTE (1))` (the `(` is followed by `1`, not
  a closing `'`), and `'('` is the character `(`. All six recognized
  escapes (`\n` `\t` `\r` `\\` `\'` `\0`) decode correctly, and — the
  opposite convention from string escapes — any other backslash-
  prefixed byte decodes to that byte alone, backslash dropped
  (`'\q'` is `'q'`). `MAKE-CHAR`/`CHAR-CODE`/`CODE-CHAR` are new kernel
  primitives (`compile_unary_hostcall`, the same shape `HASH-CODE`
  already used); `MAKE-CHAR` on a fixnum outside `0..255` is a genuine,
  catchable condition (`fail_wrong_type`/`native_throw`, matching this
  session's other native-failure work) rather than silently wrapping
  or crashing; `CODE-CHAR` returns a one-character *string*, not a
  `Char` — an asymmetric pair the spec states outright, not an
  inconsistency this kernel introduced. **`+`/`-`/`*`/`<`/`=` now
  coerce a `Char` operand to its code point** (`tests/cases/055_char_contagion.asm`),
  Part V's own contagion rule — `emit_coerce_char_in_rax`
  (`compiler.asm`) runs right after `compile_binop` compiles each
  operand, replacing a tagged `Char` in place with a tagged fixnum
  holding its code point (or leaving anything else — an ordinary
  fixnum, a symbol, a cons — completely unchanged), so `(+ 'a' 1)` is
  `98` and `(= 'a' 97)` is `T` even though `(eq 'a' 97)` correctly
  stays `NIL` (`EQ`'s own path never calls this — contagion is
  specifically an arithmetic/comparison rule, not an identity one).
  A first draft of this check shipped a real bug before it reached
  `make test`: the three "this isn't actually a Char" exit paths each
  left `rax` holding an *intermediate scratch value* (tag bits, or a
  half-computed range check) rather than restoring the operand's
  original value, silently corrupting every ordinary non-`Char`
  arithmetic operand — caught immediately by the existing regression
  suite (`043_function_sharp_quote` and others going from correct
  results to `0`/negative garbage), not shipped; fixed by giving the
  three failure sites their own explicit `rax = rcx` (the saved
  original) restore stub, with the success path jumping clean over it.
  `CHAR-CODE` only accepts a `Char`
  argument, not the reference's own "or a non-empty string's first
  code point" overload; `CODE-CHAR`/`MAKE-CHAR` are restricted to
  `0..255` (one byte) rather than the reference's full Unicode code
  point range, since this kernel's strings are plain byte buffers with
  no UTF-8 encoder yet — there is still no Unicode-codepoint string
  indexing, no environments-as-values. **Typed arrays now also
  exist** (`tests/cases/054_typed_arrays.asm`): `(typed-array n
  elem-type)` — `n` as for `ARRAY`, `elem-type` exactly the symbol
  `INT64` or `FLOAT64` — a new `HDR_TYPED_ARRAY` heapobj
  (`[8]=len [16]=elem-type [24..]=raw untagged int64/double slots`,
  `tags.inc`) storing native machine words instead of tagged values.
  `FETCH`/`STORE`/`ARRAY-LENGTH*` are one polymorphic primitive over
  both plain and typed arrays, not new names, matching Part XI exactly
  — `array_ref`/`array_set` (`arrays.asm`) check the header and
  dispatch: `FETCH` re-tags an `INT64` slot's raw word as a fixnum or
  boxes a `FLOAT64` slot's raw bits fresh via `make_float` on every
  read ("reading always yields the declared type"); `STORE` validates
  per the spec's own narrower-than-arithmetic-coercion rule — an
  `INT64` array accepts only a fixnum (a `Float` or `Char` is a
  catchable wrong-type condition, `fail_wrong_type`/`native_throw`);
  a `FLOAT64` array accepts a `Float` as-is or a fixnum converted to
  the nearest `f64`, and also rejects a `Char` — deliberately *not*
  coerced to its code point here, unlike Part V's own arithmetic
  contagion. Slots are zero-initialized (`0`/`0.0`, the same raw bit
  pattern either way). `PRINT` emits the spec's own distinct opaque
  tag, `<typed-array:int64:N>` / `<typed-array:float64:N>`, not
  reusing plain `ARRAY`'s `<array:N>`. **v0 scope**: no bounds
  checking — the same divergence plain `ARRAY` already has (see v0
  limits below), not a new one. **Symbol
  property lists (`GETP`/`PUTP`) now exist**
  (`tests/cases/046_plist.asm`, plus `file_runner_prelude`'s own GETP/
  PUTP coverage): every symbol gained a fourth mutable slot, `plist`
  (offset 40, shifting name bytes from 40 to 48 — `symtab.asm`'s own
  layout comment is the source of truth), read and written by two new
  kernel primitives, `SYMBOL-PLIST` (a `compile_unary_hostcall`) and
  `SET-SYMBOL-PLIST!` (a `compile_binary_hostcall`); `GETP`/`PUTP`
  themselves are ordinary `lib/prelude.lisp` library code over those
  two plus `CONS`/`CAR`/`CDR`/`EQ` — an ordinary `CONS`-built alist of
  `(indicator . value)` pairs, `PUTP` always prepending a fresh pair
  rather than searching for and replacing one (cons cells stay
  immutable regardless — Part XII axis 2 — and `GETP`'s first-match
  walk order means a later `PUTP` for the same indicator correctly
  shadows an earlier one without needing in-place update). **v0 scope,
  narrower than the reference on purpose**: indicator equality here is
  `EQ`, not the reference's own name-text unification (`environment.rs`
  extracts a symbol-or-string indicator's name text and keys a
  per-symbol map by that text, so a symbol indicator and a string
  indicator spelling the same name are the same property there); this
  kernel's `EQ` is genuine value equality on strings now (see above),
  so two string indicators with identical text already unify, and two
  symbol indicators of the same name already unify (interning), but a
  *symbol* indicator and a *string* indicator sharing text do not
  unify with each other here — an honest gap until a `SYMBOL-NAME`
  primitive exists to key on name text the same way the reference
  does. `EVAL`, `READ-FROM-STRING`, and
  `PRINC-TO-STRING` now exist (`tests/cases/037_eval_read_princ.asm`) —
  Part XI's own reflection primitives, and the single highest-leverage
  addition per issue #452: `EVAL` is `compile_thunk` (already existed,
  for the driver's own use) plus one indirect call, exposed to
  *compiled* Lamedh code for the first time. **`GENSYM`
  (`tests/cases/045_gensym.asm`) now also exists** — a new nullary
  compiler form (`compile_nullary_hostcall`, the same shape
  `CLEAR-ALL-FLAGS` already used) reaching a new host routine in
  `symtab.asm` that builds an ordinary `HDR_SYMBOL` object exactly like
  `intern_symbol` does, except it is never linked into
  `symtab_buckets` — so no name, however constructed, can ever look it
  up again, and it is correctly not `EQ` to anything, including an
  ordinary reader-interned symbol that happens to print with the exact
  same text (`lisp_eq`, "KERNEL.md conformance" above, is pointer
  identity for two `HDR_SYMBOL` heapobjs, so two genuinely distinct
  objects are never confused regardless of what their name bytes say).
  Naming matches the Rust reference exactly: `"G"` plus a monotonic
  per-process counter, zero-padded to at least 4 digits
  (`environment.rs`'s own `format!("G{:04}", counter)`). This also
  fixed a real, previously-documented hygiene bug in the prelude's own
  `DOTIMES` (see "The prelude" below): its internal loop-count binding
  used one fixed literal name, so a `DOTIMES` nested inside another
  using that same name as its own loop variable would silently
  collide; it now binds a fresh `GENSYM`'d name on every expansion
  instead. **`RPLACA`/`RPLACD` now also exist**, needing no new kernel
  primitive at all: KERNEL.md Part XII axis 2 requires cons cells to
  stay immutable on every host, so both must return a *new* cons cell
  sharing the untouched half of the original rather than mutating in
  place — precisely what a plain `CONS` of the replaced half onto the
  untouched other half already gives, character for character, so
  they are two one-line `lib/prelude.lisp` `DEFUN`s over existing
  `CONS`/`CAR`/`CDR` (`file_runner_prelude` in `tests/run.sh` covers
  both, including that `(RPLACA pair 9)` with the result discarded
  leaves `pair` itself printing unchanged — proof it really is
  non-destructive, not merely undocumented). The printer has no
  cycle detection (unreachable anyway — cons cells are immutable here).
  `PRINT` now emits Part III's required opaque, non-readable tags for
  the two compound types this kernel has: `<lambda>` for a closure and
  `<array:N>` for an array (`tests/cases/030_print_opaque.asm`) — before
  this, either fell through to `print_fixnum`, which reinterprets a
  tagged heapobj pointer's raw bits as a signed fixnum and printed
  meaningless garbage instead of a tag; there is still no hash table
  primitive to tag (`<hash-table>`), since hash tables here are library
  code over `ARRAY`, not a distinct value type. **`PROG`/`GO`/`RETURN`
  (Part VII) now exist** (`file_runner_prelude`'s own PROG/GO/RETURN
  coverage in `tests/run.sh` — not a standalone `tests/cases/*.asm`
  case; see why below): `compile_prog`/`compile_go`/`compile_return`
  (`compiler.asm`) bind each variable in `(PROG (vars...) items...)` to
  `NIL` (reusing `compile_let`'s own frame-building machinery), then
  walk `items` once, treating a bare symbol item as a label rather than
  evaluating it — the well-known Lisp 1.5 `PROG` gotcha, kept
  deliberately rather than "fixed": `(PROG (X) (SETQ X 42) X)` yields
  `NIL`, not `42`, because the trailing `X` is consumed as a label, never
  compiled as an expression. `(GO label)` emits an unconditional jump
  recorded in a compile-time-only bookkeeping buffer (`current_prog_ctx`,
  a fixed-capacity scratch area allocated on the *compiler's own* host
  stack via `sub rsp` for the duration of `compile_prog`, not part of
  the target program's runtime state); every `GO` and `RETURN` is
  resolved in one final backpatching pass after all of a `PROG`'s own
  labels have been seen, so forward and backward jumps need no special
  cases (`(GO LOOP)` jumping backward to re-run the loop body, and
  `(GO SKIP)` jumping forward over code that must never run, are the
  same mechanism). `(RETURN value)` (value defaults to `NIL`) and
  falling off the end of the item list both converge on the same exit
  jump target. **v0 scope, narrower than the spec on purpose**: `GO`/
  `RETURN` here are *lexical*, resolved only within the same `PROG`'s
  own body (including when nested inside an ordinary `IF`/`WHEN`/`LET`
  there) — not the spec's own dynamic-extent version, reachable through
  an arbitrary function call away from the `PROG` that established it.
  `current_prog_ctx` is explicitly reset to `0` (not saved-and-restored)
  around a nested `LAMBDA`'s own body compilation, so a `GO`/`RETURN`
  inside a closure defined within a `PROG` correctly fails to resolve
  (traps, `int3`) rather than jumping into a different, already-returned
  invocation's code — deliberately narrower than dynamic extent to avoid
  unbounded catch-stack growth on an ordinary `GO`-based loop, unlike
  `UNWIND-PROTECT`'s one-shot marker frame (see "KERNEL.md conformance"
  above). A nested `PROG`'s own labels/`GO`/`RETURN` correctly do not
  interfere with an enclosing `PROG`'s, since each has its own
  `current_prog_ctx` buffer (saved and restored around the inner
  `compile_prog` call, the same nesting pattern `current_scope`/
  `current_frame_depth` already use). `GO`/`RETURN` used outside any
  lexically enclosing `PROG` is a v0 trap (`int3`), not a catchable
  condition — this kernel has no compile-time diagnostics surface yet
  (see Roadmap). One genuine test-authoring mistake, not a compiler bug,
  was found while adding coverage for this: a first-draft standalone
  `tests/cases/056_prog.asm` case used `WHEN` in its loop-exit test, and
  `WHEN` is a `lib/prelude.lisp` `DEFMACRO`, not a kernel special
  form — invisible to the standalone harness, which links only the
  shared core and never loads the prelude (the same category of mistake
  `QUASIQUOTE`'s own test made earlier this session, see "The prelude"
  below); every backward-`GO`-loop scenario in that file compiled `WHEN`
  as a call to an unbound global instead of a conditional and printed
  `0`, while the two scenarios without `WHEN` passed, which is what
  correctly pointed at `WHEN` rather than at `compile_prog` itself
  (confirmed by piping the identical form through `build/lamedhc`, which
  does load the prelude, and getting the correct result). Moved to
  `file_runner_prelude` instead, covering a backward-counting loop, the
  trailing-bare-symbol-is-a-label gotcha, a sum loop (proving `RETURN`'s
  value is delivered correctly), a forward `GO` (jumping past code that
  must never run), and a nested `PROG` whose own labels don't interfere
  with the outer one's. This list is
  deliberately specific so it can shrink honestly, item by item, rather
  than being replaced by a vaguer
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
- No garbage collector. No bignums or first-class conditions yet.
  Dynamic variables now exist (`DEFDYNAMIC`/dynamic-extent `LET` — see
  "KERNEL.md conformance" above) but only via plain `LET`, not `LET*`,
  and only when every binding in the `LET` is dynamic. `$VAU`
  operatives now exist too (see "KERNEL.md conformance" above) but
  `(eval x e)` on a call-site lexical evaluates in the wrong (global)
  scope rather than the caller's real one, since this kernel has no
  first-class environments; only recognized at a literal call-site
  head, not via `APPLY`/`FUNCALL`/indirect use.
- Arrays are fixed-size at creation (`ARRAY` doesn't grow/shrink)
  and `FETCH`/`STORE` do no bounds checking — an out-of-range
  index reads or writes adjacent heap memory rather than raising
  anything. `STORE` is also the only mutation primitive in this
  kernel; nothing else (a cons, a closure's captured values) can be
  rewritten after creation.
- The array-backed hash table (`HT-MAKE` et al.) is a fixed 61-bucket
  table with no resizing, no key removal, and `EQ`-only key comparison
  (symbol/fixnum/string/float keys all hash and compare correctly now
  that `EQ` is value equality on those — see "KERNEL.md conformance"
  below; a list-shaped key would still need `EQUAL`-style structural
  equality this kernel doesn't have).
- Strings are immutable byte buffers: reader literals, `STRING-LENGTH`,
  `PRINT`, and now `STRING-REF`/`STRING-APPEND`/`SUBSTRING`
  (`tests/cases/036_string_ops.asm`) — the small extra surface `FORMAT`
  needs (README Roadmap). `STRING-REF` returns a byte's numeric value
  as a fixnum, not a `Char` — a `Char` type exists now (see "KERNEL.md
  conformance" above) but `STRING-REF` isn't wired to return one yet,
  so a byte's own numeric value remains the interim answer here;
  `STRING-APPEND` and `SUBSTRING` return fresh strings,
  neither argument mutated, matching every other value's immutability
  here. All three are byte-indexed, not Unicode-scalar-indexed — Part
  IV's own indexing rule remains unmet, tracked as ongoing work below.
  Escapes are limited to `\n`, `\t`, `\"`, `\\` (anything else after a
  backslash is copied through literally); a literal longer than the
  reader's 4KB scratch buffer is silently truncated.
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
- `&REST` parameters are supported for any fixed-parameter count,
  including 0, 1, or 2 (`(LAMBDA (&REST ALL) ...)` and
  `(LAMBDA (A &REST MORE) ...)` now work, not only nfixed>=3) —
  `tests/cases/033_rest_below3.asm`. The calling convention always
  places global argument indices 0/1/2 in rsi/rdx/rcx no matter how a
  given callee splits fixed vs. REST, so for nfixed<3 some REST
  elements are register-resident rather than stack-resident;
  `compile_lambda`'s prologue now always spills all 3 argument
  registers whenever a REST param exists (not just the first nfixed of
  them) and folds whichever of slots `[nfixed,2]` a call actually
  supplied onto the front of the stack-built tail, each checked against
  the real argument count at runtime independently, since those three
  slots don't form one contiguous runtime-counted range the way stack
  arguments do.
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
actually exist. `&REST` is supported for any fixed-parameter count
(`split_rest_params` splits `(A B C &REST MORE)` into the fixed list
`(A B C)` and the rest symbol `MORE` at `LAMBDA`-compile time, the same
way regardless of how many fixed params precede `&REST`). Whenever a
`&REST` param exists, the prologue spills all 3 argument registers
unconditionally, not just the fixed ones — global argument index 0/1/2
always arrives in `rsi`/`rdx`/`rcx` regardless of a given callee's own
fixed/REST split, so for `nfixed<3` a register can hold REST data
rather than a fixed parameter's value — and reserves 4 local slots
(3 register slots plus the `&REST` slot itself, always at `[rbp-32]`)
rather than `min(nfixed,3)+1`, so the REST slot never collides with a
register-resident REST element. Building the list has two parts: the
compiled prologue first walks the stack-passed tail (global index>=3)
from the last actual argument down to `max(nfixed,3)`, consing each
onto an accumulator (right-to-left, so the final list comes out in
left-to-right order); it then folds whichever of the register slots
`[nfixed,2]` the call actually supplied onto the *front* of that
accumulator, processed from index 2 down to `nfixed` so each cons lands
in the right place, each one gated by comparing the real argument count
(also passed in `rax`, stashed in the REST slot until this point) against
that slot's index at runtime — unlike the stack walk's single running
index, these three slots don't form one contiguous runtime-counted
range, so each needs its own presence check.

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
make lamedhc   # builds build/lamedhc, the real file/stdin driver
make clean
```

No Cargo project lives here (deliberately — this directory has no
`Cargo.toml` and is invisible to the workspace at the repo root).
`nasm` and `ld` are the only required tools.

`tests/run.sh` assembles the shared core once, then for each
`tests/cases/NAME.asm` (which defines `global lamedh_main`) links it
against `boot.asm` + the core and diffs the resulting binary's stdout
and exit code against `tests/cases/NAME.expected` / `.exitcode`
(exit code defaults to 0 if the `.exitcode` file is absent). It also
builds `build/lamedhc` (`src/file_runner.asm`) the same way and checks
it against a small generated `.lisp` file, both as a path argument and
piped over stdin, plus a missing-file exit code — the only case here
that isn't a `tests/cases/*.asm` file, since it's exercising the driver
itself rather than one compiled program.

```
build/lamedhc path/to/program.lisp   # run a file
producer | build/lamedhc             # or read a program from stdin
```

## The prelude

`lamedhc` runs `lib/prelude.lisp` before any user program — pulled
directly into the binary with nasm's `incbin` (`file_runner.asm`), not
looked up on disk: there is no filesystem convention, install location,
or argv-relative path resolution to invent for a freestanding, no-libc
project, so the prelude is compiled in exactly the way any other
literal datum in this compiler is, and runs through the identical
`reader_init`/`read_form`/`compile_thunk` loop the user's own source
does, just from an in-memory buffer instead of an mmap'd file.

It currently defines `DEFUN`, `NOT`, `WHEN`, `UNLESS`, `LIST`,
`REVERSE`, `FORMAT`, `1+`/`1-`, real global closures for
`+`/`-`/`*`/`</`=`, `APPEND`, `IOTA`, `REDUCE`, `DOTIMES`, `EQUAL`,
`MAPCAR`, `NUMBER->STRING`, `GETP`, `PUTP`, `RPLACA`, `RPLACD`, `DEF`,
`FUNCALL`, `>`/`>=`/`<=`, `MAX`/`MIN`, `FOR-EACH`, `FILTER`, `SOME`,
`EVERY`, `MAKE-HASH-TABLE`, `SETHASH`, `GETHASH`, `REMHASH`, `KEYS`,
`QUASIQUOTE` (with its own `UNQUOTE`/`UNQUOTE-SPLICING` reader
support), and `FOR` — nearly all of these are an ordinary `DEFMACRO`/`DEFUN`
over kernel primitives, no compiler change needed (see
`lib/prelude.lisp`'s own comments for exactly why; `DOTIMES` is
derived from `LET`/`WHILE`/`SETQ`, per KERNEL.md Part XII axis 3's
explicit license, and now hygienically `GENSYM`s its internal
loop-count binding — see "KERNEL.md conformance" above for why an
earlier fixed-name version could collide on nesting). The one
exception is `FUNCALL`, which needed one new kernel primitive,
`APPLY` (`compiler.asm`): `(APPLY fn args-list)` turned out to be
exactly the job `invoke_macro` (see "The kernel surface" above) was
already doing for macro expansion — given a closure and a raw list, it
collects the list's own elements as argument *values* into the same
register/stack layout an ordinary compiled call site would produce,
then calls the closure with the right `nargs`. It never distinguishes
"already-evaluated argument values" (what `APPLY` needs) from
"unevaluated macro-call operand forms" (what it was built for), since
it never compiles or evaluates anything itself either way — so `APPLY`
is just `compile_binary_hostcall` wired straight to `invoke_macro`,
and `FUNCALL` itself is then one line of prelude:
`(DEFUN FUNCALL (FN &REST ARGS) (APPLY FN ARGS))`. `DEF` is the
reference's own alternate top-level binding form (`SpecialForm::Def`,
`../src/evaluator/special_forms.rs`) — like `DEFINE` but evaluating to
the *symbol* rather than its value, matching `../examples/*/main.lisp`'s
own `(def $name expr)` idiom; v0 supports only the reference's 2-operand
form, not its optional third (docstring) operand. `FOR-EACH`/`FILTER`/
`SOME`/`EVERY` are deliberately narrower than the reference's own
versions (`lib/29-protocols.lisp`), which are fn-first *protocols*
generically dispatching over lists, arrays, hash tables, and strings
alike via `DEFPROTOCOL`/`DEFINSTANCE` — a full multi-type dispatch
system this kernel doesn't have yet; these are plain recursive
list-only versions, and `SOME`/`EVERY` simplify the reference's own
contract (returning the matching element, or the last predicate
result) down to a plain `T`/`NIL`.
`DEFUN` and `WHEN`/`UNLESS` take any
number of body forms directly (`(NAME PARAMS &REST BODY)`, the same way
`LAMBDA`'s own body already does) — `invoke_macro` (see "The kernel
surface" above) forwards every operand at a macro call site, not just
the first 3, so this no longer needs the single-body-form workaround an
earlier version of this prelude required. `FORMAT` walks its control
string with `STRING-REF` at macro-expansion time, recognizing only
`~a`/`~%` (v0 — see `lib/prelude.lisp`'s own comments for the exact
scope, including that the `STREAM` argument is currently accepted but
ignored — no `(format nil ...)` support yet).

**`examples/factorial/main.lisp`'s own `(dotimes (i 10) (format t "~a!
= ~a~%" (1+ i) (factorial (1+ i))))` now runs correctly end to end**,
printing all ten factorials exactly as the reference does — the
concrete conformance target's first real win. Its self-check does not
pass unmodified: `(factorial 20)` is `2432902008176640000`, which fits
the reference's 64-bit `i64` fixnum but exceeds this kernel's 62-bit
one (the low 2 bits are the tag — see "Value representation" above),
so it silently wraps (setting `OVERFLOW`, per the fixnum overflow work
above) to a different value, and the self-check's exact-equality
assertion fails, hitting an unmatched `THROW` (`(error ...)` with no
enclosing `HANDLER-CASE`) that traps rather than passing. This is a
genuine, honest representational difference this kernel's own tagged
fixnum makes (Part XII axis 1 explicitly allows a host's fixed-width
model to differ from another's), not a bug to chase — `factorial-fold`
(`REDUCE`/`IOTA`/`#'*`) agrees with the direct recursive `factorial` on
every value that fits, confirming `REDUCE`/`IOTA`/`FUNCTION` are all
correct; only the specific `20!` boundary the reference's wider fixnum
happens to still fit is where the two hosts diverge.

`FUNCTION`/`#'` (`compiler.asm`'s dispatch, plus a new `#'` reader
macro in `reader.asm` mirroring `'`'s own construction) compiles its
operand directly, exactly like any other expression position already
would — a bare symbol is the ordinary local-or-global variable read
`compile_form`'s own atom case already does (there is no separate
function namespace in this kernel), and `#'(LAMBDA ...)` is an ordinary
`LAMBDA`. This is what makes `+`/`-`/`*`/`<`/`=` needing *real* global
closures (not just their existing fast inline-operator forms, which
only trigger when one of them is the head of a list) visible at all:
`(REDUCE #'* ...)` needs a callable *value* to pass, which nothing
before this bound the bare symbol `*` to.

The reader also gained the `1+`/`1-` two-character literal symbol
production (KERNEL.md Part II: tried before ordinary number parsing,
no boundary guard) — `examples/factorial/main.lisp` uses `(1+ i)`
directly, and without this, `1+` read as the number `1` followed by
the symbol `+`, splitting a call like `(1+ i)` into two malformed forms.

Writing this much prelude surfaced five real, previously-latent
bugs no existing test had exercised:

- **The bare symbol `T`, evaluated as a variable, was unbound.**
  `T` is not a keyword and not the `IMM_TRUE` immediate `EQ`/comparisons
  return (a genuinely different value) — it is an ordinary
  reader-interned symbol, and per KERNEL.md Part IV must be "bound to
  itself in the global environment." Nothing in this kernel ever did
  that binding; every existing test that used `T` did so only via
  `EQ`/comparison results or as quoted data, never as a bare evaluated
  variable, so the gap was invisible until `lib/prelude.lisp`'s own
  `NOT` returned bare `T` directly. The observed failure was exactly
  as ugly as an unbound-variable bug gets: `PRINT` of the unbound
  symbol's `IMM_UNBOUND` cell fell through to `print_fixnum`, printing
  the fixnum `3`. `bootstrap_globals` (`symtab.asm`, called once from
  `boot.asm` right after the heaps are set up, before *any* Lisp code
  — the prelude included — runs) fixes this for every binary this
  project builds, tests included, not just `lamedhc`
  (`tests/cases/039_t_self_bound.asm`).
- **`invoke_closure_host` never set the incoming argument count**, and
  **`raw_args_to_regs` only forwarded the first 3 operand forms.**
  Both documented above under `DEFMACRO`; `tests/cases/
  038_macro_rest_below3.asm` and `040_macro_many_args.asm` are the
  regression tests.
- **A call with more than 3 arguments never cleaned up its own
  stack-passed extra arguments after returning.** `compile_call_args`
  leaves argument index 3 onward on the *target* stack, positioned for
  the callee's own `build_param_frame`-addressed parameters; a callee's
  `leave`/`ret` only unwinds what it pushed after its own `push rbp`,
  never those caller-pushed extras sitting below the return address —
  cleaning them up is the *caller*'s job, and neither of `compile_call`'s
  two paths did it. Invisible as long as a `>3`-arg call's result was
  used immediately; corrupting the moment the *enclosing* expression had
  already pushed something of its own onto the stack around the call —
  `compile_binop`'s own lhs, a `compile_binary_hostcall`'s arg1, and so
  on — since the extra bytes were still sitting in the slot the caller
  expected to pop its own saved value back from.
  `(CONS 'X (F a b c d))` for any 4+-arg `F` silently returned garbage
  instead of `X` as its `car` — `lib/prelude.lisp`'s own `FORMAT` macro
  (whose expansion is exactly a `CONS` of a literal `PROGN` onto a
  multi-argument call chain) is what surfaced this for real. Fixed by
  emitting `add rsp, (nargs-3)*8` (via `emit_sub_rsp_imm32` with a
  negative immediate) right after the call returns, on both paths —
  `nargs` is a compile-time constant per call site, so the cleanup
  amount is too. `tests/cases/041_call_stack_cleanup.asm` is the
  regression test.
- **The first draft of the `1+`/`1-` reader fix itself introduced a
  bug**, caught before it reached a commit: the one-character lookahead
  used to decide "is this `1+`/`1-` or an ordinary number" was loaded
  into `al` — the same register still holding the *original* first
  character, which every subsequent classification check in
  `read_form` assumes untouched. A bare `"1"` immediately followed by a
  non-digit, non-`+`/`-` character (`)`, a space, end of input — i.e.
  most real occurrences of the digit `1` in actual source) got
  misclassified using the lookahead byte instead. Fixed by moving the
  lookahead into its own register (`r8`/`r8b`), leaving `al` untouched
  for the fallback path. `tests/cases/042_one_plus_minus.asm` explicitly
  covers plain leading-`1` numbers (`1`, `15`, `100`, a bare `1` at the
  end of a list) alongside `1+`/`1-` themselves for exactly this reason.
- **`EQ` on strings and floats was pointer equality, not the value
  equality Part IV requires.** `EQUAL`'s own base case is `EQ` on atoms,
  so two independently-read or independently-built strings with
  identical content — exactly what `examples/fizzbuzz/main.lisp`'s
  self-check constructs and compares — reliably came back unequal.
  Diagnosed down to the minimal case, `(EQ "hi" "hi")` returning `NIL`.
  Fixed by giving `compile_eq` a real value-equality host routine,
  `lisp_eq` (`strings.asm`), for the two-heapobj case, plus
  `float_eq_exact` (`floats.asm`) for the IEEE-`==`-plus-`NaN`-carve-out
  float rule Part IV also specifies — see "KERNEL.md conformance" above
  for the exact semantics and `tests/cases/044_eq_value_equality.asm`
  for the regression coverage. `examples/fizzbuzz/main.lisp` now runs
  to completion unmodified, self-check included (`OK`, exit 0) —
  previously it printed its Fizz/Buzz output correctly but then trapped
  via an unmatched `THROW` when its self-check's `EQUAL` comparison came
  back falsely unequal.

## Roadmap

- **A second, harder conformance target now underway: `../lib/*.lisp`
  (the Rust reference's own standard library) running unmodified
  through `build/lamedhc`, up through `29-protocols.lisp` in the
  reference's own load order (`src/lib.rs`'s `STDLIB_SOURCES`),
  excluding the networking/TLS/regex/OS tiers (`30-text.lisp` through
  `44-regex.lisp`) that need external protocols or OS surfaces this
  freestanding, no-libc host has no I/O primitives for at all — those
  are out of scope on the same grounds `../examples/*/main.lisp`
  network/regex/TLS examples already are (see below). This is a much
  higher bar than the example corpus: the reference stdlib is real,
  actively-maintained Lisp using every corner of the reference's own
  language (extended `&optional`/`&key` lambda lists, `vau`
  operatives, environments-as-values, HM-inferred JIT membranes), not
  example programs written against this project's own narrower v0
  surface. **`00-core.lisp` now loads completely, unmodified**
  (confirmed via `build/lamedhc ../lib/00-core.lisp`, exit 0) — the
  first and most load-bearing file, since literally every other
  reference stdlib file depends on the `defun`/`defmacro`/`def` it
  establishes. Getting there needed four small, genuinely new
  primitives, each traced by bisecting the file line-by-line against
  successive `int3`/segfault failures until the exact next missing
  symbol was identified (`tests/cases/056_stdlib_bootstrap_primitives.asm`
  covers the three kernel-level ones; `file_runner_prelude` in
  `tests/run.sh` covers the two prelude-level ones):
  - **`STRINGP`** (`compiler.asm`, `stringp_tagged` in `strings.asm`) —
    `00-core.lisp`'s own `defun` macro peels an optional leading
    docstring off a function body with `(stringp (car body))` on every
    single expansion.
  - **`BOUNDP`** (`compiler.asm`, `boundp_tagged` in `symtab.asm`,
    reading a symbol's own value slot and comparing it against
    `IMM_UNBOUND`) — the same macro guards its optional call-graph
    bookkeeping globals (`$cg-pending`, `$call-graph`) with
    `(if (boundp '$cg-pending) ...)`, which must not treat a plausibly
    still-undefined global as an error.
  - **`JIT-OPTIMIZE`** (`compiler.asm`) — a real special form in the
    reference (`jit.rs`) that HM-infers and natively compiles an
    interpreted closure, taking its symbol operand *unevaluated*.
    `00-core.lisp`'s `defun` calls `(eval (list 'jit-optimize name))`
    after every definition (the "one-door policy": every `defun`
    silently attempts optimization). This host has no distinct
    "optimize this closure" step to perform at all — `compile_lambda`
    already turns every `LAMBDA` into real native code the moment it
    is read (see "Why this exists" above), so every function here
    already *is* what `JIT-OPTIMIZE` would produce there. Implemented
    as a documented no-op returning its own unevaluated operand,
    exactly like `QUOTE` — not a stub standing in for missing work, but
    the honestly correct behavior for an architecture that has already
    done the thing `JIT-OPTIMIZE` asks for, unconditionally, for every
    function.
  - **`REMPROP`** (`lib/prelude.lisp`, ordinary library code over the
    existing `SYMBOL-PLIST`/`SET-SYMBOL-PLIST!` primitives, the same
    shape `GETP`/`PUTP` already use) and **`DEF`'s third (docstring)
    operand** (`lib/prelude.lisp`, stored via `PUTP` under indicator
    `"docstring"`, extending `DEF` from its previous v0-documented
    2-operand-only scope) — `defun`'s expansion calls
    `(remprop ',name "pure-checked")`/`(remprop ',name "source-form")`
    unconditionally on every definition (correctly a no-op when the
    indicator was never `PUTP`'d), and `(def ,name ,lambda-expr ,doc)`
    with three arguments whenever the `DEFUN` body led with a string
    literal — `PROG2`, `00-core.lisp`'s own first `defun`, has exactly
    such a docstring, so this was the very first defun call to trap.
  **`01-list.lisp` through `06-require.lisp` now also load completely,
  unmodified**, needing exactly one further genuinely new feature —
  **`DEFDYNAMIC` / dynamic-extent `LET` rebinding (KERNEL.md Part
  VI)** — the "dynamic variables" gap this README has named as
  outstanding since the session that first wrote it. `06-require.lisp`
  is the file that needed it: `(defdynamic *require-stack* nil ...)`
  at its top, then `$require-load`'s own
  `(let ((*require-stack* (cons name *require-stack*))) ...)`, which
  must be visible to `$require-note-dependency` — an entirely separate,
  separately-compiled function called during that `LET`'s dynamic
  extent — reading `*require-stack*` as a plain global. Ordinary
  lexical shadowing (what this compiler's `LET` already did) gets that
  wrong by construction: a separately-compiled function's own body has
  no lexical knowledge of a caller's `LET`, so it would still read the
  stale global. Implemented as two additions:
  - **`DEFDYNAMIC`** (`compile_defdynamic`, `compiler.asm`) — like
    `DEFINE` (name unevaluated, init-form compiled and stored into the
    symbol's global value cell normally), plus one immediate host-side
    effect: `mark_symbol_dynamic` sets a dedicated marker indicator on
    the symbol's own plist directly (via `symbol_plist`/
    `set_symbol_plist`, bypassing the compiled `PUTP` function
    entirely) — the same "compile-time symbol metadata, visible
    immediately to every later compile-time check in this process"
    idiom `DEFMACRO`'s own macro-slot install already relies on,
    reused here instead of a new symbol-layout field. Requires
    `DEFDYNAMIC` to run, as an ordinary top-level form, before any
    `LET` that rebinds the name — the same top-level-and-before-use
    discipline `DEFMACRO` already needs for its own compile-time
    visibility.
  - **Dynamic-extent `LET`** (`compile_let`'s new `.dynamic_rewrite`
    path, `compiler.asm`) — a prescan checks every binding name against
    the marker (`is_symbol_dynamic`); if any is dynamic, the whole
    `LET` is rewritten, at compile time, into an equivalent form built
    entirely from existing special forms rather than new codegen —
    the same derivation philosophy `compile_unwind_protect`'s own
    synthetic `(LAMBDA () cleanup...)` already demonstrates, and
    `UNWIND-PROTECT` is exactly the primitive dynamic rebinding needs
    (restore-on-every-exit, including a `THROW`/`ERROR` passing
    through):
    ```
    (LET ((tmp1 dyn1) (val1 init1) (tmp2 dyn2) (val2 init2) ...)
      (UNWIND-PROTECT
          (PROGN (SETQ dyn1 val1) (SETQ dyn2 val2) ... body...)
        (SETQ dyn1 tmp1) (SETQ dyn2 tmp2) ...))
    ```
    `tmp_i`/`val_i` are fresh `GENSYM`s, ordinary *lexical* bindings of
    the outer `LET` — they never shadow `dyn_i` itself, so every
    reference to `dyn_i` anywhere, including inside the
    `UNWIND-PROTECT`'s own cleanup closure (confirmed empirically: a
    `LET`-local captured by an enclosing `UNWIND-PROTECT`'s cleanup
    lambda already worked correctly before this change, `(LET ((X 1))
    (UNWIND-PROTECT (PROGN (PRINT X) 0) (PRINT X)))` printing `1` twice
    — this is only a *read* of the captured value, so the "captured by
    value, no `SETQ` visible back" v0 limit doesn't apply), still
    resolves to the real global slot via `compile_setq`'s own ordinary
    global-write fallback. Evaluating every `tmp_i` (the *old* value)
    and `val_i` (the new init) up front, in one outer `LET`, preserves
    ordinary `LET`'s parallel-evaluation semantics even though the
    actual global writes happen sequentially afterward.
    `tests/cases/057_defdynamic.asm` covers: a separately-compiled
    function seeing the rebound value across the call, restoration on
    normal `LET` exit, the `LET` body's own reference seeing the new
    value, and restoration firing correctly when a `THROW` passes
    straight through the dynamic extent without ever reaching the
    `LET`'s own normal exit path. **v0 scope, narrower than the spec on
    purpose**: every binding in a `LET` that has *any* dynamic binding
    gets this save/restore treatment, even one that isn't itself
    `DEFDYNAMIC`'d — mixing dynamic and ordinary lexical bindings in
    one `LET` is not yet distinguished (no reference stdlib file
    exercises this yet, `06-require.lisp` included); `LET*` does not
    support dynamic bindings at all yet, only plain `LET`.

  **`$VAU` (Kernel-style operatives, John Shutt's vau-calculus) now
  exists**, and with it `08-vau.lisp` through `21-cl-compat.lisp` —
  the *entire* Prelude tier, in the reference's own load order, no
  files skipped — now load completely, unmodified
  (`tests/cases/058_vau.asm`). This was the previously-identified hard
  wall ("PROG/VAU/DEFDYNAMIC" — the last of that trio); getting there
  needed one new primitive plus finding and fixing three real,
  previously-latent bugs it exposed:
  - **`$VAU`** (`compile_vau`, `compiler.asm`): identical to `LAMBDA`
    (`defvau`'s own params are always the fixed 2-element
    `(operands-param env-param)` list) except the resulting closure is
    retagged `HDR_OPERATIVE` instead of `HDR_CLOSURE` the moment it's
    built (tags.inc: identical `[8]=code-ptr [16]=nargs [24..]=free
    vars` layout — an operative *is* an ordinary closure underneath).
    A call site `(name arg...)` whose head is a global already bound
    (at compile time, not lexically shadowed — `frame_lookup` against
    `current_scope` guards this, matching `compile_call`'s own "locally
    bound — not a global call" check) to an `HDR_OPERATIVE` value skips
    evaluating its operands entirely: the whole raw argument list is
    baked as one `(QUOTE ...)` literal, a placeholder "caller's
    environment" value (`global_environment_sentinel`, a dedicated
    interned symbol — this kernel has no first-class environments, so
    there is nothing more meaningful to pass) is baked as a second
    `(QUOTE ...)`, and the two-argument call is handed to the ordinary
    `compile_call` unchanged — reusing its self-patching inline-cache
    machinery rather than a second calling convention.
    `emit_check_callable` (the runtime check a call site's resolved
    value is actually callable) now accepts `HDR_OPERATIVE` alongside
    `HDR_CLOSURE` — it previously only recognized `HDR_CLOSURE`, so
    every operative call failed as "not a function" until this was
    added; the flow to add it correctly (restructured to a shared
    `restore:` join point both the `HDR_CLOSURE` and `HDR_OPERATIVE`
    matches funnel through) took one wrong first draft, caught
    immediately by the regression suite (the `HDR_CLOSURE` fast path's
    early jump landed *after* the "restore the original tagged
    pointer" step instead of before it, leaving target `rax` holding
    the raw header-tag integer for every ordinary closure call, not
    just operatives — ~30 of 59 tests failing instantly is what a
    clobbered universal calling-convention register looks like).
    `EVAL`'s own existing 2-argument form needed no change at all:
    `compile_unary_hostcall` already only ever compiles `EVAL`'s first
    operand, so `(eval form e)` already silently evaluates `form` in
    the one global environment this kernel has, discarding `e` — which
    is exactly right for `08-vau.lisp`'s own `$if`/`$and`/`$or`/
    `$sequence`, each of which only ever forwards `e` opaquely through
    recursive `eval` calls, never inspecting or extending it. **v0
    scope, and one substantive, documented divergence from Kernel's
    own semantics**: `(eval x e)` on a *lexical* (a call-site local
    variable or parameter, as opposed to a literal or a global) is
    silently evaluated in the wrong (global) scope rather than
    erroring — a real environment would be needed for that to work at
    all, and none of `08-vau.lisp`'s own operatives, nor any operative
    used purely at the top level (`29-protocols.lisp`,
    `20-condensation.lisp`, `24-rules.lisp`, `27-modules.lisp`, where
    global genuinely *is* the caller's environment), hit this; an
    operative used *inside a function body* over one of that
    function's own locals would. Also not yet supported: an operative
    reached via `APPLY`/`FUNCALL` or any indirect/first-class use
    (only a literal call-site head is recognized).
  - **Nested `EVAL`-during-macro-expansion corrupted the code heap**
    (`compile_thunk`, `compiler.asm`): `invoke_macro` runs a macro
    transformer as ordinary already-compiled target code, synchronously,
    *during* an outer `compile_form`'s own still-open compilation — and
    the reference's own `lib/02-cxr.lisp` does exactly this
    (`defcxr`'s `(eval operations)` evaluates one of its own macro
    parameters at expansion time, to build `CADR`/`CADDR`/etc.). Before
    this fix, a nested `compile_thunk` call (triggered by that `eval`)
    bump-allocated its own fresh function *contiguously in the code
    heap*, splicing its whole prologue/body/epilogue directly into the
    middle of the still-open outer thunk's own instruction stream at
    whatever offset the shared `code_heap_cur` cursor happened to be —
    the outer thunk would then run straight into the *nested* thunk's
    own `leave`/`ret` mid-body, popping the outer frame's saved `rbp`
    (`0` at the top level, since `boot.asm` never sets one) as a return
    address and segfaulting. Fixed the same way `compile_lambda`
    already guards its own nested-function emission: `compile_thunk`
    now emits a leading `jmp` over its own body before compiling
    anything, patched to land just past the thunk once compilation
    finishes — at the top level this is one harmless dead 5-byte `jmp`
    before the real entry point; in the nested case the outer function
    now correctly branches over the inner thunk and resumes where it
    left off. `compile_thunk` also now saves/resets/restores
    `current_scope`/`current_frame_depth`/`current_prog_ctx` around its
    own `compile_form` call (matching `compile_lambda`'s own nested-scope
    handling) — harmless at the top level (already `NIL`/`0`/`0` there)
    but necessary for the nested case: without it, a nested `EVAL`
    would silently inherit the *outer*, still-in-progress compilation's
    own frame bookkeeping, corrupting it once the outer compilation
    resumed. And `eval_form`'s own `call rax` (invoking the freshly
    compiled thunk) was a bare indirect call rather than this project's
    own documented `push rax; call qword [rsp]; add rsp, 8` safe
    idiom every other thunk invocation already uses to keep the
    target's `rsp` 16-byte-aligned for `floats.asm`'s own SSE moves —
    pre-existing, unrelated to the splicing bug, but only ever
    exercised (and only ever faulted) once nested `EVAL` was.
  - **`DEFMACRO` supported only a single body form, silently discarding
    everything past a leading docstring** (`compile_defmacro`,
    `compiler.asm`): it took the literal fourth element of the
    `(DEFMACRO name (params) body...)` form as "the" body, ignoring
    every subsequent form — fine for every macro this project's own
    `lib/prelude.lisp` had defined so far (none carry a docstring), but
    `lib/02-cxr.lisp`'s own `defcxr` macro *does*
    (`"Generate a CAR/CDR composition function"` before its real
    backquote template), so every `defcxr`-built function
    (`CADR`/`CADDR`/... — needed by `08-vau.lisp`'s own `$if`) silently
    expanded to just the docstring text and never actually defined
    anything: `CADR` ended up permanently unbound, not merely wrong, a
    much harder failure to trace back to its actual cause than a wrong
    answer would have been. Fixed to collect every form after `params`
    (not just the first) into the body, the same "multiple body forms,
    implicitly `PROGN`-wrapped" support `compile_lambda`'s own body
    already has — a docstring now just evaluates to itself and is
    discarded like any other non-final `PROGN` form, needing no special
    extraction.
  - **Bareword `NIL` was read as an ordinary (permanently unbound)
    symbol, not the empty-list value** (`read_symbol`, `reader.asm`) —
    a real, separate, and rather fundamental conformance bug this
    session's `$VAU` debugging surfaced by accident (`(IF NIL 1 2)`
    returned `1`, since an *unbound* variable read is truthy under this
    kernel's rules and only the literal `NIL` immediate is false).
    `reader.rs`'s own `read_atom` special-cases the text `"NIL"` to
    produce `LispVal::Nil` directly, never interning it as a symbol at
    all (unlike `"T"`, which the reference reads as an ordinary
    interned symbol needing its own self-binding bootstrap — see
    `bootstrap_globals`, `symtab.asm`); this kernel's reader now does
    the same. Reference stdlib code uses bareword `nil` constantly as a
    self-evaluating literal (`lib/00-core.lisp` alone dozens of times),
    so this one silently blocked essentially all of it — `(DEFUN K (N)
    'X) (K 1)` returned `NIL` instead of `X` before this fix, for
    exactly this reason (`$defun-auto-compile`'s own `(getp name
    "no-compile")` check read `nil`'s "false" branch as true).

  The confirmed-loadable set is now **the entire Prelude tier** —
  `00-core` through `21-cl-compat`, `src/lib.rs`'s own `STDLIB_SOURCES`
  order, no files skipped — **plus `20-condensation.lisp`, the first
  Optional-tier file**, which needed one more missing primitive:
  **`SYMBOLP`** (`symbolp_tagged`, `symtab.asm`) — another genuine
  Rust-level builtin (`environment.rs`), surfaced by
  `06-require.lisp`'s own `$require-canonical-name`
  (`(cond ((symbolp x) x) ((stringp x) ...) ...)`), which every
  `(provide 'name)`/`(require 'name)` call goes through —
  `tests/cases/059_symbolp.asm` covers it, including that `NIL` is
  correctly not a symbol (this reader's own "NIL" special case, see
  above). **The embedded module registry now exists too**
  (`src/modules.asm`, a new file), unblocking `27-modules.lisp`,
  `11-optimizer-vau.lisp`, and `19-call-graph.lisp`: `27-modules.lisp`
  opens with `(require 'condensation)`, and `require`'s own resolution
  path (`06-require.lisp`'s `$require-resolve`) needs two more genuine
  Rust-level builtins (`environment.rs`) neither of which existed here
  before —
  - **`$MODULE-SOURCE-LOOKUP`** (`module_source_lookup_tagged`,
    `src/modules.asm`) — `("NAME")` returns `(source . origin)` for a
    known module or `NIL`. The embedded tier is a fixed table of
    `(name, source-start, source-end)` rows built from `incbin`-ing
    the reference's own, *unmodified* `../lib/*.lisp` files directly
    (the same technique `file_runner.asm` already uses for
    `lib/prelude.lisp`, proof this is the real file rather than a
    copy) — matching the reference's own `OPTIONAL_MODULES` table
    (`src/lib.rs`) name-for-name and file-for-file, for every module
    with no networking/TLS/regex/OS dependency (`SHELL` through
    `PROTOCOLS` in the reference's own table order: `07-shell.lisp`
    through `29-protocols.lisp`, 14 files). `TEXT` (`30-text.lisp`)
    onward is out of scope for the same reason
    `../examples/*/main.lisp`'s own network/regex/TLS examples already
    are. The reference's own first resolution tier (a Rust embedder
    registering additional sources at runtime,
    `env.registered_module_source`) has no equivalent here and isn't
    needed for this conformance target; the third tier (disk,
    capability-gated) was already ordinary Lisp
    (`$require-resolve-disk`) needing no kernel support at all.
  - **`$EVAL-MODULE-SOURCE`** (`eval_module_source_tagged`,
    `src/modules.asm`) — `("name" "source text")` parses and evaluates
    every top-level form in the source string, exactly the read/
    compile/run loop `file_runner.asm`'s own `run_buffer` already uses
    for a real file, reusing `reader_init`/`read_form`/`compile_thunk`
    directly rather than duplicating that loop as a distinct mechanism.
    `reader_buf`/`reader_pos`/`reader_end` are single global cells, not
    a stack (`read_from_string_tagged`'s own comment, `reader.asm`,
    already documents this for its own single-form read) — `REQUIRE`
    reaches this while genuinely mid-file (an ordinary Lisp-level
    library call, itself invoked from another still-open
    `file_runner.asm`-driven top-level form), so the caller's own
    reader position is saved before and restored after, the same
    save/reader_init/restore pattern `read_from_string_tagged` already
    established, generalized from one form to a whole loop of them.
  - **`ASSOC`** (`lib/prelude.lisp`) — a third genuine Rust-level
    builtin (`evaluator/builtins_extra.rs`), surfaced by
    `27-modules.lisp`'s own `DEFMODULE` (`(assoc ':export sections)`).
    Ordinary library code over `EQUAL`/`ATOM`/`CAR`/`CDR`, all already
    available — ``(not (atom (car alist)))`` inlines what `CONSP`
    would check (not yet defined at this point in lamedh-asm's own
    small prelude; the reference's own `01-list.lisp` defines `CONSP`
    identically) rather than depending on it; matches the reference's
    own "malformed alist elements are skipped, not an error"
    graceful-degradation behavior, and uses `EQUAL` rather than `EQ`
    for the key comparison, matching the reference's own structural
    `==`.

  **`07-shell.lisp` now loads too**, and with it `27-modules.lisp`'s
  own `WITH-MODULE`/`DEFMODULE` machinery is fully exercised for the
  first time (`07-shell.lisp` is the first file that actually *uses*
  `WITH-MODULE`, not just defines it) — which needed four more genuine
  Rust-level builtins, each found the same way: try the next file,
  bisect to the first form that traps, check `environment.rs` for a
  name lib code calls that this host never registered:
  - **`INTERN`** (`intern_tagged`, `symtab.asm`) — `(intern x)`
    uppercases a String into a local scratch buffer (matching the
    reference's own `s.to_uppercase()`) then interns it, or returns a
    Symbol argument unchanged; `27-modules.lisp`'s own
    `$MODULE-QUALIFY` (`(intern (concat (princ-to-string module) ":"
    (princ-to-string name)))`) builds every qualified name this way —
    `tests/cases/061_intern.asm` covers both argument shapes.
  - **`CONCAT`** (`lib/prelude.lisp`) — variadic string concatenation,
    also needed by `$MODULE-QUALIFY`'s three-argument call; ordinary
    library code over the existing 2-argument `STRING-APPEND`, no new
    kernel primitive.
  - **`STRING-LENGTH*`** (`compiler.asm`) — not a new primitive at
    all, but a *naming* gap: `environment.rs` registers only
    `"STRING-LENGTH*"`, never a bare `"STRING-LENGTH"` — this kernel
    had only ever implemented the latter spelling (matching this
    project's own docs, not the reference's actual name), invisible
    until `lib/14-strings.lisp`'s own `STRING-INDEX-OF` called it by
    its real name. Now answers to both spellings over the same host
    routine. `tests/cases/061_intern.asm` covers the reference's own
    name.
  - **`SEXPR-RENAME`** (`lib/prelude.lisp`) — a genuine Rust-level
    builtin (`evaluator/builtins_core.rs`) backing `WITH-MODULE`
    itself: rebuilds a form, replacing every symbol that is a key in a
    given hash table and not already qualified (its name contains
    `":"`) with its mapped value, leaving a `QUOTE`/`QUASIQUOTE`-headed
    cons untouched at every level. Ordinary library code over
    `SYMBOLP`/`GETHASH`/`STRING-INDEX-OF`/`ATOM`/`CONS`, forward-
    referencing the latter two (defined later, in reference stdlib
    files loaded well before `27-modules.lisp`'s own first use of
    this) the same way every other forward reference in this project
    already relies on. Verified manually end to end (`(with-module
    foomod (defun bar (x) (+ x 1))) (foomod:bar 5)` => `6`) rather than
    via an automated regression case: its own `STRING-INDEX-OF`
    dependency isn't available in either the standalone kernel-only
    harness (`tests/cases/*.asm`, no prelude at all) or
    `file_runner_prelude` (lamedh-asm's own small prelude only, no
    reference stdlib files) — only the full accumulated-file
    conformance testing this section describes exercises it for real.

  **`09-lisp15.lisp` needed one more special form — `DEFEXPR`**
  (`compile_defexpr`, `compiler.asm`), Lisp 1.5's FEXPR (the reference's
  own `SpecialForm::Defexpr`): like `DEFMACRO`, a FEXPR receives its
  call's raw, unevaluated argument list, but unlike a macro there is no
  separate expansion step recompiled in the call's place — the FEXPR's
  own body runs directly and its return value is the call's result
  (`(defexpr select (args) ...)`, `09-lisp15.lisp`'s own `SELECT`,
  Lisp 1.5's `SELECT[q;(q1 e1);...;en]`, is what surfaced this). This
  host has no separate FEXPR representation at all: `DEFEXPR` is sugar
  over `$VAU`, not a new kernel mechanism — `params` is always a single
  symbol (`(args)`, never `$VAU`/`DEFVAU`'s own fixed 2-element
  `(operands-param env-param)` shape), so `compile_defexpr` just
  appends a second, fresh, never-referenced `GENSYM` parameter and
  delegates entirely to `compile_vau`, needing no change to the
  operative-call check or `emit_check_callable` at all. This works
  because a FEXPR body only ever calls 1-argument `EVAL` explicitly on
  pieces of its raw argument list, and this kernel's `EVAL` already
  ignores any second operand unconditionally — the exact same "no
  first-class environments, `EVAL` always evaluates in the one global
  environment this kernel has" v0 divergence `$VAU`'s own comment
  already documents, carrying the identical caveat: the reference's
  own 1-argument `EVAL` evaluates in the *caller's* environment (real
  dynamic extent), not a fixed global one, so a FEXPR body's `EVAL`
  resolving a caller-local lexical is silently wrong here. Verified
  both the raw-argument-list mechanics and the explicit-`EVAL`-on-a-
  global idiom `SELECT` itself uses (`tests/cases/062_defexpr.asm`).
  **This one special form unblocked five files at once**:
  `09-lisp15.lisp`, `10-testing.lisp`, `22-guard.lisp`,
  `23-match.lisp`, and `24-rules.lisp` all now load completely,
  unmodified, with no further changes needed. **`25-variants.lisp` is
  the next wall**, not yet root-caused.

- **The concrete conformance target: `../examples/*/main.lisp` running
  unmodified.** There is now a real file-loading driver
  (`make lamedhc` builds `build/lamedhc`, `src/file_runner.asm`) that
  runs `lib/prelude.lisp` and then a named file or stdin through the
  same read-compile-run loop, and calling anything that isn't a real
  closure now signals a genuine, `HANDLER-CASE`/`ERRORSET`-catchable
  condition (`emit_check_callable`, `compiler.asm`, now routes through
  `fail_wrong_type`/`native_throw`, `native_errors.asm` — the same real
  signaling machinery `CAR`/`CDR`'s own wrong-type check already uses,
  not the separate hard `exit(1)` an earlier version of this check had)
  rather than segfaulting undefined-behavior-style; an *uncaught* one
  traps (`int3`) the same way any other unmatched `THROW` does
  (`tests/cases/034_not_callable.asm`/`035_not_callable_indirect.asm`).
  **`examples/factorial/main.lisp`'s main loop now runs
  correctly, unmodified** (see "The prelude" above for the exact scope
  and the one genuine, documented divergence — its self-check hits this
  kernel's narrower 62-bit fixnum range at exactly `20!`, not a bug).
  Since the bareword-`NIL`-reader fix (see "KERNEL.md conformance"
  above), the self-check now genuinely detects that overflow and calls
  `(error "factorial self-check failed")` as its own source says to —
  an uncaught `error`, correctly, `int3`-traps (exit 133) rather than
  exiting 0 the way it silently did before that fix (the self-check's
  own `AND`/`IF` logic never actually completed either branch
  observably), which was never truly "passing" either.
  `DEFUN`, `FORMAT`, `1+`/`1-`, `FUNCTION`/`#'`, `IOTA`, `REDUCE`, and
  `DOTIMES` are all real now. **`examples/fizzbuzz/main.lisp` now runs
  unmodified too, self-check included** (`OK`, exit 0): `MAPCAR`,
  `EQUAL`, and `NUMBER->STRING` are all ordinary prelude library code
  now (`equal` is explicitly library code in the reference too — see
  KERNEL.md Part IV — needing only `EQ`/`CAR`/`CDR`, all present), and
  getting `EQUAL` to actually agree with itself on two independently
  built string lists needed one real kernel fix: `EQ` on strings/floats
  was pointer equality, not the value equality Part IV requires (see
  "KERNEL.md conformance" above, `lisp_eq`/`float_eq_exact`) — fizzbuzz's
  own self-check is exactly the case that exposed it. Examples that need
  networking, regex, or TLS
  are out of scope for this from-scratch host regardless (Part IX
  capabilities this kernel has no I/O surface for yet); everything else
  in that directory is the honest bar.
  **Measured against the full corpus** (`../examples/*/main.lisp`, 50
  files as of this writing, run unmodified through `build/lamedhc`):
  **2 pass end to end** (`fizzbuzz`, `church-numerals`) plus `factorial`
  running its main loop correctly with the one documented 62-bit
  divergence noted above. `APPLY`/`FUNCALL` (a new kernel primitive,
  see "The prelude" above), `DEF`, `>`/`>=`/`<=`/`MAX`/`MIN`, and
  list-only `FOR-EACH`/`FILTER`/`SOME`/`EVERY` were added this pass
  specifically because a frequency count across the whole corpus (every
  leading symbol in every example) named them as the most broadly used
  forms this kernel didn't have; `church-numerals` is the first newly
  passing result of that pass, not a coincidence. **The other ~48 fail
  immediately** (a bare "not a function" from calling an unbound
  global) on constructs the reference's full `lib/*.lisp` standard
  library provides that this kernel does not yet, which the same
  frequency count also surfaces as the next tier, roughly by how many
  examples each would unblock. **`MAKE-HASH-TABLE`/`SETHASH`/`GETHASH`/
  `REMHASH`/`KEYS` now exist** (see "KERNEL.md conformance" above) —
  promoted from the `HT-*` demonstration in
  `tests/cases/022_hashtable_array.asm` into real `lib/prelude.lisp`
  code under Part XI's own required names — but re-measuring the full
  corpus afterward still shows the same 2/50 passing: no example in
  this corpus happens to be blocked *only* on the hash table primitive
  itself, since a generic `PUT!`/`REF` dispatch (what the reference's
  own protocol system actually exposes to calling code) still needs
  the multi-type dispatch system below, not just the hash table value
  underneath it. `MAKE-CHAR`/`CHAR-CODE`/`CODE-CHAR` and the `Char`
  type they build on now exist too (see "KERNEL.md conformance"
  above), but the rest of the string/char surface most examples
  actually call — `STRING->LIST`, `STRING-JOIN`, `STRING-DOWNCASE` —
  is still missing; `DEFRECORD` and the protocol/dispatch system (`DEFPROTOCOL`/
  `DEFINSTANCE`, `VARIANT-CASE`) `lib/20-condensation.lisp` and
  `lib/29-protocols.lisp` provide. **`RANDOM`/`RANDOM-SEED!`
  (`src/rng.asm`) and the bitwise ops `LOGAND`/`LOGIOR`/`LOGXOR`/
  `LOGNOT`/`ASH` (`src/bitwise.asm`) now exist too**
  (`tests/cases/049_bitwise.asm`, `tests/cases/050_random.asm`) — these
  aren't primitives KERNEL.md itself mandates (Part V's numeric tower
  is `+`/`-`/`*`/`/`/comparisons/`MOD`/`REMAINDER`/`FLOAT`, no bitwise
  or randomness surface), but the reference implements them and real
  examples in this corpus call them, so they're still real conformance
  work toward "the full example corpus runs," just against the wider
  reference-stdlib bar rather than the strict kernel-primitive one.
  `RANDOM` is the exact SplitMix64 generator the reference's own
  `rng_next` uses (`../src/evaluator/builtins_extra.rs`), lazily seeded
  from `rdtsc` (no libc clock call available) unless `RANDOM-SEED!` set
  it explicitly — so a fixed seed reproduces the identical sequence on
  both hosts, the one part of this meant to be bit-for-bit
  reproducible. `LOGAND`/`LOGIOR`/`LOGXOR` reuse the same tagged-word
  trick `tags.inc` already documents for `+`/`-`: a fixnum's tag bits
  are always `00`, so a plain bitwise AND/OR/XOR of two *tagged* words
  is already correctly tagged, no untag/retag needed. **v0 scope**:
  `LOGAND`/`LOGIOR`/`LOGXOR` are fixed 2-operand, not the reference's
  own variadic fold (no general variadic-primitive-call mechanism
  exists yet — the same reason every other multi-operand builtin here
  is a fixed shape); `ASH`'s overflow/sign-extension behavior is scaled
  to this kernel's own 62-bit fixnum width rather than the reference's
  64-bit one (Part XII axis 1's existing width divergence, not a new
  one). None of this is surprising — it is the gap between "a
  kernel with a working prelude" and "the reference's full standard
  library," and it is exactly what "run 100% of the examples" now
  honestly requires, tracked here so the next pass has a measured
  starting point instead of a guess.
- Benchmark corpus + gate: a fixed set of numeric/looping Lamedh
  programs with hand-written C equivalents, checked into this tree, run
  under both `gcc -O3`/`clang -O3` and this compiler, wall-clock/cycle
  compared — the falsifiable form of "beats C."
- A real register allocator (linear-scan to start) instead of spilling
  every local to a fixed stack slot.
- Proper tail calls: frame-reuse `jmp` for calls in tail position.
- A copying or generational GC for the data heap.
- General (not single-level) free-variable propagation through nested
  lambdas.
- Shared mutable closure cells (boxed captures) so `SETQ` on a captured
  variable is visible across closures over it.
- XMM support in `codegen.asm`, so float arithmetic can be inlined as
  target SSE2 instructions instead of a host-routine call per
  operation; mixed fixnum/float arithmetic; `FLOAT<=`/`FLOAT>`/
  `FLOAT>=`/`FLOAT=`; a real (shortest round-trip or scientific-
  notation) float printer instead of fixed 6-decimal-place formatting.
- Bignums — the rest of the Lisp 1.5 + extensions surface the Rust
  interpreter (`../src`) already implements. `$VAU`/`DEFDYNAMIC`
  existing means the entire reference Prelude tier (`00-core.lisp`
  through `21-cl-compat.lisp`) now loads unmodified — see "KERNEL.md
  conformance" above; `BLOCK`/`RETURN-FROM` are the next candidate for
  the same `CATCH`/`THROW`-derivation treatment `HANDLER-CASE` already
  got.
- `EVAL` now exists (`eval_form`, exposed as `(EVAL form)`) and
  `ERRORSET` uses it to match the spec exactly, but it takes no second
  (environment) argument — there being no environment-as-value in this
  kernel yet to pass one with. Native-failure-to-condition plumbing now
  covers two cases (`CAR`/`CDR` on a non-cons/non-`NIL` argument, and
  calling a non-callable value — see "KERNEL.md conformance" above,
  `fail_wrong_type`/`native_throw`, `native_errors.asm`; the separate,
  cruder `fail_not_callable` a hard `exit(1)` version of the callable
  check used is gone, folded into the same real signaling path), and
  the same `native_throw` routine is reusable for the rest of this
  list: division by zero, index out of range, wrong arity, and an
  unbound-variable *read* still misbehave exactly as before (segfault,
  garbage, or the raw `IMM_UNBOUND` immediate, respectively) rather
  than signaling — the natural next candidates for the same treatment.
- A resizable hash table (grow the bucket array and rehash past some
  load factor, instead of a fixed 61 buckets); the hash table still
  hashes/compares keys with `EQ`
  (pointer identity on cons, now value equality on strings/floats/
  fixnums/characters/symbols — see "KERNEL.md conformance" above), not
  `EQUAL`, so a *list*-shaped key still isn't found by an independently-
  built equal list, only string/number/symbol keys benefit so far.
- `FORMAT` itself, now that its prerequisites exist: `STRING-REF`/
  `SUBSTRING` to walk a control string for `~a`/`~%` directives at
  macro-expansion time (`FORMAT` is naturally a `DEFMACRO`, not a
  function — no runtime variadic dispatch needed, since the number and
  literal text of a call's arguments are already visible to the macro
  transformer as unevaluated operand forms) and `PRINT`'s existing
  runtime-tag dispatch for `~a`'s own rendering (it already prints a
  string's raw bytes unquoted, which is exactly `~a`'s "aesthetic," not
  `~s`'s "readable," output convention). `(format nil ...)` (returning
  a string rather than writing to a stream) additionally needs a way to
  capture `PRINT`'s output into a string instead of stdout — deferred,
  since the overwhelming majority of the corpus's own `FORMAT` calls
  write directly to a stream.
- AArch64 backend (currently x86-64 Linux only).
