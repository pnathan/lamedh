//! Typed JIT, stage 1: pre-runtime monomorphic type checking + closure compilation.
//!
//! This is the first *working* slice of the design in `docs/typed-jit-design.md`.
//! It implements the parts that need no native-code backend yet:
//!
//! 1. **The type membrane.** `(deffun-typed (name ret) ((arg ty)...) body...)` is
//!    elaborated with a monomorphic bidirectional checker that runs *before*
//!    runtime and rejects ill-typed definitions. Elaboration and type checking are
//!    one pass (the Turnstile idea): [`elaborate`] returns both the typed [`Core`]
//!    and its [`Ty`].
//! 2. **A basic compile.** [`compile`] lowers the typed core to a tree of Rust
//!    closures over *unboxed* `i64` slots — no `LispVal` tags, no `Rc`, no
//!    per-node match dispatch at call time. This is the "stage 2.5" backend; a
//!    native Cranelift edition slots in behind the same [`TypedFn`] interface
//!    later (a `jit` cargo feature), per the design doc.
//! 3. **Interpret-or-compiled dispatch.** A call goes through a [`TypedFn`] cell
//!    that holds the interpreter (always correct) and an optional compiled
//!    edition, and picks at call time. Redefinition hot-swaps the edition; a call
//!    pins the current edition (clones the `Rc`) for its duration, so a swapped-out
//!    edition stays alive until in-flight callers return — the cell discipline
//!    from the design doc, in single-threaded `Rc` form (the `Arc`/`ArcSwap`
//!    upgrade is #108).
//!
//! Supported typed core: `int64`/`bool`, `+ - *`, `< > =`, `if`, and `let-typed`.
//! The boxed `LispVal` membrane ([`TypedFn::call_boxed`]) is `int64`-only for now.
//!
//! Arithmetic wraps (`wrapping_*`) rather than erroring on overflow; this diverges
//! from the checked tree-walker and will be revisited when the typed core grows a
//! proper overflow story (#67).

use crate::LispVal;
use std::cell::RefCell;
use std::rc::Rc;

/// A monomorphic type in the typed core.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ty {
    Int64,
    Bool,
}

