//! Differential tests against `malachite-q`, an arbitrary-precision rational.
//!
//! For every operation the oracle computes the exact answer with no budget, and
//! the test asserts the full rounding contract against it: exact when the exact
//! result fits (R1), correctly directed (R2), within `2^-60 * max(1, |v|)`
//! otherwise (R3), and canonical and bounded always (I1/I2).

mod common;

use common::*;
use malachite_base::num::basic::traits::One;

use malachite_q::Rational;
use verified_rational::{Dir, Q};

const DIRS: [Dir; 3] = [Dir::Down, Dir::Up, Dir::Nearest];

#[test]
fn add_matches_oracle() {
    let mut rng = Rng::new(0x5eed_0001);
    for _ in 0..20_000 {
        let (a, b) = (rng.q(), rng.q());
        let want = exact(a) + exact(b);
        for dir in DIRS {
            check_contract(a.add_dir(b, dir), &want, dir, "add");
        }
    }
}

#[test]
fn sub_matches_oracle() {
    let mut rng = Rng::new(0x5eed_0002);
    for _ in 0..20_000 {
        let (a, b) = (rng.q(), rng.q());
        let want = exact(a) - exact(b);
        for dir in DIRS {
            check_contract(a.sub_dir(b, dir), &want, dir, "sub");
        }
    }
}

#[test]
fn mul_matches_oracle() {
    let mut rng = Rng::new(0x5eed_0003);
    for _ in 0..20_000 {
        let (a, b) = (rng.q(), rng.q());
        let want = exact(a) * exact(b);
        for dir in DIRS {
            check_contract(a.mul_dir(b, dir), &want, dir, "mul");
        }
    }
}

#[test]
fn div_matches_oracle() {
    let mut rng = Rng::new(0x5eed_0004);
    for _ in 0..20_000 {
        let (a, b) = (rng.q(), rng.q());
        if b.is_zero() {
            continue;
        }
        let want = exact(a) / exact(b);
        for dir in DIRS {
            check_contract(a.div_dir(b, dir), &want, dir, "div");
        }
    }
}

#[test]
fn exhaustive_small_inputs_are_exact() {
    // Everything in this range reduces well inside the budget, so R1 says every
    // operation must be *exact* — not merely within the bound.
    for an in -12i64..=12 {
        for ad in 1i64..=12 {
            let Some(a) = Q::new(an, ad) else { continue };
            for bn in -12i64..=12 {
                for bd in 1i64..=12 {
                    let Some(b) = Q::new(bn, bd) else { continue };
                    assert_exact(a.add(b), &(exact(a) + exact(b)), "small add");
                    assert_exact(a.sub(b), &(exact(a) - exact(b)), "small sub");
                    assert_exact(a.mul(b), &(exact(a) * exact(b)), "small mul");
                    if !b.is_zero() {
                        assert_exact(a.div(b), &(exact(a) / exact(b)), "small div");
                    }
                    assert_eq!(a.le(b), exact(a) <= exact(b), "small le");
                    assert_eq!(a.lt(b), exact(a) < exact(b), "small lt");
                    assert_eq!(a.eq_exact(b), exact(a) == exact(b), "small eq");
                }
            }
        }
    }
}

#[test]
fn comparison_matches_oracle() {
    let mut rng = Rng::new(0x5eed_0005);
    for _ in 0..50_000 {
        let (a, b) = (rng.q(), rng.q());
        assert_eq!(a.le(b), exact(a) <= exact(b), "le on {a} vs {b}");
        assert_eq!(a.lt(b), exact(a) < exact(b), "lt on {a} vs {b}");
        assert_eq!(a.eq_exact(b), exact(a) == exact(b), "eq on {a} vs {b}");
        assert_eq!(a.cmp(&b), exact(a).cmp(&exact(b)), "cmp on {a} vs {b}");
        // Structural equality is mathematical equality, because of canonicality.
        assert_eq!(a == b, exact(a) == exact(b), "derived Eq on {a} vs {b}");
    }
}

#[test]
fn exact_operations_are_exact() {
    let mut rng = Rng::new(0x5eed_0006);
    for _ in 0..50_000 {
        let a = rng.q();
        assert_exact(a.neg(), &-exact(a), "neg");
        assert_exact(
            a.abs(),
            &malachite_base::num::arithmetic::traits::Abs::abs(exact(a)),
            "abs",
        );
        if !a.is_zero() {
            assert_exact(a.recip(), &(Rational::from(1) / exact(a)), "recip");
        }
        let b = rng.q();
        assert_exact(a.min(b), &exact(a).min(exact(b)), "min");
        assert_exact(a.max(b), &exact(a).max(exact(b)), "max");
    }
}

