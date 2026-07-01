# Typeclasses

Lamedh typeclasses are explicit dictionaries for saying that a type-like symbol
supports a named set of operations. The v0 system is a Lisp library, not a type
checker extension and not a sandbox capability.

This separation is deliberate:

- `SHELL`, `READ-FS`, and similar host capabilities are security permissions.
- Typeclasses are ordinary semantic dictionaries.
- Condensation can use typeclasses, but typeclasses do not depend on
  condensation metadata or `defconcept`.

## Surface

```lisp
(deftypeclass eqv (a)
  (:ops ((eqv (-> (a a) bool)))))

(defun invoice-equal (a b)
  (equal a b))

(definstance eqv invoice
  (:eqv invoice-equal))
```

Operation names in instances may be keywords or symbols. `:eqv` and `eqv`
normalize to the same operation key.

The issue-compatible aliases are also provided:

```lisp
(defcap eqv (a)
  (:ops ((eqv (-> (a a) bool)))))

(resolve-cap 'eqv 'invoice)
(cap-op 'eqv 'invoice :eqv)
```

Prefer the `typeclass` names in new code.

## Resolution

Resolution is explicit and shallow:

```lisp
(resolve-instance 'eqv 'invoice)
; => (INVOICE (EQV . INVOICE-EQUAL))

(typeclass-op 'eqv 'invoice :eqv)
; => INVOICE-EQUAL

(typeclass-call 'eqv 'invoice :eqv a b)
```

There is no global implicit search. Missing classes, missing instances, missing
operations, and unknown operations are errors.

Re-declaring an instance for the same class/type replaces the previous instance
deterministically.

## Metadata

Typeclass declarations and instances are stored on the typeclass symbol's
property list:

```lisp
(typeclass-trace 'eqv)
(instance-trace 'eqv 'invoice)
```

The trace includes the original declaration, type parameters, operation specs,
and registered instances.

## Checker Direction

The current HM checker already supports ordinary let-polymorphic schemes such as
`(forall (a) (-> (a) a))`. It does not yet support constrained schemes such as
`(forall ((a eqv)) (-> (a a) bool))`.

The next checker step should elaborate constrained calls into explicit
dictionary parameters or resolved direct calls. That keeps the model closer to
Haskell dictionary passing and away from Scala-style unbounded implicit search.

Out of scope for v0:

- implicit argument insertion,
- overlapping instances,
- instance search through imports or scopes,
- default methods,
- multi-parameter typeclasses,
- associated types,
- checker-enforced constrained polymorphism.
