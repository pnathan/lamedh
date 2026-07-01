# Position Paper: Beyond vau — First-Class Intent (verbatim source)

This is the position paper under review, carried verbatim so the response
(`response-first-class-intent.md`) can be checked against its source. It was
supplied by the project owner with the caveat "it's probably not entirely
right." It is a *different* document from `docs/condensation.md` (the
decoder/repair-cost model on `feat/typeclasses` and
`eval/condensation-analysis`), and the two disagree in ways the response makes
precise.

---

## Beyond vau: First-Class Intent and the Minimal Encoding of Change

**Abstract**

The evolution of metaprogramming in Lisp has historically been a climb toward
total control over evaluation. From the abstraction of values (lambda), to
syntax (defmacro), to the ultimate capture of dynamic runtime environments and
unevaluated operands ($vau), language designers have sought maximal
expressiveness. However, $vau represents a local maximum. It provides the raw
materials of the universe but still forces the programmer into an imperative,
mechanical role. To achieve the next level of programming power—optimizing for
pure intent and minimizing the Kolmogorov distance between human meaning and
machine execution—we must shift from capturing *evaluation contexts* to
natively capturing *transformations*. This paper outlines the theoretical leap
from $vau to "First-Class Intent," expressed as concise trees of change within
a multi-tiered execution architecture.

### 1. The Ceiling of Current Abstractions

In the pursuit of raw programming power devoid of the "wankery of adequacy,"
standard formal logic and static type systems (e.g., strict Hindley-Milner or
Lean) often degrade into golfing the compiler. The programmer is forced to
encode incidental complexity rather than actual meaning.

Within a hybrid architecture containing isolated execution zones (Typed
Compilable, Typed Interpreted, and the dynamic Any zone), the $vau calculus
rightly belongs in Any. It is the absolute definition of dynamic behavior.
Yet, relying on $vau as the ultimate tool for abstraction reveals a
fundamental limitation: it operates by manipulating absolute states. When a
programmer's intent is simply to transform state p into state p' via a
transformation t, $vau requires the manual decomposition, traversal, and
reconstruction of the entire Abstract Syntax Tree (AST) within the captured
environment.

This approach inflates the conditional Kolmogorov complexity, K_M(S | I),
where S is the source code, I is the intent, and M is the language semantics.
The encoding overhead remains too high.

### 2. The Paradigm Shift: p —t→ p' as a Primitive

To reach new levels of expressiveness, the language runtime must absorb the
mechanical burden of transformation. The fundamental primitive can no longer
be "evaluate this syntax in this environment." It must become the **Concise
Tree of Change**.

By defining t directly, we remove the boilerplate of orchestrating the
application. The architecture must evolve to allow intent to be mapped as a
structural diff, bridging the chaotic, high-power Any zone and the highly
optimized Typed zones without destroying semantic proofs.

### 3. Theoretical Frontiers for the Next Level

To implement First-Class Intent atop a Lisp engine, the runtime must integrate
one or more of the following advanced theoretical models:

* **The Bidirectional Lens Calculus (Symmetric Intent):** Transformations are
  inherently relational. Rather than writing one-way $vau operatives to map p
  to p', the language elevates Optics (Lenses and Prisms) to first-class AST
  primitives. A single geometric declaration mathematically defines the
  relationship between two shapes of data. The runtime automatically derives
  the getter, setter, and structural update mechanisms. Intent is encoded
  symmetrically.

* **Incremental Lambda Calculus (First-Class Deltas):** Standard compilers
  only understand absolute states. In an incremental paradigm, the concept of
  a derivative—the Δ type—is native to the language. Instead of redefining p'
  in its entirety, the programmer encodes only Δp. When an input changes, the
  runtime pushes the mathematical delta through the transformation pathway in
  O(1) time relative to the change size. The diff itself becomes the program.

* **Constraint-Handling Rules (Topological Rewriting):** Standard macros and
  operatives are passive; they wait to be explicitly invoked by the evaluator.
  The next tier of power makes intents *active*. By defining the structural
  boundaries of p and the logical constraints of p', the runtime acts as a
  continuous background solver. It exhaustively rewrites the graph based on
  topological rules without requiring explicit control flow.

### 4. Directing the Runtime Engine

To modernize the 3-zone execution engine and support these new models, the
boundary between the Any zone (where dynamic transformations occur) and the
Typed zones (where performance is realized) must be fortified mathematically.

* **Row-Polymorphic Environments:** The environments captured by $vau must be
  understood by the HM type-checker as dynamic, row-polymorphic records,
  allowing typed structures to pass through the Any zone without losing their
  proofs.

* **Contextual Modal Types:** Introducing typed "holes" into the quotation
  system allows $vau operatives to act as macroscopic architects. They can
  arrange, compose, and restructure compiled nodes as opaque blocks, ensuring
  that the resulting tree of change executes at native speed upon re-entering
  the typed zones.

* **Partial Evaluation / Futamura Projections:** The system must actively
  observe the Any zone. When the dynamic environment of a $vau operative
  stabilizes, a partial evaluator must collapse the untyped operative down
  into a static, compilable Lambda, automatically migrating the code into the
  fast path.

### 5. Conclusion

Syntax is solved. Extending traditional evaluation semantics yields
diminishing returns. To arm the programmer with unprecedented power, language
design must shift its focus to the encoding of change itself. By integrating
bidirectional optics, incremental delta calculus, and rigorous type boundaries
between execution zones, a $vau-based Lisp can transcend the role of a mere
metaprogramming sandbox and become an engine for the pure expression of human
intent.
