//! Exact-with-verified-rounding bounded rational arithmetic.
//!
//! `Q` is a rational `num/den` held in canonical form (`den > 0`,
//! `gcd(|num|, den) == 1`) inside a fixed width budget (`|num| <= 2^62 - 1`,
//! `den <= 2^62 - 1`). Arithmetic is exact whenever the exact result fits that
//! budget, and falls back to *directed rounding with a machine-checked error
//! bound* when it does not.
//!
//! The point of the design is determinism you can rely on and error you can
//! quote. Unlike `f64`:
//!
//! * every value has exactly one representation, so `Eq`/`Hash` mean
//!   mathematical equality and `serde` round-trips exactly;
//! * the order is total, not partial — there is no NaN;
//! * results do not depend on evaluation order beyond the fixed left-to-right
//!   fold documented for the n-ary helpers;
//! * the worst-case error is a proved theorem, not folklore.
//!
//! ```
//! use verified_rational::Q;
//!
//! // Short decimals are exact: 0.85 is 17/20, not 0.84999999999999998.
//! let r = Q::from_decimal(85, 2).unwrap();
//! assert_eq!(r.to_string(), "17/20");
//!
//! // And so is the arithmetic, as long as the exact result fits the budget.
//! let third = Q::new(1, 3).unwrap();
//! let sixth = Q::new(1, 6).unwrap();
//! assert_eq!(third.add(sixth), Q::new(1, 2).unwrap());
//! ```
//!
//! # What is verified
//!
//! The crate is written in [Verus](https://github.com/verus-lang/verus). The
//! same sources serve two toolchains: plain `cargo build` compiles them with
//! rustc (the `verus!` macro erases all ghost code), and `verus` re-reads them
//! with the ghost code kept and discharges the proof obligations. `README.md`
//! carries the obligation-by-obligation status table, and `TRUSTED.md`
//! enumerates the small unverified boundary.
//!
//! # Honest limitations
//!
//! With rounding in play, `add` and `mul` are **commutative but not associative
//! in general**. Associativity and distributivity hold exactly on the exact
//! path — any computation whose exact intermediates all fit the budget, which
//! covers small inputs entirely. Anything larger holds only up to the
//! accumulated error bound. See the `laws` module.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
// The verified modules import each other by glob for their ghost vocabulary; under
// plain rustc the ghost code is erased and some of those imports go unused.
#![allow(unused_imports)]
// Likewise, bindings whose only consumers are `proof` blocks read as unused once
// rustc sees the erased code. Both allows exist purely because one source tree
// serves two compilers.
#![allow(unused_variables)]
// `add`/`sub`/`mul`/`div`/`neg` are inherent methods on purpose. The `std::ops`
// traits are deliberately not implemented: `div` has a non-zero precondition
// that an operator cannot carry, and `a + b * c` written with operators hides
// exactly the rounding steps a caller of this crate should be looking at.
#![allow(clippy::should_implement_trait)]
// `(-BUDGET..=BUDGET).contains(&i)` is outside the executable subset Verus
// accepts, and this source is read by both compilers.
#![allow(clippy::manual_range_contains)]

use vstd::prelude::*;

verus! {

pub mod arith;

pub mod cmp;

pub mod convert;

pub mod gcd;

pub mod laws;

pub mod nary;

pub mod q;

pub mod round;

// Outside the verified region. See TRUSTED.md.
#[verifier::external]
pub mod float;

#[verifier::external]
pub mod traits;

} // verus!

pub use float::{from_f64, from_f64_dir, to_f64};
pub use q::{Q, BUDGET};
pub use round::Dir;
