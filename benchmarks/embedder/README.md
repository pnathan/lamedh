# Embedder benchmark — the "Call Me Maybe" ladder (epic #427)

Measures what a host program pays, per call, to talk to an embedded Lamedh
image — each rung an embedder can stand on today, against the pure-Rust
floor. This is the acceptance benchmark for the 0.5 release: #423 (fast-call
API) and #424 (raw native entry points) exist to collapse these rungs.

Run it:

```sh
cd benchmarks/embedder/rust
cargo build --release
./target/release/embedder-bench
```

## Numbers (i7-9750H, 2026-07-17, post #429 + #434 + #435)

History on the A3 rung (typed membrane call): 215ns at 0.4.0 → **92ns** by
name and **41.5ns** via `JitFnHandle` (pinned id + defs-generation check,
allocation-free scalar path) — both under the 100ns target, redefinition
still picked up like a by-name call.

History on the B3 rung (whole-loop native SDF vs Rust): 3.3x at baseline →
2.8x with the Cranelift optimizer on (#434) → 2.2x with SSA locals (#435) →
**2.0x** with cold-branch overflow-flag stores. Table below reflects the
original baseline run; the ratios in the history are the current truth.

### A. Call overhead — trivial tick fn, the game-loop pattern

| rung | mechanism | ns/call | vs Rust |
|---|---|---|---|
| A1 | `eval_str` (string + parse + eval — what embedders do today) | 1987 | 904x |
| A2 | pre-parsed form, `evaluator::eval` | 562 | 256x |
| A2.5 | `call_function` (fast-call API, #423) | 532 | 250x |
| A2.6 | `FnHandle::call` (fast-call API, pinned symbol, #423) | 481 | 226x |
| A3 | `jit_call` into a NATIVE `defun*` | 215 | 98x |
| A4 | pure Rust function | 2.2 | 1x |

`#424`'s raw entry pointer targets single-digit ns.

History on the A rung: `#423` landed `call_function`/`FnHandle` (A2.5/A2.6
above, measured 2026-07-17) — skipping the reader/printer alone drops the
trivial-tick call from ~2000ns to ~530/~480ns, a ~3.8-4.2x win over
`eval_str` with no typed signature required. That number sits right next to
A2 (pre-parsed + `evaluator::eval`, 562ns), confirming fast-call's own
overhead (name resolution + `apply`) is close to zero — the A2.5→A3 gap is
the interpreted-vs-NATIVE-compiled *callee* cost, not anything left on the
table by the fast-call API itself. `#424`'s raw entry pointer remains the
rung for closing that gap without a typed signature.

### B. Sphere-SDF kernel, 1M evaluations — where the membrane bites

| rung | mechanism | ns/eval | vs Rust |
|---|---|---|---|
| rung | mechanism | ns/eval | vs Rust |
|---|---|---|---|
| B1 | interpreted loop + interpreted kernel | 2294 | 1032x |
| B2 | Rust loop, per-sample `jit_call` into NATIVE kernel | 274 | 123x |
| B2.5 | Rust loop, per-sample **raw native entry** (#424) | 10.9 | 3.8x |
| B3 | whole loop compiled — ONE `jit_call` | 7.2 | 3.3x |
| B4 | pure Rust loop | 2.2 | 1x |

The lesson embedders need: **crossing the membrane per sample costs ~40x
more than the work itself** (B2 vs B3). Push the loop across, not the
samples — or, when you can't (a host marching-cubes loop that samples a Lisp
SDF one point at a time), extract a raw native entry point (#424): rung B2.5
calls the compiled kernel's machine code directly with no boxing, no
membrane, no dispatch. Measured 2026-07-17 (i7-9750H): **10.9 ns/eval**, a
~30x drop from the per-sample `jit_call` membrane (B2, 332 ns on that run)
and within ~1.8x of the whole-loop-native B3 (6.0 ns) — the missing per-
sample rung, at near-B3 economics, with the sum still bit-identical to Rust.

History on the B2.5 rung: introduced with #424 at 10.9 ns/eval — the raw
`fn(f64,f64,f64) -> f64` snapshot handle. The residual gap to B3 is the
per-call `Ctx` build + the indirect call the whole-loop edition amortizes
away; the gap to B4 (pure Rust, 2.8 ns on that run) is that plus Cranelift's
own codegen vs LTO'd Rust.

### C. Dot product, 100k f64 typed arrays (zero-copy membrane)

| rung | mechanism | ns/elem | vs Rust |
|---|---|---|---|
| C1 | `defun*` NATIVE loop over typed arrays | 4.1 | 3.9x |
| C2 | Rust iterator dot | 1.1 | 1x |

Results are bit-identical to Rust in every scenario (the sums agree to the
last digit). Historical note: this benchmark is what exposed #428 — before
the fix, no float64-signature function had a native edition at all, and
activating them surfaced the pending-tail-call clobber the same hour.
