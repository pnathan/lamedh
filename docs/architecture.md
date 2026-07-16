# Lamedh Internals: Architecture Guide

This document describes the internal architecture of Lamedh for contributors and embedders who need a deep understanding of how the interpreter works.

---

## Overview

Lamedh is a tree-walking interpreter organized into five core modules:

```
Source text
    │
    ▼
reader.rs ──── Parses s-expressions into LispVal trees (nom-based)
    │
    ▼
evaluator.rs ── Evaluates LispVal trees (special forms + builtins + function calls)
    │   ▲
    │   └── environment.rs  (variable lookup, symbol table, scoping)
    │
    ▼
printer.rs ──── Formats LispVal back to readable text

lib.rs ────────── Public API: types, constants, embed-facing functions
```

The CLI crate (`cli/src/main.rs`) sits on top of the library and adds argument parsing (clap) and line editing (rustyline).

Beyond the five core modules, the library carries the typed JIT under
`src/jit/` (`elaboration.rs`, `native.rs`, `registry.rs`, `runtime.rs`,
`types.rs`, `infer.rs`, `parse.rs`) and a set of agent-facing and
diagnostic modules added since v0.3.0:

| Module | Purpose |
|--------|---------|
| `src/teaching_errors.rs` | Did-you-mean (Levenshtein over bound names) and Common-Lisp-ism guidance appended to unbound-symbol / undefined-function errors. |
| `src/check.rs` | Static verification for `lamedh --check` — parse, unbound-operator, and arity linting without executing the file. |
| `src/fmt.rs` | The canonical formatter for `lamedh --fmt`/`--fmt-check` — indentation/whitespace only, meaning-preserving. |
| `src/test_runner.rs` | The `deftest` runner behind `lamedh --test`, producing human or sexpr findings. |
| `cli/src/mcp.rs` | The `lamedh --mcp` Model Context Protocol server (JSON-RPC 2.0 over stdio) driving one persistent, sandboxed interpreter. |

---

## The Data Model (`src/lib.rs`)

### LispVal

Every Lisp value is represented by the `LispVal` enum:

```rust
pub enum LispVal {
    Symbol(Rc<RefCell<Symbol>>),
    Number(i64),
    Float(f64),
    String(String),
    Nil,
    Cons { car: Rc<LispVal>, cdr: Rc<LispVal> },
    Builtin(BuiltinFunc),
    Lambda(Lambda),
    Fexpr(Fexpr),
    Macro(Macro),
    Vau(Vau),
    HashTable(Rc<RefCell<HashMap<LispVal, LispVal>>>),
    Native(Rc<NativeFn>),
    Environment(Rc<Environment>),
    Array(Rc<RefCell<Vec<LispVal>>>),
    TypedArray(Shared<TypedArrayObj>),   // flat int64/float64 buffer (JIT membrane)
    Struct(Shared<StructObj>),           // defrecord / defstruct-typed instance
    Char(u8),
    Error(Rc<ErrorObj>),                 // first-class condition value
    Extension(Rc<dyn LispValExtension>),
}
```

(Abbreviated — see `src/lib.rs` for the exact, current variant set. `Vau`
and the typed-JIT-related variants also live on this enum.)

**Key design decisions:**

- **Cons cells use `Rc<LispVal>`** (not `Box<LispVal>`), enabling O(1) structural sharing. Cloning a list head is a refcount bump, not a deep copy. This makes passing lists around cheap but also means no mutable in-place modification — `RPLACA`/`RPLACD` return new cells.

- **Symbols use `Rc<RefCell<Symbol>>`** so multiple references to the same symbol share the same property list. Symbol interning ensures two occurrences of the same symbol name refer to the same `Rc`.

- **`Native`** is a Rust closure registered via `env.register_fn()`. It's the embedding hook for host functions.

- **`Extension`** allows host code to store arbitrary typed Rust values in the Lisp world via the `LispValExtension` trait.

### Symbol and Property Lists

```rust
pub struct Symbol {
    pub name: String,
    pub plist: HashMap<String, LispVal>,
}
```

Every interned symbol has a property list (`plist`) — a key/value store for metadata. The canonical use is storing docstrings under the key `"docstring"`. `GETP`/`PUTP`/`REMPROP` expose this to Lisp code.

### Closure Types

| Type | Parameters | Body evaluation |
|------|-----------|----------------|
| `Lambda` | Evaluated at call site | Body evaluated in new child env |
| `Fexpr` | Passed unevaluated as a list | Body sees raw argument forms |
| `Macro` | Passed unevaluated | Body produces code; that code is evaluated |
| `Vau` | Operands list + caller's env | Kernel-style operative |

