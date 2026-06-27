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
    /// chain until reaching a non-variable or an unbound variable. Shallow at the
    /// top level — it does not descend into a compound type's *components* (those
    /// are walked by `unify`/`resolve` as needed).
    pub(crate) fn walk(&self, t: &Ty) -> Ty {
        let mut cur = t.clone();
        while let Ty::Var(v) = cur {
            match self.subst.get(&v) {
                Some(next) => cur = next.clone(),
                None => break,
            }
        }
        cur
    }

    /// Does variable `v` occur anywhere in `t` (under the current
    /// substitution)? The occurs-check that keeps [`bind`](Infer::bind) from
    /// constructing a cyclic (infinite) type such as `α = (array α)`.
    pub(crate) fn occurs(&self, v: u32, t: &Ty) -> bool {
        match self.walk(t) {
            Ty::Var(w) => v == w,
            // Compound types recurse into their components.
            Ty::Array(elem) | Ty::List(elem) => self.occurs(v, &elem),
            Ty::Pair(a, b) => self.occurs(v, &a) || self.occurs(v, &b),
            Ty::Fn(ps, r) => ps.iter().any(|p| self.occurs(v, p)) || self.occurs(v, &r),
            Ty::Struct(def) => def.fields.iter().any(|(_, ft)| self.occurs(v, ft)),
            // Scalars and nullary checkable types contain no variables.
            Ty::Int64 | Ty::Float64 | Ty::Bool | Ty::Char | Ty::Symbol | Ty::Str | Ty::Any => false,
        }
    }

    /// Bind variable `v` to `t`, rejecting a cyclic binding via the occurs-check.
    fn bind(&mut self, v: u32, t: Ty) -> Result<(), String> {
        if self.occurs(v, &t) {
            return Err(format!("occurs-check: type variable ?{v} occurs in itself"));
        }
        self.subst.insert(v, t);
        Ok(())
    }

    /// Unify two types, extending the substitution so that they become equal,
    /// or returning an error describing the clash. Variables bind (with an
    /// occurs-check); `Any` (the gradual top, #162) absorbs anything without
    /// binding; equal scalars succeed; arrays/lists/pairs/arrows unify
    /// structurally; structs unify by identity; everything else is a mismatch.
    pub(crate) fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), String> {
        let a = self.walk(a);
        let b = self.walk(b);
        match (a, b) {
            (Ty::Var(x), Ty::Var(y)) if x == y => Ok(()),
            // `Any` is absorbing — the operative/`eval` frontier makes no claim
            // and unifies with everything. Checked *before* variable binding so a
            // variable meeting `Any` is left free rather than pinned to `Any`.
            (Ty::Any, _) | (_, Ty::Any) => Ok(()),
            (Ty::Var(x), t) | (t, Ty::Var(x)) => self.bind(x, t),
            (Ty::Int64, Ty::Int64)
            | (Ty::Float64, Ty::Float64)
            | (Ty::Bool, Ty::Bool)
            | (Ty::Char, Ty::Char)
            | (Ty::Symbol, Ty::Symbol)
            | (Ty::Str, Ty::Str) => Ok(()),
            (Ty::Array(ea), Ty::Array(eb)) | (Ty::List(ea), Ty::List(eb)) => self.unify(&ea, &eb),
            (Ty::Pair(a1, a2), Ty::Pair(b1, b2)) => {
                self.unify(&a1, &b1)?;
                self.unify(&a2, &b2)
            }
            (Ty::Fn(pa, ra), Ty::Fn(pb, rb)) if pa.len() == pb.len() => {
                for (x, y) in pa.iter().zip(pb.iter()) {
                    self.unify(x, y)?;
                }
                self.unify(&ra, &rb)
            }
            (Ty::Struct(sa), Ty::Struct(sb)) if sa == sb => Ok(()),
            (x, y) => Err(format!(
                "cannot unify {} with {}",
                super::ty_name(&x),
                super::ty_name(&y)
            )),
        }
    }

    /// Resolve `t` to a concrete monomorphic **compileable** type under the final
    /// substitution — the codegen gate. Errors on an unresolved variable (no
    /// representation) *or* any checkable-but-not-compileable type (#162), so the
    /// native backend only ever sees the unboxable lattice.
    pub(crate) fn resolve(&self, t: &Ty) -> Result<Ty, String> {
        let w = self.walk(t);
        match &w {
            Ty::Var(v) => Err(format!(
                "cannot infer type (ambiguous: type variable ?{v} is unconstrained)"
            )),
            Ty::Int64 | Ty::Float64 | Ty::Bool | Ty::Char | Ty::Struct(_) => Ok(w),
            Ty::Array(elem) => Ok(Ty::Array(Box::new(self.resolve(elem)?))),
            other => Err(format!("type {} is not compileable", super::ty_name(other))),
        }
    }

    /// Resolve `t` to any concrete **checkable** type (the checker's resolve,
    /// #162), descending into every compound. Only an unresolved variable is an
    /// error (until generalization lands, Stage 2). Unlike [`resolve`], it
    /// accepts the non-compileable types (`List`/`Pair`/`Symbol`/`Str`/`Fn`/
    /// `Any`) — a value can be *well-typed* without having a *compileable* type.
    #[allow(dead_code)] // wired to the checking surface in a later stage (#162)
    pub(crate) fn resolve_checked(&self, t: &Ty) -> Result<Ty, String> {
        match self.walk(t) {
            Ty::Var(v) => Err(format!("cannot infer type (unresolved type variable ?{v})")),
            Ty::Array(e) => Ok(Ty::Array(Box::new(self.resolve_checked(&e)?))),
            Ty::List(e) => Ok(Ty::List(Box::new(self.resolve_checked(&e)?))),
            Ty::Pair(a, b) => Ok(Ty::Pair(
                Box::new(self.resolve_checked(&a)?),
                Box::new(self.resolve_checked(&b)?),
            )),
            Ty::Fn(ps, r) => {
                let mut rps = Vec::with_capacity(ps.len());
                for p in &ps {
                    rps.push(self.resolve_checked(p)?);
                }
                Ok(Ty::Fn(rps, Box::new(self.resolve_checked(&r)?)))
            }
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
        inf.unify(&a, &Ty::Int64).unwrap();
        assert_eq!(inf.resolve(&a).unwrap(), Ty::Int64);
    }

    #[test]
    fn unify_propagates_through_a_chain() {
        // a = b, b = c, c = float64  ⇒  all three resolve to float64.
        let mut inf = Infer::new();
        let (a, b, c) = (inf.fresh(), inf.fresh(), inf.fresh());
        inf.unify(&a, &b).unwrap();
        inf.unify(&b, &c).unwrap();
        inf.unify(&c, &Ty::Float64).unwrap();
        assert_eq!(inf.resolve(&a).unwrap(), Ty::Float64);
        assert_eq!(inf.resolve(&b).unwrap(), Ty::Float64);
        assert_eq!(inf.resolve(&c).unwrap(), Ty::Float64);
    }

    #[test]
    fn unify_same_variable_is_noop() {
        let mut inf = Infer::new();
        let a = inf.fresh();
        inf.unify(&a, &a).unwrap();
        assert!(inf.resolve(&a).is_err(), "still unconstrained");
    }

    #[test]
    fn conflicting_unification_is_rejected() {
        let mut inf = Infer::new();
        let a = inf.fresh();
        inf.unify(&a, &Ty::Int64).unwrap();
        let err = inf.unify(&a, &Ty::Float64).unwrap_err();
        assert!(err.contains("cannot unify"), "got: {err}");
    }

    #[test]
    fn scalar_mismatch_is_rejected() {
        let mut inf = Infer::new();
        let err = inf.unify(&Ty::Bool, &Ty::Char).unwrap_err();
        assert!(err.contains("cannot unify"), "got: {err}");
    }

    #[test]
    fn arrays_unify_element_wise() {
        // (array α) ~ (array int64)  ⇒  α = int64.
        let mut inf = Infer::new();
        let a = inf.fresh();
        let arr_a = Ty::Array(Box::new(a.clone()));
        inf.unify(&arr_a, &Ty::Array(Box::new(Ty::Int64))).unwrap();
        assert_eq!(inf.resolve(&a).unwrap(), Ty::Int64);
        assert_eq!(inf.resolve(&arr_a).unwrap(), Ty::Array(Box::new(Ty::Int64)));
    }

    #[test]
    fn array_element_conflict_is_rejected() {
        let mut inf = Infer::new();
        let err = inf
            .unify(
                &Ty::Array(Box::new(Ty::Int64)),
                &Ty::Array(Box::new(Ty::Float64)),
            )
            .unwrap_err();
        assert!(err.contains("cannot unify"), "got: {err}");
    }

    #[test]
    fn occurs_check_rejects_cyclic_array_type() {
        // α ~ (array α) must be rejected (infinite type).
        let mut inf = Infer::new();
        let a = inf.fresh();
        let arr_a = Ty::Array(Box::new(a.clone()));
        let err = inf.unify(&a, &arr_a).unwrap_err();
        assert!(err.contains("occurs-check"), "got: {err}");
    }

    #[test]
    fn occurs_check_is_local() {
        // A scalar never contains a variable.
        let mut inf = Infer::new();
        let a = inf.fresh();
        assert!(!inf.occurs(0, &Ty::Int64));
        // A variable occurs in itself.
        assert!(matches!(a, Ty::Var(0)));
        assert!(inf.occurs(0, &a));
    }

    #[test]
    fn resolve_unconstrained_variable_errors() {
        let inf = {
            let mut inf = Infer::new();
            let _ = inf.fresh();
            inf
        };
        assert!(inf.resolve(&Ty::Var(0)).is_err());
    }

    #[test]
    fn resolve_passes_concrete_through() {
        let inf = Infer::new();
        for t in [Ty::Int64, Ty::Float64, Ty::Bool, Ty::Char] {
            assert_eq!(inf.resolve(&t).unwrap(), t);
        }
    }

    // --- checker type language (#162) --------------------------------------

    #[test]
    fn any_is_absorbing_and_does_not_bind() {
        let mut inf = Infer::new();
        // Any ~ scalar succeeds without constraining anything.
        inf.unify(&Ty::Any, &Ty::Int64).unwrap();
        inf.unify(&Ty::Str, &Ty::Any).unwrap();
        // A variable meeting Any stays free (not pinned to Any), so it can still
        // be constrained to a real type afterwards.
        let a = inf.fresh();
        inf.unify(&a, &Ty::Any).unwrap();
        inf.unify(&a, &Ty::Bool).unwrap();
        assert_eq!(inf.resolve(&a).unwrap(), Ty::Bool);
    }

    #[test]
    fn lists_and_pairs_unify_structurally() {
        let mut inf = Infer::new();
        let a = inf.fresh();
        inf.unify(
            &Ty::List(Box::new(a.clone())),
            &Ty::List(Box::new(Ty::Int64)),
        )
        .unwrap();
        assert_eq!(inf.resolve_checked(&a).unwrap(), Ty::Int64);

        let mut inf = Infer::new();
        let (x, y) = (inf.fresh(), inf.fresh());
        inf.unify(
            &Ty::Pair(Box::new(x.clone()), Box::new(y.clone())),
            &Ty::Pair(Box::new(Ty::Symbol), Box::new(Ty::Str)),
        )
        .unwrap();
        assert_eq!(inf.resolve_checked(&x).unwrap(), Ty::Symbol);
        assert_eq!(inf.resolve_checked(&y).unwrap(), Ty::Str);
    }

    #[test]
    fn arrow_types_unify_by_arity_and_components() {
        let mut inf = Infer::new();
        let r = inf.fresh();
        let lhs = Ty::Fn(vec![Ty::Int64], Box::new(r.clone()));
        let rhs = Ty::Fn(vec![Ty::Int64], Box::new(Ty::Bool));
        inf.unify(&lhs, &rhs).unwrap();
        assert_eq!(inf.resolve_checked(&r).unwrap(), Ty::Bool);
        // Arity mismatch is a clash.
        let mut inf = Infer::new();
        assert!(
            inf.unify(
                &Ty::Fn(vec![Ty::Int64], Box::new(Ty::Bool)),
                &Ty::Fn(vec![Ty::Int64, Ty::Int64], Box::new(Ty::Bool)),
            )
            .is_err()
        );
    }

    #[test]
    fn occurs_check_rejects_cyclic_list_and_pair() {
        let mut inf = Infer::new();
        let a = inf.fresh();
        assert!(
            inf.unify(&a, &Ty::List(Box::new(a.clone())))
                .unwrap_err()
                .contains("occurs-check")
        );
        let mut inf = Infer::new();
        let b = inf.fresh();
        assert!(
            inf.unify(&b, &Ty::Pair(Box::new(Ty::Int64), Box::new(b.clone())))
                .is_err()
        );
    }

    #[test]
    fn compileable_resolve_rejects_checkable_only_types() {
        let inf = Infer::new();
        // resolve (codegen gate) rejects non-compileable types...
        for t in [
            Ty::Symbol,
            Ty::Str,
            Ty::List(Box::new(Ty::Int64)),
            Ty::Pair(Box::new(Ty::Int64), Box::new(Ty::Int64)),
            Ty::Any,
        ] {
            assert!(inf.resolve(&t).is_err(), "resolve should reject {t:?}");
            // ...but resolve_checked accepts them.
            assert!(
                inf.resolve_checked(&t).is_ok(),
                "resolve_checked should accept {t:?}"
            );
        }
    }

    #[test]
    fn is_compileable_partitions_the_lattice() {
        use super::super::{StructDef, is_compileable};
        use std::rc::Rc;
        assert!(is_compileable(&Ty::Int64));
        assert!(is_compileable(&Ty::Array(Box::new(Ty::Char))));
        assert!(is_compileable(&Ty::Struct(Rc::new(StructDef {
            name: "P".into(),
            fields: vec![("X".into(), Ty::Int64)],
        }))));
        assert!(!is_compileable(&Ty::List(Box::new(Ty::Int64))));
        assert!(!is_compileable(&Ty::Symbol));
        assert!(!is_compileable(&Ty::Any));
        assert!(!is_compileable(&Ty::Var(0)));
        // An array of a non-compileable element is not compileable.
        assert!(!is_compileable(&Ty::Array(Box::new(Ty::Symbol))));
    }
}
