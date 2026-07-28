//! Standard-library trait impls. Outside the verified region — Verus does not
//! model `Ord`, `Display` or `serde`, and none of these do arithmetic of their
//! own: each one delegates to a verified function or to the two field
//! accessors.
//!
//! `Ord` is written out by hand rather than derived. Deriving it would give the
//! lexicographic order on `(num, den)`, which reports `1/3 > 1/2` — the derived
//! impl is not merely suboptimal, it is wrong. `PartialEq`/`Eq`/`Hash` *are*
//! derived, and that is correct precisely because canonical form makes
//! structural equality coincide with mathematical equality.

use crate::q::Q;
use core::cmp::Ordering;
use core::fmt;

impl PartialOrd for Q {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Q {
    /// Exact comparison by cross-multiplication in `i128`. Total: `Q` has no
    /// NaN, so unlike `f64` this really is an `Ord` and not just a
    /// `PartialOrd`.
    fn cmp(&self, other: &Self) -> Ordering {
        let lhs = (self.numerator() as i128) * (other.denominator() as i128);
        let rhs = (other.numerator() as i128) * (self.denominator() as i128);
        lhs.cmp(&rhs)
    }
}

impl fmt::Display for Q {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator(), self.denominator())
    }
}

impl Default for Q {
    fn default() -> Self {
        Q::zero()
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::Q;
    use serde::de::{Deserialize, Deserializer, Error as DeError};
    use serde::ser::{Serialize, Serializer};

    /// Serialized as the `(num, den)` integer pair. Exact round-trip — unlike
    /// any `f64` encoding, nothing is lost and nothing is reinterpreted.
    impl Serialize for Q {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            (self.numerator(), self.denominator()).serialize(s)
        }
    }

    impl<'de> Deserialize<'de> for Q {
        /// Rebuilds through `Q::new`, so a hand-edited or non-canonical pair is
        /// canonicalized, and one that cannot be represented is rejected rather
        /// than smuggled in past the type invariant.
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Q, D::Error> {
            let (num, den) = <(i64, i64)>::deserialize(d)?;
            Q::new(num, den).ok_or_else(|| {
                D::Error::custom(format!(
                    "not a representable rational: {num}/{den} \
                     (denominator zero, or reduced form outside the 2^62-1 budget)"
                ))
            })
        }
    }
}
