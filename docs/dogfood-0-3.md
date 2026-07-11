# The Classics Dogfood Log

Fifty classic programs under `examples/<name>/main.lisp`, written against
0.3 as it ships. Every friction point hit while writing them is recorded
here with a disposition: **fixed** (PR#), **filed** (issue#), or **ruled**
(worked as designed once the design was read). Predecessor: the wordcount
pilot, which found the dotted-pair checker gap (#337) and the derived
nil-as-list imprecision (#336).

## Pre-flight probes

- `random` exists (kernel builtin), distribution looks healthy. No
  `random-float`; scale `(random n)` when a float is wanted.
- `monotonic-micros` baselines at 0 on first call and progresses — fine.
- `load` does NOT exist — multi-file examples use `-i` or the module
  system. Disposition: noted; a READ-FS-gated `load` is conventional and
  may be worth adding if an example genuinely needs runtime loading.
- `read-line` does NOT exist — no interactive examples in this suite.

## Findings

### Batch A (basics/numeric)

- **`random` was not a PRNG** — every call returned
  `nanos_since_epoch % n`: a monotonic wall-clock ramp, heavily biased
  and serially correlated (the code's own comment claimed an LCG that
  wasn't there). Found because monte-carlo-pi converged to ~2.57, not
  3.14. **Fixed**: SplitMix64 with persistent thread-local state,
  time-seeded once, plus `random-seed!` for reproducible runs.
- `step-count` returns `(steps . value)` — steps in the car. **Ruled**
  (documented shape; the example now reads it correctly), but worth
  knowing the value is in the CDR, not a second list element.
- 8/10 batch A programs ran correctly on first write — protocol names,
  staples (`sort-by`, `enumerate`, `take`), `dotimes`/`while`,
  `format`, and float arithmetic behaved as documented.

### Batch B (sorting/searching/strings)

- 10/10 pass; the only iteration was an authoring slip (`letter-p` for
  the real `alpha-p`). Notable positives under load: `take-while`/
  `drop`/`take` compose cleanly for run grouping; Option + `variant-case`
  is exactly right as a binary-search result type (no nil-index
  ambiguity); the string `map` instance carries caesar rot13 in one
  line; seeded `random` (#340) makes the quicksort/base-conversion
  torture tests reproducible.
- `string-index-of` returning nil-on-miss composes fine when the miss is
  handled at the edge (base-conversion's `digit-value`) — the pattern
  the type table's honesty rule 1 assumes.

### Batch C (DP/graphs)

- **`flatten` eats dotted pairs** — `(flatten '((1 . 2) (3 . 4)))` is
  `(1 2 3 4)`: it recurses into cons structure, so it silently destroys
  coordinate/alist-shaped data. Bit twice in one batch (bfs-maze
  find-cell, game-of-life neighbor census), and the failure mode is a
  quietly wrong world, not an error. **Ruled**: flatten's contract is
  structural, and `mapcan` (which exists) is the right one-level tool —
  but the trap deserves a doc warning where flatten is taught.
- Checker/evaluator numeric strictness caught an `assoc`-miss nil
  flowing into `=` in game-of-life (isolated live cell has no census
  entry) — a bug class that would have silently mis-evolved worlds in a
  lenient Lisp. The strictness is earning its keep.
- Self-check honesty: my remembered knapsack constant was for a
  different instance; replaced with an in-file exhaustive-subset oracle
  (12 items, 4096 subsets, instant). Examples now prefer independent
  oracles over remembered constants where feasible.
- Sequence/pair machinery held up under real load: (pair int64 int64)
  hash keys for LCS memo and BFS predecessor maps; variants as huffman
  trees with variant-case recursion; frequencies as the life census.