impl Ty {
    /// Parse a type name as written in source (symbols are upper-cased by the reader).
    fn parse(name: &str) -> Option<Ty> {
        match name {
            "INT64" => Some(Ty::Int64),
            "BOOL" => Some(Ty::Bool),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
}

#[derive(Clone, Copy, Debug)]
pub enum CmpOp {
    Lt,
    Gt,
    Eq,
}

/// The typed core IR. Variables are resolved to fixed slot indices at elaboration
/// time, so neither the interpreter nor the compiler does any name lookup.
#[derive(Clone, Debug)]
pub enum Core {
    Lit(i64),
    Var(usize),
    Bin(BinOp, Box<Core>, Box<Core>),
    Cmp(CmpOp, Box<Core>, Box<Core>),
    If(Box<Core>, Box<Core>, Box<Core>),
    /// `Let(slot, init, body)`: evaluate `init`, store it in `slot`, evaluate `body`.
    /// Slots are fixed homes (not a growable stack), so mutually-exclusive branches
    /// may reuse a slot and nested bindings never collide.
    Let(usize, Box<Core>, Box<Core>),
}

// ---------------------------------------------------------------------------
// Elaboration = lowering + monomorphic type checking, in one pass.
// ---------------------------------------------------------------------------

/// Lexical scope: name → type, vector index is the runtime slot.
type Scope = Vec<(String, Ty)>;

/// Elaborate one source form into typed core, threading the lexical `scope` and
/// the high-water `max_slots` (the size the runtime slot vector must have).
/// Returns the core and its synthesized type, or a type error.
fn elaborate(
    form: &LispVal,
    scope: &mut Scope,
    max_slots: &mut usize,
) -> Result<(Core, Ty), String> {
    match form {
        LispVal::Number(n) => Ok((Core::Lit(*n), Ty::Int64)),
        LispVal::Symbol(s) => {
            let name = &s.borrow().name;
            // Search innermost-first so shadowing works.
            match scope.iter().rposition(|(n, _)| n == name) {
                Some(slot) => Ok((Core::Var(slot), scope[slot].1)),
                None => Err(format!("unbound variable: {name}")),
            }
        }
        LispVal::Cons { .. } => {
            let items = list_to_vec(form);
            let head = match items.first() {
                Some(LispVal::Symbol(s)) => s.borrow().name.clone(),
                _ => return Err("typed core: call head must be a symbol".to_string()),
            };
            let args = &items[1..];
            match head.as_str() {
                "+" | "-" | "*" => elaborate_bin(&head, args, scope, max_slots),
                "<" | ">" | "=" => elaborate_cmp(&head, args, scope, max_slots),
                "IF" => elaborate_if(args, scope, max_slots),
                "LET-TYPED" => elaborate_let(args, scope, max_slots),
                other => Err(format!("typed core: unsupported operator `{other}`")),
            }
        }
        other => Err(format!("typed core: unsupported literal {other:?}")),
    }
}

fn elaborate_bin(
    op: &str,
    args: &[LispVal],
    scope: &mut Scope,
    max: &mut usize,
) -> Result<(Core, Ty), String> {
    if args.len() != 2 {
        return Err(format!("`{op}` expects 2 args, got {}", args.len()));
    }
    let (a, ta) = elaborate(&args[0], scope, max)?;
    let (b, tb) = elaborate(&args[1], scope, max)?;
    if ta != Ty::Int64 || tb != Ty::Int64 {
        return Err(format!(
            "`{op}` expects int64 operands, got {ta:?} and {tb:?}"
        ));
    }
    let op = match op {
        "+" => BinOp::Add,
        "-" => BinOp::Sub,
        _ => BinOp::Mul,
    };
    Ok((Core::Bin(op, Box::new(a), Box::new(b)), Ty::Int64))
}

fn elaborate_cmp(
    op: &str,
    args: &[LispVal],
    scope: &mut Scope,
    max: &mut usize,
) -> Result<(Core, Ty), String> {
    if args.len() != 2 {
        return Err(format!("`{op}` expects 2 args, got {}", args.len()));
    }
    let (a, ta) = elaborate(&args[0], scope, max)?;
    let (b, tb) = elaborate(&args[1], scope, max)?;
    if ta != Ty::Int64 || tb != Ty::Int64 {
        return Err(format!(
            "`{op}` expects int64 operands, got {ta:?} and {tb:?}"
        ));
    }
    let op = match op {
        "<" => CmpOp::Lt,
        ">" => CmpOp::Gt,
        _ => CmpOp::Eq,
    };
    Ok((Core::Cmp(op, Box::new(a), Box::new(b)), Ty::Bool))
}

fn elaborate_if(
    args: &[LispVal],
    scope: &mut Scope,
    max: &mut usize,
) -> Result<(Core, Ty), String> {
    if args.len() != 3 {
        return Err(format!(
            "`if` expects (if cond then else), got {} args",
            args.len()
        ));
    }
    let (c, tc) = elaborate(&args[0], scope, max)?;
    if tc != Ty::Bool {
        return Err(format!("`if` condition must be bool, got {tc:?}"));
    }
    let saved = scope.len();
    let (t, tt) = elaborate(&args[1], scope, max)?;
    scope.truncate(saved);
    let (e, te) = elaborate(&args[2], scope, max)?;
    scope.truncate(saved);
    if tt != te {
        return Err(format!("`if` branches disagree: {tt:?} vs {te:?}"));
    }
    Ok((Core::If(Box::new(c), Box::new(t), Box::new(e)), tt))
}

/// `(let-typed ((name ty init) ...) body...)` — sequential bindings, last form is
/// the value. Bindings are lexically scoped to the body.
fn elaborate_let(
    args: &[LispVal],
    scope: &mut Scope,
    max: &mut usize,
) -> Result<(Core, Ty), String> {
    let bindings = args
        .first()
        .map(list_to_vec)
        .ok_or_else(|| "`let-typed` needs a binding list".to_string())?;
    let body = &args[1..];
    if body.is_empty() {
        return Err("`let-typed` needs a body".to_string());
    }

    let saved = scope.len();
    let mut writes: Vec<(usize, Core)> = Vec::with_capacity(bindings.len());
    for b in &bindings {
        let parts = list_to_vec(b);
        let (name, ty_sym, init) = match parts.as_slice() {
            [LispVal::Symbol(n), LispVal::Symbol(t), init] => {
                (n.borrow().name.clone(), t.borrow().name.clone(), init)
            }
            _ => return Err("`let-typed` binding must be (name type init)".to_string()),
        };
        let ty = Ty::parse(&ty_sym).ok_or_else(|| format!("unknown type `{ty_sym}`"))?;
        let (init_core, init_ty) = elaborate(init, scope, max)?;
        if init_ty != ty {
            return Err(format!(
                "binding `{name}` declared {ty:?} but init is {init_ty:?}"
            ));
        }
        let slot = scope.len();
        scope.push((name, ty));
        *max = (*max).max(scope.len());
        writes.push((slot, init_core));
    }

    let (body_core, body_ty) = elaborate_body(body, scope, max)?;
    scope.truncate(saved);

    // Fold the bindings outward so the first binding is the outermost Let.
    let mut acc = body_core;
    for (slot, init_core) in writes.into_iter().rev() {
        acc = Core::Let(slot, Box::new(init_core), Box::new(acc));
    }
    Ok((acc, body_ty))
}

/// Elaborate a body sequence; every form is type-checked, the last is the value.
fn elaborate_body(
    forms: &[LispVal],
    scope: &mut Scope,
    max: &mut usize,
) -> Result<(Core, Ty), String> {
    let mut last = None;
    for f in forms {
        last = Some(elaborate(f, scope, max)?);
    }
    last.ok_or_else(|| "empty body".to_string())
}

// ---------------------------------------------------------------------------
// Interpreter over the typed core (the always-correct fallback edition).
// ---------------------------------------------------------------------------

fn eval_core(core: &Core, env: &mut [i64]) -> i64 {
    match core {
        Core::Lit(n) => *n,
        Core::Var(i) => env[*i],
        Core::Bin(op, a, b) => {
            let (x, y) = (eval_core(a, env), eval_core(b, env));
            match op {
                BinOp::Add => x.wrapping_add(y),
                BinOp::Sub => x.wrapping_sub(y),
                BinOp::Mul => x.wrapping_mul(y),
            }
        }
        Core::Cmp(op, a, b) => {
            let (x, y) = (eval_core(a, env), eval_core(b, env));
            let r = match op {
                CmpOp::Lt => x < y,
                CmpOp::Gt => x > y,
                CmpOp::Eq => x == y,
            };
            r as i64
        }
        Core::If(c, t, e) => {
            if eval_core(c, env) != 0 {
                eval_core(t, env)
            } else {
                eval_core(e, env)
            }
        }
        Core::Let(slot, init, body) => {
            let v = eval_core(init, env);
            env[*slot] = v;
            eval_core(body, env)
        }
    }
}

// ---------------------------------------------------------------------------
// The "basic compile": typed core -> tree of unboxed i64 closures.
// ---------------------------------------------------------------------------

/// A compiled edition: a closure over an unboxed slot vector. `Rc` so a call can
/// pin the edition while a redefinition swaps in a new one.
pub type Compiled = Rc<dyn Fn(&mut [i64]) -> i64>;

/// Lower the typed core to a closure. Each node becomes a closure that captures
/// its already-compiled children — removing the per-node `match` the interpreter
/// pays on every call, and operating entirely on raw `i64`.
pub fn compile(core: &Core) -> Compiled {
    match core {
        Core::Lit(n) => {
            let n = *n;
            Rc::new(move |_env| n)
        }
        Core::Var(i) => {
            let i = *i;
            Rc::new(move |env| env[i])
        }
        Core::Bin(op, a, b) => {
            let (ca, cb, op) = (compile(a), compile(b), *op);
            Rc::new(move |env| {
                let (x, y) = (ca(env), cb(env));
                match op {
                    BinOp::Add => x.wrapping_add(y),
                    BinOp::Sub => x.wrapping_sub(y),
                    BinOp::Mul => x.wrapping_mul(y),
                }
            })
        }
        Core::Cmp(op, a, b) => {
            let (ca, cb, op) = (compile(a), compile(b), *op);
            Rc::new(move |env| {
                let (x, y) = (ca(env), cb(env));
                let r = match op {
                    CmpOp::Lt => x < y,
                    CmpOp::Gt => x > y,
                    CmpOp::Eq => x == y,
                };
                r as i64
            })
        }
        Core::If(c, t, e) => {
            let (cc, ct, ce) = (compile(c), compile(t), compile(e));
            Rc::new(move |env| if cc(env) != 0 { ct(env) } else { ce(env) })
        }
        Core::Let(slot, init, body) => {
            let (slot, ci, cb) = (*slot, compile(init), compile(body));
            Rc::new(move |env| {
                let v = ci(env);
                env[slot] = v;
                cb(env)
            })
        }
    }
}

// ---------------------------------------------------------------------------
// The function cell: interpret-or-compiled dispatch + redefinition.
// ---------------------------------------------------------------------------

/// A typed function: its HM-checked signature (the ABI), the typed core (the
/// reference interpreter), and an optional hot-swappable compiled edition.
pub struct TypedFn {
    pub name: String,
    pub params: Vec<(String, Ty)>,
    pub ret: Ty,
    core: Core,
    slots: usize,
    compiled: RefCell<Option<Compiled>>,
    generation: RefCell<u64>,
}

impl std::fmt::Debug for TypedFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedFn")
            .field("name", &self.name)
            .field("params", &self.params)
            .field("ret", &self.ret)
            .field("compiled", &self.is_compiled())
            .field("generation", &self.generation())
            .finish()
    }
}

