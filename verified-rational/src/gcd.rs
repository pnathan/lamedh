//! Divisibility, the ghost GCD, and the executable Euclidean algorithm.
//!
//! This module discharges verification obligation **V5**: the `u128` Euclid
//! loop terminates and computes the mathematical GCD, where "the mathematical
//! GCD" is pinned down by *divisibility*-maximality (`lemma_gcd_greatest`),
//! not merely by `<=`-maximality. Divisibility-maximality is the form the
//! canonicalization proof actually needs.
//!
//! Everything here is stated over Verus's unbounded ghost `int`/`nat`, so none
//! of it inherits the machine-width limits of the `Q` representation.

use vstd::prelude::*;

verus! {

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::{lemma_fundamental_div_mod, lemma_mod_bound};

/// `d` divides `n`: there is an integer `k` with `n == d * k`.
///
/// Note `divides(0, 0)` holds and `divides(0, n)` fails for `n != 0`, which is
/// the standard convention and the one the GCD lemmas below assume.
pub open spec fn divides(d: int, n: int) -> bool {
    exists|k: int| n == #[trigger] (d * k)
}

/// Euclid's algorithm as a ghost function. Total on `nat` because `spec_gcd(a, 0) == a`.
pub open spec fn spec_gcd(a: nat, b: nat) -> nat
    decreases b,
{
    if b == 0 {
        a
    } else {
        spec_gcd(b, (a % b) as nat)
    }
}

/// `|x|` as a `nat`. Used to state the canonicality invariant, which reduces
/// the *magnitude* of the numerator against the denominator.
pub open spec fn iabs(x: int) -> nat {
    if x >= 0 {
        x as nat
    } else {
        (-x) as nat
    }
}

// ---------------------------------------------------------------------------
// Divisibility algebra
// ---------------------------------------------------------------------------
/// Everything divides `0`.
pub proof fn lemma_divides_zero(d: int)
    ensures
        divides(d, 0),
{
    assert(0 == d * 0) by (nonlinear_arith);
}

/// Everything divides itself.
pub proof fn lemma_divides_refl(d: int)
    ensures
        divides(d, d),
{
    assert(d == d * 1) by (nonlinear_arith);
}

/// Divisibility is closed under taking multiples.
pub proof fn lemma_divides_mul(d: int, a: int, m: int)
    requires
        divides(d, a),
    ensures
        divides(d, a * m),
{
    let k = choose|k: int| a == #[trigger] (d * k);
    assert(a * m == d * (k * m)) by (nonlinear_arith)
        requires
            a == d * k,
    ;
}

/// Divisibility is closed under addition.
pub proof fn lemma_divides_add(d: int, a: int, b: int)
    requires
        divides(d, a),
        divides(d, b),
    ensures
        divides(d, a + b),
{
    let ka = choose|k: int| a == #[trigger] (d * k);
    let kb = choose|k: int| b == #[trigger] (d * k);
    assert(a + b == d * (ka + kb)) by (nonlinear_arith)
        requires
            a == d * ka,
            b == d * kb,
    ;
}

/// Divisibility is closed under subtraction.
pub proof fn lemma_divides_sub(d: int, a: int, b: int)
    requires
        divides(d, a),
        divides(d, b),
    ensures
        divides(d, a - b),
{
    let ka = choose|k: int| a == #[trigger] (d * k);
    let kb = choose|k: int| b == #[trigger] (d * k);
    assert(a - b == d * (ka - kb)) by (nonlinear_arith)
        requires
            a == d * ka,
            b == d * kb,
    ;
}

/// Divisibility is transitive.
pub proof fn lemma_divides_trans(d: int, a: int, b: int)
    requires
        divides(d, a),
        divides(a, b),
    ensures
        divides(d, b),
{
    let k1 = choose|k: int| a == #[trigger] (d * k);
    let k2 = choose|k: int| b == #[trigger] (a * k);
    assert(b == d * (k1 * k2)) by (nonlinear_arith)
        requires
            a == d * k1,
            b == a * k2,
    ;
}

/// A positive divisor of a positive number is no larger than it.
pub proof fn lemma_divides_le(d: int, n: int)
    requires
        divides(d, n),
        d > 0,
        n > 0,
    ensures
        d <= n,
{
    let k = choose|k: int| n == #[trigger] (d * k);
    assert(k >= 1) by (nonlinear_arith)
        requires
            n == d * k,
            d > 0,
            n > 0,
    ;
    assert(d * k >= d * 1) by (nonlinear_arith)
        requires
            k >= 1,
            d > 0,
    ;
}

/// Divisibility respects negation on the left.
pub proof fn lemma_divides_neg_left(d: int, n: int)
    requires
        divides(d, n),
    ensures
        divides(-d, n),
{
    let k = choose|k: int| n == #[trigger] (d * k);
    assert(n == (-d) * (-k)) by (nonlinear_arith)
        requires
            n == d * k,
    ;
}

// ---------------------------------------------------------------------------
// GCD correctness (V5)
// ---------------------------------------------------------------------------
/// `spec_gcd(a, b)` divides both arguments.
pub proof fn lemma_gcd_divides(a: nat, b: nat)
    ensures
        divides(spec_gcd(a, b) as int, a as int),
        divides(spec_gcd(a, b) as int, b as int),
    decreases b,
{
    if b == 0 {
        lemma_divides_refl(a as int);
        lemma_divides_zero(a as int);
    } else {
        let r = (a % b) as nat;
        lemma_mod_bound(a as int, b as int);
        lemma_gcd_divides(b, r);
        let g = spec_gcd(a, b) as int;
        assert(g == spec_gcd(b, r) as int);
        // g | b and g | (a % b); a = b*(a/b) + a%b, so g | a.
        lemma_fundamental_div_mod(a as int, b as int);
        lemma_divides_mul(g, b as int, a as int / b as int);
        lemma_divides_add(g, b as int * (a as int / b as int), r as int);
    }
}

/// Any common divisor of `a` and `b` divides `spec_gcd(a, b)`.
///
/// This is the *divisibility*-maximality characterisation; it is strictly
/// stronger than `<=`-maximality and is what `lemma_gcd_reduce_coprime` needs.
pub proof fn lemma_gcd_greatest(a: nat, b: nat, d: int)
    requires
        divides(d, a as int),
        divides(d, b as int),
    ensures
        divides(d, spec_gcd(a, b) as int),
    decreases b,
{
    if b == 0 {
    } else {
        let r = (a % b) as nat;
        lemma_mod_bound(a as int, b as int);
        // a % b == a - b * (a / b)
        lemma_fundamental_div_mod(a as int, b as int);
        lemma_divides_mul(d, b as int, a as int / b as int);
        lemma_divides_sub(d, a as int, b as int * (a as int / b as int));
        assert(r as int == a as int - b as int * (a as int / b as int));
        lemma_gcd_greatest(b, r, d);
    }
}

/// The GCD of a pair that is not `(0, 0)` is strictly positive.
pub proof fn lemma_gcd_positive(a: nat, b: nat)
    requires
        a > 0 || b > 0,
    ensures
        spec_gcd(a, b) > 0,
    decreases b,
{
    if b == 0 {
    } else {
        let r = (a % b) as nat;
        lemma_mod_bound(a as int, b as int);
        lemma_gcd_positive(b, r);
    }
}

/// The GCD never exceeds a positive argument.
pub proof fn lemma_gcd_le(a: nat, b: nat)
    requires
        a > 0,
    ensures
        spec_gcd(a, b) <= a,
{
    lemma_gcd_divides(a, b);
    lemma_gcd_positive(a, b);
    lemma_divides_le(spec_gcd(a, b) as int, a as int);
}

/// `spec_gcd` is symmetric on positive inputs — proved through mutual
/// divisibility rather than by unfolding Euclid twice.
pub proof fn lemma_gcd_comm(a: nat, b: nat)
    requires
        a > 0 || b > 0,
    ensures
        spec_gcd(a, b) == spec_gcd(b, a),
{
    lemma_gcd_divides(a, b);
    lemma_gcd_divides(b, a);
    lemma_gcd_positive(a, b);
    lemma_gcd_positive(b, a);
    lemma_gcd_greatest(b, a, spec_gcd(a, b) as int);
    lemma_gcd_greatest(a, b, spec_gcd(b, a) as int);
    lemma_divides_le(spec_gcd(a, b) as int, spec_gcd(b, a) as int);
    lemma_divides_le(spec_gcd(b, a) as int, spec_gcd(a, b) as int);
}

/// Dividing out the GCD leaves a coprime pair — the heart of canonicality (I1).
pub proof fn lemma_gcd_reduce_coprime(a: nat, b: nat)
    requires
        a > 0 || b > 0,
    ensures
        spec_gcd(
            (a / spec_gcd(a, b)) as nat,
            (b / spec_gcd(a, b)) as nat,
        ) == 1,
{
    let g = spec_gcd(a, b);
    lemma_gcd_positive(a, b);
    lemma_gcd_divides(a, b);
    let a1 = (a / g) as nat;
    let b1 = (b / g) as nat;
    // a == g * a1 and b == g * b1, because g divides both exactly.
    lemma_exact_div(a as int, g as int);
    lemma_exact_div(b as int, g as int);
    assert(a as int == g as int * a1 as int);
    assert(b as int == g as int * b1 as int);
    assert(a1 > 0 || b1 > 0) by {
        if a > 0 {
            assert(a1 > 0) by (nonlinear_arith)
                requires
                    a as int == g as int * a1 as int,
                    a > 0,
                    g > 0,
                    a1 >= 0,
            ;
        } else {
            assert(b > 0);
            assert(b1 > 0) by (nonlinear_arith)
                requires
                    b as int == g as int * b1 as int,
                    b > 0,
                    g > 0,
                    b1 >= 0,
            ;
        }
    }
    let h = spec_gcd(a1, b1);
    lemma_gcd_positive(a1, b1);
    lemma_gcd_divides(a1, b1);
    // g*h divides a and b, hence divides g, hence g*h <= g, hence h <= 1.
    lemma_divides_scale(h as int, a1 as int, g as int);
    lemma_divides_scale(h as int, b1 as int, g as int);
    assert(divides(g as int * h as int, a as int));
    assert(divides(g as int * h as int, b as int));
    lemma_gcd_greatest(a, b, g as int * h as int);
    assert(g as int * h as int > 0) by (nonlinear_arith)
        requires
            g > 0,
            h > 0,
    ;
    lemma_divides_le(g as int * h as int, g as int);
    assert(h <= 1) by (nonlinear_arith)
        requires
            g as int * h as int <= g as int,
            g > 0,
            h > 0,
    ;
}

/// If `d | n` with `d > 0` then `n == d * (n / d)` — `n / d` is exact.
pub proof fn lemma_exact_div(n: int, d: int)
    requires
        divides(d, n),
        d > 0,
    ensures
        n == d * (n / d),
{
    let k = choose|k: int| n == #[trigger] (d * k);
    assert((d * k) / d == k) by (nonlinear_arith)
        requires
            d > 0,
    ;
}

/// If `h | a1` then `g*h | g*a1`.
pub proof fn lemma_divides_scale(h: int, a1: int, g: int)
    requires
        divides(h, a1),
    ensures
        divides(g * h, g * a1),
{
    let k = choose|k: int| a1 == #[trigger] (h * k);
    assert(g * a1 == (g * h) * k) by (nonlinear_arith)
        requires
            a1 == h * k,
    ;
}

// ---------------------------------------------------------------------------
// Executable Euclid (V5: termination + agreement with the ghost function)
// ---------------------------------------------------------------------------
/// Euclid's algorithm on `u128`. Total, panic-free, and provably equal to
/// `spec_gcd`.
///
/// `u128` (not `u64`) because the widest reduction this crate performs is on
/// the exact `i128` intermediates of `mul`/`add`, whose magnitudes reach 2^125.
pub fn gcd_u128(a: u128, b: u128) -> (r: u128)
    ensures
        r as nat == spec_gcd(a as nat, b as nat),
        a > 0 || b > 0 ==> r > 0,
        a > 0 ==> r <= a,
        b > 0 ==> r <= b,
{
    let mut x: u128 = a;
    let mut y: u128 = b;
    while y != 0
        invariant
            spec_gcd(x as nat, y as nat) == spec_gcd(a as nat, b as nat),
        decreases y,
    {
        proof {
            lemma_mod_bound(x as int, y as int);
        }
        let t = x % y;
        x = y;
        y = t;
    }
    proof {
        if a > 0 || b > 0 {
            lemma_gcd_positive(a as nat, b as nat);
        }
        if a > 0 {
            lemma_gcd_le(a as nat, b as nat);
        }
        if b > 0 {
            lemma_gcd_comm(a as nat, b as nat);
            lemma_gcd_le(b as nat, a as nat);
        }
    }
    x
}

} // verus!
