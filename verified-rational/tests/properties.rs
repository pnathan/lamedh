//! Property tests: the invariants and laws that must hold for every value, and
//! the ones the verifier cannot state because they are about the *compiled*
//! function rather than its contract.
//!
//! Bit-level determinism and commutativity of the implementation live here
//! rather than in the proofs, for a specific reason: Verus models a call to an
//! `exec` function by its postcondition, so two calls with equal arguments are
//! not automatically provably equal. The proofs establish that `add(a, b)` and
//! `add(b, a)` present identical exact fractions to identical rounding; these
//! tests confirm the compiled code turns that into identical bits.

mod common;

use common::*;
use verified_rational::{Dir, Q};

const DIRS: [Dir; 3] = [Dir::Down, Dir::Up, Dir::Nearest];

#[test]
fn every_result_is_canonical_and_bounded() {
    let mut rng = Rng::new(0xc0ff_ee01);
    for _ in 0..30_000 {
        let (a, b) = (rng.q(), rng.q());
        for dir in DIRS {
            check_canonical(a.add_dir(b, dir), "add");
            check_canonical(a.sub_dir(b, dir), "sub");
            check_canonical(a.mul_dir(b, dir), "mul");
            if !b.is_zero() {
                check_canonical(a.div_dir(b, dir), "div");
            }
        }
        check_canonical(a.neg(), "neg");
        check_canonical(a.abs(), "abs");
        if !a.is_zero() {
            check_canonical(a.recip(), "recip");
        }
        check_canonical(a.min(b), "min");
        check_canonical(a.max(b), "max");
    }
}

#[test]
fn add_and_mul_are_commutative() {
    let mut rng = Rng::new(0xc0ff_ee02);
    for _ in 0..50_000 {
        let (a, b) = (rng.q(), rng.q());
        for dir in DIRS {
            assert_eq!(a.add_dir(b, dir), b.add_dir(a, dir), "add({a}, {b}) not commutative");
            assert_eq!(a.mul_dir(b, dir), b.mul_dir(a, dir), "mul({a}, {b}) not commutative");
        }
    }
}

#[test]
fn results_are_bit_identical_across_repeated_runs() {
    // The crate's headline claim over f64 is reproducibility. Same inputs, same
    // bits, every time — including across threads.
    let mut rng = Rng::new(0xc0ff_ee03);
    let inputs: Vec<(Q, Q)> = (0..2_000).map(|_| (rng.q(), rng.q())).collect();
    let run = |inputs: &[(Q, Q)]| -> Vec<(i64, i64)> {
        inputs
            .iter()
            .flat_map(|&(a, b)| {
                let s = a.add(b);
                let p = a.mul(b);
                [
                    (s.numerator(), s.denominator()),
                    (p.numerator(), p.denominator()),
                ]
            })
            .collect()
    };
    let first = run(&inputs);
    assert_eq!(first, run(&inputs), "second run differed");
    let inputs2 = inputs.clone();
    let handle = std::thread::spawn(move || run(&inputs2));
    assert_eq!(first, handle.join().unwrap(), "other thread differed");
}

#[test]
fn negation_and_reciprocal_are_involutions() {
    let mut rng = Rng::new(0xc0ff_ee04);
    for _ in 0..50_000 {
        let a = rng.q();
        assert_eq!(a.neg().neg(), a, "neg is not an involution on {a}");
        assert_eq!(a.abs().abs(), a.abs(), "abs is not idempotent on {a}");
        if !a.is_zero() {
            assert_eq!(a.recip().recip(), a, "recip is not an involution on {a}");
        }
    }
}

#[test]
fn the_order_is_a_total_order() {
    let mut rng = Rng::new(0xc0ff_ee05);
    for _ in 0..20_000 {
        let (a, b, c) = (rng.q(), rng.q(), rng.q());
        assert!(a.le(a), "reflexivity");
        assert!(a.le(b) || b.le(a), "totality");
        if a.le(b) && b.le(a) {
            assert_eq!(a, b, "antisymmetry: {a} and {b}");
        }
        if a.le(b) && b.le(c) {
            assert!(a.le(c), "transitivity: {a} <= {b} <= {c}");
        }
        // The manual Ord must agree with the exact comparison.
        assert_eq!(a <= b, a.le(b), "Ord disagrees with le");
    }
}

