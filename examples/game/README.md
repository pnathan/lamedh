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
