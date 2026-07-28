//! Shared oracle plumbing for the differential tests.
//!
//! `malachite-q` is an arbitrary-precision rational, so it computes what `Q`
//! *should* have computed with no budget at all. Every test here compares
//! against that, never against another `Q` computation.
//!
//! malachite is LGPL-3.0-only and therefore a **dev-dependency only**; see
//! `scripts/check-no-lgpl.sh`, which fails the build if it ever appears in the
//! distributable dependency tree.

#![allow(dead_code)]

use malachite_base::num::arithmetic::traits::Abs;
use malachite_base::num::basic::traits::{One, Zero};
use malachite_q::Rational;
use verified_rational::{Dir, Q};

/// `2^62 - 1`, the width budget.
pub const BUDGET: i64 = 0x3fff_ffff_ffff_ffff;

/// Exact value of a `Q` as an oracle rational.
pub fn exact(q: Q) -> Rational {
    Rational::from_integers(q.numerator().into(), q.denominator().into())
}

/// Exact value of an arbitrary `(num, den)` pair.
pub fn rat(num: i64, den: i64) -> Rational {
    Rational::from_integers(num.into(), den.into())
}

/// `2^-60`.
pub fn eps() -> Rational {
    Rational::from_integers(1i64.into(), (1i64 << 60).into())
}

/// The R3 bound for an exact value: `2^-60 * max(1, |exact|)`.
pub fn r3_bound(exact_value: &Rational) -> Rational {
    let mag = (*exact_value).clone().abs();
    let scale = if mag > Rational::ONE { mag } else { Rational::ONE };
    eps() * scale
}

/// Assert that `got` is within the R3 bound of `want`, and report usefully when
/// it is not.
pub fn assert_within_r3(got: Q, want: &Rational, what: &str) {
    let g = exact(got);
    let err = (g.clone() - want.clone()).abs();
    let bound = r3_bound(want);
    assert!(
        err <= bound,
        "{what}: {got} = {g} is outside the R3 bound of {want}\n  error {err} > bound {bound}"
    );
}

/// Assert that `got` is *exactly* `want` — the R1 obligation.
pub fn assert_exact(got: Q, want: &Rational, what: &str) {
    let g = exact(got);
    assert_eq!(&g, want, "{what}: expected the exact value, got {got}");
}

/// Does the exact reduced form of `want` fit the budget? If so R1 applies and
/// the operation must be exact.
pub fn fits_budget(want: &Rational) -> bool {
    let budget = Rational::from(BUDGET);
    let nn = Rational::from(want.numerator_ref().clone());
    let dd = Rational::from(want.denominator_ref().clone());
    nn <= budget && dd <= budget
}

/// Is the magnitude outside the representable range (so the result saturates
/// and R2/R3 are switched off)?
pub fn saturates(want: &Rational) -> bool {
    (*want).clone().abs() >= BUDGET
}

/// Check the full rounding contract for one operation result.
pub fn check_contract(got: Q, want: &Rational, dir: Dir, what: &str) {
    check_canonical(got, what);
    if saturates(want) {
        let sat = if *want >= Rational::ZERO {
            Q::max_value()
        } else {
            Q::min_value()
        };
        assert_eq!(got, sat, "{what}: expected saturation, got {got}");
        return;
    }
    if fits_budget(want) {
        assert_exact(got, want, what);
    }
    assert_within_r3(got, want, what);
    let g = exact(got);
    match dir {
        Dir::Down => assert!(g <= *want, "{what}: Dir::Down overshot ({g} > {want})"),
        Dir::Up => assert!(g >= *want, "{what}: Dir::Up undershot ({g} < {want})"),
        Dir::Nearest => {}
    }
}

/// Check I1 (canonical) and I2 (bounded) on a value.
pub fn check_canonical(q: Q, what: &str) {
    let (n, d) = (q.numerator(), q.denominator());
    assert!(d > 0, "{what}: denominator {d} is not positive");
    assert!(d <= BUDGET, "{what}: denominator {d} exceeds the budget");
    assert!(
        n.checked_abs().map(|a| a <= BUDGET).unwrap_or(false),
        "{what}: numerator {n} exceeds the budget"
    );
    assert_eq!(gcd(n.unsigned_abs(), d as u64), 1, "{what}: {q} is not reduced");
    if n == 0 {
        assert_eq!(d, 1, "{what}: zero must be 0/1, got {q}");
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// A tiny deterministic PRNG. Deterministic on purpose: a failing seed is a
/// reproducible failing seed, and this crate's whole claim is reproducibility.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A value in `-range ..= range`.
    pub fn signed(&mut self, range: i64) -> i64 {
        let v = (self.next_u64() % (2 * range as u64 + 1)) as i64;
        v - range
    }

    /// A value in `1 ..= range`.
    pub fn positive(&mut self, range: i64) -> i64 {
        (self.next_u64() % range as u64) as i64 + 1
    }

    /// A `Q` drawn from a mix of small, medium and budget-edge magnitudes.
    pub fn q(&mut self) -> Q {
        let shape = self.next_u64() % 4;
        let (nr, dr) = match shape {
            0 => (10, 10),                 // tiny
            1 => (10_000, 10_000),         // short-decimal scale
            2 => (1 << 40, 1 << 40),       // mid
            _ => (BUDGET, BUDGET),         // budget edge
        };
        loop {
            let n = self.signed(nr);
            let d = self.positive(dr);
            if let Some(q) = Q::new(n, d) {
                return q;
            }
        }
    }
}
