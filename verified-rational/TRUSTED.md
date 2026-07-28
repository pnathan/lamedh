# Trusted computing base

This crate contains **zero `assume` and zero `admit`**. Everything that is
verified is verified from first principles against Verus's ghost `int`/`nat`.

What follows is the complete list of everything the proofs *do* rest on that is
not itself proved here. It is short on purpose.

---

## 1. `src/float.rs` — the `f64` boundary

The whole module is marked `#[verifier::external]` in `src/lib.rs`. Verus has no
model of IEEE-754 floating point, so nothing here could be proved without first
axiomatising `f64` — which would move the trust, not remove it.

### 1.1 `decompose(v: f64) -> Option<(i64, i32)>`

**Assumed specification.** For finite `v`, returns `Some((m, e))` with
`v == m · 2^e` exactly, `|m| ≤ 2^53`. Returns `None` for NaN and ±∞.

**What is actually trusted.** Only the IEEE-754 binary64 memory layout: that
`f64::to_bits` yields a `u64` whose bit 63 is the sign, bits 62..52 are the
biased exponent, and bits 51..0 are the fraction; that a biased exponent of
`0x7ff` means NaN or infinity; that a biased exponent of `0` means a subnormal
with no implicit leading bit and a fixed exponent of `-1074`; and that any other
biased exponent `b` means an implicit leading `1` at bit 52 with exponent
`b - 1075`.

Everything after `to_bits` is integer shifting and masking. There is no float
arithmetic in this function. `to_bits` itself is a bit-cast, not a computation.

**Backing tests.** `tests/oracle.rs::from_f64_respects_the_contract` converts a
fixture set spanning zero, ±1, short decimals, π, e, `f64::MIN_POSITIVE`, the
smallest subnormal `5e-324`, and `1e18`, and asserts against `malachite_q`'s own
`Rational::try_from(f64)` — an independent implementation of the same
decomposition — that the R3 bound and the R2 direction both hold.

### 1.2 `from_f64_dir(v: f64, dir: Dir) -> Option<Q>`

**Assumed specification.** `None` for NaN, ±∞, and any magnitude past
`2^62 − 1`. Otherwise the result satisfies R2 and R3 against the exact real
value of `v`.

**What is trusted beyond §1.1.** Two flush rules:

* *Magnitude rejection.* For `e ≥ 0` the function computes `|m| << e` in `i128`
  and rejects anything past the budget. This is exact integer arithmetic; the
  only trusted part is that `e ≤ 62` and `|m| ≤ 2^53` keep the shift inside
  `i128` (it reaches at most `2^115`).
* *Underflow flush.* For `e < -124` the exact value satisfies `|v| < 2^-72`, and
  the function returns `0` — or `±1/2^61` in the directed mode that must not
  cross the true value. R3 holds with enormous room (`2^-72 ≪ 2^-60`) and
  directedness holds by construction. This reasoning is written out here rather
  than machine-checked, because carrying a `2^1074` denominator into the
  verified core to prove a triviality is not a good trade.

Everything in the representable range is handed to `Q::from_dyadic`, which is
**fully verified** and carries the same `rounded` contract as every arithmetic
operation. That is the point of the split: the trusted part decides *which*
integers to pass, and the verified part does all the arithmetic.

### 1.3 `to_f64(q: Q) -> f64`

**Assumed specification.** Returns an `f64` approximation of `q`. **No error
bound is claimed, proved, or implied.**

This is the one function in the crate that performs float arithmetic. It exists
for display and DTO boundaries only. A value that has been through it must never
re-enter `Q` arithmetic — that is exactly how order-dependence gets back in.

**Backing tests.** `tests/oracle.rs::to_f64_is_close_enough_for_display` checks
20 000 random values against the oracle within `2^-45` relative, which is a
display-grade tolerance and is deliberately looser than anything the verified
core promises.

---

## 2. `src/traits.rs` — standard-library trait impls

Marked `#[verifier::external]`. Verus does not model `Ord`, `Display`, `Default`
or `serde`. None of these functions does arithmetic of its own beyond a single
`i128` cross-multiplication in `Ord::cmp`, which is the same expression the
verified `Q::le` uses and is bounded by the same `(2^62−1)² < 2^124` argument.

**Backing tests.** `tests/properties.rs::the_order_is_a_total_order` checks that
`Ord` agrees with the verified `Q::le` and satisfies the order axioms;
`derived_ord_would_have_been_wrong` guards the decision not to derive it;
`hash_agrees_with_equality` and `serde_round_trips_exactly` cover the rest.

---

## 3. The verifier and its dependencies

Standard for any Verus development, listed for completeness:

* **Verus itself** — the translation from Rust to SMT, and its axiomatisation of
  machine integers.
* **Z3 4.12.5** — the SMT solver.
* **`vstd`** — the Verus standard library. This crate uses its `arithmetic`
  module: `div_mod` (`lemma_fundamental_div_mod`, `lemma_mod_bound`,
  `lemma_div_pos_is_pos`, `lemma_div_is_ordered_by_denominator`,
  `lemma_div_basics`), `power2` (`pow2`, `lemma_pow2_adds`, `lemma_pow2_pos`,
  `lemma2_to64`, `lemma2_to64_rest`), `power`, and `mul`
  (`group_mul_properties`). `vstd` is itself verified as part of the Verus
  build.
* **rustc** — the compiler that turns the ghost-erased sources into the binary
  that actually runs. Verus verifies the source; it does not verify the code
  generator.

No third-party crate is trusted for arithmetic. `malachite-q` appears only as a
differential-test oracle and is excluded from the distributable dependency tree
by `scripts/check-no-lgpl.sh`.

---

## 4. Not trusted, but not yet proved

These are stated in `README.md` under "Verification status". They are listed
again here so this file is a complete account of what a reader should not assume:

* **R4 (monotone rounding).** True, argued in the README, not machine-checked.
* **Bit-level commutativity and determinism of the compiled operations.** The
  proofs establish that commuted operands reach identical rounding with
  identical inputs; that the compiled function is a function is covered by
  `tests/properties.rs`, not by a proof.
* **V7 (Lipschitz lemmas)** and **V8 (n-ary accumulated error bound)** — SHOULD
  tier, not started and not proved respectively. V8 is exercised by a 10⁴-step
  differential test.
