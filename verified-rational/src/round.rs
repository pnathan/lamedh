//! Canonicalization and the rounding contract (**V4**, rules R1–R4).
//!
//! [`round_to_budget`] is the single place where a value can lose exactness.
//! Every arithmetic operation in this crate computes its exact result as an
//! `i128` pair and hands it here, so R1–R4 are proved *once* and inherited by
//! every operation rather than re-proved per operation.
//!
//! # The algorithm
//!
//! Given an exact `n/d` (`d > 0`):
//!
//! 1. Reduce by `gcd(|n|, d)`.
//! 2. If the reduced pair already satisfies I2, return it verbatim. **This is
//!    R1** — and its consequence is the exactness theorem: a computation whose
//!    exact intermediates all fit the budget never rounds at all.
//! 3. Otherwise split off the integer part, `|n|/d = qi + f/d`, and snap the
//!    fractional part onto the dyadic grid `k / 2^s`, with `s` chosen from the
//!    magnitude of `qi` so that the reassembled numerator still fits the budget
//!    and the grid is fine enough for R3.
//! 4. If `qi` itself exceeds the budget the value is not representable at all;
//!    the result saturates and the `ensures` says so explicitly rather than
//!    pretending the error bound still holds.
//!
//! The fractional snap is an *exact* shift-and-subtract long division
//! (`shift_div`), not a wide multiply. That matters twice over: it keeps every
//! intermediate inside `i128` with no pre-scaling, and it makes R2 (directed
//! rounding) exact rather than approximate, because the remainder tells us
//! precisely which side of the grid point the true value lies on.

use crate::gcd::*;
use crate::q::*;
use vstd::prelude::*;

verus! {

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::{lemma_pow2_adds, lemma_pow2_pos, pow2};

/// Upper bound on the magnitude of an exact numerator handed to
/// [`round_to_budget`]: `2^125`. The widest producer is `add`, at
/// `2*(2^62-1)^2 < 2^125`.
pub const NMAX: i128 = 42535295865117307932921825928971026432;

/// Upper bound on an exact denominator: `2^124`. The widest producer is any of
/// `add`/`sub`/`mul`/`div`, at `(2^62-1)^2 < 2^124`.
pub const DMAX: i128 = 21267647932558653966460912964485513216;

/// Rounding direction.
///
/// `Down`/`Up` are directed *on the value*, not on the magnitude: `Down` never
/// returns a result above the exact value and `Up` never returns one below it,
/// for negative values too. They exist so that a future interval type can
/// bracket the exact answer without any new proofs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    /// Toward negative infinity.
    Down,
    /// Toward positive infinity.
    Up,
    /// Nearest representable, ties away from zero.
    Nearest,
}

impl Dir {
    /// The direction to apply to a *magnitude* in order to realise `self` on a
    /// value of the given sign.
    pub fn for_magnitude(self, negative: bool) -> (r: Dir)
        ensures
            !negative ==> r == self,
            negative && self == Dir::Down ==> r == Dir::Up,
            negative && self == Dir::Up ==> r == Dir::Down,
            negative && self == Dir::Nearest ==> r == Dir::Nearest,
    {
        if negative {
            match self {
                Dir::Down => Dir::Up,
                Dir::Up => Dir::Down,
                Dir::Nearest => Dir::Nearest,
            }
        } else {
            self
        }
    }
}

/// `max` on ghost integers.
pub open spec fn imax(a: int, b: int) -> int {
    if a >= b {
        a
    } else {
        b
    }
}

/// The exact reduced form of `n/d` fits the budget, i.e. R1 applies.
pub open spec fn reduced_fits(n: int, d: int) -> bool {
    let g = spec_gcd(iabs(n), iabs(d)) as int;
    &&& g > 0
    &&& iabs(n) as int / g <= BUDGET as int
    &&& iabs(d) as int / g <= BUDGET as int
}

// ---------------------------------------------------------------------------
// Powers of two, without bit-vector reasoning
// ---------------------------------------------------------------------------
/// `2^s` as an `i128`, computed by repeated doubling so the proof stays in
/// ordinary integer arithmetic (`pow2`) instead of 128-bit bit-vector theory.
pub fn pow2_i128(s: u32) -> (r: i128)
    requires
        s <= 124,
    ensures
        r as nat == pow2(s as nat),
        r >= 1,
{
    let mut acc: i128 = 1;
    let mut i: u32 = 0;
    proof {
        lemma_pow2_small();
    }
    while i < s
        invariant
            i <= s,
            s <= 124,
            acc as nat == pow2(i as nat),
            acc >= 1,
            acc <= 21267647932558653966460912964485513216,
        decreases s - i,
    {
        proof {
            lemma_pow2_adds(i as nat, 1nat);
            lemma_pow2_small();
            lemma_pow2_monotonic_le((i + 1) as nat, 124nat);
        }
        acc *= 2;
        i += 1;
    }
    acc
}

