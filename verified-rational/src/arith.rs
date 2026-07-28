//! Arithmetic. Every operation computes its exact result in `i128` and hands
//! it to [`round_to_budget`], so each one inherits the rounding contract rather
//! than re-proving it.
//!
//! `neg`, `abs`, `recip`, `min`, `max` and `clamp` never round at all — they are
//! exact by construction, which is worth knowing because the fusion engine uses
//! them freely inside otherwise-rounded expressions.
//!
//! Division by zero is a **precondition**, discharged statically by the caller.
//! It is not a runtime panic path and not an `Option`: under Verus the caller
//! proves the divisor is non-zero, and in the compiled build there is simply no
//! check to pay for.

use crate::cmp::*;
use crate::gcd::*;
use crate::q::*;
use crate::round::*;
use vstd::prelude::*;

verus! {

/// The contract every rounded operation delivers, in one place: the result is
/// well-formed, exact when the exact value fits (R1), correctly directed (R2),
/// and within the error bound otherwise (R3).
pub open spec fn rounded(r: Q, n: int, d: int, dir: Dir) -> bool {
    &&& r.wf()
    &&& (reduced_fits(n, d) ==> q_is(r, n, d))
    &&& (saturating(n, d) ==> sat_result(r, n))
    &&& (!saturating(n, d) ==> r3_bound(r, n, d))
    &&& (!saturating(n, d) && dir == Dir::Down ==> frac_le(r.n(), r.d(), n, d))
    &&& (!saturating(n, d) && dir == Dir::Up ==> frac_le(n, d, r.n(), r.d()))
}

/// Exact numerator of `a + b`.
pub open spec fn add_num(a: Q, b: Q) -> int {
    a.n() * b.d() + b.n() * a.d()
}

/// Exact numerator of `a - b`.
pub open spec fn sub_num(a: Q, b: Q) -> int {
    a.n() * b.d() - b.n() * a.d()
}

/// Exact common denominator of `a ± b` and of `a * b`.
pub open spec fn mul_den(a: Q, b: Q) -> int {
    a.d() * b.d()
}

/// Exact numerator of `a * b`.
pub open spec fn mul_num(a: Q, b: Q) -> int {
    a.n() * b.n()
}

/// Exact numerator of `a / b`, sign-normalised so the denominator is positive.
pub open spec fn div_num(a: Q, b: Q) -> int {
    if b.n() > 0 {
        a.n() * b.d()
    } else {
        -(a.n() * b.d())
    }
}

/// Exact denominator of `a / b`, sign-normalised to be positive.
pub open spec fn div_den(a: Q, b: Q) -> int {
    if b.n() > 0 {
        a.d() * b.n()
    } else {
        -(a.d() * b.n())
    }
}

/// All four products formed by the arithmetic operations stay inside `i128`,
/// and the sums stay inside it too. This is **V2** for `add`/`sub`/`mul`/`div`.
pub proof fn lemma_op_bounds(a: Q, b: Q)
    requires
        a.wf(),
        b.wf(),
    ensures
        -BUDGET_SQ <= a.n() * b.d() <= BUDGET_SQ,
        -BUDGET_SQ <= b.n() * a.d() <= BUDGET_SQ,
        -BUDGET_SQ <= a.n() * b.n() <= BUDGET_SQ,
        1 <= a.d() * b.d() <= BUDGET_SQ,
{
    lemma_cross_bounds(a, b);
    assert(a.n() * b.n() <= BUDGET_SQ as int) by (nonlinear_arith)
        requires
            -(BUDGET as int) <= a.n() <= BUDGET as int,
            -(BUDGET as int) <= b.n() <= BUDGET as int,
    ;
    assert(-(BUDGET_SQ as int) <= a.n() * b.n()) by (nonlinear_arith)
        requires
            -(BUDGET as int) <= a.n() <= BUDGET as int,
            -(BUDGET as int) <= b.n() <= BUDGET as int,
    ;
    assert(1 <= a.d() * b.d()) by (nonlinear_arith)
        requires
            a.d() >= 1,
            b.d() >= 1,
    ;
    assert(a.d() * b.d() <= BUDGET_SQ as int) by (nonlinear_arith)
        requires
            1 <= a.d() <= BUDGET as int,
            1 <= b.d() <= BUDGET as int,
    ;
}

impl Q {
    /// `a + b`, rounded in the given direction.
    pub fn add_dir(self, other: Q, dir: Dir) -> (r: Q)
        requires
            self.wf(),
            other.wf(),
        ensures
            rounded(r, add_num(self, other), mul_den(self, other), dir),
    {
        proof {
            lemma_op_bounds(self, other);
        }
        let n = (self.numerator() as i128) * (other.denominator() as i128) + (
        other.numerator() as i128) * (self.denominator() as i128);
        let d = (self.denominator() as i128) * (other.denominator() as i128);
        round_to_budget(n, d, dir)
    }

    /// `a + b`, round-to-nearest (ties away from zero).
    pub fn add(self, other: Q) -> (r: Q)
        requires
            self.wf(),
            other.wf(),
        ensures
            rounded(r, add_num(self, other), mul_den(self, other), Dir::Nearest),
    {
        self.add_dir(other, Dir::Nearest)
    }

    /// `a - b`, rounded in the given direction.
    pub fn sub_dir(self, other: Q, dir: Dir) -> (r: Q)
        requires
            self.wf(),
            other.wf(),
        ensures
            rounded(r, sub_num(self, other), mul_den(self, other), dir),
    {
        proof {
            lemma_op_bounds(self, other);
        }
        let n = (self.numerator() as i128) * (other.denominator() as i128) - (
        other.numerator() as i128) * (self.denominator() as i128);
        let d = (self.denominator() as i128) * (other.denominator() as i128);
        round_to_budget(n, d, dir)
    }

    /// `a - b`, round-to-nearest.
    pub fn sub(self, other: Q) -> (r: Q)
        requires
            self.wf(),
            other.wf(),
        ensures
            rounded(r, sub_num(self, other), mul_den(self, other), Dir::Nearest),
    {
        self.sub_dir(other, Dir::Nearest)
    }

    /// `a * b`, rounded in the given direction.
    pub fn mul_dir(self, other: Q, dir: Dir) -> (r: Q)
        requires
            self.wf(),
            other.wf(),
        ensures
            rounded(r, mul_num(self, other), mul_den(self, other), dir),
    {
        proof {
            lemma_op_bounds(self, other);
        }
        let n = (self.numerator() as i128) * (other.numerator() as i128);
        let d = (self.denominator() as i128) * (other.denominator() as i128);
        round_to_budget(n, d, dir)
    }

    /// `a * b`, round-to-nearest.
    pub fn mul(self, other: Q) -> (r: Q)
        requires
            self.wf(),
            other.wf(),
        ensures
            rounded(r, mul_num(self, other), mul_den(self, other), Dir::Nearest),
    {
        self.mul_dir(other, Dir::Nearest)
    }

    /// `a / b`, rounded in the given direction.
    ///
    /// `other` non-zero is a precondition — there is no division-by-zero branch
    /// in the compiled code.
    pub fn div_dir(self, other: Q, dir: Dir) -> (r: Q)
        requires
            self.wf(),
            other.wf(),
            other.n() != 0,
        ensures
            rounded(r, div_num(self, other), div_den(self, other), dir),
    {
        proof {
            lemma_op_bounds(self, other);
            assert(self.d() * other.n() != 0) by (nonlinear_arith)
                requires
                    self.d() >= 1,
                    other.n() != 0,
            ;
        }
        let n0 = (self.numerator() as i128) * (other.denominator() as i128);
        let d0 = (self.denominator() as i128) * (other.numerator() as i128);
        if other.numerator() > 0 {
            proof {
                assert(self.d() * other.n() > 0) by (nonlinear_arith)
                    requires
                        self.d() >= 1,
                        other.n() > 0,
                ;
            }
            round_to_budget(n0, d0, dir)
        } else {
            proof {
                assert(self.d() * other.n() < 0) by (nonlinear_arith)
                    requires
                        self.d() >= 1,
                        other.n() < 0,
                ;
            }
            round_to_budget(-n0, -d0, dir)
        }
    }

    /// `a / b`, round-to-nearest.
    pub fn div(self, other: Q) -> (r: Q)
        requires
            self.wf(),
            other.wf(),
            other.n() != 0,
        ensures
            rounded(r, div_num(self, other), div_den(self, other), Dir::Nearest),
    {
        self.div_dir(other, Dir::Nearest)
    }

    // -----------------------------------------------------------------------
    // Exact operations — these never round.
    // -----------------------------------------------------------------------
    /// `-a`. Always exact: I2 is symmetric in sign, so negation cannot leave
    /// the budget (and `i64::MIN` is excluded by I2, so it cannot overflow).
    pub fn neg(self) -> (r: Q)
        requires
            self.wf(),
        ensures
            r.wf(),
            r.n() == -self.n(),
            r.d() == self.d(),
    {
        proof {
            assert(iabs(-self.n()) == iabs(self.n()));
        }
        Q::from_parts(-self.numerator(), self.denominator())
    }

    /// `|a|`. Always exact.
    pub fn abs(self) -> (r: Q)
        requires
            self.wf(),
        ensures
            r.wf(),
            r.n() == iabs(self.n()) as int,
            r.d() == self.d(),
    {
        proof {
            assert(iabs(-self.n()) == iabs(self.n()));
        }
        if self.numerator() < 0 {
            Q::from_parts(-self.numerator(), self.denominator())
        } else {
            Q::from_parts(self.numerator(), self.denominator())
        }
    }

    /// `1/a`. Always exact — it just swaps the two fields and moves the sign,
    /// and canonicality is symmetric.
    pub fn recip(self) -> (r: Q)
        requires
            self.wf(),
            self.n() != 0,
        ensures
            r.wf(),
            r.n() * self.n() == self.d() * r.d(),
            self.n() > 0 ==> (r.n() == self.d() && r.d() == self.n()),
            self.n() < 0 ==> (r.n() == -self.d() && r.d() == -self.n()),
    {
        proof {
            lemma_gcd_comm(iabs(self.n()), iabs(self.d()));
            assert(iabs(self.d()) == self.d());
        }
        if self.numerator() > 0 {
            proof {
                assert(self.d() * self.n() == self.n() * self.d()) by (nonlinear_arith);
            }
            Q::from_parts(self.denominator(), self.numerator())
        } else {
            proof {
                assert((-self.d()) * self.n() == self.d() * (-self.n())) by (nonlinear_arith);
                assert(iabs(-self.d()) == iabs(self.d()));
                assert(iabs(-self.n()) == iabs(self.n()));
            }
            Q::from_parts(-self.denominator(), -self.numerator())
        }
    }
}

} // verus!