```rust
pub struct Lambda {
    pub params: Vec<String>,
    pub rest_param: Option<String>,  // &REST variable
    pub body: Box<LispVal>,
    pub env: Rc<Environment>,        // Captured lexical environment
}
```

### Error Types

```rust
pub enum LispError {
    Generic(String),        // Runtime errors surfaced to users
    Return(Box<LispVal>),   // RETURN signal — carries value out of PROG
    Go(String),             // GO signal — carries label name
}
```

`Return` and `Go` are control-flow signals, not true errors. They propagate up the call stack via Rust's `?` operator until `PROG` catches them.

---

## The Reader (`src/reader.rs`)

The reader converts source text into `LispVal` trees using nom parser combinators. The entry points are:

```rust
pub fn read(input: &str, env: &Rc<Environment>) -> Result<LispVal, String>;
pub fn read_all(input: &str, env: &Rc<Environment>) -> Result<Vec<LispVal>, String>;
```

The environment is passed in so the reader can intern symbols into the global symbol table.

### Parsing pipeline

```
read()
  └── parse_expr()
        ├── parse_number()          "123", "-456", "3.14e5"
        ├── parse_octal()           "177Q" → 127
        ├── parse_string()          "hello\nworld"
        ├── parse_quote()           'expr  → (QUOTE expr)
        ├── parse_quasiquote()      `expr  → (QUASIQUOTE expr)
        ├── parse_unquote()         ,expr  → (UNQUOTE expr)
        ├── parse_function()        #'sym  → (FUNCTION sym)
        ├── parse_list()            (a b c) or (a . b)
        └── parse_atom()            Symbol, T, NIL, operators
```

### Symbol normalization

All symbol names are converted to uppercase during interning. `foo`, `FOO`, and `Foo` all resolve to the same interned symbol `FOO`.

### Octal literals

Lamedh 1.5 supports Lisp 1.5 octal notation: `177Q` means 127 in decimal. This is rarely used in practice but maintained for historical compatibility.

---

## The Evaluator (`src/evaluator.rs`)

The evaluator is the heart of the interpreter at approximately 3,800 lines. Its structure is carefully designed to support tail-call optimization (TCO) without requiring a continuation-passing style transformation.

### Trampolining TCO

The key insight: in a tree-walking interpreter, tail calls normally consume Rust stack frames. Lamedh avoids this via a trampoline. Since Milestone 1 of the compile→execute IR (issue #200), there are two representations that can appear in tail position — a raw AST form (tree-walker) and a compiled `Code` node (`src/evaluator/compile.rs`) — so one loop, `run_trampoline`, drives both:

```
eval(expr, env)                       ← acquires depth guard, calls run_trampoline(Val(expr), env)
exec_entry(code, env)                 ← acquires depth guard, calls run_trampoline(Code(code), env)
  └── run_trampoline(current, env)    ← loop: steps eval_step or exec_step depending on `current`
        ├── eval_step(expr, env) → TcoStep       (when current = Current::Val)
        └── exec_step(code, env) → TcoStep       (when current = Current::Code)
              ├── TcoStep::Done(val)                  ← return value, exit loop
              ├── TcoStep::TailCall(expr, env)        ← continue as Current::Val
              ├── TcoStep::TailCallWithGuards(...)     ← continue as Current::Val, accumulate dynamic-binding guards
              └── TcoStep::ExecTail(code, env)         ← continue as Current::Code
```

This means tail calls in `IF` branches, `PROGN` last form, `LET` body, and lambda bodies consume no additional Rust stack frames — including when a tail call crosses between the tree-walker and the compiled executor (e.g. a compiled lambda tail-calling an uncompiled special form, or a tree-walked call into a compiled lambda body). Splitting that into two independent trampolines was tried in M1 and caused a real regression: each crossing was a plain (non-tail) Rust call, so both the native stack and the eval-depth counter grew per iteration until the depth guard tripped on deep tail recursion. See issue #200 for the postmortem.

Non-tail recursive calls (e.g., evaluating arguments to a function) still go through `eval()`/`exec()`, so the depth guard applies to them.

### Recursion depth guard

```rust
thread_local! {
    static EVAL_DEPTH: Cell<usize> = Cell::new(0);
    static EVAL_DEPTH_LIMIT: Cell<usize> = Cell::new(DEFAULT_EVAL_DEPTH_LIMIT);
}

