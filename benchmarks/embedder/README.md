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

History on the B3 rung (whole-loop native SDF vs Rust): 3.3x at baseline →
2.8x with the Cranelift optimizer on (#434) → 2.2x with SSA locals (#435) →
**2.0x** with cold-branch overflow-flag stores. Table below reflects the
original baseline run; the ratios in the history are the current truth.

### A. Call overhead — trivial tick fn, the game-loop pattern

| rung | mechanism | ns/call | vs Rust |
|---|---|---|---|
| A1 | `eval_str` (string + parse + eval — what embedders do today) | 1987 | 904x |
| A2 | pre-parsed form, `evaluator::eval` | 562 | 256x |
| A3 | `jit_call` into a NATIVE `defun*` | 215 | 98x |
| A4 | pure Rust function | 2.2 | 1x |

`#423`'s `call_function`/`FnHandle` targets the A2→A3 gap without requiring
a typed signature; `#424`'s raw entry pointer targets single-digit ns.

### B. Sphere-SDF kernel, 1M evaluations — where the membrane bites

| rung | mechanism | ns/eval | vs Rust |
|---|---|---|---|
| B1 | interpreted loop + interpreted kernel | 2294 | 1032x |
| B2 | Rust loop, per-sample `jit_call` into NATIVE kernel | 274 | 123x |
| B3 | whole loop compiled — ONE `jit_call` | 7.2 | 3.3x |
| B4 | pure Rust loop | 2.2 | 1x |

The lesson embedders need: **crossing the membrane per sample costs ~40x
more than the work itself** (B2 vs B3). Push the loop across, not the
samples. `#424`'s raw `fn(f64,f64,f64) -> f64` pointer is the missing rung —
per-sample calls from host loops (marching cubes) at near-B3 economics.

### C. Dot product, 100k f64 typed arrays (zero-copy membrane)

| rung | mechanism | ns/elem | vs Rust |
|---|---|---|---|
| C1 | `defun*` NATIVE loop over typed arrays | 4.1 | 3.9x |
| C2 | Rust iterator dot | 1.1 | 1x |

Results are bit-identical to Rust in every scenario (the sums agree to the
last digit). Historical note: this benchmark is what exposed #428 — before
the fix, no float64-signature function had a native edition at all, and
activating them surfaced the pending-tail-call clobber the same hour.
