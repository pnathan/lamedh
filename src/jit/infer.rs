//! HM-lite type inference substrate for the typed JIT (issue #135).
//!
//! The typed-core elaborator (`super::Cx`) is a bidirectional checker, but the
//! *types it cannot see in the surface syntax* — most importantly an array's
//! element type (#137/#138) — have to be **inferred**. This module is that
//! foundation: type variables, a substitution, `unify` with an occurs-check,
//! and a `resolve` pass that monomorphizes a type before codegen.
//!
//! It is deliberately tiny and self-contained so it can be hardened and
//! adversarially unit-tested in isolation (per #135): every downstream type
//! (`char`, `string = (array char)`, numeric arrays) monomorphizes on top of
//! it, so a bug here would masquerade as a codegen bug everywhere else.
//!
//! ## Model
//!
//! - [`Infer`] holds a monotonic fresh-variable counter and a substitution
//!   (`var-id → Ty`). A type variable is a [`Ty::Var`] carrying that id.
//! - [`Infer::unify`] makes two types equal by binding variables, walking under
//!   the current substitution first, and **occurs-checking** every bind so it
//!   can never build an infinite type.
//! - [`Infer::resolve`] reads a type back under the final substitution and
//!   requires it to be concrete; a still-free variable is a def-time error
//!   ("cannot infer …"), because codegen needs a concrete representation.

use super::Ty;
use std::collections::HashMap;

/// The inference state threaded through one elaboration: a fresh-variable
/// counter (deterministic — no `gensym` randomness) and the substitution built
/// up by [`unify`](Infer::unify).
#[derive(Debug, Default)]
pub(crate) struct Infer {
    next: u32,
    subst: HashMap<u32, Ty>,
}

impl Infer {
    pub(crate) fn new() -> Infer {
        Infer::default()
    }

    /// A fresh, currently-unbound type variable.
    pub(crate) fn fresh(&mut self) -> Ty {
        let v = self.next;
        self.next += 1;
        Ty::Var(v)
    }

    /// Follow the substitution to `t`'s representative: chase a bound variable's
    /// chain until reaching a non-variable or an unbound variable. Shallow — it
    /// does not descend into compound types (their *components* are walked by
    /// `unify`/`occurs`/`resolve` as needed).
    pub(crate) fn walk(&self, mut t: Ty) -> Ty {
        while let Ty::Var(v) = t {
            match self.subst.get(&v) {
                Some(&next) => t = next,
                None => break,
            }
        }
        t
    }

    /// Does variable `v` occur anywhere in `t` (under the current
    /// substitution)? The occurs-check that keeps [`bind`](Infer::bind) from
    /// constructing a cyclic (infinite) type such as `α = α → α`.
    pub(crate) fn occurs(&self, v: u32, t: Ty) -> bool {
        match self.walk(t) {
            Ty::Var(w) => v == w,
            // Scalars contain no variables. Compound types (e.g. a future
            // `Ty::Array(elem)`) recurse into their components here.
            Ty::Int64 | Ty::Float64 | Ty::Bool | Ty::Char => false,
        }
    }

    /// Bind variable `v` to `t`, rejecting a cyclic binding via the occurs-check.
    fn bind(&mut self, v: u32, t: Ty) -> Result<(), String> {
        if self.occurs(v, t) {
            return Err(format!("occurs-check: type variable ?{v} occurs in itself"));
        }
        self.subst.insert(v, t);
        Ok(())
    }

    /// Unify two types, extending the substitution so that they become equal,
    /// or returning an error describing the clash. Variables bind (with an
    /// occurs-check); equal scalars succeed; everything else is a mismatch.
    pub(crate) fn unify(&mut self, a: Ty, b: Ty) -> Result<(), String> {
        let a = self.walk(a);
        let b = self.walk(b);
        match (a, b) {
            (Ty::Var(x), Ty::Var(y)) if x == y => Ok(()),
            (Ty::Var(x), t) | (t, Ty::Var(x)) => self.bind(x, t),
            (Ty::Int64, Ty::Int64)
            | (Ty::Float64, Ty::Float64)
            | (Ty::Bool, Ty::Bool)
            | (Ty::Char, Ty::Char) => Ok(()),
            (x, y) => Err(format!(
                "cannot unify {} with {}",
                super::ty_name(x),
                super::ty_name(y)
            )),
        }
    }