struct DepthGuard;
impl DepthGuard {
    fn acquire() -> Result<Self, LispError> {
        let d = EVAL_DEPTH.get();
        if d >= EVAL_DEPTH_LIMIT.get() {
            return Err(LispError::Generic("recursion depth limit exceeded".into()));
        }
        EVAL_DEPTH.set(d + 1);
        Ok(DepthGuard)
    }
}
impl Drop for DepthGuard {
    fn drop(&mut self) { EVAL_DEPTH.set(EVAL_DEPTH.get() - 1); }
}
```

The default limit is 10,000 frames. This fires before the native Rust stack exhausts on a 512 MiB stack. The limit is per-thread and adjustable via `lamedh::set_eval_depth_limit(n)`.

### Special form dispatch

`eval_step` matches on the head of a cons cell to decide how to handle it:

```rust
match head_symbol.as_str() {
    "QUOTE"      => ...,
    "QUASIQUOTE" => ...,
    "IF"         => ...,
    "COND"       => ...,
    "AND"        => ...,
    "OR"         => ...,
    "DEF"        => ...,
    "DEFDYNAMIC" | "DEFVAR" => ...,
    "LAMBDA"     => ...,
    "DEFMACRO"   => ...,
    "DEFEXPR"    => ...,
    "DEFSTRUCT"  => ...,
    "PROGN"      => ...,
    "SETQ"       => ...,
    "PROG"       => ...,
    "RETURN"     => ...,
    "GO"         => ...,
    "LET"        => ...,
    "LET*"       => ...,
    "VAU" | "$VAU" => ...,
    // ... more
    _ => apply(evaluated_head, evaluated_args, env),
}
```

Forms not in this list fall through to `apply()`, which handles lambdas, fexprs, macros, vaus, builtins, and natives.

### Function application in `apply()`

```
apply(func, args, env)
    ├── Macro → expand body in macro env → TailCall(expansion, caller_env)
    ├── Vau  → bind operands + env_param → TailCall(body, vau_env)
    ├── Fexpr → bind unevaluated args → TailCall(body, fexpr_env)
    ├── Lambda → bind evaluated args → TailCall(body, lambda_env)
    ├── Builtin → dispatch to Rust function → Done(result)
    └── Native → call Rust closure → Done(result)
```

Macros, vaus, fexprs, and lambdas all return `TailCall` from `apply()`, so their bodies execute in the trampoline loop without consuming an additional Rust frame beyond what `eval()` already allocated.

### PROG implementation

`PROG` implements Lisp 1.5 labeled-statement control flow:

1. Collect all symbol atoms in the body as labels → HashMap<name, index>
2. Create local environment with vars initialized to NIL
3. Execute statements in order via an index counter
4. When `GO label` is executed → `LispError::Go(label)` propagates up to the PROG loop, which looks up the label's index and continues there
5. When `RETURN val` is executed → `LispError::Return(val)` propagates up to PROG, which catches it and returns the value

This is the only use of `LispError` for control flow rather than errors.

### DEFSTRUCT expansion

`(DEFSTRUCT name field1 field2 ...)` generates at evaluation time:

- `make-name` — constructor: `(make-name :field1 val1 :field2 val2 ...)`
- `name-p` — type predicate: `(name-p x)` → T if x is a `name` struct
- `name-field` — accessor: `(name-field x)` → value of field
- `set-name-field!` — mutator: `(set-name-field! x val)` → sets field

Structs are implemented as hash tables with a special `__type__` key storing the struct name.

### Quasiquote implementation

Quasiquote is processed recursively in `eval_step`:

```
`(a ,b (c ,d))
→ cons(a, cons(eval(b), cons(cons(c, cons(eval(d), nil)), nil)))
```

- Traverses the quoted structure
- When `(UNQUOTE x)` is encountered, evaluates `x` and splices the result
- Handles splicing (`@` in some Lisps, not yet in Lamedh)
- Nested quasiquotes increment a nesting level counter

---

## The Environment (`src/environment.rs`)

### Structure

```rust
pub struct Environment {
    parent: Option<Rc<Environment>>,              // Lexical parent
    bindings: Rc<RefCell<HashMap<String, LispVal>>>,
    shared: Rc<SharedState>,                      // Shared across env chain
    dynamic_parent: Option<Rc<Environment>>,      // Call-site parent
}

struct SharedState {
    symbols: RefCell<SymbolTable>,
    condition_flags: RefCell<HashMap<String, bool>>,
    dynamic_vars: RefCell<HashSet<String>>,
    features: RefCell<HashSet<String>>,
}
```

**`shared` is shared across all environments in a chain.** This means `SymbolTable`, condition flags, dynamic variable registry, and feature flags are all global-per-interpreter. Every `Environment::new_child()` receives a clone of the parent's `Rc<SharedState>`, so they all point to the same allocation.

