# Change Is Data, Not a Calculus — a Response to "Beyond vau: First-Class Intent"

A position developed against three bodies of evidence: the position paper
(carried verbatim in `position-first-class-intent.md`), the condensation
design work (`docs/condensation.md` and `docs/condensation_next_steps.md` on
`feat/typeclasses`; the four-system evaluation on `eval/condensation-analysis`),
and the code that actually shipped to `main` (`lib/20-condensation.lisp`,
`docs/condensation_library.md`, the tier architecture in
`docs/typed-region-design.md`, and the merge record itself).

**Thesis in one line:** the paper is right that the next primitive is
*change*, but it locates change in the wrong plane (runtime evaluation
semantics instead of the source/expansion plane) and optimizes the wrong cost
term (one-shot generation instead of lifetime repair). In Lamedh, first-class
intent should mean *change as inspectable, re-verifiable data* — a diff with
laws attached — not a new evaluation calculus. Most of what the paper asks for
in its runtime section is, unknowingly, the tier ladder Lamedh already built.

---

## 1. What the merge record already decided

Before arguing theory, read the repository as evidence. Since the four-system
eval ran:

- The condensation metadata substrate (#181), self-evaluating keywords (#180),
  and — decisively — the variadic `AND`/`OR` checker soundness fix (#202, the
  bug two independent vendors reproduced) all **merged to `main`**. The weak
  test assertion the eval flagged was hardened into a regression test that
  names #202 explicitly (`tests/test_condensation_typecheck.rs`).
- The typeclass dictionary layer (`lib/19-typeclasses.lisp`) did **not**
  merge; `main`'s `lib/19-` slot went to a call-graph library instead. The
  constrained-HM/generic-function/JIT-lowering arc of
  `condensation_next_steps.md` steps 1–7 was not built.

That is the project ruling in favor of the eval consensus with its merge
button: substrate and soundness in, speculative type machinery out. Any new
proposal — including this paper — should be read against that revealed
preference and against `docs/roadmap_1_0.md` ("the goal of this ramp is not
more surface area").

Still open from the 4/4-consensus list, and still visible in the code:

- `condense-check-type-status` (`lib/20-condensation.lisp:64`) still
  substring-matches `"type error"` / `"any"` out of `check-type`'s
  human-readable string. Structured checker output remains unbuilt.
- The repair/fault-injection benchmark — the falsifiability lever every one of
  the four systems ranked first — remains unbuilt and unscheduled.

Those two facts anchor everything below.

## 2. The paper's central error: a C_gen argument in an r·C_rep world

The paper states its objective openly: "optimizing for pure intent and
minimizing the Kolmogorov distance between human meaning and machine
execution," formalized as inflated K_M(S | I) being the disease. That is
`argmin |s|` — the generation-cost objective.

`docs/condensation.md` exists to reject exactly that objective, and its
argument survives all four independent reviews: generation is paid once,
repair recurs over the maintenance horizon, so the lifetime cost
`J = C_gen + C_ver + r·C_rep` is dominated by the repair term. The Kolmogorov
seed `s_K` is the floor, and the floor is where edit-sensitivity σ is worst:
every symbol load-bearing, every repair edit maximally perturbing. The
document's payoff line — "the optimum is not maximum compression; it is
maximum compression conditioned on bounded repair" — is a direct refutation of
the paper's stated goal, written before the paper.

The tell is what the paper never mentions: **verification and attribution do
not appear once**. There is no C_ver, no contract, no answer to "when the
condensed form misbehaves, which site is to blame?" Every proposed primitive
is evaluated on how little the author writes, never on how a maintainer
localizes a fault. The paper optimizes the write path and is silent on the
read/repair path, which is the dominant term. Whatever "the wankery of
adequacy" is meant to dismiss, adequacy — the σ ≤ κ constraint, the contracts,
the audit trail — is precisely the margin above the Kolmogorov floor that
makes a short program survivable. A paper about minimal encoding of change
that never says "verify" has re-derived the fragile seed and called it the
destination.

There is also an internal contradiction worth naming: §1 derides static type
systems as compiler-golf, then §4 demands row-polymorphic environments,
contextual modal types, and partial evaluation — a heavier type-theoretic
apparatus than anything it criticizes. The resolution Lamedh already
implements is the correct one: **types as a gate, not an obligation**. HM runs
under the hood (`defun*`, HM-under-the-hood inference); success admits a form
to the native tier, failure costs nothing and leaves it dynamic
(`docs/typed-region-design.md` §1.2). Nobody golfs a compiler they never has
to satisfy.

## 3. The three calculi, judged by the lifetime objective

Apply the design work's own test to each of §3's proposals: does it lower
`r·C_rep` (and C_ver), or only C_gen? And apply Sonnet 5's razor from the
eval: does the case survive with the grand framing stripped off?

### 3a. Bidirectional lenses — admit, but demote from calculus to derive target

This is the paper's best idea, and its correct form is small. A lens is a
contract that makes a class of drift bugs unrepresentable: write the
relationship once, and get/put coherence is guaranteed by construction, with
the round-trip laws (GetPut, PutGet) mechanically checkable. That is exactly
the "contracts as error-correcting redundancy on intent" that
`condensation.md` demands — lenses lower repair cost, not just generation
cost. They pass the razor: stripped of "Symmetric Intent," they are
bidirectional codecs with laws, which is ordinary good engineering.

But nothing about this requires "first-class AST primitives" or a runtime
calculus. `derive` already generates one direction (`invoice->plist`); the
other direction plus round-trip `deflaw` entries is tens of lines in
`lib/20-condensation.lisp`, zero Rust — squarely inside both the CLAUDE.md
kernel discipline and the eval's "extend derive" consensus:

```lisp
(derive invoice lens)          ; => invoice->plist, plist->invoice
                               ;    + laws: (plist->invoice (invoice->plist x)) ≡ x
                               ;    + registered in condense-trace
```

The deeper value of the lens frame is reflexive, and the paper misses its own
best application: **condensation itself is currently a one-way lens**.
`defconcept` is a `get` from seed to expansion with no `put` back. If anyone
edits a generated function, the seed silently lies — `condense-trace` will
keep reporting a source form that no longer corresponds to the code. This is
the metadata-rot risk from the eval (Opus 4.6) restated with more precision,
and it has a cheap, correct resolution that is *not* building a putback:
regenerate-only discipline, enforced by a staleness check. Record a hash of
each generated definition at derivation time; have `condense-trace` and
`condense-check` compare and flag drift as part of the dynamic frontier. The
lens laws for condensation are then checkable: seed → expansion → seed is
identity or the trace says, loudly, that it is not.

### 3b. Incremental lambda calculus — reject

ILC is a performance technology: change structures and derivatives buy
O(|Δinput|) *recomputation*. The paper sells it as an intent encoding ("the
diff itself becomes the program"), but these are different planes. The diffs a
programmer or an LLM authors are edits to *source*; ILC deltas are runtime
values flowing through a dataflow graph. Making Δ native means defining a
change structure for every type and a derivative for every primitive — a
kernel-wide investment that contradicts `roadmap_1_0.md` and CLAUDE.md's
keep-the-kernel-small rule, to buy incremental recomputation that no Lamedh
use case has asked for and that does nothing for repair cost. If incremental
recomputation is ever wanted, it is a Tier-1/Tier-2 optimization concern
behind the existing gate, not a semantics concern. The legitimate residue of
this section is source-plane diffing, which is §5's `condense-diff` — and that
needs a list-diff function, not a calculus.

### 3c. Constraint-handling rules — reject, and note it is anti-condensation

This is the proposal the design work's own axioms condemn most directly. A
continuous background solver that "exhaustively rewrites the graph … without
requiring explicit control flow" attacks all four of `condensation.md`'s
load-bearing properties at once:

- **Determinism/totality:** confluence and termination of CHR rulesets are
  undecidable in general; `sem` stops being a point and becomes a distribution
  over rule-firing orders.
- **Compositional locality:** a rule can fire from anywhere in the store, so
  the localization span q(s) is pushed toward n, not log n. A behavioral fault
  no longer bisects to a command; it diffuses into an emergent interaction of
  the ruleset.
- **Attribution:** "which rule caused this?" has no local answer, which is
  the verifiability floor removed, not raised.

Active rewriting is the single proposal here that would *raise* lifetime cost
by construction. The salvageable kernel is that rule-based rewriting is
valuable when it is **staged, opt-in, inspectable, and semantics-preserving by
contract** — which Lamedh already has as the optimizer-pass architecture
(`lib/11-optimizer-vau.lisp`), where a pass is a Lisp-to-Lisp transform you
can read, test, and disable. Grow that; do not install a solver in the
evaluator.

## 4. §4 already exists: the tier ladder is the paper's runtime section, made decidable

The paper's "3-zone engine" (Typed Compilable / Typed Interpreted / Any) is
Lamedh's Tier 2 / Tier 1 / Tier 0 (`docs/typed-region-design.md`), and its
three §4 mechanisms map onto the architecture as follows:

- **Futamura / partial evaluation:** "when the dynamic environment of a $vau
  operative stabilizes, collapse it into a static compilable Lambda" — this is
  speculative online specialization, and Wand's triviality theorem (already
  cited by the typed-JIT design as the reason operatives are structurally
  excluded from Tier 2) tells you the general case is not there to be had:
  fexpr-equivalence collapses to α-equivalence, so there is no non-trivial
  stability to observe. Lamedh's answer is the same projection made
  *decidable*: the HM gate. Write applicative code and elaboration success —
  not runtime observation — is the operational trigger that promotes it to
  native (`typed-region-design.md` §1.2). vau stays in the Any zone by
  theorem, not by policy. The paper independently converged on the
  architecture and then asked for the one version of it that cannot work.
- **Row-polymorphic environments:** the stated goal ("typed structures pass
  through the Any zone without losing their proofs") is already achieved by a
  different, cheaper mechanism: the membrane. Typed values crossing into
  dynamic land are boxed; proofs live on the island and do not need to survive
  vau capture, because the gate re-checks at the boundary. Typing captured
  environments as row-polymorphic records is a research program with the same
  cost/benefit profile as the constrained-HM arc the eval voted 4/4 to freeze
  — freeze it with the rest.
- **Contextual modal types (typed holes in quotation):** the one §4 item with
  a plausible cheap v0 — a quasiquote whose unquote sites carry type
  expectations checked at splice time. But note what already routes the same
  failure mode today: `condense-check-type` runs the checker over generated
  symbols and records what stayed dynamic. Typed holes are sugar that moves
  the same check earlier. Worth a small experiment *after* the benchmark
  exists (§5); not before, because we cannot yet measure whether earlier
  detection pays.

## 5. My position on the design work itself

The paper is not the only document under review; the condensation corpus has
its own soft spots, and I differ from the four-system eval in emphasis at a
few points.

**Where I co-sign the consensus:** the decoder/lifetime-cost model is a
prioritization lens, not a computable model; the Kolmogorov-floor fragility
claim is asserted, not proven, though the conclusion follows from r being
large anyway; the typeclass→constrained-HM→JIT arc was drift, and the merge
record has since ratified that judgment; measurement is the top gap.

**What I add:**

1. **r is endogenous, and that strengthens the thesis.** The model treats
   repair episodes r as an exogenous horizon, but contracts and laws reduce
   the *failure rate* itself, not just per-episode cost — the redundancy triad
   buys down both factors of the dominant term. Conversely this sharpens the
   small-r objection (Opus 4.6): for throwaway scripts the seed really is
   optimal, and the language should not tax that path. Lamedh's shape is
   already right here — `defconcept` and plain `defun` coexist; condensation
   is opt-in redundancy, priced only when the horizon justifies it.
2. **The trace is theoretically load-bearing, not a nice-to-have.** Opus 4.8's
   context-economics point deserves promotion to a first-class term: the real
   generation cost of a *repair* is dominated by reading context, not writing
   the edit. `condense-trace` is precisely a context-compression device — it
   is the artifact that turns "read the whole expansion" into "read the
   alist." That is why structured checker output matters so much: a trace
   assembled by substring-matching a pretty-printed string
   (`lib/20-condensation.lisp:64`) is a context-compression device that can
   lie, and a lying trace is worse than none (GPT-5.5's false-confidence
   point, still live).
3. **On the contested typeclass question (the eval's 2–2 split):** side with
   decouple-and-defer, with one refinement — the dictionary layer
   (`lib/19-typeclasses.lisp` on `feat/typeclasses`) is pure Lisp, kernel-free,
   and harmless as a shelved library; the expensive part was always the
   constrained-HM weld. Keep the branch as a design record; do not merge it
   until a benchmark shows generic dispatch lowering repair cost, which I
   doubt it will (ad-hoc polymorphism raises the localization span — the
   Sonnet 5 / Opus 4.8 half of the split had the better argument).
4. **On destination scope:** the Opus 4.8 / GPT-5.5 generalization
   (condensation over any contract-bearing definition — functions, protocols,
   state machines — not just record schemas) is right as a destination, and
   schemas remain right as the sequencing, because concept derivations are
   deterministic and therefore measurable first.

## 6. The program I would actually run

Ranked; the first two are the standing 4/4-consensus items that remain
unbuilt, and the middle two are this paper's legitimate residue.

1. **Build the repair benchmark, and make it the admission test for
   primitives.** Fault-injection over derived concepts: mutate the expansion
   (the σ(s) proxy), then measure fix-rate and diff-locality for an agent
   given (a) seed + trace, (b) expansion only, (c) hand-written baseline. This
   is the falsifiability lever for the whole epic — and it converts debates
   like this one from rhetoric into measurement. Lens derivation, typed
   holes, generic dispatch: each earns its way in by winning on the benchmark
   or stays out.
2. **Structured `check-type` output.** Return a tagged form (status, scheme,
   frontier reason) alongside or instead of the pretty string; delete the
   substring parse at `lib/20-condensation.lisp:64-69`. Small Rust change,
   unblocks a trustworthy trace, kills the false-confidence failure mode.
3. **`condense-diff` + staleness detection + auto re-verify.** This is
   "first-class change" done in the correct plane. Hash generated definitions
   at derivation time; on redefinition of a concept, record the structural
   diff of the expansion, flag hand-edited drift in the trace, and re-run
   laws/examples automatically. Change becomes a datum with an audit trail
   and a re-verification trigger — the paper's p —t→ p', landed as data
   instead of as semantics.
4. **`derive lens`** (and the eval's other cheap templates: serializer,
   hasher, comparator): the paper's §3a admitted at its true size — tens of
   lines of Lisp, round-trip laws attached as `deflaw` entries, instances
   recorded in the trace.
5. **Freeze the rest, explicitly:** no Δ types, no CHR/background solver, no
   row-polymorphic environment typing, no speculative vau-collapse. vau
   remains the Any-zone floor, structurally excluded from the fast tier by
   the gate; the optimizer-pass layer remains the sanctioned home for
   rewriting. Revisit any of these only with benchmark evidence in hand.

## 7. Coda: what "first-class intent" should mean here

The paper ends by declaring that syntax is solved and evaluation semantics
exhausted, and concludes the frontier must therefore be a new calculus of
change. Half of that is right. The frontier is not evaluation semantics — but
it is not a calculus either. The scarce resource in the LLM-authoring regime
is **attribution**: the ability to trace a behavior back through generated
code to the seed that meant it, cheaply enough to repair it. Lamedh's actual
inventory — seed, deterministic expansion, generated-symbol registry, laws,
examples, checker status, dynamic frontier, all inspectable as data on a
plist — is further along toward first-class intent than any of the paper's
three calculi would take it. Intent became first-class in Lamedh the day
`condense-record!` landed. What remains is to make the record impossible to
fool (structured checker output, staleness detection) and to prove it earns
its margin (the benchmark). The change tree the paper wants is
`condense-trace`'s next field, not the evaluator's next primitive.