/// `pow2` is monotone.
pub proof fn lemma_pow2_monotonic_le(a: nat, b: nat)
    requires
        a <= b,
    ensures
        pow2(a) <= pow2(b),
{
    vstd::arithmetic::power2::lemma_pow2(a);
    vstd::arithmetic::power2::lemma_pow2(b);
    vstd::arithmetic::power::lemma_pow_increases(2nat, a, b);
}

/// Number of significant bits of a non-negative `i128`, i.e. the least `r` with
/// `x < 2^r`. `bitlen(0) == 0`.
pub fn bitlen_i128(x: i128) -> (r: u32)
    requires
        x >= 0,
        x <= BUDGET128,
    ensures
        r <= 62,
        (x as nat) < pow2(r as nat),
        r > 0 ==> pow2((r - 1) as nat) <= x as nat,
        x == 0 ==> r == 0,
{
    let mut r: u32 = 0;
    let mut bound: i128 = 1;  // bound == 2^r
    proof {
        lemma_pow2_small();
    }
    while r < 62 && bound <= x
        invariant
            r <= 62,
            0 <= x <= BUDGET128,
            bound as nat == pow2(r as nat),
            1 <= bound <= 4611686018427387904,
            r > 0 ==> pow2((r - 1) as nat) <= x as nat,
            x == 0 ==> r == 0,
        decreases 62 - r,
    {
        proof {
            lemma_pow2_adds(r as nat, 1nat);
            lemma_pow2_small();
            lemma_pow2_monotonic_le((r + 1) as nat, 62nat);
        }
        bound *= 2;
        r += 1;
    }
    proof {
        lemma_pow2_small();
        if r == 62 {
            assert(pow2(62nat) == 4611686018427387904);
        }
    }
    r
}

/// The small `pow2` values the loops above need as literals.
pub proof fn lemma_pow2_small()
    ensures
        pow2(0nat) == 1,
        pow2(1nat) == 2,
        pow2(61nat) == 2305843009213693952,
        pow2(62nat) == 4611686018427387904,
        pow2(124nat) == 21267647932558653966460912964485513216,
{
    vstd::arithmetic::power2::lemma2_to64();
    vstd::arithmetic::power2::lemma2_to64_rest();
    lemma_pow2_adds(62nat, 62nat);
}

// ---------------------------------------------------------------------------
// Exact scaled division: the fractional snap
// ---------------------------------------------------------------------------
/// Exact long division of `f * 2^s` by `d`, returning `(k, rem)` with
/// `f * 2^s == k*d + rem` and `0 <= rem < d`.
///
/// Because it is exact, `rem` decides the rounding direction with no slack:
/// `rem == 0` means the snap is exact, and comparing `2*rem` with `d` decides
/// nearest. Done as `s` doubling steps so no intermediate ever exceeds `2*d`.
pub fn shift_div(f: i128, d: i128, s: u32) -> (r: (i128, i128))
    requires
        0 <= f < d,
        d <= DMAX,
        s <= 62,
    ensures
        ({
            let (k, rem) = r;
            &&& 0 <= rem < d
            &&& 0 <= k
            &&& k as int * d as int + rem as int == f as int * pow2(s as nat) as int
            &&& (k as nat) < pow2(s as nat)
        }),
{
    let mut k: i128 = 0;
    let mut rem: i128 = f;
    let mut i: u32 = 0;
    proof {
        lemma_pow2_small();
    }
    while i < s
        invariant
            i <= s,
            s <= 62,
            0 <= f < d,
            d <= DMAX,
            0 <= rem < d,
            0 <= k <= 4611686018427387904,
            k as int * d as int + rem as int == f as int * pow2(i as nat) as int,
        decreases s - i,
    {
        let ghost k0: int = k as int;
        let ghost rem0: int = rem as int;
        let ghost dd: int = d as int;
        let ghost pi: int = pow2(i as nat) as int;
        proof {
            lemma_pow2_adds(i as nat, 1nat);
            lemma_pow2_small();
            lemma_pow2_pos(i as nat);
            lemma_k_bound(k0, dd, rem0, f as int, pi);
            lemma_pow2_monotonic_le(i as nat, 61nat);
            assert(pow2((i + 1) as nat) == 2 * pi);
            assert(f as int * (2 * pi) == 2 * (f as int * pi)) by (nonlinear_arith);
            assert((2 * k0) * dd == 2 * (k0 * dd)) by (nonlinear_arith);
            assert((2 * k0 + 1) * dd == 2 * (k0 * dd) + dd) by (nonlinear_arith);
        }
        rem *= 2;
        k *= 2;
        if rem >= d {
            rem -= d;
            k += 1;
        }
        i += 1;
    }
    proof {
        lemma_pow2_pos(s as nat);
        lemma_k_lt_pow2(k as int, d as int, rem as int, f as int, pow2(s as nat) as int);
    }
    (k, rem)
}