### Lookup algorithm

`get_var(name)`:
1. If name is in `dynamic_vars`: use `get_dynamic(name)` (dynamic lookup)
2. Otherwise: use `get(name)` (lexical lookup)

`get(name)` (lexical):
1. Check `self.bindings`
2. If not found, recurse via `self.parent.get(name)`
3. Return error if not found anywhere

`get_dynamic(name)` (dynamic):
1. Check `self.bindings`
2. If not found, recurse via `self.dynamic_parent.get_dynamic(name)`
3. Fall back to lexical lookup (for global values)

### Child environments

When a function is called:

```rust
let call_env = Environment::new_child_with_dynamic(
    &closure.env,    // Lexical parent: closure's definition environment
    &caller_env,     // Dynamic parent: call site environment
);
// Bind parameters in call_env
```

This is the mechanism that gives lambdas lexical scope (they see their definition environment) while allowing dynamic variables to propagate via the call stack.

### Symbol interning

```rust
pub fn intern_symbol(&self, name: &str) -> LispVal {
    let upper = name.to_uppercase();
    let mut table = self.shared.symbols.borrow_mut();
    table.intern(&upper)  // Returns existing or creates new Rc<RefCell<Symbol>>
}
```

`EQ` in Lisp compares symbols by `Rc::ptr_eq()` — pointer identity. Two symbols with the same name are guaranteed to have the same pointer because interning returns the same `Rc` for the same name.

### Dynamic variables

```rust
pub fn mark_dynamic(&self, name: &str) {
    self.shared.dynamic_vars.borrow_mut().insert(name.to_uppercase());
}
```

Once marked, all lookups of that name use the dynamic parent chain. `DEFDYNAMIC` and `DEFVAR` call this after binding.

### Features (capability gating)

Features are strings stored in `SharedState::features`. All operations guarded
by features (`SHELL`, `READ-FS`, `CREATE-FS`, `TEMP-FS`, `IO`) check
`env.feature_enabled(name)` before proceeding. Since `SharedState` is shared,
enabling a feature in any child environment enables it for all.

---

## The Printer (`src/printer.rs`)

The printer converts `LispVal` trees back to readable text:

```rust
pub fn print(val: &LispVal) -> String;
```

| Value | Output format |
|-------|--------------|
| `Symbol` | Uppercased name |
| `Number(n)` | Decimal integer |
| `Float(f)` | Always includes `.` (e.g., `3.0` not `3`) |
| `String(s)` | Double-quoted with escape sequences |
| `Nil` | `()` |
| `Cons { car, cdr }` | Walks cdr: `(a b c)` for proper lists, `(a . b)` for improper |
| `Builtin` | `<builtin>` |
| `Lambda` | `<lambda>` |
| `Macro` | `<macro>` |
| `Fexpr` | `<fexpr>` |
| `Vau` | `<vau>` |
| `HashTable` | `<hash-table>` |
| `Array(a)` | `<array:N>` where N is length |
| `Native` | `<native>` |
| `Environment` | `<environment>` |
| `Extension` | via `LispValExtension::display()` |

Cons printing detects proper lists (cdr chains ending in Nil) and omits dotted-pair notation for them.

---

## Standard Library Loading

The standard library Lisp files are embedded in the binary at compile time using `include_str!()`:

```rust
const LIB_00_CORE: &str = include_str!("../lib/00-core.lisp");
// ...

pub fn load_stdlib(env: &Rc<Environment>) {
    for (name, source) in STDLIB_FILES {
        eval_all(source, env)
            .unwrap_or_else(|e| panic!("stdlib {} failed: {}", name, e));
    }
}
```

`Environment::with_stdlib()` serves a **deep-copy fork of a per-thread
prototype**: the first call on a thread runs `new_with_builtins()` +
`load_stdlib()` once into a private prototype world, and every call
(including the first) returns `Environment::fork_world(prototype)` — an
isomorphic copy with a fresh symbol table, fresh global value cells, and
fresh closures/containers, built in milliseconds. Forked worlds share no
mutable cell, so isolation between environments is exactly that of
independent full loads. `Environment::with_stdlib_fresh()` bypasses the
prototype and runs the loader directly (the CLI uses it, since it builds
exactly one environment per process). `with_prelude()` /
`with_prelude_fresh()` mirror the same split.

**Consequence:** The stdlib is always exactly what was compiled in. Users cannot replace stdlib files at runtime unless they call `load_file()` explicitly afterward (which can shadow definitions).

---

## Stack Size and Threading

