//! Exact comparison. No epsilon, no partial order, no NaN.
//!
//! Every comparison is a single `i128` cross-multiplication. Under I2 both
//! products are bounded by `(2^62-1)^2 < 2^124`, so the operation is total and
//! provably overflow-free (**V2**), and it agrees with the ghost order by
//! construction (**V3**, **V6**).

use crate::q::*;
use vstd::prelude::*;

verus! {

/// `(2^62 - 1)^2`, the widest cross-multiplication product.
pub const BUDGET_SQ: i128 = 21267647932558653957237540927630737409;

impl Q {
    /// Exact `<=`.
    pub fn le(self, other: Q) -> (r: bool)
        requires
            self.wf(),
            other.wf(),
        ensures
            r == q_le(self, other),
    {
        proof {
            lemma_cross_bounds(self, other);
        }
        (self.numerator() as i128) * (other.denominator() as i128) <= (other.numerator() as i128) * (
        self.denominator() as i128)
    }

    /// Exact `<`.
    pub fn lt(self, other: Q) -> (r: bool)
        requires
            self.wf(),
            other.wf(),
        ensures
            r == q_lt(self, other),
    {
        proof {
            lemma_cross_bounds(self, other);
        }
        (self.numerator() as i128) * (other.denominator() as i128) < (other.numerator() as i128) * (
        self.denominator() as i128)
    }

    /// Exact equality.
    ///
    /// Because both operands are canonical this is the same relation as
    /// structural equality of the fields; see `lemma_eq_is_structural`.
    pub fn eq_exact(self, other: Q) -> (r: bool)
        requires
            self.wf(),
            other.wf(),
        ensures
            r == q_eq(self, other),
    {
        proof {
            lemma_cross_bounds(self, other);
        }
        (self.numerator() as i128) * (other.denominator() as i128) == (other.numerator() as i128) * (
        self.denominator() as i128)
    }

    /// `0 <= self <= 1` — the predicate the fusion engine checks constantly on
    /// beliefs, disbeliefs and uncertainties.
    pub fn in_unit_interval(self) -> (r: bool)
        requires
            self.wf(),
        ensures
            r == (self.n() >= 0 && self.n() <= self.d()),
    {
        let n = self.numerator();
        let d = self.denominator();
        0 <= n && n <= d
    }

    /// The smaller of two values. Exact — `min` never rounds.
    pub fn min(self, other: Q) -> (r: Q)
        requires
            self.wf(),
            other.wf(),
        ensures
            r.wf(),
            q_le(self, other) ==> r == self,
            !q_le(self, other) ==> r == other,
            q_le(r, self),
            q_le(r, other),
    {
        proof {
            lemma_le_refl(self);
            lemma_le_refl(other);
            lemma_le_total(self, other);
        }
        if self.le(other) {
            self
        } else {
            other
        }
    }

    /// The larger of two values. Exact — `max` never rounds.
    pub fn max(self, other: Q) -> (r: Q)
        requires
            self.wf(),
            other.wf(),
        ensures
            r.wf(),
            q_le(self, other) ==> r == other,
            !q_le(self, other) ==> r == self,
            q_le(self, r),
            q_le(other, r),
    {
        proof {
            lemma_le_refl(self);
            lemma_le_refl(other);
            lemma_le_total(self, other);
        }
        if self.le(other) {
            other
        } else {
            self
        }
    }

    /// Clamp into `[lo, hi]`. Exact. `lo <= hi` is a **precondition**, not a
    /// runtime check — the caller discharges it.
    pub fn clamp(self, lo: Q, hi: Q) -> (r: Q)
        requires
            self.wf(),
            lo.wf(),
            hi.wf(),
            q_le(lo, hi),
        ensures
            r.wf(),
            q_le(lo, r),
            q_le(r, hi),
    {
        proof {
            lemma_le_refl(self);
            lemma_le_refl(lo);
            lemma_le_refl(hi);
            lemma_le_total(self, lo);
            lemma_le_total(self, hi);
        }
        if self.lt(lo) {
            lo
        } else if hi.lt(self) {
            hi
        } else {
            self
        }
    }
}

/// Both cross-products stay inside `i128` under I2.
pub proof fn lemma_cross_bounds(a: Q, b: Q)
    requires
        a.wf(),
        b.wf(),
    ensures
        -BUDGET_SQ <= a.n() * b.d() <= BUDGET_SQ,
        -BUDGET_SQ <= b.n() * a.d() <= BUDGET_SQ,
{
    assert(a.n() * b.d() <= BUDGET_SQ as int) by (nonlinear_arith)
        requires
            a.n() <= BUDGET as int,
            b.d() <= BUDGET as int,
            b.d() >= 1,
            a.n() >= -(BUDGET as int),
    ;
    assert(-(BUDGET_SQ as int) <= a.n() * b.d()) by (nonlinear_arith)
        requires
            a.n() >= -(BUDGET as int),
            b.d() <= BUDGET as int,
            b.d() >= 1,
            a.n() <= BUDGET as int,
    ;
    assert(b.n() * a.d() <= BUDGET_SQ as int) by (nonlinear_arith)
        requires
            b.n() <= BUDGET as int,
            a.d() <= BUDGET as int,
            a.d() >= 1,
            b.n() >= -(BUDGET as int),
    ;
    assert(-(BUDGET_SQ as int) <= b.n() * a.d()) by (nonlinear_arith)
        requires
            b.n() >= -(BUDGET as int),
            a.d() <= BUDGET as int,
            a.d() >= 1,
            b.n() <= BUDGET as int,
    ;
}

// ---------------------------------------------------------------------------
// V6: the order is a total order agreeing with the ghost order
// ---------------------------------------------------------------------------
/// `<=` is reflexive.
pub proof fn lemma_le_refl(a: Q)
    ensures
        q_le(a, a),
{
}

/// `<=` is total: for any two values one direction holds.
pub proof fn lemma_le_total(a: Q, b: Q)
    requires
        a.wf(),
        b.wf(),
    ensures
        q_le(a, b) || q_le(b, a),
{
}

/// `<=` is antisymmetric (as a relation on values).
pub proof fn lemma_le_antisym(a: Q, b: Q)
    requires
        a.wf(),
        b.wf(),
        q_le(a, b),
        q_le(b, a),
    ensures
        q_eq(a, b),
{
}

/// `<=` is transitive. This is the one order law that genuinely needs
/// nonlinear reasoning: `a/b <= c/d <= e/f` has to be re-multiplied through the
/// middle denominator.
pub proof fn lemma_le_trans(a: Q, b: Q, c: Q)
    requires
        a.wf(),
        b.wf(),
        c.wf(),
        q_le(a, b),
        q_le(b, c),
    ensures
        q_le(a, c),
{
    // a.n*b.d <= b.n*a.d and b.n*c.d <= c.n*b.d, all denominators positive.
    assert((a.n() * b.d()) * c.d() <= (b.n() * a.d()) * c.d()) by (nonlinear_arith)
        requires
            a.n() * b.d() <= b.n() * a.d(),
            c.d() > 0,
    ;
    assert((b.n() * c.d()) * a.d() <= (c.n() * b.d()) * a.d()) by (nonlinear_arith)
        requires
            b.n() * c.d() <= c.n() * b.d(),
            a.d() > 0,
    ;
    assert((b.n() * a.d()) * c.d() == (b.n() * c.d()) * a.d()) by (nonlinear_arith);
    assert((a.n() * b.d()) * c.d() == (a.n() * c.d()) * b.d()) by (nonlinear_arith);
    assert((c.n() * b.d()) * a.d() == (c.n() * a.d()) * b.d()) by (nonlinear_arith);
    assert((a.n() * c.d()) * b.d() <= (c.n() * a.d()) * b.d());
    assert(a.n() * c.d() <= c.n() * a.d()) by (nonlinear_arith)
        requires
            (a.n() * c.d()) * b.d() <= (c.n() * a.d()) * b.d(),
            b.d() > 0,
    ;
}

} // verus!
