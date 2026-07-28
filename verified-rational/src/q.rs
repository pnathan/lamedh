//! The `Q` type: a canonical, bounded rational, plus its ghost model.
//!
//! Verification obligation **V1** lives here: `Q::wf` is the type invariant
//! (I1 canonical ∧ I2 bounded), and every public operation in this crate
//! `requires` it on inputs and `ensures` it on outputs.
//!
//! The ghost model is deliberately *division-free*: values are compared and
//! specified by cross-multiplication over unbounded `int`, never by SMT
//! division. `frac_eq`/`frac_le` below are the only vocabulary the rest of the
//! crate uses to say what a result is worth.

use crate::gcd::*;
use vstd::prelude::*;

verus! {

/// The width budget: `2^62 - 1`.
///
/// Chosen so that every exact `i128` intermediate this crate forms is provably
/// in range. The binding constraint is `add`, whose numerator is
/// `n1*d2 + n2*d1 <= 2*(2^62-1)^2 < 2^125`; a `2^63` budget would push the same
/// expression to `2^127`, which overflows `i128::MAX == 2^127 - 1`.
pub const BUDGET: i64 = 0x3fff_ffff_ffff_ffff;

/// `BUDGET` widened once, for the `i128` arithmetic paths.
pub const BUDGET128: i128 = 0x3fff_ffff_ffff_ffff;

/// A rational number `num / den` in canonical, bounded form.
///
/// Fields are private: the only way to obtain a `Q` is through a constructor
/// that establishes [`Q::wf`], so the invariant cannot be broken from outside
/// the crate even in the unverified (plain `cargo build`) configuration.
/// Ghost code observes the value through [`Q::n`] and [`Q::d`].
///
/// `PartialEq`/`Eq`/`Hash` are derived, and that is sound *because* of
/// canonicality: structural equality of the two fields is exactly mathematical
/// equality. `Ord` is **not** derived — the derived lexicographic order on
/// `(num, den)` would report `1/3 > 1/2`. It is implemented by
/// cross-multiplication in [`crate::cmp`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Q {
    num: i64,
    den: i64,
}

impl Q {
    /// Ghost numerator.
    pub closed spec fn n(self) -> int {
        self.num as int
    }

    /// Ghost denominator.
    pub closed spec fn d(self) -> int {
        self.den as int
    }

    /// The type invariant: **I1** (canonical) ∧ **I2** (bounded).
    ///
    /// `spec_gcd(|num|, den) == 1` subsumes the `num == 0 ==> den == 1` clause,
    /// because `spec_gcd(0, d) == d`.
    pub open spec fn wf(self) -> bool {
        &&& self.d() > 0
        &&& self.d() <= BUDGET as int
        &&& -(BUDGET as int) <= self.n()
        &&& self.n() <= BUDGET as int
        &&& spec_gcd(iabs(self.n()), iabs(self.d())) == 1
    }

    /// Executable numerator. Zero-cost; the `ensures` ties it to the ghost model.
    pub fn numerator(self) -> (r: i64)
        ensures
            r as int == self.n(),
    {
        self.num
    }

    /// Executable denominator.
    pub fn denominator(self) -> (r: i64)
        ensures
            r as int == self.d(),
    {
        self.den
    }

    /// Unchecked internal constructor. Establishes only the field relation; the
    /// caller owes [`Q::wf`].
    pub(crate) fn from_parts(num: i64, den: i64) -> (r: Q)
        ensures
            r.n() == num as int,
            r.d() == den as int,
    {
        Q { num, den }
    }
}

// ---------------------------------------------------------------------------
// The relational ghost model (division-free)
// ---------------------------------------------------------------------------
/// `n1/d1 == n2/d2`, stated without division. Meaningful when `d1, d2 > 0`.
pub open spec fn frac_eq(n1: int, d1: int, n2: int, d2: int) -> bool {
    n1 * d2 == n2 * d1
}

/// `n1/d1 <= n2/d2`, stated without division. Requires `d1, d2 > 0` to be sound
/// (a negative denominator would flip the inequality).
pub open spec fn frac_le(n1: int, d1: int, n2: int, d2: int) -> bool {
    n1 * d2 <= n2 * d1
}