The evaluator recurses in Rust for non-tail calls. With the default system stack (~2 MiB for most platforms), even moderate Lisp recursion hits a native stack overflow before the depth guard fires.

The solution is running on a large-stack thread:

```rust
pub const INTERPRETER_STACK_SIZE: usize = 512 * 1024 * 1024;  // 512 MiB

pub fn with_large_stack<F, T>(f: F) -> T
where F: FnOnce() -> T + Send, T: Send
{
    std::thread::Builder::new()
        .stack_size(INTERPRETER_STACK_SIZE)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}
```

**Important constraint:** `LispVal` and `Environment` are `!Send` (they use `Rc`, not `Arc`). You must create the environment _inside_ the `with_large_stack` closure. The interpreter is single-threaded by design.

---

## Builtin Registration

Builtins are registered in `Environment::new_with_builtins()` as `LispVal::Builtin(BuiltinFunc)`:

```rust
pub type BuiltinFunc = fn(&[LispVal], &Rc<Environment>) -> Result<LispVal, LispError>;

fn builtin_car(args: &[LispVal], _env: &Rc<Environment>) -> Result<LispVal, LispError> {
    match args {
        [LispVal::Cons { car, .. }] => Ok((**car).clone()),
        [LispVal::Nil]              => Ok(LispVal::Nil),
        _ => Err(LispError::Generic("car: expected cons or nil".into())),
    }
}

// Registration in new_with_builtins():
env.set("CAR", LispVal::Builtin(builtin_car));
```

Native functions registered by embedders use `Rc<NativeFn>` instead:

```rust
pub type NativeFn = dyn Fn(&[LispVal], &Rc<Environment>) -> Result<LispVal, LispError>;

pub fn register_fn(&self, name: &str, f: impl Fn(&[LispVal], &Rc<Environment>) -> Result<LispVal, LispError> + 'static) {
    self.set(&name.to_uppercase(), LispVal::Native(Rc::new(f)));
}
```

The difference: `BuiltinFunc` is a function pointer (zero-size, copiable), `NativeFn` is a trait object that can capture state via closure.

---

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Symbol lookup | O(depth) | Environment chain depth |
| Symbol interning | O(n) amortized | HashMap in SymbolTable |
| `EQ` | O(1) | Pointer equality |
| `EQUAL` | O(size) | Recursive tree walk |
| List clone | O(1) | Rc refcount bump |
| Cons car/cdr | O(1) | Direct field access |
| `ASSOC` | O(n) | Linear alist scan |
| `GETHASH` | O(1) amortized | HashMap |
| Function call | O(params) | Bind params, extend env |
| Macro expansion | O(body) | Eval body, re-eval result |
| Tail call | O(1) stack | Trampoline reuses frame |

**Bottlenecks:**
1. Deep environment chains — long closure chains slow lookup
2. Large alist scans — use hash tables for O(1) lookup when size is non-trivial
3. Macro expansion — each call re-evaluates the macro body; no caching

---

## Adding a New Builtin

1. Write the function in `src/evaluator.rs`:

```rust
fn builtin_my_fn(args: &[LispVal], env: &Rc<Environment>) -> Result<LispVal, LispError> {
    // Validate args
    if args.len() != 1 {
        return Err(LispError::Generic("my-fn: expected 1 argument".into()));
    }
    // Extract value
    let n = match &args[0] {
        LispVal::Number(n) => *n,
        _ => return Err(LispError::Generic("my-fn: expected number".into())),
    };
    // Return result
    Ok(LispVal::Number(n * 2))
}
```

2. Register it in `Environment::new_with_builtins()`:

```rust
env.set("MY-FN", LispVal::Builtin(builtin_my_fn));
```

3. Add a structured help record in `lib/99-help-data.lisp`:

```lisp
(register-doc 'my-fn
  (list
    (cons 'NAME 'my-fn)
    (cons 'DESCRIPTION "Double a number.")))
```

4. Write tests in `tests/` and a test in `tests/lisp/`.

---

## Adding a New Special Form

Special forms are handled directly in `eval_step()` in `src/evaluator.rs`. Add a new match arm:

```rust
"MY-FORM" => {
    let args = collect_list_args(&rest)?;
    // args[0], args[1], ...
    // Return TcoStep::Done(result) or TcoStep::TailCall(expr, env)
    Ok(TcoStep::Done(result))
}
```

Special forms differ from builtins in that they receive **unevaluated** arguments (the raw `LispVal` cons cells from the reader). If your form needs to evaluate some arguments, call `eval(arg, env)?` explicitly.

---

*See also: [Embedding Lamedh](embedding.md), [Special Forms](special_forms.md)*