impl TypedFn {
    /// Type-check and build a function from a `(deffun-typed ...)` form. The
    /// function is interpretable immediately; call [`TypedFn::compile_now`] (or
    /// rely on eager compile by the caller) to install a compiled edition.
    pub fn from_form(form: &LispVal) -> Result<TypedFn, String> {
        let items = list_to_vec(form);
        match items.first() {
            Some(LispVal::Symbol(s)) if s.borrow().name == "DEFFUN-TYPED" => {}
            _ => return Err("expected a (deffun-typed ...) form".to_string()),
        }
        if items.len() < 4 {
            return Err(
                "deffun-typed: (deffun-typed (name ret) ((arg ty)...) body...)".to_string(),
            );
        }

        // (name ret)
        let sig = list_to_vec(&items[1]);
        let (name, ret) = match sig.as_slice() {
            [LispVal::Symbol(n), LispVal::Symbol(r)] => {
                let ret = Ty::parse(&r.borrow().name)
                    .ok_or_else(|| format!("unknown return type `{}`", r.borrow().name))?;
                (n.borrow().name.clone(), ret)
            }
            _ => return Err("deffun-typed: signature must be (name return-type)".to_string()),
        };

        // ((arg ty) ...)
        let mut params = Vec::new();
        let mut scope: Scope = Vec::new();
        for p in list_to_vec(&items[2]) {
            let parts = list_to_vec(&p);
            match parts.as_slice() {
                [LispVal::Symbol(a), LispVal::Symbol(t)] => {
                    let ty = Ty::parse(&t.borrow().name)
                        .ok_or_else(|| format!("unknown param type `{}`", t.borrow().name))?;
                    let aname = a.borrow().name.clone();
                    params.push((aname.clone(), ty));
                    scope.push((aname, ty));
                }
                _ => return Err("deffun-typed: each parameter must be (name type)".to_string()),
            }
        }

        let mut max_slots = scope.len();
        let (core, body_ty) = elaborate_body(&items[3..], &mut scope, &mut max_slots)?;
        if body_ty != ret {
            return Err(format!(
                "{name}: declared return {ret:?} but body has type {body_ty:?}"
            ));
        }

        Ok(TypedFn {
            name,
            params,
            ret,
            core,
            slots: max_slots,
            compiled: RefCell::new(None),
            generation: RefCell::new(0),
        })
    }

