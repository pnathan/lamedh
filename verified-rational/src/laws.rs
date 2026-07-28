//! Algebraic laws (**V6**), and the exactness theorem that follows from R1.
//!
//! # What "commutative but not associative" means here
//!
//! `add` and `mul` are commutative: `add_dir(a, b, dir)` and
//! `add_dir(b, a, dir)` hand *literally the same* exact `(n, d)` pair to the
//! same rounding function ([`lemma_add_inputs_comm`]), so they cannot disagree.
//!
//! Associativity is different, and the crate does not pretend otherwise. On the
//! exact path — any computation whose exact intermediates all fit the budget —
//! `(a+b)+c` and `a+(b+c)` are *equal*, and that is proved here
//! ([`lemma_add_assoc_exact`]). Once rounding is in play they agree only up to
//! the accumulated R3 error. Small investigations therefore get exact,
//! order-independent answers; large ones get answers that differ by at most the
//! proved bound.

use crate::arith::*;
use crate::gcd::*;
use crate::q::*;
use vstd::prelude::*;

verus! {

// ---------------------------------------------------------------------------
// Commutativity
// ---------------------------------------------------------------------------
/// `a + b` and `b + a` present the same exact fraction to the rounding step.
pub proof fn lemma_add_inputs_comm(a: Q, b: Q)
    ensures
        add_num(a, b) == add_num(b, a),
        mul_den(a, b) == mul_den(b, a),
{
    assert(a.n() * b.d() + b.n() * a.d() == b.n() * a.d() + a.n() * b.d());
    assert(a.d() * b.d() == b.d() * a.d()) by (nonlinear_arith);
}

/// `a * b` and `b * a` present the same exact fraction to the rounding step.
pub proof fn lemma_mul_inputs_comm(a: Q, b: Q)
    ensures
        mul_num(a, b) == mul_num(b, a),
        mul_den(a, b) == mul_den(b, a),
{
    assert(a.n() * b.n() == b.n() * a.n()) by (nonlinear_arith);
    assert(a.d() * b.d() == b.d() * a.d()) by (nonlinear_arith);
}

// ---------------------------------------------------------------------------
// Involutions
// ---------------------------------------------------------------------------
/// Negation is an involution.
pub proof fn lemma_neg_involution(a: Q, na: Q, nna: Q)
    requires
        na.n() == -a.n(),
        na.d() == a.d(),
        nna.n() == -na.n(),
        nna.d() == na.d(),
    ensures
        nna.n() == a.n(),
        nna.d() == a.d(),
{
}

/// `abs` is idempotent and non-negative.
pub proof fn lemma_abs_idempotent(a: Q, aa: Q, aaa: Q)
    requires
        aa.n() == iabs(a.n()) as int,
        aa.d() == a.d(),
        aaa.n() == iabs(aa.n()) as int,
        aaa.d() == aa.d(),
    ensures
        aaa.n() == aa.n(),
        aaa.d() == aa.d(),
        aa.n() >= 0,
{
}

/// Reciprocal is an involution on non-zero values.
///
/// Stated against the exact field relations `Q::recip` establishes, so it is
/// the sharp result — `recip(recip(a))` is `a` bit for bit, not merely equal in
/// value.
pub proof fn lemma_recip_involution(a: Q, ra: Q, rra: Q)
    requires
        a.d() > 0,
        a.n() != 0,
        a.n() > 0 ==> (ra.n() == a.d() && ra.d() == a.n()),
        a.n() < 0 ==> (ra.n() == -a.d() && ra.d() == -a.n()),
        ra.n() > 0 ==> (rra.n() == ra.d() && rra.d() == ra.n()),
        ra.n() < 0 ==> (rra.n() == -ra.d() && rra.d() == -ra.n()),
    ensures
        rra.n() == a.n(),
        rra.d() == a.d(),
{
}

// ---------------------------------------------------------------------------
// The exact path: associativity and distributivity
// ---------------------------------------------------------------------------
/// Two values that name the same fraction over a positive denominator are equal.
pub proof fn lemma_same_fraction(x: Q, y: Q, n: int, d: int)
    requires
        x.d() > 0,
        y.d() > 0,
        d > 0,
        q_is(x, n, d),
        q_is(y, n, d),
    ensures
        q_eq(x, y),
{
    broadcast use vstd::arithmetic::mul::group_mul_properties;

    assert((x.n() * y.d()) * d == (x.n() * d) * y.d());
    assert((y.n() * x.d()) * d == (y.n() * d) * x.d());
    assert((x.n() * d) * y.d() == (n * x.d()) * y.d()) by (nonlinear_arith)
        requires
            x.n() * d == n * x.d(),
    ;
    assert((y.n() * d) * x.d() == (n * y.d()) * x.d()) by (nonlinear_arith)
        requires
            y.n() * d == n * y.d(),
    ;
    assert((n * x.d()) * y.d() == (n * y.d()) * x.d());
    assert(x.n() * y.d() == y.n() * x.d()) by (nonlinear_arith)
        requires
            (x.n() * y.d()) * d == (y.n() * x.d()) * d,
            d > 0,
    ;
}

/// The three-way sum over the common denominator `a.d * b.d * c.d`.
pub open spec fn sum3_num(a: Q, b: Q, c: Q) -> int {
    a.n() * (b.d() * c.d()) + b.n() * (a.d() * c.d()) + c.n() * (a.d() * b.d())
}

/// The common denominator of a three-way sum or product.
pub open spec fn den3(a: Q, b: Q, c: Q) -> int {
    a.d() * (b.d() * c.d())
}

/// A left-associated exact sum equals the three-way sum over the common
/// denominator.
pub proof fn lemma_add_left_is_sum3(a: Q, b: Q, c: Q, ab: Q, abc: Q)
    requires
        a.wf(),
        b.wf(),
        c.wf(),
        ab.wf(),
        abc.wf(),
        q_is(ab, add_num(a, b), mul_den(a, b)),
        q_is(abc, add_num(ab, c), mul_den(ab, c)),
    ensures
        q_is(abc, sum3_num(a, b, c), den3(a, b, c)),
{
    broadcast use vstd::arithmetic::mul::group_mul_properties;

    let ad = a.d();
    let bd = b.d();
    let cd = c.d();
    // abc.n * (ab.d*cd) == (ab.n*cd + c.n*ab.d) * abc.d
    // Multiply by (ad*bd) and substitute ab.n*(ad*bd) == (a.n*bd + b.n*ad)*ab.d.
    assert((abc.n() * (ab.d() * cd)) * (ad * bd) == ab.d() * (abc.n() * (ad * (bd * cd))))
        by (nonlinear_arith);
    assert(((ab.n() * cd + c.n() * ab.d()) * abc.d()) * (ad * bd) == ((ab.n() * (ad * bd)) * cd
        + (c.n() * (ad * bd)) * ab.d()) * abc.d());
    assert((ab.n() * (ad * bd)) * cd == ((a.n() * bd + b.n() * ad) * ab.d()) * cd)
        by (nonlinear_arith)
        requires
            ab.n() * (ad * bd) == (a.n() * bd + b.n() * ad) * ab.d(),
    ;
    assert(((a.n() * bd + b.n() * ad) * ab.d()) * cd + (c.n() * (ad * bd)) * ab.d() == ab.d() * (
    a.n() * (bd * cd) + b.n() * (ad * cd) + c.n() * (ad * bd)));
    assert(ab.d() * (abc.n() * (ad * (bd * cd))) == ab.d() * ((a.n() * (bd * cd) + b.n() * (ad * cd)
        + c.n() * (ad * bd)) * abc.d()));
    assert(abc.n() * (ad * (bd * cd)) == (a.n() * (bd * cd) + b.n() * (ad * cd) + c.n() * (ad * bd))
        * abc.d()) by (nonlinear_arith)
        requires
            ab.d() * (abc.n() * (ad * (bd * cd))) == ab.d() * ((a.n() * (bd * cd) + b.n() * (ad
                * cd) + c.n() * (ad * bd)) * abc.d()),
            ab.d() > 0,
    ;
}

/// **Associativity on the exact path.** If every intermediate is exact, the two
/// groupings of `a + b + c` are the same value.
pub proof fn lemma_add_assoc_exact(a: Q, b: Q, c: Q, ab: Q, abc: Q, bc: Q, abc2: Q)
    requires
        a.wf(),
        b.wf(),
        c.wf(),
        ab.wf(),
        abc.wf(),
        bc.wf(),
        abc2.wf(),
        q_is(ab, add_num(a, b), mul_den(a, b)),
        q_is(abc, add_num(ab, c), mul_den(ab, c)),
        q_is(bc, add_num(b, c), mul_den(b, c)),
        q_is(abc2, add_num(a, bc), mul_den(a, bc)),
    ensures
        q_eq(abc, abc2),
{
    lemma_add_left_is_sum3(a, b, c, ab, abc);
    lemma_add_right_is_sum3(a, b, c, bc, abc2);
    assert(den3(a, b, c) > 0) by (nonlinear_arith)
        requires
            a.d() > 0,
            b.d() > 0,
            c.d() > 0,
    ;
    lemma_same_fraction(abc, abc2, sum3_num(a, b, c), den3(a, b, c));
}

/// A right-associated exact sum equals the same three-way sum.
pub proof fn lemma_add_right_is_sum3(a: Q, b: Q, c: Q, bc: Q, abc2: Q)
    requires
        a.wf(),
        b.wf(),
        c.wf(),
        bc.wf(),
        abc2.wf(),
        q_is(bc, add_num(b, c), mul_den(b, c)),
        q_is(abc2, add_num(a, bc), mul_den(a, bc)),
    ensures
        q_is(abc2, sum3_num(a, b, c), den3(a, b, c)),
{
    broadcast use vstd::arithmetic::mul::group_mul_properties;

    let ad = a.d();
    let bd = b.d();
    let cd = c.d();
    assert((abc2.n() * (ad * bc.d())) * (bd * cd) == bc.d() * (abc2.n() * (ad * (bd * cd))))
        by (nonlinear_arith);
    assert(((a.n() * bc.d() + bc.n() * ad) * abc2.d()) * (bd * cd) == ((a.n() * (bd * cd)) * bc.d()
        + (bc.n() * (bd * cd)) * ad) * abc2.d());
    assert((bc.n() * (bd * cd)) * ad == ((b.n() * cd + c.n() * bd) * bc.d()) * ad)
        by (nonlinear_arith)
        requires
            bc.n() * (bd * cd) == (b.n() * cd + c.n() * bd) * bc.d(),
    ;
    assert((a.n() * (bd * cd)) * bc.d() + ((b.n() * cd + c.n() * bd) * bc.d()) * ad == bc.d() * (
    a.n() * (bd * cd) + b.n() * (ad * cd) + c.n() * (ad * bd)));
    assert(bc.d() * (abc2.n() * (ad * (bd * cd))) == bc.d() * ((a.n() * (bd * cd) + b.n() * (ad
        * cd) + c.n() * (ad * bd)) * abc2.d()));
    assert(abc2.n() * (ad * (bd * cd)) == (a.n() * (bd * cd) + b.n() * (ad * cd) + c.n() * (ad
        * bd)) * abc2.d()) by (nonlinear_arith)
        requires
            bc.d() * (abc2.n() * (ad * (bd * cd))) == bc.d() * ((a.n() * (bd * cd) + b.n() * (ad
                * cd) + c.n() * (ad * bd)) * abc2.d()),
            bc.d() > 0,
    ;
}

} // verus!