/// `k*d + rem == f*p`, `0 <= rem`, `0 <= f < d`, `d > 0` ⟹ `k <= f*p/d < p`.
proof fn lemma_k_lt_pow2(k: int, d: int, rem: int, f: int, p: int)
    requires
        d > 0,
        0 <= f < d,
        0 <= rem,
        p > 0,
        k * d + rem == f * p,
        k >= 0,
    ensures
        k < p,
{
    assert(k * d <= f * p) by (nonlinear_arith)
        requires
            k * d + rem == f * p,
            rem >= 0,
    ;
    assert(f * p <= (d - 1) * p) by (nonlinear_arith)
        requires
            f <= d - 1,
            p > 0,
    ;
    assert((d - 1) * p < d * p) by (nonlinear_arith)
        requires
            p > 0,
    ;
    assert(k * d < p * d) by (nonlinear_arith)
        requires
            k * d <= f * p,
            f * p < d * p,
    ;
    assert(k < p) by (nonlinear_arith)
        requires
            k * d < p * d,
            d > 0,
    ;
}

/// Bounds the running quotient so the doubling step cannot overflow `i128`.
proof fn lemma_k_bound(k: int, d: int, rem: int, f: int, p: int)
    requires
        d > 0,
        0 <= f < d,
        0 <= rem < d,
        p > 0,
        k >= 0,
        k * d + rem == f * p,
    ensures
        k < p,
{
    lemma_k_lt_pow2(k, d, rem, f, p);
}



// ---------------------------------------------------------------------------
// Ghost vocabulary for the rounding contract
// ---------------------------------------------------------------------------
/// `|n/d| >= 2^62 - 1`: the value's *magnitude* is outside the representable
/// range, so no `Q` can carry it and the result saturates. Stated division-free.
///
/// This case is unreachable for the consuming engine (all of its values live in
/// `[0, 1]` or are small counts) but the type must still be total, and the
/// `ensures` says plainly that R2/R3 do not apply here rather than quietly
/// pretending otherwise.
pub open spec fn saturating(n: int, d: int) -> bool {
    iabs(n) as int >= BUDGET as int * d
}

/// The saturated result: `±(2^62 - 1) / 1`, with the sign of the exact value.
pub open spec fn sat_result(r: Q, n: int) -> bool {
    &&& r.d() == 1
    &&& (n >= 0 ==> r.n() == BUDGET as int)
    &&& (n < 0 ==> r.n() == -(BUDGET as int))
}

/// **R3**: `|r - n/d| <= 2^-60 * max(1, |n/d|)`, cleared of division.
///
/// Multiplying through by `r.d() * d > 0` turns the statement into
/// `|r.n()*d - n*r.d()| * 2^60 <= r.d() * max(d, |n|)`, which is what is
/// actually proved. It is stated as two one-sided inequalities rather than
/// through `iabs` so the SMT solver never has to case-split on a sign inside a
/// product.
pub open spec fn r3_bound(r: Q, n: int, d: int) -> bool {
    &&& (r.n() * d - n * r.d()) * pow2(60) <= r.d() * imax(d, iabs(n) as int)
    &&& -(r.d() * imax(d, iabs(n) as int)) <= (r.n() * d - n * r.d()) * pow2(60)
}

// ---------------------------------------------------------------------------
// Reduction
// ---------------------------------------------------------------------------
/// Reduce a non-negative magnitude over a positive denominator to lowest terms,
/// returning `(a/g, d/g, g)`.
pub fn reduce_mag(a: i128, d: i128) -> (r: (i128, i128, i128))
    requires
        0 <= a <= NMAX,
        0 < d <= DMAX,
    ensures
        ({
            let (a1, d1, g) = r;
            &&& g >= 1
            &&& 0 <= a1 <= a
            &&& 1 <= d1 <= d
            &&& a as int == g as int * a1 as int
            &&& d as int == g as int * d1 as int
            &&& spec_gcd(a1 as nat, d1 as nat) == 1
            &&& g as nat == spec_gcd(a as nat, d as nat)
            &&& a1 as int == a as int / g as int
            &&& d1 as int == d as int / g as int
        }),
{
    let g = gcd_u128(a as u128, d as u128) as i128;
    proof {
        lemma_gcd_positive(a as nat, d as nat);
        lemma_gcd_le(d as nat, a as nat);
        lemma_gcd_comm(a as nat, d as nat);
        lemma_gcd_divides(a as nat, d as nat);
        lemma_exact_div(a as int, g as int);
        lemma_exact_div(d as int, g as int);
        lemma_gcd_reduce_coprime(a as nat, d as nat);
        lemma_div_le_self(a as int, g as int);
        lemma_div_le_self(d as int, g as int);
        lemma_div_pos(d as int, g as int);
    }
    (a / g, d / g, g)
}

/// `0 <= x`, `g >= 1` ⟹ `x / g <= x`.
pub proof fn lemma_div_le_self(x: int, g: int)
    requires
        x >= 0,
        g >= 1,
    ensures
        x / g <= x,
        x / g >= 0,
{
    vstd::arithmetic::div_mod::lemma_div_is_ordered_by_denominator(x, 1, g);
    vstd::arithmetic::div_mod::lemma_div_basics(x);
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(x, g);
}

