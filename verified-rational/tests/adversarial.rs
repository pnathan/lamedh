//! Adversarial fixtures: the specific values most likely to break the width
//! analysis, the sign handling, or the canonicalization.

mod common;

use common::*;
use malachite_q::Rational;
use verified_rational::{Dir, Q};

const DIRS: [Dir; 3] = [Dir::Down, Dir::Up, Dir::Nearest];

/// Values chosen to sit on the edges: the budget itself, one either side of it,
/// powers of two around the grid boundaries, and the smallest and largest
/// representable magnitudes.
fn edge_values() -> Vec<Q> {
    let mut v = Vec::new();
    let raw: [(i64, i64); 22] = [
        (0, 1),
        (1, 1),
        (-1, 1),
        (BUDGET, 1),
        (-BUDGET, 1),
        (1, BUDGET),
        (-1, BUDGET),
        (BUDGET, BUDGET - 1),
        (BUDGET - 1, BUDGET),
        (1, 1 << 61),
        (1, (1 << 61) + 1),
        ((1 << 61) - 1, 1 << 61),
        ((1 << 62) - 1, (1 << 62) - 2),
        (1 << 31, (1 << 31) + 1),
        (-(1 << 61), 3),
        (7, 1 << 60),
        (2, 4),
        (-6, 9),
        (17, 20),
        (85, 100),
        (1, 3),
        (2, 3),
    ];
    for (n, d) in raw {
        if let Some(q) = Q::new(n, d) {
            v.push(q);
        }
    }
    v
}

#[test]
fn budget_edges_obey_the_contract() {
    let vals = edge_values();
    for &a in &vals {
        check_canonical(a, "edge value");
        for &b in &vals {
            for dir in DIRS {
                check_contract(a.add_dir(b, dir), &(exact(a) + exact(b)), dir, "edge add");
                check_contract(a.sub_dir(b, dir), &(exact(a) - exact(b)), dir, "edge sub");
                check_contract(a.mul_dir(b, dir), &(exact(a) * exact(b)), dir, "edge mul");
                if !b.is_zero() {
                    check_contract(a.div_dir(b, dir), &(exact(a) / exact(b)), dir, "edge div");
                }
            }
        }
    }
}

#[test]
fn i64_min_is_excluded_not_mishandled() {
    // |i64::MIN| overflows, so I2's bound of 2^62 - 1 keeps it out of the type
    // entirely. The constructors must say so rather than wrap.
    assert!(Q::new(i64::MIN, 1).is_none());
    assert!(Q::new(1, i64::MIN).is_none());
    assert!(Q::new(i64::MIN, i64::MIN).is_some(), "MIN/MIN reduces to 1/1");
    assert_eq!(Q::new(i64::MIN, i64::MIN).unwrap(), Q::one());
    assert!(Q::from_int(i64::MIN).is_none());
    assert!(Q::from_int(i64::MAX).is_none(), "2^63-1 is past the budget");
    assert!(Q::from_int(BUDGET).is_some());
    assert!(Q::from_int(-BUDGET).is_some());
    assert!(Q::from_int(BUDGET + 1).is_none());

    // i64::MIN as a *denominator* after sign flipping is the classic trap.
    for n in [-3i64, -1, 0, 1, 3] {
        assert!(Q::new(n, i64::MIN).is_none() || Q::new(n, i64::MIN).is_some());
        if let Some(q) = Q::new(n, i64::MIN) {
            check_canonical(q, "n / i64::MIN");
        }
    }
}

#[test]
fn zero_denominator_is_rejected() {
    assert!(Q::new(0, 0).is_none());
    assert!(Q::new(1, 0).is_none());
    assert!(Q::new(-1, 0).is_none());
}

#[test]
fn zero_is_canonical_however_it_arises() {
    for d in [1i64, 2, 7, BUDGET] {
        assert_eq!(Q::new(0, d).unwrap(), Q::zero());
        assert_eq!(Q::new(0, -d).unwrap(), Q::zero());
    }
    let a = Q::new(3, 7).unwrap();
    assert_eq!(a.sub(a), Q::zero());
    assert_eq!(a.mul(Q::zero()), Q::zero());
    assert_eq!(Q::zero().numerator(), 0);
    assert_eq!(Q::zero().denominator(), 1);
}