    /// Resolve `t` to a concrete monomorphic type under the final substitution.
    /// A type variable that is still unbound is an error: codegen needs a
    /// concrete representation, so an un-pinned type is ambiguous and rejected
    /// at definition time.
    pub(crate) fn resolve(&self, t: Ty) -> Result<Ty, String> {
        match self.walk(t) {
            Ty::Var(v) => Err(format!(
                "cannot infer type (ambiguous: type variable ?{v} is unconstrained)"
            )),
            concrete => Ok(concrete),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_variables_are_distinct() {
        let mut inf = Infer::new();
        let a = inf.fresh();
        let b = inf.fresh();
        assert_ne!(a, b);
        assert!(matches!(a, Ty::Var(0)));
        assert!(matches!(b, Ty::Var(1)));
    }

    #[test]
    fn unify_binds_variable_to_scalar() {
        let mut inf = Infer::new();
        let a = inf.fresh();
        inf.unify(a, Ty::Int64).unwrap();
        assert_eq!(inf.resolve(a).unwrap(), Ty::Int64);
    }

    #[test]
    fn unify_propagates_through_a_chain() {
        // a = b, b = c, c = float64  ⇒  all three resolve to float64.
        let mut inf = Infer::new();
        let (a, b, c) = (inf.fresh(), inf.fresh(), inf.fresh());
        inf.unify(a, b).unwrap();
        inf.unify(b, c).unwrap();
        inf.unify(c, Ty::Float64).unwrap();
        assert_eq!(inf.resolve(a).unwrap(), Ty::Float64);
        assert_eq!(inf.resolve(b).unwrap(), Ty::Float64);
        assert_eq!(inf.resolve(c).unwrap(), Ty::Float64);
    }

    #[test]
    fn unify_same_variable_is_noop() {
        let mut inf = Infer::new();
        let a = inf.fresh();
        inf.unify(a, a).unwrap();
        assert!(inf.resolve(a).is_err(), "still unconstrained");
    }

    #[test]
    fn conflicting_unification_is_rejected() {
        let mut inf = Infer::new();
        let a = inf.fresh();
        inf.unify(a, Ty::Int64).unwrap();
        let err = inf.unify(a, Ty::Float64).unwrap_err();
        assert!(err.contains("cannot unify"), "got: {err}");
    }

    #[test]
    fn scalar_mismatch_is_rejected() {
        let mut inf = Infer::new();
        let err = inf.unify(Ty::Bool, Ty::Char).unwrap_err();
        assert!(err.contains("cannot unify"), "got: {err}");
    }

    #[test]
    fn occurs_check_is_local() {
        // A scalar never contains a variable.
        let mut inf = Infer::new();
        let a = inf.fresh();
        assert!(!inf.occurs(
            match a {
                Ty::Var(v) => v,
                _ => unreachable!(),
            },
            Ty::Int64
        ));
        // A variable occurs in itself.
        inf.unify(a, a).unwrap();
        assert!(inf.occurs(0, a));
    }

    #[test]
    fn resolve_unconstrained_variable_errors() {
        let inf = {
            let mut inf = Infer::new();
            let _ = inf.fresh();
            inf
        };
        assert!(inf.resolve(Ty::Var(0)).is_err());
    }

    #[test]
    fn resolve_passes_concrete_through() {
        let inf = Infer::new();
        for t in [Ty::Int64, Ty::Float64, Ty::Bool, Ty::Char] {
            assert_eq!(inf.resolve(t).unwrap(), t);
        }
    }
}
