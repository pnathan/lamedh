# Kernel contract — draft

Status: draft for review. Closes the open item in KERNEL.md Part XIII
("enumerate the exact builtin-name table `lib/*.lisp` calls") and proposes the
rule that makes one shared stdlib possible.

Regenerate the data with `scripts/kernel-inventory.py`; the table is
`docs/kernel-inventory.tsv`.

## Goal

One `lib/*.lisp` for every kernel. A kernel supplies a small, versioned
contract. Anything definable in Lisp over the contract lives in the stdlib.
A kernel may override a stdlib definition natively; it must not be required to.
Optional capabilities ship as auxiliary libraries that a kernel loads or omits.

## Findings

`lib/*.lisp` calls 300 distinct Rust builtins (1,726 distinct operator names
in all: 673 Lisp-defined, 14 macros, the rest special forms and locals).

| tier | count | meaning |
|---|---:|---|
| K | 104 | kernel: cannot be written in Lisp, or defines the language |
| L | 52 | Lisp-definable over K; move to the shared stdlib |
| A | 11 | Lisp-definable but slow; native override recommended |
| U | 7 | Unicode tables: case mapping, casefold, char classes |
| R | 8 | reflection tied to the reference's typed JIT |
| H-io | 77 | files, ports, OS, shell, clock, randomness |
| H-net | 27 | tcp, udp, tls, name resolution |
| H-regex | 12 | regular expressions |
| H-conc | 2 | threads, channels |

So 104 names are the real kernel; a further 15 or so in K are aliases or
derivable (`make-array`/`array`, `fetch`/`aref`, `store`/`aset`, `funcall`
over `apply`, `remprop` over `getp`/`putp`) and can be cut once reviewed.

### K, the proposed kernel

- Pairs and identity: `cons car cdr eq atom`
- Numbers: `+ - * / < = > mod remainder floor ceiling round truncate sqrt sin cos tan exp log expt logand lognot logxor ash`
- Predicates: `numberp fixp floatp stringp symbolp charp functionp macrop arrayp hash-table-p typed-array-p extension-p`
- Symbols and bindings: `intern gensym set boundp getp putp remprop`
- Strings and chars: `concat substring string-length* char-code code-char make-char string->number number->string`
- Arrays: `array make-array aref fetch aset store array-length* typed-array`
- Hash tables: `make-hash-table gethash sethash remhash keys hash-code`
- Records: `record-new record-ref record-brand record-fields record-compiled-p`
- Evaluation: `eval apply funcall macroexpand the-environment make-environment`
- Conditions: `error make-error error-p error-message error-data errorset`
- Printer and reader: `prin1 princ print terpri prin1-to-string princ-to-string read read-string`
- Fences: `set-flag clear-flag flag-set-p kernel-fuel-remaining kernel-fuel-set! feature-enabled-p features capability-mask-allows-p`

### What moves out of the kernel

- **`string-downcase*` and the other six U names** become an optional
  `unicode` library. Without it, case mapping is ASCII-only and the kernel
  declares that as a host trait (`+HOST-TRAITS+`, KERNEL.md Part XII axis 1).
  A kernel with native tables (Rust) overrides.
- **`string->list*`, `string-split*`, `string-join*`, UTF-8 encode and decode,
  `sort`, array bulk ops (A)** get portable Lisp definitions. Issue #510 made
  the first three native because `SUBSTRING` indexes by character and a Lisp
  loop is quadratic. The contract must pin whether `SUBSTRING` indexes by
  character or by byte. Pin it as an O(1) character index where the kernel
  stores code points, otherwise keep these as required accelerators.
- **The 52 L names** (list and number helpers, Lisp 1.5 aliases such as `add1`
  and `plus`, `rplaca`/`rplacd`, which KERNEL.md Part XII axis 2 already
  defines as `cons` over the other half) are plain stdlib definitions.
- **H-*** are auxiliary libraries. `ports` over file descriptors is the
  largest; the asm host already has one.

### Override rule

A kernel may define any L or A name natively. The stdlib defines it only when
unbound: `(unless (boundp 'x) (defun x ...))`, or a load-time feature check once
#459 lands. No stdlib file may redefine a name the kernel supplied, and no kernel
prelude may define a global the stdlib also defines. That second clause is the
rule the asm port broke: its prelude `FORMAT` macro called a helper named
`FORMAT-BUILD`, which `lib/18-format.lisp` redefines with a different arity, so
`format` failed on every example once the stdlib loaded. Internal helpers get a
`$` prefix.

## Performance: what "par with SBCL" requires

Measured on this machine, `fib(32)`:

| implementation | time |
|---|---:|
| SBCL native, untyped `defun` | 0.040 s |
| SBCL native, fixnum-declared | 0.016 s |
| lamedh-asm (`lamedhc`), net of 6 ms startup | 0.052 s |

The benchmark in `benchmarks/tri-impl` labels a column `sbcl`, but that column
is the CL port's evaluator, not natively compiled SBCL. It measures the port,
not the ceiling. A real gate needs a hand-written CL edition of each kernel,
compiled with `(optimize speed)`, as the reference line.

On integer calls the asm host is within about 1.3x of untyped SBCL. It is far
away on float code: scaled `mandel` is 6.6 s on asm against 0.15 s on the Rust
typed JIT, because float operations are runtime calls on boxed values. Reaching
SBCL parity there needs unboxed float arithmetic in compiled code, a type
inference pass, and register allocation in place of stack-slot locals. All three
are in the asm roadmap.

Four spec rules cost speed on any compiled host. Each is a candidate to revisit;
none has been measured in isolation:

1. **Cons cells are immutable** (Part XII axis 2). `rplaca` allocates. This
   forbids in-place list update and destructive `nreverse`-style idioms.
2. **Fuel** (Part X) charges a counter on every evaluation step. A compiled host
   must either emit the decrement in every function prologue and loop back-edge
   or run unfenced code on a separate path.
3. **`EQ` on strings and floats** is value equality in the asm host (Part IV), so
   `eq` is not a single compare.
4. **Wraparound `int64`** (Part XII axis 1) does not fit a 62-bit tagged fixnum.
   `ARBITRARY-PRECISION` is allowed, which suits SBCL; asm has neither.

## Decisions needed

1. Is `SUBSTRING` indexed by character or by byte? It decides whether A names
   can be portable.
2. Do we keep the ~15 aliases in K, or cut them to one spelling each?
3. Integer model for asm: boxed int64, bignum-lite, or a declared
   `ARBITRARY-PRECISION` host with a 62-bit fast path and promotion.
4. Add a native-SBCL reference column to the benchmark before choosing any
   optimization target.
5. Whether `#459` (reader feature dispatch) lands before the override rule, or the
   `boundp` guard is the interim form.