#[test]
fn derived_ord_would_have_been_wrong() {
    // Guards the decision not to derive Ord: the derived lexicographic order on
    // (num, den) reports 1/3 > 1/2. Getting this wrong would be silent.
    let third = Q::new(1, 3).unwrap();
    let half = Q::new(1, 2).unwrap();
    assert!(third < half, "1/3 must be less than 1/2");
    assert!(third.numerator() == half.numerator() && third.denominator() > half.denominator());
}

#[test]
fn hash_agrees_with_equality() {
    use std::collections::HashMap;
    let mut rng = Rng::new(0xc0ff_ee06);
    let mut seen: HashMap<Q, (i64, i64)> = HashMap::new();
    for _ in 0..20_000 {
        let a = rng.q();
        // A different unreduced spelling of the same value must land in the
        // same bucket, because canonicalization happens at construction.
        if let Some(scaled) = a
            .numerator()
            .checked_mul(3)
            .zip(a.denominator().checked_mul(3))
            .and_then(|(n, d)| Q::new(n, d))
        {
            assert_eq!(scaled, a, "3n/3d must canonicalize back to {a}");
        }
        seen.insert(a, (a.numerator(), a.denominator()));
    }
    for (k, v) in &seen {
        assert_eq!((k.numerator(), k.denominator()), *v);
    }
}

#[test]
fn unit_interval_values_stay_in_the_unit_interval() {
    // The property the fusion engine's clamp logic depends on: rounding a
    // belief/disbelief/uncertainty never pushes it out of [0, 1].
    let mut rng = Rng::new(0xc0ff_ee07);
    let zero = Q::zero();
    let one = Q::one();
    for _ in 0..30_000 {
        let d = rng.positive(BUDGET);
        let n = (rng.next_u64() % (d as u64 + 1)) as i64;
        let Some(a) = Q::new(n, d) else { continue };
        assert!(a.in_unit_interval(), "{a} should be in [0, 1]");
        let Some(b) = Q::new((rng.next_u64() % (d as u64 + 1)) as i64, d) else {
            continue;
        };
        for dir in DIRS {
            let p = a.mul_dir(b, dir);
            assert!(p.in_unit_interval(), "{a} * {b} = {p} left [0, 1]");
            let c = a.clamp(zero, one);
            assert!(c.in_unit_interval(), "clamp left [0, 1]");
        }
    }
}

#[test]
fn nary_helpers_agree_with_folds() {
    let mut rng = Rng::new(0xc0ff_ee08);
    for _ in 0..2_000 {
        let n = (rng.next_u64() % 8) as usize;
        let xs: Vec<Q> = (0..n).map(|_| rng.q()).collect();
        let want_sum = xs.iter().fold(Q::zero(), |acc, &x| acc.add(x));
        let want_prod = xs.iter().fold(Q::one(), |acc, &x| acc.mul(x));
        assert_eq!(Q::sum(&xs), want_sum, "sum is not the left fold");
        assert_eq!(Q::product(&xs), want_prod, "product is not the left fold");
    }
}

#[test]
fn weighted_mean_matches_its_definition() {
    let mut rng = Rng::new(0xc0ff_ee09);
    for _ in 0..2_000 {
        let n = (rng.next_u64() % 6) as usize + 1;
        let pairs: Vec<(Q, Q)> = (0..n).map(|_| (rng.q(), rng.q())).collect();
        let num = pairs.iter().fold(Q::zero(), |acc, &(w, x)| acc.add(w.mul(x)));
        let den = pairs.iter().fold(Q::zero(), |acc, &(w, _)| acc.add(w));
        match Q::weighted_mean(&pairs) {
            None => assert!(den.is_zero(), "None only when the weights vanish"),
            Some(m) => {
                assert!(!den.is_zero());
                assert_eq!(m, num.div(den));
                check_canonical(m, "weighted_mean");
            }
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_exactly() {
    let mut rng = Rng::new(0xc0ff_ee0a);
    for _ in 0..20_000 {
        let a = rng.q();
        let json = serde_json::to_string(&a).unwrap();
        let back: Q = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back, "serde round trip lost {a} (json {json})");
    }
    // A zero denominator must be rejected, not smuggled past the invariant.
    assert!(serde_json::from_str::<Q>("[1,0]").is_err());
    // A non-canonical pair is accepted and canonicalized.
    let q: Q = serde_json::from_str("[2,4]").unwrap();
    assert_eq!(q, Q::new(1, 2).unwrap());
}
