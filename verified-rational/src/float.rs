//! The `f64` boundary. **This module is outside the verified region** — see
//! `TRUSTED.md`.
//!
//! It is kept as small as it can be. `from_f64_dir` does no float arithmetic
//! at all: it takes the IEEE-754 bits apart with integer operations and hands
//! the resulting `(mantissa, exponent)` pair to the fully verified
//! [`Q::from_dyadic`], because an `f64` *is* the rational `m · 2^e`. The only
//! genuinely unproved step is the claim that `f64::to_bits` lays out sign,
//! exponent and fraction where IEEE-754 says it does.
//!
//! `to_f64` is the one function that performs float arithmetic. It exists for
//! display and DTO boundaries only; its output must never be fed back into `Q`
//! arithmetic. Both functions are covered by differential tests rather than
//! proofs.

use crate::q::Q;
use crate::round::Dir;

/// `2^61`, the denominator of the smallest positive grid value the rounding
/// core can produce.
const TINY_DEN: i64 = 2305843009213693952;

/// Split an `f64` into `(m, e)` with `value == m * 2^e` exactly, or `None` for
/// NaN and the infinities.
///
/// Pure integer work: `to_bits` is a bit-cast, and everything after it is
/// shifting and masking.
fn decompose(v: f64) -> Option<(i64, i32)> {
    let bits = v.to_bits();
    let negative = (bits >> 63) == 1;
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let frac = (bits & 0x000f_ffff_ffff_ffff) as i64;
    if biased == 0x7ff {
        // NaN or ±inf: not a rational.
        return None;
    }
    // Subnormals (and zero) have no implicit leading bit and a fixed exponent.
    let (mag, exp) = if biased == 0 {
        (frac, -1074)
    } else {
        (frac + (1i64 << 52), biased - 1075)
    };
    Some((if negative { -mag } else { mag }, exp))
}

/// Convert an `f64` to a `Q` with the given rounding direction.
///
/// Returns `None` for NaN, ±inf, and any magnitude past `2^62 - 1` — values no
/// `Q` can carry. Reporting those rather than silently saturating is the point:
/// a conversion that promises a bounded error should not quietly return a
/// number that is off by an arbitrary amount. Otherwise the result satisfies
/// the same R2/R3 contract as any other rounded operation, against the exact
/// real value of `v`.
///
/// Magnitudes below `2^-124` are flushed: the result is `0` (or, in the
/// directed modes that must not cross the true value, `±1/2^61`). Since such a
/// `v` is within `2^-72` of zero, the R3 bound `2^-60` holds with enormous room
/// to spare, and directedness is preserved by construction.
pub fn from_f64_dir(v: f64, dir: Dir) -> Option<Q> {
    let (m, e) = decompose(v)?;
    if m == 0 {
        return Some(Q::zero());
    }
    // Stripping trailing zeros of the mantissa widens the usable exponent range
    // for free and never grows |m|.
    let tz = m.trailing_zeros() as i32;
    let m = m >> tz;
    let e = e + tz;

    if e >= 0 {
        // |v| = |m| * 2^e must fit the budget for the result to mean anything.
        if e > 62 {
            return None;
        }
        let magnitude = (m.unsigned_abs() as i128) << e;
        if magnitude > crate::q::BUDGET as i128 {
            return None;
        }
    }
    if e >= -124 {
        return Some(Q::from_dyadic(m, e, dir));
    }
    // |v| < 2^-72. Round to zero unless the direction forbids crossing it.
    let positive = m > 0;
    let r = match dir {
        Dir::Nearest => Q::zero(),
        Dir::Down => {
            if positive {
                Q::zero()
            } else {
                Q::new(-1, TINY_DEN).expect("1/2^61 is representable")
            }
        }
        Dir::Up => {
            if positive {
                Q::new(1, TINY_DEN).expect("1/2^61 is representable")
            } else {
                Q::zero()
            }
        }
    };
    Some(r)
}

/// Convert an `f64` with round-to-nearest.
pub fn from_f64(v: f64) -> Option<Q> {
    from_f64_dir(v, Dir::Nearest)
}

/// Approximate `q` as an `f64`.
///
/// **Display and DTO boundary only.** This is the one place in the crate where
/// a rounding error is neither directed nor bounded by a proof, and a value
/// that has been through it must never re-enter `Q` arithmetic — that is how
/// order-dependence gets back in.
pub fn to_f64(q: Q) -> f64 {
    q.numerator() as f64 / q.denominator() as f64
}