/// A positive multiple stays positive after exact division by its divisor.
pub proof fn lemma_div_pos(x: int, g: int)
    requires
        x > 0,
        g >= 1,
        divides(g, x),
    ensures
        x / g >= 1,
{
    lemma_exact_div(x, g);
    vstd::arithmetic::div_mod::lemma_div_pos_is_pos(x, g);
    assert(x / g != 0) by (nonlinear_arith)
        requires
            x == g * (x / g),
            x > 0,
    ;
}

// ---------------------------------------------------------------------------
// The R3 algebra, isolated from the algorithm
// ---------------------------------------------------------------------------
/// Turns "the effective fraction `num_eff/two_s` is within `d1/two_s` of
/// `a1/d1`" into R3 against the original unreduced pair `(a, d)`.
///
/// Everything below is one long chain of monotone multiplications; it is
/// factored out of `round_nonneg` so the solver sees a small, purely algebraic
/// context instead of the whole algorithm at once.
pub proof fn lemma_r3_core(
    rn: int,
    rd: int,
    a: int,
    d: int,
    a1: int,
    d1: int,
    g: int,
    two_s: int,
    num_eff: int,
)
    requires
        g >= 1,
        d1 >= 1,
        a1 >= 0,
        rd >= 1,
        rn >= 0,
        two_s >= 1,
        a == g * a1,
        d == g * d1,
        rn * two_s == num_eff * rd,
        -d1 <= num_eff * d1 - a1 * two_s <= d1,
        d * pow2(60) <= imax(d, a) * two_s,
    ensures
        (rn * d - a * rd) * pow2(60) <= rd * imax(d, a),
        -(rd * imax(d, a)) <= (rn * d - a * rd) * pow2(60),
{
    lemma_pow2_pos(60nat);
    let c = num_eff * d1 - a1 * two_s;
    let ee = rn * d1 - a1 * rd;
    // two_s * ee == rd * c
    assert(two_s * ee == rd * c) by (nonlinear_arith)
        requires
            rn * two_s == num_eff * rd,
            ee == rn * d1 - a1 * rd,
            c == num_eff * d1 - a1 * two_s,
    ;
    assert(rn * d - a * rd == g * ee) by (nonlinear_arith)
        requires
            a == g * a1,
            d == g * d1,
            ee == rn * d1 - a1 * rd,
    ;
    // |two_s * ee| = rd * |c| <= rd * d1
    assert(rd * c <= rd * d1) by (nonlinear_arith)
        requires
            c <= d1,
            rd >= 1,
    ;
    assert(-(rd * d1) <= rd * c) by (nonlinear_arith)
        requires
            -d1 <= c,
            rd >= 1,
    ;
    // Multiply the goal by two_s > 0 and chain.
    assert(((rn * d - a * rd) * pow2(60)) * two_s == (g * pow2(60)) * (two_s * ee))
        by (nonlinear_arith)
        requires
            rn * d - a * rd == g * ee,
    ;
    assert((g * pow2(60)) * (two_s * ee) <= (g * pow2(60)) * (rd * d1)) by (nonlinear_arith)
        requires
            two_s * ee <= rd * d1,
            g >= 1,
            pow2(60) >= 1,
    ;
    assert((g * pow2(60)) * (rd * d1) == rd * (d * pow2(60))) by (nonlinear_arith)
        requires
            d == g * d1,
    ;
    assert(rd * (d * pow2(60)) <= rd * (imax(d, a) * two_s)) by (nonlinear_arith)
        requires
            d * pow2(60) <= imax(d, a) * two_s,
            rd >= 1,
    ;
    assert(rd * (imax(d, a) * two_s) == (rd * imax(d, a)) * two_s) by (nonlinear_arith);
    assert(((rn * d - a * rd) * pow2(60)) * two_s <= (rd * imax(d, a)) * two_s);
    assert((rn * d - a * rd) * pow2(60) <= rd * imax(d, a)) by (nonlinear_arith)
        requires
            ((rn * d - a * rd) * pow2(60)) * two_s <= (rd * imax(d, a)) * two_s,
            two_s >= 1,
    ;
    // Symmetric lower bound.
    assert((g * pow2(60)) * (-(rd * d1)) <= (g * pow2(60)) * (two_s * ee)) by (nonlinear_arith)
        requires
            -(rd * d1) <= two_s * ee,
            g >= 1,
            pow2(60) >= 1,
    ;
    assert((g * pow2(60)) * (-(rd * d1)) == -(rd * (d * pow2(60)))) by (nonlinear_arith)
        requires
            d == g * d1,
    ;
    assert(-((rd * imax(d, a)) * two_s) <= -(rd * (d * pow2(60)))) by (nonlinear_arith)
        requires
            rd * (d * pow2(60)) <= (rd * imax(d, a)) * two_s,
    ;
    assert(-((rd * imax(d, a)) * two_s) <= ((rn * d - a * rd) * pow2(60)) * two_s);
    assert(-(rd * imax(d, a)) <= (rn * d - a * rd) * pow2(60)) by (nonlinear_arith)
        requires
            -((rd * imax(d, a)) * two_s) <= ((rn * d - a * rd) * pow2(60)) * two_s,
            two_s >= 1,
    ;
}

