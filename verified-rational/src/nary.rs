//! N-ary helpers, defined as **left-to-right binary folds with a fixed order**.
//!
//! The fixed order is the point: it is what makes results reproducible bit for
//! bit regardless of how the caller happens to iterate. Accumulating in `i128`
//! instead would reopen the overflow analysis for no benefit, so these are
//! deliberately ordinary folds — each step inherits V2 and the R1–R3 contract
//! from the binary operation it calls.
//!
//! The accumulated error after `k` elements is `k * 2^-60 * max(1, |value|)` by
//! induction on R3. That bound (obligation V8) is stated in the crate
//! documentation and exercised by the long-fold differential tests; it is not
//! yet carried in these functions' `ensures`.

use crate::q::*;
use vstd::prelude::*;

verus! {

impl Q {
    /// `x0 + x1 + ... `, left to right. Empty slice sums to zero.
    pub fn sum(xs: &[Q]) -> (r: Q)
        requires
            forall|i: int| 0 <= i < xs@.len() ==> #[trigger] xs@[i].wf(),
        ensures
            r.wf(),
    {
        let mut acc = Q::zero();
        let mut i: usize = 0;
        while i < xs.len()
            invariant
                acc.wf(),
                i <= xs@.len(),
                forall|j: int| 0 <= j < xs@.len() ==> #[trigger] xs@[j].wf(),
            decreases xs@.len() - i,
        {
            acc = acc.add(xs[i]);
            i += 1;
        }
        acc
    }

    /// `x0 * x1 * ... `, left to right. Empty slice multiplies to one.
    pub fn product(xs: &[Q]) -> (r: Q)
        requires
            forall|i: int| 0 <= i < xs@.len() ==> #[trigger] xs@[i].wf(),
        ensures
            r.wf(),
    {
        let mut acc = Q::one();
        let mut i: usize = 0;
        while i < xs.len()
            invariant
                acc.wf(),
                i <= xs@.len(),
                forall|j: int| 0 <= j < xs@.len() ==> #[trigger] xs@[j].wf(),
            decreases xs@.len() - i,
        {
            acc = acc.mul(xs[i]);
            i += 1;
        }
        acc
    }

    /// `sum(w_i * x_i) / sum(w_i)` — the averaging-belief-fusion shape.
    ///
    /// Returns `None` when the weights sum to zero, so the caller never has to
    /// discharge a non-zero precondition it cannot know statically.
    pub fn weighted_mean(pairs: &[(Q, Q)]) -> (r: Option<Q>)
        requires
            forall|i: int|
                0 <= i < pairs@.len() ==> (#[trigger] pairs@[i]).0.wf() && pairs@[i].1.wf(),
        ensures
            r is Some ==> r->Some_0.wf(),
    {
        let mut num = Q::zero();
        let mut den = Q::zero();
        let mut i: usize = 0;
        while i < pairs.len()
            invariant
                num.wf(),
                den.wf(),
                i <= pairs@.len(),
                forall|j: int|
                    0 <= j < pairs@.len() ==> (#[trigger] pairs@[j]).0.wf() && pairs@[j].1.wf(),
            decreases pairs@.len() - i,
        {
            let (w, x) = pairs[i];
            num = num.add(w.mul(x));
            den = den.add(w);
            i += 1;
        }
        if den.is_zero() {
            None
        } else {
            Some(num.div(den))
        }
    }
}

} // verus!
