# Embedding Lamedh

Lamedh can be embedded in any Rust application as a scripting layer.
This page documents the public embedding API.

## Adding as a dependency

```toml
[dependencies]
lamedh = { path = "../lamedh" }   # or publish to crates.io and use version
```

## Quick start

```rust
use lamedh::{LispError, LispVal, environment::Environment, eval_str};
use std::convert::TryFrom;

fn main() {
    // Lamedh uses large stack frames — run on a dedicated thread.
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        let result = eval_str("(+ 1 2)", &env).unwrap();
        let n: i64 = i64::try_from(result).unwrap();
        println!("{n}");  // 3
    });
}
```

See `examples/embedding.rs` for the full walkthrough.

## Stack size

The tree-walking interpreter uses large stack frames. If you embed it on the
main thread or a thread with a small stack you will hit a stack overflow before
the recursion-depth guard fires. Two options:

- **Recommended**: wrap your entire embedding entry point in
  `lamedh::with_large_stack(|| { ... })`.  This spawns a single 512 MiB thread.
- **Alternative**: lower the recursion limit with
  `lamedh::set_eval_depth_limit(n)` so the guard fires earlier.

`LispVal` and `Environment` are `!Send`, so you must create the environment
*inside* the closure passed to `with_large_stack`.

## Creating an environment

```rust
// Builtins only (no defun, list utilities, etc.)
let env = Environment::new_with_builtins();

// Builtins + the full embedded standard library (recommended)
let env = Environment::with_stdlib();
```

`with_stdlib` evaluates the stdlib Lisp files that are baked into the binary
at compile time.  It panics if the embedded stdlib fails to parse or evaluate,
which should never happen in a correctly-built binary.

## Evaluating Lisp

```rust
use lamedh::{eval_str, eval_all};

// Evaluate one expression, get the typed result.
let val: LispVal = eval_str("(+ 1 2)", &env)?;

// Evaluate multiple top-level expressions; get all results.
let vals: Vec<LispVal> = eval_all("(def x 1) (+ x 2)", &env)?;
```

Both functions return `Err(LispError::Generic(msg))` on parse or runtime errors.

## Value conversion

### Rust → Lisp (`From<T>`)

| Rust type    | LispVal produced              |
|-------------|-------------------------------|
| `i64`        | `LispVal::Number`             |
| `f64`        | `LispVal::Float`              |
| `bool`       | `LispVal::Symbol("T")` / Nil  |
| `String`     | `LispVal::String`             |
| `&str`       | `LispVal::String`             |
| `Vec<LispVal>` | proper list (Cons chain)    |

```rust
let v: LispVal = LispVal::from(42i64);
let list = LispVal::list([1i64, 2, 3]);   // (1 2 3)
```

### Lisp → Rust (`TryFrom<LispVal>`)

```rust
let n: i64    = i64::try_from(val)?;
let f: f64    = f64::try_from(val)?;   // also accepts Number, coerces
let b: bool   = bool::try_from(val)?;  // Nil → false, anything else → true
let s: String = String::try_from(val)?;
let v: Vec<LispVal> = Vec::try_from(val)?;  // proper list only
```

### LispVal helpers

```rust
val.as_number()?     // → i64
val.as_float()?      // → f64 (coerces Number)
val.as_str_val()?    // → &str
val.as_list_vec()?   // → Vec<LispVal>
val.is_truthy()      // → bool
```

## Registering host functions

`register_fn` installs a Rust closure as a callable Lisp function.

```rust
env.register_fn("add-one", |args, _env| {
    if args.len() != 1 {
        return Err(LispError::Generic("add-one takes 1 arg".to_string()));
    }
    let n = args[0].as_number()?;
    Ok(LispVal::from(n + 1))
});

eval_str("(add-one 41)", &env)  // => Number(42)
```

The name is automatically uppercased to follow Lisp symbol conventions.
The function receives evaluated arguments (same as any built-in).

### Accessing the environment from a host function

The second argument is `&Rc<Environment>`. You can call `eval_str` or look up
symbols from inside the host function if needed, but be careful about
re-entrant evaluation.

## Error handling

```rust
match eval_str("(/ 1 0)", &env) {
    Ok(val)  => { /* use val */ }
    Err(LispError::Generic(msg)) => eprintln!("Lisp error: {msg}"),
    Err(e)   => eprintln!("internal error: {e}"),
}
```

`LispError::Return` and `LispError::Go` are internal control-flow signals for
`PROG`/`RETURN`/`GO` and should not escape `eval_str` under normal use.  If
they do, treat them as internal errors.

## Capability flags (sandboxing)

Features gate access to potentially dangerous operations.  All features are
disabled by default.

```rust
// Grant shell access to the script.
env.enable_feature("SHELL");

// Revoke it.
env.disable_feature("SHELL");

// Check from Rust.
env.feature_enabled("SHELL");
```

From Lisp, the same flags are checked automatically when the `SHELL` function
is called.  You can also expose custom flags to Lisp scripts via
`register_fn` wrappers that call `env.feature_enabled("YOUR_FLAG")`.

Current built-in feature flags:

| Flag    | Description                              |
|---------|------------------------------------------|
| `SHELL` | Allow `(shell ...)` to run subprocesses  |

## Condition flags

Condition flags are boolean markers in the environment, distinct from feature
flags. They are used to pass state between Lisp and host code.

```rust
env.set_flag("DEBUG");
env.flag_set("DEBUG");   // → true
env.clear_flag("DEBUG");
env.clear_all_flags();
```

From Lisp: `(set-flag "DEBUG")`, `(flag-set-p "DEBUG")`, `(clear-flag "DEBUG")`.