/// Sign of the effective error transfers to the directed comparison (R2).
pub proof fn lemma_r2_core(
    rn: int,
    rd: int,
    a: int,
    d: int,
    a1: int,
    d1: int,
    g: int,
    two_s: int,
    num_eff: int,
)
    requires
        g >= 1,
        d1 >= 1,
        a1 >= 0,
        rd >= 1,
        two_s >= 1,
        a == g * a1,
        d == g * d1,
        rn * two_s == num_eff * rd,
    ensures
        num_eff * d1 - a1 * two_s <= 0 ==> rn * d <= a * rd,
        num_eff * d1 - a1 * two_s >= 0 ==> a * rd <= rn * d,
{
    assert((rn * d1 - a1 * rd) * two_s == rd * (num_eff * d1 - a1 * two_s)) by (nonlinear_arith)
        requires
            rn * two_s == num_eff * rd,
    ;
    assert(rn * d - a * rd == g * (rn * d1 - a1 * rd)) by (nonlinear_arith)
        requires
            a == g * a1,
            d == g * d1,
    ;
    if num_eff * d1 - a1 * two_s <= 0 {
        assert(rd * (num_eff * d1 - a1 * two_s) <= 0) by (nonlinear_arith)
            requires
                num_eff * d1 - a1 * two_s <= 0,
                rd >= 1,
        ;
        assert(rn * d1 - a1 * rd <= 0) by (nonlinear_arith)
            requires
                (rn * d1 - a1 * rd) * two_s <= 0,
                two_s >= 1,
        ;
        assert(g * (rn * d1 - a1 * rd) <= 0) by (nonlinear_arith)
            requires
                rn * d1 - a1 * rd <= 0,
                g >= 1,
        ;
    }
    if num_eff * d1 - a1 * two_s >= 0 {
        assert(rd * (num_eff * d1 - a1 * two_s) >= 0) by (nonlinear_arith)
            requires
                num_eff * d1 - a1 * two_s >= 0,
                rd >= 1,
        ;
        assert(rn * d1 - a1 * rd >= 0) by (nonlinear_arith)
            requires
                (rn * d1 - a1 * rd) * two_s >= 0,
                two_s >= 1,
        ;
        assert(g * (rn * d1 - a1 * rd) >= 0) by (nonlinear_arith)
            requires
                rn * d1 - a1 * rd >= 0,
                g >= 1,
        ;
    }
}


/// The magnitude-dependent grid choice leaves R3 enough resolution.
///
/// `s` is picked as `61` when the value is below 1, and as `62 - bitlen(qi)`
/// otherwise. Both cases land on `d * 2^60 <= max(d, a) * 2^s` with a factor of
/// two to spare, which is exactly the hypothesis `lemma_r3_core` consumes.
pub proof fn lemma_grid_hyp(
    a: int,
    d: int,
    a1: int,
    d1: int,
    g: int,
    qi: int,
    f: int,
    two_s: int,
    e: nat,
)
    requires
        g >= 1,
        d1 >= 1,
        a1 >= 0,
        qi >= 0,
        0 <= f < d1,
        a == g * a1,
        d == g * d1,
        a1 == qi * d1 + f,
        e <= 62,
        (e == 0 && qi == 0 && two_s == pow2(61nat)) || (e >= 1 && pow2((e - 1) as nat) <= qi
            && two_s == pow2((62 - e) as nat)),
    ensures
        d * pow2(60) <= imax(d, a) * two_s,
{
    lemma_pow2_pos(60nat);
    lemma_pow2_monotonic_le(60nat, 61nat);
    assert(d >= 1) by (nonlinear_arith)
        requires
            d == g * d1,
            g >= 1,
            d1 >= 1,
    ;
    if e == 0 {
        assert(a1 < d1);
        assert(a < d) by (nonlinear_arith)
            requires
                a == g * a1,
                d == g * d1,
                a1 < d1,
                g >= 1,
        ;
        assert(imax(d, a) == d);
        assert(d * pow2(60) <= d * pow2(61)) by (nonlinear_arith)
            requires
                pow2(60) <= pow2(61),
                d >= 1,
        ;
    } else {
        lemma_pow2_pos((e - 1) as nat);
        lemma_pow2_adds((e - 1) as nat, (62 - e) as nat);
        assert(pow2((e - 1) as nat) * pow2((62 - e) as nat) == pow2(61nat));
        assert(qi >= 1);
        assert(a1 >= qi * d1) by (nonlinear_arith)
            requires
                a1 == qi * d1 + f,
                f >= 0,
        ;
        assert(a >= qi * d) by (nonlinear_arith)
            requires
                a == g * a1,
                d == g * d1,
                a1 >= qi * d1,
                g >= 1,
        ;
        assert(qi * d >= d) by (nonlinear_arith)
            requires
                qi >= 1,
                d >= 1,
        ;
        assert(imax(d, a) == a);
        assert(two_s >= 1) by {
            lemma_pow2_pos((62 - e) as nat);
        }
        assert(a * two_s >= (qi * d) * two_s) by (nonlinear_arith)
            requires
                a >= qi * d,
                two_s >= 1,
        ;
        assert((qi * d) * two_s >= (pow2((e - 1) as nat) * d) * two_s) by (nonlinear_arith)
            requires
                qi >= pow2((e - 1) as nat),
                d >= 1,
                two_s >= 1,
        ;
        assert((pow2((e - 1) as nat) * d) * two_s == (pow2((e - 1) as nat) * two_s) * d)
            by (nonlinear_arith);
        assert(pow2((e - 1) as nat) * two_s == pow2(61nat));
        assert(pow2(61nat) * d >= pow2(60) * d) by (nonlinear_arith)
            requires
                pow2(61nat) >= pow2(60),
                d >= 1,
        ;
        assert(pow2(60) * d == d * pow2(60)) by (nonlinear_arith);
    }
}

