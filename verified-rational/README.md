# verified-rational

Exact-with-verified-rounding bounded rational arithmetic, machine-checked with
[Verus](https://github.com/verus-lang/verus).

`Q` is a rational `num/den` kept in canonical form (`den > 0`,
`gcd(|num|, den) == 1`) inside a fixed width budget (`|num| ≤ 2^62 − 1`,
`den ≤ 2^62 − 1`). Arithmetic is **exact whenever the exact result fits that
budget** and falls back to **directed rounding with a proved error bound** when
it does not.

```rust
use verified_rational::Q;

let r = Q::from_decimal(85, 2).unwrap();   // 0.85, exactly: 17/20
let third = Q::new(1, 3).unwrap();
let sixth = Q::new(1, 6).unwrap();
assert_eq!(third.add(sixth), Q::new(1, 2).unwrap());
```

## Why

The consuming project is a subjective-logic fusion engine whose mathematics
(Jøsang cumulative/averaging belief fusion, opinion algebra) is rational-closed.
Its `f64` implementation is non-deterministic across evaluation orders,
non-associative, and unverifiable. External bignum crates were not an option:
the best one, `malachite-q`, is LGPL-3.0-only — a blocker for statically linked
proprietary binaries — and under Verus *any* external crate's arithmetic enters
the proof as an unverified axiom. There is no verified bignum or rational
anywhere in the Verus ecosystem, so this crate is first-of-kind.

Full verified arbitrary precision (`Vec<u64>` limbs) is deliberately out of
scope. It is the escalation path if benchmarks show the rounding bites; it is an
order of magnitude larger as a verification project.

## What you get over `f64`

| | `f64` | `Q` |
|---|---|---|
| Representation | many bit patterns per value, NaN | one canonical pair per value |
| Equality / hashing | unsafe to derive | `Eq`/`Hash` derived, and *sound* |
| Ordering | `PartialOrd` | `Ord` — total, no NaN |
| Reproducibility | order- and platform-dependent | bit-identical, fixed fold order |
| Serialisation | lossy | exact `(num, den)` pair |
| Error bound | folklore | proved: `2^-60 · max(1, |v|)` per operation |
| Small cases | still approximate | **exact**, no rounding at all |

## The rounding contract

`round_to_budget` is the single place a value can lose exactness. Every
operation computes its exact result in `i128` and hands it there, so the
contract is proved once and inherited.

* **R1 — identity on representables.** If the exact reduced result satisfies the
  budget, the operation returns it bit for bit. The consequence is the
  *exactness theorem*: any computation whose exact intermediates all fit is
  end-to-end exact. Small investigations pay zero rounding.
* **R2 — directed.** `Dir::Down` never overshoots and `Dir::Up` never
  undershoots, for negative values too. A future interval type can bracket the
  exact answer without new proofs.
* **R3 — bounded.** Otherwise `|result − exact| ≤ 2^-60 · max(1, |exact|)`.
* **Saturation.** If the *magnitude* exceeds `2^62 − 1` no `Q` can carry the
  value; the result saturates and the postcondition says so explicitly instead
  of pretending R2/R3 still hold. Every value in the consuming engine lives in
  `[0, 1]` or is a small count, so this case is unreachable there — but the type
  is total.

### The algorithm, briefly

Reduce by `gcd`; return verbatim if it fits (R1); otherwise split off the
integer part `|n|/d = qi + f/d` and snap the fractional part onto the dyadic
grid `k/2^s`, with `s` chosen from the magnitude of `qi` so the reassembled
numerator still fits and the grid stays fine enough for R3.

The fractional snap is an **exact shift-and-subtract long division**, not a wide
multiply. That keeps every intermediate inside `i128` without pre-scaling, and
it makes R2 exact rather than approximate: the remainder says precisely which
side of the grid point the true value lies on.

### Why the budget is 2^62 and not 2^63

Every intermediate is computed exactly in `i128`. The binding constraint is
`add`, whose numerator is `n₁d₂ + n₂d₁ ≤ 2·(2^62−1)² < 2^125`. A `2^63` budget
pushes the same expression to `2^127`, which overflows `i128::MAX = 2^127 − 1`.
Verus checks this mechanically, with overflow checks on and no `wrapping_*`
anywhere.

## Honesty: what does *not* hold

With rounding in play, `add` and `mul` are **commutative but not associative in
general**. Associativity and distributivity hold *exactly on the exact path* —
any computation whose exact intermediates all fit the budget — and
`laws::lemma_add_assoc_exact` proves it. Beyond that they agree only up to the
accumulated R3 error.

So order-independence claims made by a consuming engine hold exactly for small
cases and up to the proved bound in general. At the engine's reachable worst
case (~2×10⁴ sequential operations on one value path) that bound is
`2×10⁴ · 2^-60 ≈ 2^-45.7 ≈ 2×10^-14` relative — the same precision class as
`f64` accumulation, but deterministic and *proved* rather than assumed.

## Verification status

Run `./scripts/verify.sh` (see below). Current result: **216 obligations
verified, 0 errors, zero `assume`/`admit` anywhere in the crate.**

| # | Obligation | Tier | Status |
|---|---|---|---|
| V1 | I1 ∧ I2 preserved by every public op | MUST | **proved** — `Q::wf` is required on inputs and ensured on outputs throughout |
| V2 | No panic, no overflow; every `i128` intermediate in range | MUST | **proved** — overflow checks on, no `wrapping_*` |
| V3 | Value correctness vs the ghost model, division-free cross-multiplication | MUST | **proved** |
| V4 | Rounding contract R1–R4 | MUST | **R1, R2, R3 proved. R4 (monotonicity) not proved** — see below |
| V5 | GCD correctness (divides both, divisibility-maximal) + termination | MUST | **proved**, including Bezout-free `lemma_gcd_greatest` and `lemma_gcd_reduce_coprime` |
| V6 | Algebraic laws | MUST | **partly proved** — see below |
| V7 | Error-propagation (Lipschitz) lemmas | SHOULD | **not started** |
| V8 | n-ary accumulated bound `k·2^-B` | SHOULD | **not proved**; exercised by the 10⁴-step differential test |

### The two MUST-tier gaps, stated precisely

**R4 (monotone rounding): `x ≤ y ⟹ round(x, dir) ≤ round(y, dir)`.** Not proved.
It is *true*, and the argument is short: the grids are nested (`s` is
non-increasing in the magnitude, and `2^{s_y} | 2^{s_x}`), so either both values
land in the same band — where monotonicity is just monotonicity of floor on a
common grid — or `qi_x < qi_y`, in which case `round(x) < qi_x + 1 ≤ qi_y ≤
round(y)` because band boundaries are grid points of both grids. Formalising it
means relating two separate invocations of `round_nonneg`, which needs the
functional characterisation described below. Two consequences that the consuming
engine's clamp logic actually depends on *are* covered: values in `[0, 1]` stay
in `[0, 1]`, and `clamp` is proved to land inside its bounds
(`Q::clamp`'s postcondition).

**V6 commutativity, at bit level.** What is proved is that `add(a, b)` and
`add(b, a)` present *identical* exact `(n, d)` pairs to *identical* rounding
(`laws::lemma_add_inputs_comm`, `lemma_mul_inputs_comm`). Turning that into
`add(a, b) == add(b, a)` as values requires knowing the rounding function is a
function — and Verus models a call to an `exec` function by its postcondition,
so two calls with equal arguments are not *automatically* provably equal.
Closing this properly means giving `round_to_budget` a postcondition of the form
`r == round_spec(n, d, dir)` for a `closed spec fn` mirroring the algorithm.
That mirror is genuinely short — `spec_gcd`, `pow2` and spec-level `/`/`%`
already exist and cover three of its four loops — and it would close R4 as well.
Until then, bit-level commutativity and cross-run/cross-thread determinism are
covered by `tests/properties.rs`.

Proved under V6 today: the order is a total order (reflexive, total,
antisymmetric, transitive) agreeing with the ghost order; `neg`/`abs`/`recip`
involution laws; and associativity of `add` on the exact path. Distributivity on
the exact path is not yet written.

### Deviations from the original specification

Two places where the spec as written could not be implemented as written:

* **`Q::new` cannot return `None` only for `den == 0`.** The spec says inputs
  within `i64` always fit I2 after reduction. They do not: `Q::new(i64::MAX, 1)`
  reduces to `(2^63 − 1)/1`, well past `2^62 − 1`. `Q::new` therefore also
  returns `None` when the reduced fraction is outside the budget. Silently
  rounding a constructor that promises exactness would have been worse.
* **`Ord` must not be derived.** The spec lists `Ord` among the derivable-safe
  traits. The derived lexicographic order on `(num, den)` reports `1/3 > 1/2`;
  it is wrong, not merely suboptimal. `Ord` is implemented by
  cross-multiplication. `PartialEq`/`Eq`/`Hash` *are* derived, and that is sound
  precisely because canonical form makes structural equality coincide with
  mathematical equality. `tests/properties.rs` guards the distinction.

## Trusted boundary

Two functions touch `f64`; both are enumerated with their assumed
specifications in [`TRUSTED.md`](TRUSTED.md) and covered by differential tests
instead of proofs. Everything else is integer arithmetic inside the verified
region.

## Building, testing, verifying

```sh
cargo build                       # plain rustc; the verus! macro erases ghost code
cargo test --all-features         # differential + property + adversarial suites
cargo clippy --all-targets --all-features -- -D warnings
./scripts/check-no-lgpl.sh        # malachite must never leave the dev tree
VERUS=/path/to/verus ./scripts/verify.sh
```

The same sources serve both toolchains: `cargo build` compiles them with rustc,
and `verus` re-reads them with the ghost code kept.

`verus` needs a matching Rust toolchain — currently 1.97.1 with the `rustc-dev`
and `llvm-tools` components — plus Z3 4.12.5. See `.github/workflows/ci.yml` for
a working installation.

## Test harness

* **Oracle** — `malachite-q` (arbitrary precision) computes what `Q` *should*
  have computed. Every operation is checked against it on 20k random inputs per
  direction plus an exhaustive small-input sweep, asserting the full contract:
  exact when the exact result fits, correctly directed, within the R3 bound
  otherwise, canonical and bounded always.
* **Properties** — canonicality after every operation, commutativity, the total
  order laws, involutions, unit-interval closure, `serde` round-trip, and
  bit-identical results across repeated runs and across threads.
* **Adversarial** — budget-edge values (`den = 2^62 − 1`), sign edges, the
  `i64::MIN` exclusion, saturation, division by negative divisors, and a 10⁴-step
  fold tracked against the oracle.

`malachite-q` is LGPL-3.0-only and is a **dev-dependency only**;
`scripts/check-no-lgpl.sh` fails the build if it ever reaches the distributable
dependency tree.

## Licensing note

The crate currently carries `AGPL-3.0-only`, inherited from the host repository.
That is worth a second look before this is used as intended: the whole reason
`malachite-q` was rejected is that LGPL-3.0 blocks static linking into
proprietary binaries — and AGPL is *more* restrictive than LGPL for that use
case, not less. If this crate is to be the numeric backbone of a statically
linked proprietary engine, it needs a permissive licence (MIT/Apache-2.0). That
is the repository owner's call, not something this crate should decide for
itself.