#[test]
fn signs_are_normalised_into_the_numerator() {
    let cases = [(1i64, -2i64), (-1, 2), (-1, -2), (3, -9), (-3, 9)];
    for (n, d) in cases {
        let q = Q::new(n, d).unwrap();
        assert!(q.denominator() > 0, "{q} has a non-positive denominator");
        check_canonical(q, "sign normalisation");
        let want = Rational::from_integers(n.into(), d.into());
        assert_exact(q, &want, "sign normalisation");
    }
}

#[test]
fn saturation_is_reported_as_saturation() {
    // The one case where R2/R3 do not apply. It must be reachable, obvious, and
    // land exactly on the extremum rather than wrapping.
    let big = Q::from_int(BUDGET).unwrap();
    let two = Q::from_int(2).unwrap();
    let over = big.mul(two);
    assert_eq!(over, Q::max_value(), "positive overflow must saturate");
    assert_eq!(big.neg().mul(two), Q::min_value(), "negative overflow must saturate");
    check_canonical(over, "saturated");
    // Just under the ceiling is still exact.
    let half = Q::new(1, 2).unwrap();
    assert_eq!(big.mul(half).mul(two), big, "exact round trip below the ceiling");
}

#[test]
fn division_by_a_negative_divisor_keeps_the_sign_straight() {
    let mut rng = Rng::new(0xdead_0001);
    for _ in 0..20_000 {
        let a = rng.q();
        let b = rng.q();
        if b.is_zero() {
            continue;
        }
        let nb = b.neg();
        let want = exact(a) / exact(nb);
        for dir in DIRS {
            check_contract(a.div_dir(nb, dir), &want, dir, "div by negative");
        }
    }
}

#[test]
fn directed_modes_bracket_the_exact_value() {
    // The property a future interval type needs: Down <= exact <= Up, always.
    let mut rng = Rng::new(0xdead_0002);
    for _ in 0..30_000 {
        let (a, b) = (rng.q(), rng.q());
        for (lo, hi, want, name) in [
            (
                a.add_dir(b, Dir::Down),
                a.add_dir(b, Dir::Up),
                exact(a) + exact(b),
                "add",
            ),
            (
                a.mul_dir(b, Dir::Down),
                a.mul_dir(b, Dir::Up),
                exact(a) * exact(b),
                "mul",
            ),
        ] {
            if saturates(&want) {
                continue;
            }
            assert!(exact(lo) <= want, "{name}: Down bound {lo} exceeded the exact value");
            assert!(want <= exact(hi), "{name}: Up bound {hi} fell short");
            assert!(lo.le(hi), "{name}: the bracket is inverted");
        }
    }
}

#[test]
fn long_chains_of_short_decimals_stay_exact_while_they_fit() {
    // The R1 consequence the design is sold on: a small investigation pays no
    // rounding at all. Sixteen two-place decimals multiplied together still
    // reduce inside the budget.
    let mut acc = Q::one();
    let mut oracle = Rational::from(1);
    for k in 1..=16i64 {
        let x = Q::from_decimal(k * 5, 2).unwrap();
        acc = acc.mul(x);
        oracle *= exact(x);
        if fits_budget(&oracle) {
            assert_exact(acc, &oracle, "short-decimal chain");
        }
    }
}

#[test]
fn from_decimal_rejects_what_it_cannot_represent() {
    assert!(Q::from_decimal(1, 19).is_none());
    assert!(Q::from_decimal(1, 255).is_none());
    assert!(Q::from_decimal(i64::MAX, 0).is_none(), "2^63-1 is past the budget");
    assert!(Q::from_decimal(i64::MIN, 0).is_none());
    // 18 places is the widest that fits.
    assert!(Q::from_decimal(1, 18).is_some());
}