// ---------------------------------------------------------------------------
// The rounding step
// ---------------------------------------------------------------------------
/// Round a non-negative exact fraction `a/d` to a canonical, bounded `Q`.
///
/// `mdir` is the direction applied to this *magnitude*; `round_to_budget`
/// flips it for negative values so that `Dir::Down`/`Dir::Up` stay directed on
/// the value.
pub fn round_nonneg(a: i128, d: i128, mdir: Dir) -> (r: Q)
    requires
        0 <= a <= NMAX,
        0 < d <= DMAX,
    ensures
        r.wf(),
        r.n() >= 0,
        reduced_fits(a as int, d as int) ==> q_is(r, a as int, d as int),
        saturating(a as int, d as int) ==> (r.n() == BUDGET as int && r.d() == 1),
        !saturating(a as int, d as int) ==> r3_bound(r, a as int, d as int),
        !saturating(a as int, d as int) && mdir == Dir::Down ==> frac_le(
            r.n(),
            r.d(),
            a as int,
            d as int,
        ),
        !saturating(a as int, d as int) && mdir == Dir::Up ==> frac_le(
            a as int,
            d as int,
            r.n(),
            r.d(),
        ),
{
    let (a1, d1, g) = reduce_mag(a, d);
    let ghost ai: int = a as int;
    let ghost di: int = d as int;
    let ghost a1i: int = a1 as int;
    let ghost d1i: int = d1 as int;
    let ghost gi: int = g as int;

    // ----- R1: the exact reduced result already fits.
    if a1 <= BUDGET128 && d1 <= BUDGET128 {
        proof {
            assert(a1i * di == ai * d1i) by (nonlinear_arith)
                requires
                    ai == gi * a1i,
                    di == gi * d1i,
            ;
            assert(di >= 1) by (nonlinear_arith)
                requires
                    di == gi * d1i,
                    gi >= 1,
                    d1i >= 1,
            ;
            if saturating(ai, di) {
                assert(ai >= BUDGET as int * di);
                assert(a1i >= BUDGET as int * d1i) by (nonlinear_arith)
                    requires
                        ai == gi * a1i,
                        di == gi * d1i,
                        ai >= BUDGET as int * di,
                        gi >= 1,
                ;
                assert(d1i == 1) by (nonlinear_arith)
                    requires
                        a1i >= BUDGET as int * d1i,
                        a1i <= BUDGET as int,
                        d1i >= 1,
                ;
            }
            assert(imax(di, iabs(ai) as int) >= 0);
            assert(a1i * di - ai * d1i == 0);
            assert(d1i * imax(di, iabs(ai) as int) >= 0) by (nonlinear_arith)
                requires
                    d1i >= 1,
                    imax(di, iabs(ai) as int) >= 0,
            ;
        }
        return Q::from_parts(a1 as i64, d1 as i64);
    }

    // ----- Split off the integer part.
    let qi = a1 / d1;
    let f = a1 % d1;
    proof {
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(a1i, d1i);
        vstd::arithmetic::div_mod::lemma_mod_bound(a1i, d1i);
        vstd::arithmetic::div_mod::lemma_div_pos_is_pos(a1i, d1i);
    }

    // ----- Magnitude overflow: no `Q` can carry this value.
    if qi >= BUDGET128 {
        proof {
            assert(a1i >= BUDGET as int * d1i) by (nonlinear_arith)
                requires
                    a1i == d1i * (qi as int) + f as int,
                    qi as int >= BUDGET as int,
                    f as int >= 0,
                    d1i >= 1,
            ;
            assert(ai >= BUDGET as int * di) by (nonlinear_arith)
                requires
                    ai == gi * a1i,
                    di == gi * d1i,
                    a1i >= BUDGET as int * d1i,
                    gi >= 1,
            ;
            lemma_gcd_any_one(iabs(BUDGET as int));
        }
        return Q::from_parts(BUDGET, 1);
    }

    // Everything from here on is in-range: `qi < BUDGET`.
    proof {
        assert(a1i < BUDGET as int * d1i) by (nonlinear_arith)
            requires
                a1i == d1i * (qi as int) + f as int,
                qi as int <= BUDGET as int - 1,
                (f as int) < d1i,
                d1i >= 1,
        ;
        assert(ai < BUDGET as int * di) by (nonlinear_arith)
            requires
                ai == gi * a1i,
                di == gi * d1i,
                a1i < BUDGET as int * d1i,
                gi >= 1,
        ;
    }

    // ----- Pick the dyadic grid and snap the fractional part exactly.
    let e = bitlen_i128(qi);
    let s: u32 = if e == 0 { 61 } else { 62 - e };
    let two_s = pow2_i128(s);
    let (k, rem) = shift_div(f, d1, s);
    let kk: i128 = match mdir {
        Dir::Down => k,
        Dir::Up => if rem > 0 {
            k + 1
        } else {
            k
        },
        Dir::Nearest => if 2 * rem >= d1 {
            k + 1
        } else {
            k
        },
    };
    proof {
        lemma_pow2_small();
        if e == 0 {
            assert(qi == 0);
        } else {
            lemma_pow2_monotonic_le((62 - e) as nat, 61nat);
        }
        lemma_pow2_pos(s as nat);
        lemma_pow2_monotonic_le(s as nat, 61nat);
        if e == 0 {
            assert((qi as int) * (two_s as int) + (two_s as int) == two_s as int)
                by (nonlinear_arith)
                requires
                    qi as int == 0,
            ;
            lemma_pow2_monotonic_le(61nat, 62nat);
        } else {
            lemma_pow2_adds(e as nat, s as nat);
            assert(pow2(e as nat) * pow2(s as nat) == pow2(62nat));
            assert((qi as int) * (two_s as int) + (two_s as int) <= pow2(62nat))
                by (nonlinear_arith)
                requires
                    (qi as int) < pow2(e as nat),
                    two_s as int == pow2(s as nat),
                    pow2(e as nat) * pow2(s as nat) == pow2(62nat),
                    qi as int >= 0,
                    two_s as int >= 1,
            ;
        }
        assert((qi as int) * (two_s as int) + (two_s as int) <= pow2(62nat));
        assert(two_s as int <= pow2(61nat));
    }
    let num_eff: i128 = qi * two_s + kk;

    // ----- Carry: the fractional part rounded up to a whole unit.
    if kk == two_s {
        proof {
            lemma_gcd_any_one(iabs((qi + 1) as int));
            assert(((qi as int) + 1) * (two_s as int) == (num_eff as int) * 1)
                by (nonlinear_arith)
                requires
                    kk as int == two_s as int,
                    num_eff as int == (qi as int) * (two_s as int) + kk as int,
            ;
            lemma_round_body(
                (qi + 1) as int,
                1,
                ai,
                di,
                a1i,
                d1i,
                gi,
                two_s as int,
                num_eff as int,
                qi as int,
                f as int,
                k as int,
                rem as int,
                kk as int,
                e as nat,
                mdir,
            );
        }
        return Q::from_parts((qi + 1) as i64, 1);
    }

    // ----- Reassemble and canonicalize.
    let g2 = gcd_u128(num_eff as u128, two_s as u128) as i128;
    proof {
        lemma_gcd_positive(num_eff as nat, two_s as nat);
        lemma_gcd_comm(num_eff as nat, two_s as nat);
        lemma_gcd_le(two_s as nat, num_eff as nat);
        lemma_gcd_divides(num_eff as nat, two_s as nat);
        lemma_exact_div(num_eff as int, g2 as int);
        lemma_exact_div(two_s as int, g2 as int);
        lemma_gcd_reduce_coprime(num_eff as nat, two_s as nat);
        lemma_div_le_self(num_eff as int, g2 as int);
        lemma_div_le_self(two_s as int, g2 as int);
        lemma_div_pos(two_s as int, g2 as int);
    }
    let n2 = num_eff / g2;
    let d2 = two_s / g2;
    proof {
        assert((n2 as int) * (two_s as int) == (num_eff as int) * (d2 as int)) by (nonlinear_arith)
            requires
                num_eff as int == (g2 as int) * (n2 as int),
                two_s as int == (g2 as int) * (d2 as int),
        ;
        lemma_round_body(
            n2 as int,
            d2 as int,
            ai,
            di,
            a1i,
            d1i,
            gi,
            two_s as int,
            num_eff as int,
            qi as int,
            f as int,
            k as int,
            rem as int,
            kk as int,
            e as nat,
            mdir,
        );
    }
    Q::from_parts(n2 as i64, d2 as i64)
}