    /// Install (or replace) the compiled edition. Eager/AOT: the type is proven,
    /// so there is no call-count warmup. The previous edition's `Rc` is dropped
    /// here, but any in-flight call holds its own clone and is unaffected.
    pub fn compile_now(&self) {
        *self.compiled.borrow_mut() = Some(compile(&self.core));
        *self.generation.borrow_mut() += 1;
    }

    /// Drop the compiled edition; subsequent calls fall back to the interpreter.
    pub fn deoptimize(&self) {
        *self.compiled.borrow_mut() = None;
    }

    pub fn is_compiled(&self) -> bool {
        self.compiled.borrow().is_some()
    }

    pub fn generation(&self) -> u64 {
        *self.generation.borrow()
    }

    /// A call is a runtime dispatch: compiled edition if present, else interpret.
    /// The edition is *pinned* (the `Rc` is cloned out and the cell borrow
    /// released) before we run it, so a concurrent redefinition could not free
    /// code out from under an in-flight call.
    pub fn call(&self, args: &[i64]) -> i64 {
        let mut env = vec![0i64; self.slots];
        env[..args.len()].copy_from_slice(args);
        let edition = self.compiled.borrow().clone();
        match edition {
            Some(f) => f(&mut env),
            None => eval_core(&self.core, &mut env),
        }
    }

