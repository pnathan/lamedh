# Game demo: Lamedh as an extension language

A tiny, self-contained example of driving a Rust "game" with Lamedh scripts.

```bash
cargo run --example game_demo
```

## What it shows

The demo is split across two files:

- **`examples/game_demo.rs`** — the host (the "engine"). It defines a Rust
  object type, exposes it and a handful of functions to Lisp, loads the script
  below, and runs the turn loop.
- **`examples/game/ai.lisp`** — the script (the "behaviour"). It decides what an
  entity does on its turn, using only the primitives the host registered.

### 1. A host object type exposed to Lisp

`GameEntity` is an ordinary Rust struct whose state lives behind
`Rc<RefCell<..>>`. It implements the `LispValExtension` trait, so it can be
wrapped with `LispVal::ext(..)` and handed to Lisp as a first-class value.
Because the state is shared, mutating an entity through any handle is visible
everywhere — exactly what game objects need.

### 2. Host functions registered with `env.register_fn`

| Lisp call | Effect |
|-----------|--------|
| `(spawn-entity name x y hp atk)` | construct an entity |
| `(entity-name e)` / `-x` / `-y` / `-hp` / `-attack` | read a field |
| `(entity-alive? e)` | `T` if `hp > 0`, else `NIL` |
| `(move-entity! e dx dy)` | move (clamped to the grid) — **mutates** |
| `(damage! e n)` / `(heal! e n)` | change hp — **mutates** |
| `(game-log ...)` | print mixed values, space-separated |

### 3. Behaviour written in Lisp

`ai.lisp` builds higher-level behaviour (`chebyshev` distance, `adjacent?`,
`step-toward`, `strike`) out of those primitives and the standard library
(`defun`, `let`, `cond`, `abs`, `max`, ...). The single entry point the engine
calls each turn is `(take-turn self target)`.

### 4. The engine drives the loop from Rust

Each round, `game_demo.rs` calls the Lisp-defined `take-turn` for each
combatant. Lisp mutates the shared host objects; Rust reads the result back to
render the board and to decide the win condition.

## Why this split is the point

The engine — turn order, the grid, the win condition — stays in compiled Rust.
The behaviour — how a goblin chases and fights — lives in plain text Lisp you
can edit and reload without recompiling. Swap in a different `ai.lisp` (one that
flees at low hp, say) and the engine is none the wiser.

## Performance

After the fight, the demo measures one king-move (Chebyshev) distance 200k
times, several ways. Representative numbers (release build; absolute values
vary by machine, the *ratios* are the point):

```
1. native Rust (baseline)                            1.9 ns/op
2. host-side (entity-distance), cached form        758   ns/op  (~400x)
3. Lisp fixed-arity (chebyshev-fast), cached     31600   ns/op  (~16000x)
4. Lisp variadic stdlib (chebyshev), cached      87700   ns/op  (~46000x)
5. same as 4, but eval_str re-parses each call   90600   ns/op  (~48000x)
```

Reading it:

- **The host/Lisp boundary is cheap.** A bare host accessor (`entity-x`) costs
  about the same as a built-in (~0.5 µs). The trait-object downcast plus `Rc`
  refcount bump that retrieves the entity is ~1–3% of that — not worth
  optimizing. The interpreter's per-call cost (env allocation, symbol lookup)
  dominates, not the boundary.
- **`&rest` is expensive.** The stdlib `max` is variadic *and* recurses through
  `(apply #'max ...)`. Replacing it with a fixed-arity `max2` (and `abs1` for
  `abs`, which calls `minusp`) is ~2.8x faster (row 4 → row 3) for identical
  results. See `abs1` / `max2` / `chebyshev-fast` in `ai.lisp`.
- **Caching the parse is a free win.** `eval_str` re-parses the source every
  call (~3 µs of nom parsing). Parse once with `reader::read` and call
  `evaluator::eval(&form, env)` repeatedly (rebinding globals as needed) to skip
  it. The naive turn loop uses `eval_str` for readability; the perf section
  shows the cached path.
- **Pushing hot math host-side is the big lever.** `entity-distance` does the
  whole computation in Rust in one boundary crossing — ~115x faster than the
  best interpreted version. Rule of thumb: every line of arithmetic in a
  per-frame Lisp path costs ~5–60 µs; the same work behind one `register_fn` is
  ~0.5 µs. Keep behaviour/decisions in Lisp; keep tight numeric loops in Rust.