/// The shared post-condition proof for both tails of `round_nonneg`.
///
/// Given the loop-invariant facts the algorithm established — the integer split,
/// the exact scaled division, and `rn/rd == num_eff/two_s` — this discharges R3
/// and both R2 directions in one place.
pub proof fn lemma_round_body(
    rn: int,
    rd: int,
    a: int,
    d: int,
    a1: int,
    d1: int,
    g: int,
    two_s: int,
    num_eff: int,
    qi: int,
    f: int,
    k: int,
    rem: int,
    kk: int,
    e: nat,
    mdir: Dir,
)
    requires
        g >= 1,
        d1 >= 1,
        a1 >= 0,
        rd >= 1,
        rn >= 0,
        two_s >= 1,
        qi >= 0,
        a == g * a1,
        d == g * d1,
        a1 == d1 * qi + f,
        0 <= f < d1,
        k * d1 + rem == f * two_s,
        0 <= rem < d1,
        0 <= k,
        kk == k || kk == k + 1,
        num_eff == qi * two_s + kk,
        rn * two_s == num_eff * rd,
        e <= 62,
        (e == 0 && qi == 0 && two_s == pow2(61nat)) || (e >= 1 && pow2((e - 1) as nat) <= qi
            && two_s == pow2((62 - e) as nat)),
        mdir == Dir::Down ==> kk == k,
        mdir == Dir::Up ==> (kk == k + 1 || rem == 0),
    ensures
        (rn * d - a * rd) * pow2(60) <= rd * imax(d, iabs(a) as int),
        -(rd * imax(d, iabs(a) as int)) <= (rn * d - a * rd) * pow2(60),
        mdir == Dir::Down ==> rn * d <= a * rd,
        mdir == Dir::Up ==> a * rd <= rn * d,
{
    // The effective error, exactly.
    assert(num_eff * d1 - a1 * two_s == (kk - k) * d1 - rem) by (nonlinear_arith)
        requires
            num_eff == qi * two_s + kk,
            a1 == d1 * qi + f,
            k * d1 + rem == f * two_s,
    ;
    if kk == k {
        assert((kk - k) * d1 == 0) by (nonlinear_arith)
            requires
                kk == k,
        ;
    } else {
        assert((kk - k) * d1 == d1) by (nonlinear_arith)
            requires
                kk == k + 1,
        ;
    }
    assert(-d1 <= num_eff * d1 - a1 * two_s <= d1);
    assert(mdir == Dir::Down ==> num_eff * d1 - a1 * two_s <= 0);
    assert(mdir == Dir::Up ==> num_eff * d1 - a1 * two_s >= 0);
    assert(a >= 0) by (nonlinear_arith)
        requires
            a == g * a1,
            a1 >= 0,
            g >= 1,
    ;
    assert(iabs(a) as int == a);
    lemma_grid_hyp(a, d, a1, d1, g, qi, f, two_s, e);
    lemma_r3_core(rn, rd, a, d, a1, d1, g, two_s, num_eff);
    lemma_r2_core(rn, rd, a, d, a1, d1, g, two_s, num_eff);
}


