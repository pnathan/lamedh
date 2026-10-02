# Tri-implementation benchmark

Times the three Lamedh implementations in this repository against each
other on the same programs:

| column | implementation | how it runs the kernel |
|---|---|---|
| `rust-interp` | Rust reference (`target/release/lamedh`) | tree-walking evaluator, forced: `run.sh`/`gen.sh` prepend `(declaim (no-compile f g ...))` naming every `DEFUN` in the file |
| `rust-default` | Rust reference | plain `DEFUN`, i.e. what a user gets by default: one-door auto-compile (`$defun-auto-compile` → `jit-optimize`) turns a function NATIVE only when HM inference fully types it |
| `rust-jit` | Rust reference | hand-typed `DEFUN-TYPED` editions in `kernels-typed/`, each asserting `(compiled-p 'f)` = `NATIVE` before running (the program errors otherwise) |
| `sbcl` | SBCL port (`sbcl/`) | the port's own evaluator, loaded through ASDF with the embedded stdlib |
| `asm` | x86-64 assembly port (`lamedh-asm/build/lamedhc`) | every top-level form compiled to native code on read; only `lib/prelude.lisp` is available |

`asm-scaled` / `rust-jit-scaled` rerun the natively compiled kernels on
the two native tiers with larger inputs (each kernel's `;; scaled:` line),
because at interpreter-sized inputs both finish in tens of milliseconds and
the Rust number cannot be resolved against its ~0.26 s startup.

## Files

- `kernels/*.lisp`: one source per kernel, written once in the subset all
  three implementations share. The asm port has no generic float
  arithmetic, so float code uses its `F+ F- F* F/ F<` operators, and string
  code uses its `STRING-APPEND`/`STRING-LENGTH`. `gen.sh` rewrites those
  names (and only those) to `+ - * / <` and `CONCAT`/`LENGTH` for the Rust
  and SBCL hosts. Nothing else differs between the hosts.
- `kernels-typed/*.lisp`: the same algorithms as `DEFUN-TYPED`, for the
  kernels the typed tier can express (`MAKE-ARRAY`/`AREF`/`ASET` there are
  the typed tier's spellings of `ARRAY`/`FETCH`/`STORE`; an `IF` with a
  `NIL` arm becomes `0`, because the typed core has no `NIL` literal).
- `gen.sh`: writes `build/<column>/<kernel>.lisp`.
- `run.sh`: builds, checks outputs, reports Rust tiers, measures startup,
  times every cell with `hyperfine`, writes `results/`.
- `summarize.py`: `results/json` → `results/summary.md`.
- `sbcl-run.sh`: runs one file on the SBCL port.

## Kernels

| kernel | what it stresses | size |
|---|---|---|
| `fib` | non-tail calls, integer `+`/`<` | fib(32) |
| `ack` | deep non-tail recursion | A(3,8) |
| `tailsum` | self tail call, accumulator | 1..10^7 |
| `whileloop` | `WHILE`/`SETQ`, `MOD` | 5·10^6 iterations |
| `lists` | cons allocation / reclamation | build + reverse 10^6 cells, ×3 |
| `msort` | list processing | merge sort of 10^5 LCG integers |
| `sieve` | array store/fetch | sieve to 10^6, ×3 |
| `prefix` | array read-modify-write | prefix sums over 10^6, ×3 |
| `mandel` | boxed/unboxed float arithmetic | 300×300 grid, 100 iterations |
| `closures` | lambdas, captured variables, HOFs | map/filter/fold over 10^4, ×60 |
| `strings` | string allocation and copying | 2000 two-byte appends, ×150 |

Constraints of the shared subset, all imposed by the asm port: fixnums are
62-bit (checksums are kept well below 2^61); there are no bignums; closures
capture by value (no kernel mutates a captured variable); deep non-tail
recursion is bounded by the native stack, so every list walk is written
tail-recursively with an accumulator; floats print with a fixed 6 decimals,
so every kernel prints an integer; `>`/`<=`/`NOT` are prelude functions
rather than compiled operators, so the kernels use only `<` and `=`. Every
helper (`rev`, `fold`, `map-onto`, ...) is defined in the kernel rather
than taken from a host stdlib, so all hosts run the same algorithm and no
host gets a natively implemented builtin where another runs Lisp.

## Method

1. **Correctness first.** Every kernel runs once on every column; the
   printed value is normalized (whitespace removed, SBCL's `;`-prefixed
   loader warnings dropped) and must be identical across columns
   (`results/outputs.tsv`, "Output agreement" in the summary). The SBCL
   CLI exits 1 after every program (a known `:UNIX-STATUS` TYPE-ERROR
   after the program finishes), so exit status is not used as a
   correctness signal anywhere; `hyperfine` runs with `-i`.
2. **Tiers.** For the three Rust columns `run.sh` records
   `(compiled-p 'f)` for every function (`results/tiers.txt`), so it is
   visible which tier actually ran.
3. **Startup.** An empty program is timed on each host
   (`2×RUNS` runs). The Rust and SBCL hosts load their embedded stdlib at
   startup; the asm binary loads nothing beyond its prelude.
4. **Timing.** `hyperfine -N` (no shell), 1 warmup + `RUNS` (default 5)
   timed runs per cell. The table reports **net = median(kernel) −
   median(startup of that host)**, the raw median and minimum, and the
   speed relative to `rust-interp` (net(rust-interp) / net(column)). A net
   time smaller than 3σ of that host's startup noise is marked `~` and
   *startup-bound*: its ratio would be noise.
5. Inputs are sized so the slowest column (the Rust interpreter) stays at
   roughly 5–12 s per run. With a ~100× spread between the interpreters
   and the native tiers, the brief's "0.5–10 s on the fastest
   implementation" cannot hold at the same time; the scaled native-only
   table covers that end instead.

## Running

```sh
apt-get install -y sbcl nasm hyperfine
benchmarks/tri-impl/run.sh              # ~30 min on a 4-core cloud VM
RUNS=10 benchmarks/tri-impl/run.sh      # tighter numbers
ONLY="fib mandel" benchmarks/tri-impl/run.sh
```

## Caveats

- One machine, one run of the harness. A shared cloud VM has neighbours;
  compare the reported minimums against the medians to judge noise.
- The subset is the asm port's language. It excludes most of what the Rust
  and SBCL hosts are for (the stdlib, records, protocols, conditions,
  bignums), so these numbers say nothing about those features.
- Startup subtraction assumes startup cost is additive and independent of
  the program, which is approximately but not exactly true (e.g. heap
  growth, page faults).
- The Rust typed JIT only covers kernels that are typed islands (integers,
  floats, flat arrays, loops, self calls). Lists, closures and strings have
  no `rust-jit` cell.
