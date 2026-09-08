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
| `10` | heapobj   | address of a header word naming the object's kind (symbol, closure) |
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
- Binary `+ - * < =` operating on unboxed tagged fixnums.
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

## v0 limits (known, not silent)

- At most 3 arguments per call/lambda (registers `rsi`,`rdx`,`rcx`; no
  stack-spilled extra args yet).
- A lambda body is a single expression (no implicit `PROGN`).
- A nested `LAMBDA` may only capture free variables from its
  *immediately* enclosing lambda's own frame — a variable needed from
  two levels up needs manual re-threading through the middle lambda for
  now; deeper transitive free-variable propagation isn't implemented.
- Captured variables are captured **by value** at closure-creation time,
  not as shared mutable cells — there is no `SETQ` on a captured
  variable visible to the closure that captured it (or vice versa).
- No garbage collector. No bignums, floats, strings, hash tables,
  vectors, macros, `vau`, conditions, or dynamic variables yet.
- Proper tail-call frame reuse (`jmp` instead of `call`+`ret`, reusing
  the caller's stack frame) is not yet implemented for ordinary Lisp
  calls; the inline-cache trampoline's *own* internal dispatch already
  ends in a tail-jump, but the enclosing function's call site itself
  still uses `call`.
- No benchmark corpus gate yet (see below).

None of these are silent traps in the sense of producing wrong answers
within their stated scope — they are things the compiler simply doesn't
attempt yet.

## Calling convention

This project owns its own convention; there is no C ABI to honor.
Compiled functions: `fn(rdi=closure_ptr, rsi=arg0, rdx=arg1, rcx=arg2) ->
rax`. Compiled code preserves **no** register across a call into other
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
- More than 3 arguments; multi-expression lambda bodies (`PROGN`).
- General (not single-level) free-variable propagation through nested
  lambdas.
- Shared mutable closure cells (boxed captures) so `SETQ` on a captured
  variable is visible across closures over it.
- Bignums, floats, strings, hash tables, arrays, macros, `vau`,
  conditions, dynamic variables — the rest of the Lisp 1.5 + extensions
  surface the Rust interpreter (`../src`) already implements.
- AArch64 backend (currently x86-64 Linux only).