    /// The boxed `LispVal` membrane: unbox `Number` args → run → re-box the
    /// result. This is the contract coercion / calling-convention adapter that the
    /// design doc identifies as the native↔interpreter boundary. `int64`-only for
    /// now.
    pub fn call_boxed(&self, args: &[LispVal]) -> Result<LispVal, String> {
        if self.ret != Ty::Int64 || self.params.iter().any(|(_, t)| *t != Ty::Int64) {
            return Err("call_boxed: only all-int64 signatures are supported for now".to_string());
        }
        if args.len() != self.params.len() {
            return Err(format!(
                "{}: expected {} args, got {}",
                self.name,
                self.params.len(),
                args.len()
            ));
        }
        let mut unboxed = Vec::with_capacity(args.len());
        for (i, a) in args.iter().enumerate() {
            match a {
                LispVal::Number(n) => unboxed.push(*n),
                other => {
                    return Err(format!(
                        "{}: arg {i} must be a number, got {other:?}",
                        self.name
                    ));
                }
            }
        }
        Ok(LispVal::Number(self.call(&unboxed)))
    }
}

/// Collect a proper list into a vector (improper tails are ignored). Local copy
/// so the JIT module stays decoupled from the evaluator's private helper.
fn list_to_vec(list: &LispVal) -> Vec<LispVal> {
    let mut out = Vec::new();
    let mut cur = list;
    while let LispVal::Cons { car, cdr } = cur {
        out.push(car.as_ref().clone());
        cur = cdr;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Environment;
    use crate::reader::read;

    fn build(src: &str) -> Result<TypedFn, String> {
        let env = Environment::new_with_builtins();
        let form = read(src, &env)?;
        TypedFn::from_form(&form)
    }

    #[test]
    fn sq_typechecks_and_both_paths_agree() {
        let f = build("(deffun-typed (sq int64) ((x int64)) (* x x))").unwrap();
        assert_eq!(f.ret, Ty::Int64);
        // Interpreter edition.
        assert!(!f.is_compiled());
        assert_eq!(f.call(&[7]), 49);
        // Compiled edition — identical result.
        f.compile_now();
        assert!(f.is_compiled());
        assert_eq!(f.call(&[7]), 49);
        assert_eq!(f.call(&[-9]), 81);
    }

    #[test]
    fn boxed_membrane_roundtrips() {
        let f = build("(deffun-typed (add int64) ((x int64) (y int64)) (+ x y))").unwrap();
        f.compile_now();
        let r = f
            .call_boxed(&[LispVal::Number(20), LispVal::Number(22)])
            .unwrap();
        assert_eq!(r, LispVal::Number(42));
    }

    #[test]
    fn if_and_comparison() {
        let f = build("(deffun-typed (absish int64) ((x int64)) (if (< x 0) (- 0 x) x))").unwrap();
        f.compile_now();
        assert_eq!(f.call(&[-5]), 5);
        assert_eq!(f.call(&[5]), 5);
    }

    #[test]
    fn let_typed_binds_and_scopes() {
        let f = build(
            "(deffun-typed (poly int64) ((x int64)) (let-typed ((y int64 (* x x))) (+ y x)))",
        )
        .unwrap();
        f.compile_now();
        // x=3 -> y=9 -> 9+3 = 12
        assert_eq!(f.call(&[3]), 12);
        assert_eq!(f.call(&[3]), eval_core(&f.core, &mut [3, 0]));
    }

    #[test]
    fn nested_let_in_branch_does_not_corrupt_outer_slot() {
        // The inner binding inside the `if` reuses a slot; the outer `a` must
        // still read its own value (regression for fixed-slot scoping).
        let src = "(deffun-typed (f int64) ((c int64)) \
                   (let-typed ((a int64 (if (> c 0) (let-typed ((tmp int64 1)) tmp) 0))) \
                     (+ a 100)))";
        let f = build(src).unwrap();
        f.compile_now();
        assert_eq!(f.call(&[5]), 101); // a = 1
        assert_eq!(f.call(&[-5]), 100); // a = 0
    }

    #[test]
    fn redefinition_swaps_edition_and_pins_in_flight() {
        let f = build("(deffun-typed (sq int64) ((x int64)) (* x x))").unwrap();
        f.compile_now();
        let g0 = f.generation();
        let pinned = f.compiled.borrow().clone().unwrap(); // an in-flight caller's pinned edition
        assert_eq!(pinned(&mut [6]), 36);

        // "Redefine" by swapping in a new edition (cube). The pinned old edition
        // is still valid and reachable until dropped.
        let cube = build("(deffun-typed (sq int64) ((x int64)) (* x (* x x)))").unwrap();
        *f.compiled.borrow_mut() = Some(compile(&cube.core));
        *f.generation.borrow_mut() += 1;
        assert!(f.generation() > g0);
        assert_eq!(pinned(&mut [6]), 36); // old edition unchanged
        assert_eq!(f.compiled.borrow().clone().unwrap()(&mut [6]), 216); // new edition
    }

    #[test]
    fn ill_typed_is_rejected() {
        // `<` returns bool; multiplying a bool is a type error caught pre-runtime.
        let err = build("(deffun-typed (bad int64) ((x int64)) (* (< x 1) x))").unwrap_err();
        assert!(err.contains("int64 operands"), "got: {err}");
    }

    #[test]
    fn return_type_mismatch_is_rejected() {
        let err = build("(deffun-typed (bad int64) ((x int64)) (< x 1))").unwrap_err();
        assert!(err.contains("declared return"), "got: {err}");
    }

    #[test]
    fn unbound_variable_is_rejected() {
        let err = build("(deffun-typed (bad int64) ((x int64)) (+ x y))").unwrap_err();
        assert!(err.contains("unbound"), "got: {err}");
    }
}
