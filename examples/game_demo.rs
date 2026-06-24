//! Using Lamedh as the extension language for a tiny Rust "game".
//!
//! Run with:  cargo run --example game_demo
//!
//! This demonstrates the full embedding loop you'd use to script a real game:
//!
//!   1. Define a host (Rust) object type — `GameEntity` — and expose it to Lisp
//!      as a first-class `LispVal` via the `LispValExtension` trait.
//!   2. Register host functions that read and *mutate* those objects.
//!   3. Load a Lisp script (`examples/game/ai.lisp`) that defines the entities'
//!      behaviour using only the primitives the host exposes.
//!   4. Run the simulation from Rust: each tick, Rust calls the Lisp-defined
//!      `take-turn` function on each entity. Lisp decides what to do; the shared
//!      host objects are mutated in place and the Rust loop observes the result.
//!
//! The key idea: the *engine* (turn order, win condition, the world grid) lives
//! in Rust, while the *behaviour* (how a goblin chases and fights) lives in
//! hot-reloadable Lisp. Both sides operate on the same mutable `GameEntity`
//! objects.

use lamedh::{
    LispError, LispVal, LispValExtension, environment::Environment, eval_str, load_file, printer,
};
use std::cell::RefCell;
use std::hash::Hasher;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// 1. The host object type.
// ---------------------------------------------------------------------------

/// The mutable state of one creature in the world.
#[derive(Debug)]
struct EntityData {
    name: String,
    x: i64,
    y: i64,
    hp: i64,
    max_hp: i64,
    attack: i64,
}

/// A handle to a game entity that can live inside a `LispVal`.
///
/// The state sits behind `Rc<RefCell<..>>`, so cloning the `LispVal` (which the
/// interpreter does freely) yields another handle to the *same* creature.
/// A host function — or Lisp code calling one — can therefore mutate an entity
/// and every other reference sees the change. This is exactly what you want for
/// game objects.
#[derive(Debug, Clone)]
struct GameEntity {
    data: Rc<RefCell<EntityData>>,
}

impl GameEntity {
    fn new(name: &str, x: i64, y: i64, hp: i64, attack: i64) -> Self {
        GameEntity {
            data: Rc::new(RefCell::new(EntityData {
                name: name.to_string(),
                x,
                y,
                hp,
                max_hp: hp,
                attack,
            })),
        }
    }
}

impl LispValExtension for GameEntity {
    fn type_name(&self) -> &str {
        "entity"
    }

    fn display(&self) -> String {
        let d = self.data.borrow();
        format!(
            "#<entity {} @({},{}) {}/{}hp atk:{}>",
            d.name, d.x, d.y, d.hp, d.max_hp, d.attack
        )
    }

    fn eq_ext(&self, other: &dyn LispValExtension) -> bool {
        // Identity equality: two handles are EQ iff they point at the same
        // creature, regardless of its (mutable) field values.
        other
            .as_any()
            .downcast_ref::<GameEntity>()
            .is_some_and(|o| Rc::ptr_eq(&self.data, &o.data))
    }