/// Round an exact `i128` fraction `n/d` to a canonical, bounded `Q`.
///
/// This is the single rounding entry point for the whole crate. Its contract is
/// the rounding contract:
///
/// * **R1** — when the exact reduced value fits the budget the result *is* that
///   value, bit for bit.
/// * **R2** — `Dir::Down` never overshoots and `Dir::Up` never undershoots,
///   including for negative values.
/// * **R3** — otherwise `|result - n/d| <= 2^-60 * max(1, |n/d|)`.
///
/// Saturation (the value's magnitude exceeding `2^62 - 1`) is the one case
/// where R2 and R3 are switched off, and the `ensures` says so explicitly.
pub fn round_to_budget(n: i128, d: i128, dir: Dir) -> (r: Q)
    requires
        -NMAX <= n <= NMAX,
        0 < d <= DMAX,
    ensures
        r.wf(),
        reduced_fits(n as int, d as int) ==> q_is(r, n as int, d as int),
        saturating(n as int, d as int) ==> sat_result(r, n as int),
        !saturating(n as int, d as int) ==> r3_bound(r, n as int, d as int),
        !saturating(n as int, d as int) && dir == Dir::Down ==> frac_le(
            r.n(),
            r.d(),
            n as int,
            d as int,
        ),
        !saturating(n as int, d as int) && dir == Dir::Up ==> frac_le(
            n as int,
            d as int,
            r.n(),
            r.d(),
        ),
{
    if n >= 0 {
        round_nonneg(n, d, dir)
    } else {
        let mdir = dir.for_magnitude(true);
        let m = round_nonneg(-n, d, mdir);
        let ghost a: int = -(n as int);
        let ghost di: int = d as int;
        proof {
            assert(iabs(n as int) as int == a);
            assert(iabs(a) as int == a);
            assert(imax(di, iabs(n as int) as int) == imax(di, iabs(a) as int));
            let x: int = m.n() * di - a * m.d();
            let y: int = (-m.n()) * di - (n as int) * m.d();
            assert(y == -x) by (nonlinear_arith)
                requires
                    x == m.n() * di - a * m.d(),
                    y == (-m.n()) * di - (n as int) * m.d(),
                    a == -(n as int),
            ;
            lemma_pow2_pos(60nat);
            assert(y * pow2(60) == -(x * pow2(60))) by (nonlinear_arith)
                requires
                    y == -x,
            ;
        }
        Q::from_parts(-m.numerator(), m.denominator())
    }
}

} // verus!
