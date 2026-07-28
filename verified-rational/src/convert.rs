//! Constructors that take data from the outside world: explicit fractions,
//! short decimals, and dyadic (mantissa × 2^exponent) values.
//!
//! `from_decimal` is the primary ingestion path for the fusion engine, whose
//! reliability/competence/weight inputs arrive as short decimals — `(85, 2)` is
//! `0.85`, exactly, with no float ever involved.
//!
//! `from_dyadic` is the integer half of the `f64` boundary: an `f64` *is* the
//! rational `m · 2^e`, so decomposing the bits and calling this function
//! converts a float exactly with no float reasoning anywhere. See
//! `crate::float` for the (trusted, tiny) decomposition itself.

use crate::gcd::*;
use crate::q::*;
use crate::round::*;
use vstd::prelude::*;

verus! {

#[cfg(verus_keep_ghost)]
use crate::arith::rounded;
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::pow2;

/// `10^e` as a ghost natural.
pub open spec fn pow10(e: nat) -> nat
    decreases e,
{
    if e == 0 {
        1nat
    } else {
        10 * pow10((e - 1) as nat)
    }
}

/// The eighteen table entries agree with `pow10`.
pub proof fn lemma_pow10_table(e: u8)
    requires
        e <= 18,
    ensures
        pow10(e as nat) >= 1,
        e == 0 ==> pow10(0nat) == 1,
        e == 1 ==> pow10(1nat) == 10,
        e == 2 ==> pow10(2nat) == 100,
        e == 3 ==> pow10(3nat) == 1000,
        e == 4 ==> pow10(4nat) == 10000,
        e == 5 ==> pow10(5nat) == 100000,
        e == 6 ==> pow10(6nat) == 1000000,
        e == 7 ==> pow10(7nat) == 10000000,
        e == 8 ==> pow10(8nat) == 100000000,
        e == 9 ==> pow10(9nat) == 1000000000,
        e == 10 ==> pow10(10nat) == 10000000000,
        e == 11 ==> pow10(11nat) == 100000000000,
        e == 12 ==> pow10(12nat) == 1000000000000,
        e == 13 ==> pow10(13nat) == 10000000000000,
        e == 14 ==> pow10(14nat) == 100000000000000,
        e == 15 ==> pow10(15nat) == 1000000000000000,
        e == 16 ==> pow10(16nat) == 10000000000000000,
        e == 17 ==> pow10(17nat) == 100000000000000000,
        e == 18 ==> pow10(18nat) == 1000000000000000000,
{
    assert(pow10(0nat) == 1) by (compute_only);
    assert(pow10(1nat) == 10) by (compute_only);
    assert(pow10(2nat) == 100) by (compute_only);
    assert(pow10(3nat) == 1000) by (compute_only);
    assert(pow10(4nat) == 10000) by (compute_only);
    assert(pow10(5nat) == 100000) by (compute_only);
    assert(pow10(6nat) == 1000000) by (compute_only);
    assert(pow10(7nat) == 10000000) by (compute_only);
    assert(pow10(8nat) == 100000000) by (compute_only);
    assert(pow10(9nat) == 1000000000) by (compute_only);
    assert(pow10(10nat) == 10000000000) by (compute_only);
    assert(pow10(11nat) == 100000000000) by (compute_only);
    assert(pow10(12nat) == 1000000000000) by (compute_only);
    assert(pow10(13nat) == 10000000000000) by (compute_only);
    assert(pow10(14nat) == 100000000000000) by (compute_only);
    assert(pow10(15nat) == 1000000000000000) by (compute_only);
    assert(pow10(16nat) == 10000000000000000) by (compute_only);
    assert(pow10(17nat) == 100000000000000000) by (compute_only);
    assert(pow10(18nat) == 1000000000000000000) by (compute_only);
}

/// The exact numerator of the dyadic value `m * 2^e`.
pub open spec fn dyadic_num(m: int, e: int) -> int {
    if e >= 0 {
        m * pow2(e as nat)
    } else {
        m
    }
}

/// The exact (positive) denominator of the dyadic value `m * 2^e`.
pub open spec fn dyadic_den(e: int) -> int {
    if e >= 0 {
        1int
    } else {
        pow2((-e) as nat) as int
    }
}

impl Q {
    /// Exact construction from an explicit fraction.
    ///
    /// Returns `None` when `den == 0`, and **also** when the reduced fraction
    /// does not fit the budget — for example `Q::new(i64::MAX, 1)`, whose
    /// numerator is `2^63 - 1`, well past `2^62 - 1`. (The original
    /// specification claimed any `i64` pair fits after reduction; it does not,
    /// and silently rounding a constructor that promises exactness would be
    /// worse than reporting the failure.) Never rounds.
    pub fn new(num: i64, den: i64) -> (r: Option<Q>)
        ensures
            den == 0 ==> r is None,
            r is Some ==> {
                &&& r->Some_0.wf()
                &&& r->Some_0.n() * den as int == num as int * r->Some_0.d()
            },
            (den != 0 && reduced_fits(num as int, den as int)) ==> r is Some,
    {
        if den == 0 {
            return None;
        }
        let ghost sg: int = if den < 0 {
            -1int
        } else {
            1int
        };
        let n1: i128 = if den < 0 {
            -(num as i128)
        } else {
            num as i128
        };
        let d1: i128 = if den < 0 {
            -(den as i128)
        } else {
            den as i128
        };
        let neg: bool = n1 < 0;
        let a: i128 = if neg {
            -n1
        } else {
            n1
        };
        proof {
            assert(iabs(num as int) == a as int);
            assert(iabs(den as int) == d1 as int);
        }
        let (a2, d2, g) = reduce_mag(a, d1);
        if a2 > BUDGET128 || d2 > BUDGET128 {
            return None;
        }
        let n2: i128 = if neg {
            -a2
        } else {
            a2
        };
        proof {
            assert(iabs(n2 as int) == a2 as int);
            assert(n1 as int == (g as int) * (n2 as int)) by (nonlinear_arith)
                requires
                    a as int == (g as int) * (a2 as int),
                    neg ==> (n1 as int == -(a as int) && n2 as int == -(a2 as int)),
                    !neg ==> (n1 as int == a as int && n2 as int == a2 as int),
            ;
            assert(num as int == sg * (n1 as int));
            assert(den as int == sg * (d1 as int));
            assert((n2 as int) * (den as int) == (num as int) * (d2 as int)) by (nonlinear_arith)
                requires
                    n1 as int == (g as int) * (n2 as int),
                    d1 as int == (g as int) * (d2 as int),
                    num as int == sg * (n1 as int),
                    den as int == sg * (d1 as int),
                    sg * sg == 1,
            ;
        }
        Some(Q::from_parts(n2 as i64, d2 as i64))
    }

    /// Exact construction from a short decimal: `from_decimal(85, 2) == 0.85`.
    ///
    /// `None` when `dec_places > 18` (`10^19` is past the budget) or when the
    /// reduced value does not fit. A table rather than a loop: eighteen entries
    /// are cheaper to read, cheaper to run, and cheaper to verify.
    pub fn from_decimal(mantissa: i64, dec_places: u8) -> (r: Option<Q>)
        ensures
            dec_places > 18 ==> r is None,
            r is Some ==> {
                &&& r->Some_0.wf()
                &&& r->Some_0.n() * pow10(dec_places as nat) as int == mantissa as int
                    * r->Some_0.d()
            },
    {
        let p: i64 = match dec_places {
            0 => 1,
            1 => 10,
            2 => 100,
            3 => 1000,
            4 => 10000,
            5 => 100000,
            6 => 1000000,
            7 => 10000000,
            8 => 100000000,
            9 => 1000000000,
            10 => 10000000000,
            11 => 100000000000,
            12 => 1000000000000,
            13 => 10000000000000,
            14 => 100000000000000,
            15 => 1000000000000000,
            16 => 10000000000000000,
            17 => 100000000000000000,
            18 => 1000000000000000000,
            _ => return None,
        };
        proof {
            lemma_pow10_table(dec_places);
        }
        Q::new(mantissa, p)
    }

    /// The exact rational `m * 2^e`, rounded to the budget.
    ///
    /// This is where `f64` conversion lands after its bits have been taken
    /// apart. The exponent range covers every `f64` whose magnitude is at least
    /// `2^-124`; smaller magnitudes are handled by the caller (they are within
    /// `2^-124` of zero, far inside the R3 bound).
    pub fn from_dyadic(m: i64, e: i32, dir: Dir) -> (r: Q)
        requires
            -9007199254740992 <= m <= 9007199254740992,
            -124 <= e <= 62,
        ensures
            rounded(r, dyadic_num(m as int, e as int), dyadic_den(e as int), dir),
    {
        if e >= 0 {
            let p = pow2_i128(e as u32);
            proof {
                lemma_dyadic_up_bounds(m as int, e as int, p as int);
            }
            round_to_budget((m as i128) * p, 1, dir)
        } else {
            let p = pow2_i128((-e) as u32);
            proof {
                lemma_dyadic_down_bounds(m as int, e as int, p as int);
            }
            round_to_budget(m as i128, p, dir)
        }
    }
}

/// `|m| * 2^e <= NMAX` for `|m| <= 2^53` and `e <= 62`.
pub proof fn lemma_dyadic_up_bounds(m: int, e: int, p: int)
    requires
        -9007199254740992 <= m <= 9007199254740992,
        0 <= e <= 62,
        p == pow2(e as nat),
    ensures
        -NMAX <= m * p <= NMAX,
{
    lemma_pow2_small();
    lemma_pow2_monotonic_le(e as nat, 62nat);
    assert(p <= 4611686018427387904);
    assert(p >= 1) by {
        vstd::arithmetic::power2::lemma_pow2_pos(e as nat);
    }
    assert(m * p <= NMAX as int) by (nonlinear_arith)
        requires
            m <= 9007199254740992,
            1 <= p <= 4611686018427387904,
            m >= -9007199254740992,
    ;
    assert(-(NMAX as int) <= m * p) by (nonlinear_arith)
        requires
            m >= -9007199254740992,
            1 <= p <= 4611686018427387904,
            m <= 9007199254740992,
    ;
}

/// `2^(-e) <= DMAX` for `e >= -124`.
pub proof fn lemma_dyadic_down_bounds(m: int, e: int, p: int)
    requires
        -9007199254740992 <= m <= 9007199254740992,
        -124 <= e < 0,
        p == pow2((-e) as nat),
    ensures
        0 < p <= DMAX,
        -NMAX <= m <= NMAX,
{
    lemma_pow2_small();
    lemma_pow2_monotonic_le((-e) as nat, 124nat);
    vstd::arithmetic::power2::lemma_pow2_pos((-e) as nat);
}

} // verus!