    fn hash_ext(&self, state: &mut dyn Hasher) {
        // `state` is an unsized `dyn Hasher`, so call its methods directly
        // rather than going through `Hash::hash` (which needs a sized hasher).
        state.write_usize(Rc::as_ptr(&self.data) as *const () as usize);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Helpers for moving values across the Rust/Lisp boundary.
// ---------------------------------------------------------------------------

/// Pull a `GameEntity` back out of a `LispVal` argument, or produce a tidy error.
fn entity_arg(val: &LispVal, fname: &str) -> Result<GameEntity, LispError> {
    match val {
        LispVal::Extension(ext) => ext
            .as_any()
            .downcast_ref::<GameEntity>()
            .cloned()
            .ok_or_else(|| {
                LispError::Generic(format!(
                    "{fname}: expected an entity, got a {} extension",
                    ext.type_name()
                ))
            }),
        other => Err(LispError::Generic(format!(
            "{fname}: expected an entity, got {other:?}"
        ))),
    }
}

fn expect_args(args: &[LispVal], n: usize, fname: &str) -> Result<(), LispError> {
    if args.len() != n {
        return Err(LispError::Generic(format!(
            "{fname}: expected {n} argument(s), got {}",
            args.len()
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Register the host API the Lisp scripts are allowed to use.
// ---------------------------------------------------------------------------

/// The width/height of the (toroidal-free) square arena. Movement is clamped
/// to `[0, GRID_SIZE)` by the host so a script can never wander off the board.
const GRID_SIZE: i64 = 8;

fn register_game_api(env: &Rc<Environment>) {
    // --- constructors -----------------------------------------------------
    // (spawn-entity name x y hp attack) -> entity
    env.register_fn("spawn-entity", |args, _env| {
        expect_args(args, 5, "spawn-entity")?;
        let name = args[0].as_str_val()?;
        let x = args[1].as_number()?;
        let y = args[2].as_number()?;
        let hp = args[3].as_number()?;
        let attack = args[4].as_number()?;
        Ok(LispVal::ext(GameEntity::new(name, x, y, hp, attack)))
    });

    // --- accessors (entity -> field) -------------------------------------
    env.register_fn("entity-name", |args, _env| {
        expect_args(args, 1, "entity-name")?;
        let e = entity_arg(&args[0], "entity-name")?;
        Ok(LispVal::from(e.data.borrow().name.clone()))
    });
    env.register_fn("entity-x", |args, _env| {
        expect_args(args, 1, "entity-x")?;
        let e = entity_arg(&args[0], "entity-x")?;
        Ok(LispVal::from(e.data.borrow().x))
    });
    env.register_fn("entity-y", |args, _env| {
        expect_args(args, 1, "entity-y")?;
        let e = entity_arg(&args[0], "entity-y")?;
        Ok(LispVal::from(e.data.borrow().y))
    });
    env.register_fn("entity-hp", |args, _env| {
        expect_args(args, 1, "entity-hp")?;
        let e = entity_arg(&args[0], "entity-hp")?;
        Ok(LispVal::from(e.data.borrow().hp))
    });
    env.register_fn("entity-attack", |args, _env| {
        expect_args(args, 1, "entity-attack")?;
        let e = entity_arg(&args[0], "entity-attack")?;
        Ok(LispVal::from(e.data.borrow().attack))
    });
    // (entity-alive? e) -> T or NIL
    env.register_fn("entity-alive?", |args, _env| {
        expect_args(args, 1, "entity-alive?")?;
        let e = entity_arg(&args[0], "entity-alive?")?;
        Ok(LispVal::from(e.data.borrow().hp > 0))
    });

    // --- mutators (the interesting part) ---------------------------------
    // (move-entity! e dx dy) -> e   ; clamped to the grid by the host
    env.register_fn("move-entity!", |args, _env| {
        expect_args(args, 3, "move-entity!")?;
        let e = entity_arg(&args[0], "move-entity!")?;
        let dx = args[1].as_number()?;
        let dy = args[2].as_number()?;
        {
            let mut d = e.data.borrow_mut();
            d.x = (d.x + dx).clamp(0, GRID_SIZE - 1);
            d.y = (d.y + dy).clamp(0, GRID_SIZE - 1);
        }
        Ok(args[0].clone())
    });

    // (damage! e amount) -> remaining-hp
    env.register_fn("damage!", |args, _env| {
        expect_args(args, 2, "damage!")?;
        let e = entity_arg(&args[0], "damage!")?;
        let amount = args[1].as_number()?;
        let hp = {
            let mut d = e.data.borrow_mut();
            d.hp = (d.hp - amount).max(0);
            d.hp
        };
        Ok(LispVal::from(hp))
    });

    // (heal! e amount) -> new-hp   ; never exceeds max-hp
    env.register_fn("heal!", |args, _env| {
        expect_args(args, 2, "heal!")?;
        let e = entity_arg(&args[0], "heal!")?;
        let amount = args[1].as_number()?;
        let hp = {
            let mut d = e.data.borrow_mut();
            d.hp = (d.hp + amount).min(d.max_hp);
            d.hp
        };
        Ok(LispVal::from(hp))
    });

    // --- narration: let scripts print mixed values without string-building -
    // (game-log a b c ...) -> NIL   ; prints args space-separated
    env.register_fn("game-log", |args, _env| {
        let line: Vec<String> = args
            .iter()
            .map(|a| match a {
                LispVal::String(s) => s.clone(),
                other => printer::print(other),
            })
            .collect();
        println!("    | {}", line.join(" "));
        Ok(LispVal::Nil)
    });
}

// ---------------------------------------------------------------------------
// 3 + 4. Wire it together and run the simulation from Rust.
// ---------------------------------------------------------------------------

fn main() {
    // Lamedh uses large stack frames; run everything on a dedicated big stack.
    lamedh::with_large_stack(run);
}

fn run() {
    let env = Environment::with_stdlib();
    register_game_api(&env);

    // Load the behaviour script. Resolve relative to the crate so it works no
    // matter what directory `cargo run` is invoked from.
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/game/ai.lisp");
    if let Err(e) = load_file(script, &env) {
        eprintln!("failed to load AI script {script}: {e}");
        std::process::exit(1);
    }

    // Spawn the cast as host objects and hand them to Lisp as global bindings.
    let hero = LispVal::ext(GameEntity::new("Hero", 0, 0, 30, 6));
    let goblin = LispVal::ext(GameEntity::new("Goblin", 7, 7, 22, 4));
    env.set("*HERO*".to_string(), hero.clone());
    env.set("*GOBLIN*".to_string(), goblin.clone());

    println!("== Lamedh game demo: Hero vs Goblin on an {GRID_SIZE}x{GRID_SIZE} grid ==\n");
    print_world(&env, &[("Hero", &hero), ("Goblin", &goblin)]);

    // The engine owns turn order and the win condition; the scripts own behaviour.
    let combatants = [("*HERO*", "*GOBLIN*"), ("*GOBLIN*", "*HERO*")];
    let max_rounds = 20;

    for round in 1..=max_rounds {
        println!("\n-- round {round} --");
        for (actor, target) in &combatants {
            if !alive(&env, actor) || !alive(&env, target) {
                continue;
            }
            // Call the Lisp-defined behaviour. The script mutates the shared
            // host objects through the functions we registered above.
            let call = format!("(take-turn {actor} {target})");
            if let Err(e) = eval_str(&call, &env) {
                eprintln!("error running {call}: {e}");
                std::process::exit(1);
            }
        }

        print_world(&env, &[("Hero", &hero), ("Goblin", &goblin)]);

        if !alive(&env, "*HERO*") || !alive(&env, "*GOBLIN*") {
            break;
        }
    }

    // Read the final state back into Rust to decide the outcome.
    println!("\n== Result ==");
    let winner = match (alive(&env, "*HERO*"), alive(&env, "*GOBLIN*")) {
        (true, false) => "Hero wins!",
        (false, true) => "Goblin wins!",
        (true, true) => "Stalemate — both still standing after the round limit.",
        (false, false) => "Both fell.",
    };
    println!("{winner}");
}

/// Query an entity's liveness from Rust by evaluating a tiny Lisp expression.
fn alive(env: &Rc<Environment>, var: &str) -> bool {
    eval_str(&format!("(entity-alive? {var})"), env)
        .map(|v| v.is_truthy())
        .unwrap_or(false)
}

/// Render the board by reading the host objects' display strings.
fn print_world(env: &Rc<Environment>, named: &[(&str, &LispVal)]) {
    for (label, val) in named {
        let shown = printer::print(val);
        println!("   {label:<7} {shown}");
    }
    // Suppress unused warning for env in case the helper grows later.
    let _ = env;
}