#[test]
fn long_fold_error_stays_within_the_accumulated_bound() {
    // 10^4 sequential operations, the worst case the size analysis describes.
    // The accumulated bound is k * 2^-60 * max(1, |v|).
    let mut rng = Rng::new(0x5eed_0007);
    for _ in 0..20 {
        let mut acc = Q::from_decimal(5, 1).unwrap();
        let mut oracle = rat(5, 10);
        let mut steps: i64 = 0;
        for _ in 0..10_000 {
            let x = Q::from_decimal(rng.positive(9999), 4).unwrap();
            acc = acc.mul(x);
            oracle *= exact(x);
            steps += 1;
            if acc.is_zero() {
                break;
            }
        }
        check_canonical(acc, "long fold");
        let err = malachite_base::num::arithmetic::traits::Abs::abs(exact(acc) - oracle.clone());
        let mag = malachite_base::num::arithmetic::traits::Abs::abs(oracle.clone());
        let scale = if mag > Rational::ONE { mag } else { Rational::ONE };
        let bound = Rational::from(steps) * eps() * scale;
        assert!(
            err <= bound,
            "long fold drifted past the accumulated bound: {err} > {bound}"
        );
    }
}

#[test]
fn from_decimal_is_exact() {
    for places in 0u8..=18 {
        for m in [-99991i64, -1, 0, 1, 85, 12345, 999_999_937] {
            let Some(q) = Q::from_decimal(m, places) else {
                continue;
            };
            check_canonical(q, "from_decimal");
            let want = Rational::from_integers(m.into(), 10i64.pow(places as u32).into());
            assert_exact(q, &want, "from_decimal");
        }
    }
    assert!(Q::from_decimal(1, 19).is_none(), "10^19 is past the budget");
}

#[test]
fn from_f64_respects_the_contract() {
    let cases: [f64; 14] = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        0.1,
        -0.85,
        1e-30,
        -1e-30,
        1e18,
        f64::MIN_POSITIVE,
        5e-324,
        core::f64::consts::PI,
        core::f64::consts::E,
    ];
    for v in cases {
        for dir in DIRS {
            let q = verified_rational::from_f64_dir(v, dir).expect("finite");
            check_canonical(q, "from_f64");
            let want = Rational::try_from(v).expect("finite");
            let g = exact(q);
            let err = malachite_base::num::arithmetic::traits::Abs::abs(g.clone() - want.clone());
            assert!(err <= r3_bound(&want), "from_f64({v}, {dir:?}): {err} too large");
            match dir {
                Dir::Down => assert!(g <= want, "from_f64 Down overshot on {v}"),
                Dir::Up => assert!(g >= want, "from_f64 Up undershot on {v}"),
                Dir::Nearest => {}
            }
        }
    }
    assert!(verified_rational::from_f64(f64::NAN).is_none());
    assert!(verified_rational::from_f64(f64::INFINITY).is_none());
    assert!(verified_rational::from_f64(f64::NEG_INFINITY).is_none());
    assert!(verified_rational::from_f64(1e30).is_none(), "past 2^63");
}

#[test]
fn to_f64_is_close_enough_for_display() {
    // to_f64 is the one trusted boundary. It is not proved; it is tested.
    let mut rng = Rng::new(0x5eed_0008);
    for _ in 0..20_000 {
        let q = rng.q();
        let got = verified_rational::to_f64(q);
        if !got.is_finite() {
            continue;
        }
        let want = exact(q);
        let back = Rational::try_from(got).expect("finite");
        let err = malachite_base::num::arithmetic::traits::Abs::abs(back - want.clone());
        let mag = malachite_base::num::arithmetic::traits::Abs::abs(want);
        let scale = if mag > Rational::ONE { mag } else { Rational::ONE };
        // Two f64 roundings (numerator, denominator) plus the division.
        let tol = Rational::from_integers(1i64.into(), (1i64 << 45).into()) * scale;
        assert!(err <= tol, "to_f64({q}) = {got}, error {err} > {tol}");
    }
}
