# Condensation Next Steps

The condensation epic now has its first substrate: `defconcept`, `derive`,
laws/examples, checker traces, and the standalone typeclass dictionary layer.
The next phase should make condensation less like macro-generated code with
metadata and more like a semantic compression system: ordinary code should carry
enough type, law, instance, and derivation information that humans and LLMs can
write the short form and still inspect the expanded proof trail.

## 1. Land The Typeclass Dictionary Layer

The standalone typeclass dictionary layer should land before the next
condensation slice. It gives the language a general, non-condensation-specific
vocabulary:

```lisp
(deftypeclass eqv (a)
  (:ops ((eqv (-> (a a) bool)))))

(definstance eqv invoice
  (:eqv invoice-equal))
```

This is the base layer, not the final experience. It says that there is a
dictionary for a semantic meaning, but it does not yet make ordinary calls infer
constraints.

Naming discipline matters:

- Use `typeclass` for semantic dictionaries.
- Keep host/security rights named `capability`, such as `SHELL` and `READ-FS`.
- Keep `defcap` and `cap-op` as compatibility aliases only.

## 2. Add Generic Functions As The User Surface

The next language move should be CLOS-flavored but HM-disciplined:

```lisp
(defgeneric eqv (a b)
  (:class eqv)
  (:type (-> (a a) bool)))

(defmethod eqv ((a int64) (b int64))
  (= a b))

(defmethod eqv ((a invoice) (b invoice))
  (invoice-equal a b))
```

Then ordinary use becomes the compressed surface:

```lisp
(defun same (x y)
  (eqv x y))
```

The programmer should not have to write `typeclass-call` in ordinary code. The
call to `eqv` is ordinary source, but because `eqv` is a known generic operation,
the checker treats it as constrained.

## 3. Extend Schemes With Constraints

The checker should represent constrained polymorphism explicitly:

```rust
struct Constraint {
    class: String,
    ty: Ty,
}

struct Scheme {
    vars: Vec<u32>,
    constraints: Vec<Constraint>,
    ty: Ty,
}
```

Then this:

```lisp
(defun same (x y)
  (eqv x y))
```

can print as:

```lisp
(forall (a) (=> ((EQV a)) (-> (a a) bool)))
```

Do not put `EQV` inside `Ty`. A typeclass is not a type. It is a predicate over
types.

## 4. Infer Constraints From Ordinary Calls

When the checker sees:

```lisp
(eqv x y)
```

and `eqv` is registered as a generic operation, inference should:

- instantiate the generic operation scheme,
- unify `x` and `y` against the operation argument types,
- emit a wanted constraint such as `(EQV a)`,
- return the operation result type.

The current HM unifier should remain mostly unchanged. Constraints are a side
channel accumulated during inference.

## 5. Separate Well-Typed From JIT-Eligible

This is the central rule:

```text
Constrained polymorphic code may be well-typed.
Only ground, concrete, compileable constraints may become native.
```

So:

```lisp
(defun same (x y)
  (eqv x y))
```

is well-typed but not JIT-native.

But:

```lisp
(defun-typed (same-int bool) ((x int64) (y int64))
  (eqv x y))
```

can become native if `EQV int64` resolves to a typed method.

## 6. Lower Ground Generic Calls To Direct Methods

Native code must not receive dictionaries.

If inference resolves:

```text
(EQV int64)
```

then the JIT path should lower:

```lisp
(eqv x y)
```

to a direct method call:

```lisp
(eqv-int64 x y)
```

or the equivalent internal `Core::Call(method_id, args)`.

If the constraint remains polymorphic, keep it dynamic. If the concrete instance
is missing, reject it as a type error.

## 7. Use Dynamic Dictionaries Only Outside Native Code

The explicit dictionary layer remains useful:

```lisp
(resolve-instance 'eqv 'invoice)
(typeclass-op 'eqv 'invoice :eqv)
(typeclass-call 'eqv 'invoice :eqv a b)
```

But that is the dynamic/runtime path. The native path should specialize away the
dictionary entirely.

The clean split:

```text
polymorphic constrained function -> checked, dynamic
ground concrete instance         -> direct method lowering, maybe native
missing ground instance          -> type error
dynamic/unknown frontier         -> checked if possible, not native
```

## 8. Teach Condensation To Use Typeclasses

Once generic calls work, condensation can become much more powerful.

Today:

```lisp
(derive invoice equality)
```

generates an `invoice-equal` function.

Next, it should also be able to install:

```lisp
(definstance eqv invoice
  (:eqv invoice-equal))
```

Likewise:

```lisp
(derive invoice printer)
```

can attach a `show`, `render`, or `to-plist` instance.

This turns `derive` into a bridge from concept metadata to generic behavior.

## 9. Make Concepts Typed When Possible

`defconcept` currently lowers to tagged lists. That is good for v0, but not ideal
for JIT.

Next:

```lisp
(defconcept invoice
  (:fields ((id int64)
            (amount int64)
            (status symbol)))
  (:invariant (>= amount 0)))
```

should eventually choose one of two lowerings:

- dynamic tagged-list concept, current behavior;
- typed struct concept, when fields are concrete and supported by the typed
  island.

Then typeclass instances over typed concepts can become native.

## 10. Add Condensed Pragmatic Bundles

The deeper condensation goal is not just shorter syntax. It is preserving
pragmatic intent.

We want bundles like:

```lisp
(defconcept invoice
  (:fields ...)
  (:derive eqv show validate)
  (:laws ...)
  (:examples ...))
```

That should generate:

- constructor, predicate, and accessors,
- validator,
- typeclass instances,
- examples,
- laws,
- checker trace,
- dynamic frontier trace,
- derivation metadata.

The short form should be dense, but the expansion must remain inspectable.

## 11. Improve Trace Output

`condense-trace` should eventually show the whole semantic chain:

```lisp
(condense-trace 'invoice)
```

It should answer:

- What source form created this?
- What code expanded from it?
- What functions did it generate?
- What typeclass instances did it install?
- Which generic methods came from which derivation?
- Which generated functions type-check?
- Which are native?
- Which remain dynamic?
- Which laws/examples cover it?

That trace is the repair handle for humans and LLMs.

## 12. Testing Plan

The next test sequence should be:

- `deftypeclass` registers generic operation metadata.
- `defmethod` installs runtime and checker-visible method metadata.
- `(eqv 1 1)` works as an ordinary call.
- `(check-type (defun same (x y) (eqv x y)))` prints a constrained scheme.
- typed `int64` use resolves to the concrete method.
- missing concrete instance is a type error.
- polymorphic constrained function is checked but not native.
- `derive invoice equality` installs an `EQV invoice` instance.
- `condense-trace` shows the derived instance and checker result.

## Destination

The target shape is:

```lisp
(defconcept invoice
  (:fields ((id int64) (amount int64) (status symbol)))
  (:invariant (>= amount 0))
  (:derive eqv show validate)
  (:example draft-valid
    (:given (make-invoice 1 100 'draft))
    (:expect (validate-invoice *it*))))
```

From that one form, Lamedh should know how to construct, validate, compare,
print, inspect, test, type-check, and partially compile the concept.

That is the condensation promise: a small source seed, but not a magical one. A
seed with an audit trail.