/// `n1/d1 < n2/d2`. Requires `d1, d2 > 0`.
pub open spec fn frac_lt(n1: int, d1: int, n2: int, d2: int) -> bool {
    n1 * d2 < n2 * d1
}

/// Mathematical equality of two `Q`s.
pub open spec fn q_eq(a: Q, b: Q) -> bool {
    frac_eq(a.n(), a.d(), b.n(), b.d())
}

/// Mathematical `<=` on two `Q`s.
pub open spec fn q_le(a: Q, b: Q) -> bool {
    frac_le(a.n(), a.d(), b.n(), b.d())
}

/// Mathematical `<` on two `Q`s.
pub open spec fn q_lt(a: Q, b: Q) -> bool {
    frac_lt(a.n(), a.d(), b.n(), b.d())
}

/// `q` equals the (not necessarily reduced, not necessarily bounded) fraction `n/d`.
pub open spec fn q_is(q: Q, n: int, d: int) -> bool {
    frac_eq(q.n(), q.d(), n, d)
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------
impl Q {
    /// `0`, as `0/1`.
    pub fn zero() -> (r: Q)
        ensures
            r.wf(),
            r.n() == 0,
            r.d() == 1,
    {
        proof {
            assert(spec_gcd(0nat, 1nat) == 1) by (compute_only);
        }
        Q::from_parts(0, 1)
    }

    /// `1`, as `1/1`.
    pub fn one() -> (r: Q)
        ensures
            r.wf(),
            r.n() == 1,
            r.d() == 1,
    {
        proof {
            assert(spec_gcd(1nat, 1nat) == 1) by (compute_only);
        }
        Q::from_parts(1, 1)
    }

    /// The largest representable value, `(2^62 - 1) / 1`.
    pub fn max_value() -> (r: Q)
        ensures
            r.wf(),
            r.n() == BUDGET as int,
            r.d() == 1,
    {
        proof {
            lemma_gcd_any_one(iabs(BUDGET as int));
        }
        Q::from_parts(BUDGET, 1)
    }

    /// The smallest representable value, `-(2^62 - 1) / 1`.
    pub fn min_value() -> (r: Q)
        ensures
            r.wf(),
            r.n() == -(BUDGET as int),
            r.d() == 1,
    {
        proof {
            lemma_gcd_any_one(iabs(-(BUDGET as int)));
        }
        Q::from_parts(-BUDGET, 1)
    }

    /// Exact integer injection. `None` when `|i|` exceeds the budget.
    pub fn from_int(i: i64) -> (r: Option<Q>)
        ensures
            (i < -BUDGET || i > BUDGET) ==> r is None,
            (-BUDGET <= i && i <= BUDGET) ==> r is Some,
            r is Some ==> {
                &&& r->Some_0.wf()
                &&& r->Some_0.n() == i as int
                &&& r->Some_0.d() == 1
            },
    {
        if i < -BUDGET || i > BUDGET {
            None
        } else {
            proof {
                lemma_gcd_any_one(iabs(i as int));
            }
            Some(Q::from_parts(i, 1))
        }
    }

    /// Whether this value is zero.
    pub fn is_zero(self) -> (r: bool)
        requires
            self.wf(),
        ensures
            r == (self.n() == 0),
    {
        self.numerator() == 0
    }

    /// Whether this value is one.
    pub fn is_one(self) -> (r: bool)
        requires
            self.wf(),
        ensures
            r == (self.n() == self.d()),
    {
        self.numerator() == self.denominator()
    }

    /// `-1`, `0`, or `1` according to the sign.
    pub fn signum(self) -> (r: i64)
        requires
            self.wf(),
        ensures
            self.n() > 0 ==> r == 1,
            self.n() == 0 ==> r == 0,
            self.n() < 0 ==> r == -1,
    {
        let n = self.numerator();
        if n > 0 {
            1
        } else if n < 0 {
            -1
        } else {
            0
        }
    }
}

/// `spec_gcd(a, 1) == 1` for every `a` — the canonicality witness every
/// integer-valued constructor needs.
pub proof fn lemma_gcd_any_one(a: nat)
    ensures
        spec_gcd(a, 1nat) == 1,
{
    assert(a % 1nat == 0nat);
    assert(spec_gcd(a, 1nat) == spec_gcd(1nat, 0nat));
}

} // verus!
