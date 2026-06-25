//! Typed JIT prototype: pre-runtime monomorphic type checking + closure compilation.
//!
//! Working slice of `docs/typed-jit-design.md` that needs no native-code backend
//! (no external deps; Cranelift slots in behind the same [`TypedFn`] interface
//! later, as a `jit` cargo feature).
//!
//! ## What works
//! - **Type membrane.** `(deffun-typed (name ret) ((arg ty)...) body...)` is
//!   elaborated by a monomorphic bidirectional checker that runs *before* runtime
//!   and rejects ill-typed definitions. Elaboration *is* type checking
//!   (Turnstile-style): [`Cx::elab`] returns the typed [`Core`] and its [`Ty`].
//! - **Basic compile.** [`compile`] lowers the typed core to a tree of closures
//!   over *unboxed* machine words. Runtime values are raw `u64`s: `int64` is the
//!   word, `float64` is `f64::to_bits`, `bool` is `0`/`1`. The static type tells
//!   each node how to read its word, so there is no tag and no `Rc` in the hot
//!   path.
//! - **Calls + recursion.** A [`Jit`] registry gives every function a stable id;
//!   calls go through the registry cell (design policy (a)), so self-recursion,
//!   cross-function calls, and — via [`Jit::declare`] — mutual recursion all work,
//!   and redefining a callee is just an edition swap.
//! - **Interpret-or-compiled dispatch + redefinition.** A call picks the compiled
//!   edition if present, else interprets; a call pins (`Rc`-clones) the edition it
//!   runs, so a swapped-out edition survives until in-flight callers return (the
//!   `Arc`/`ArcSwap` upgrade is #108).
//!
//! Core: `int64`/`float64`/`bool`; `+ - * / mod` and comparisons `< > <= >= = /=`
//! (operand-type directed), `and`/`or`/`not`, `if`, `let-typed`, and calls.
//! Integer arithmetic wraps and integer `/`,`mod` by zero yield `0` (no panics);
//! this diverges from the checked tree-walker and is revisited with #67.

use crate::LispVal;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

#[cfg(feature = "jit")]
mod native;

// ---------------------------------------------------------------------------
// Types and runtime values.
// ---------------------------------------------------------------------------

/// A monomorphic type in the typed core.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ty {
    Int64,
    Float64,
    Bool,
}

impl Ty {
    fn parse(name: &str) -> Option<Ty> {
        match name {
            "INT64" => Some(Ty::Int64),
            "FLOAT64" => Some(Ty::Float64),
            "BOOL" => Some(Ty::Bool),
            _ => None,
        }
    }

    fn as_num(self) -> Option<NumTy> {
        match self {
            Ty::Int64 => Some(NumTy::I),
            Ty::Float64 => Some(NumTy::F),
            Ty::Bool => None,
        }
    }
}

/// Which machine interpretation a numeric op uses on its `u64` words.
#[derive(Clone, Copy, Debug)]
enum NumTy {
    I,
    F,
}

/// A boxed value at the public boundary (the unboxed runtime uses raw `u64`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl Value {
    fn to_word(self, ty: Ty) -> Result<u64, String> {
        match (self, ty) {
            (Value::Int(n), Ty::Int64) => Ok(n as u64),
            (Value::Float(f), Ty::Float64) => Ok(f.to_bits()),
            (Value::Bool(b), Ty::Bool) => Ok(b as u64),
            _ => Err(format!("value {self:?} does not match type {ty:?}")),
        }
    }

    fn from_word(w: u64, ty: Ty) -> Value {
        match ty {
            Ty::Int64 => Value::Int(w as i64),
            Ty::Float64 => Value::Float(f64::from_bits(w)),
            Ty::Bool => Value::Bool(w != 0),
        }
    }
}

#[inline]
fn as_i(w: u64) -> i64 {
    w as i64
}
#[inline]
fn from_i(x: i64) -> u64 {
    x as u64
}
#[inline]
fn as_f(w: u64) -> f64 {
    f64::from_bits(w)
}
#[inline]
fn from_f(x: f64) -> u64 {
    x.to_bits()
}

#[derive(Clone, Copy, Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Clone, Copy, Debug)]
pub enum CmpOp {
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
}

/// The typed core IR. Variables are fixed slot indices; calls are function ids.
#[derive(Clone, Debug)]
pub enum Core {
    LitI(i64),
    LitF(f64),
    Var(usize),
    Bin(NumKind, BinOp, Box<Core>, Box<Core>),
    Cmp(NumKind, CmpOp, Box<Core>, Box<Core>),
    Not(Box<Core>),
    And(Box<Core>, Box<Core>),
    Or(Box<Core>, Box<Core>),
    If(Box<Core>, Box<Core>, Box<Core>),
    Let(usize, Box<Core>, Box<Core>),
    Call(usize, Vec<Core>),
}

/// Public mirror of [`NumTy`] so [`Core`] can derive `Debug`/`Clone` cleanly.
#[derive(Clone, Copy, Debug)]
pub enum NumKind {
    I,
    F,
}
impl From<NumTy> for NumKind {
    fn from(n: NumTy) -> Self {
        match n {
            NumTy::I => NumKind::I,
            NumTy::F => NumKind::F,
        }
    }
}

// ---------------------------------------------------------------------------
// Elaboration = lowering + monomorphic type checking, in one pass.
// ---------------------------------------------------------------------------

type Scope = Vec<(String, Ty)>;

/// Elaboration context: read-only access to the signatures of all registered
/// functions, so call sites can be type-checked (and self/forward references
/// resolved) while a body is being elaborated.
struct Cx<'a> {
    funcs: &'a [Rc<TypedFn>],
    by_name: &'a HashMap<String, usize>,
}

impl Cx<'_> {
    fn elab(
        &self,
        form: &LispVal,
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        match form {
            LispVal::Number(n) => Ok((Core::LitI(*n), Ty::Int64)),
            LispVal::Float(f) => Ok((Core::LitF(*f), Ty::Float64)),
            LispVal::Symbol(s) => {
                let name = &s.borrow().name;
                if name == "TRUE" {
                    return Ok((Core::LitI(1), Ty::Bool));
                }
                if name == "FALSE" {
                    return Ok((Core::LitI(0), Ty::Bool));
                }
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
                    "+" | "-" | "*" | "/" | "MOD" => self.elab_bin(&head, args, scope, max),
                    "<" | ">" | "<=" | ">=" | "=" | "/=" => self.elab_cmp(&head, args, scope, max),
                    "NOT" => self.elab_not(args, scope, max),
                    "AND" | "OR" => self.elab_logic(&head, args, scope, max),
                    "IF" => self.elab_if(args, scope, max),
                    "LET-TYPED" => self.elab_let(args, scope, max),
                    _ => self.elab_call(&head, args, scope, max),
                }
            }
            other => Err(format!("typed core: unsupported literal {other:?}")),
        }
    }

    fn elab_bin(
        &self,
        op: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`{op}` expects 2 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (b, tb) = self.elab(&args[1], scope, max)?;
        if ta != tb {
            return Err(format!("`{op}` operands disagree: {ta:?} vs {tb:?}"));
        }
        let num = ta
            .as_num()
            .ok_or_else(|| format!("`{op}` expects numeric operands, got {ta:?}"))?;
        let bop = match op {
            "+" => BinOp::Add,
            "-" => BinOp::Sub,
            "*" => BinOp::Mul,
            "/" => BinOp::Div,
            _ => BinOp::Mod,
        };
        if matches!(bop, BinOp::Mod) && !matches!(num, NumTy::I) {
            return Err("`mod` is int64-only".to_string());
        }
        Ok((Core::Bin(num.into(), bop, Box::new(a), Box::new(b)), ta))
    }

    fn elab_cmp(
        &self,
        op: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`{op}` expects 2 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (b, tb) = self.elab(&args[1], scope, max)?;
        if ta != tb {
            return Err(format!("`{op}` operands disagree: {ta:?} vs {tb:?}"));
        }
        let num = ta
            .as_num()
            .ok_or_else(|| format!("`{op}` expects numeric operands, got {ta:?}"))?;
        let cop = match op {
            "<" => CmpOp::Lt,
            ">" => CmpOp::Gt,
            "<=" => CmpOp::Le,
            ">=" => CmpOp::Ge,
            "=" => CmpOp::Eq,
            _ => CmpOp::Ne,
        };
        Ok((
            Core::Cmp(num.into(), cop, Box::new(a), Box::new(b)),
            Ty::Bool,
        ))
    }

    fn elab_not(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`not` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        if ta != Ty::Bool {
            return Err(format!("`not` expects bool, got {ta:?}"));
        }
        Ok((Core::Not(Box::new(a)), Ty::Bool))
    }

    fn elab_logic(
        &self,
        op: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`{op}` expects 2 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (b, tb) = self.elab(&args[1], scope, max)?;
        if ta != Ty::Bool || tb != Ty::Bool {
            return Err(format!(
                "`{op}` expects bool operands, got {ta:?} and {tb:?}"
            ));
        }
        let node = if op == "AND" {
            Core::And(Box::new(a), Box::new(b))
        } else {
            Core::Or(Box::new(a), Box::new(b))
        };
        Ok((node, Ty::Bool))
    }

    fn elab_if(
        &self,
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
        let (c, tc) = self.elab(&args[0], scope, max)?;
        if tc != Ty::Bool {
            return Err(format!("`if` condition must be bool, got {tc:?}"));
        }
        let saved = scope.len();
        let (t, tt) = self.elab(&args[1], scope, max)?;
        scope.truncate(saved);
        let (e, te) = self.elab(&args[2], scope, max)?;
        scope.truncate(saved);
        if tt != te {
            return Err(format!("`if` branches disagree: {tt:?} vs {te:?}"));
        }
        Ok((Core::If(Box::new(c), Box::new(t), Box::new(e)), tt))
    }

    fn elab_let(
        &self,
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
            let (init_core, init_ty) = self.elab(init, scope, max)?;
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
        let (body_core, body_ty) = self.elab_body(body, scope, max)?;
        scope.truncate(saved);
        let mut acc = body_core;
        for (slot, init_core) in writes.into_iter().rev() {
            acc = Core::Let(slot, Box::new(init_core), Box::new(acc));
        }
        Ok((acc, body_ty))
    }

    fn elab_call(
        &self,
        name: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        let id = *self
            .by_name
            .get(name)
            .ok_or_else(|| format!("call to unknown function `{name}`"))?;
        let callee = &self.funcs[id];
        let params = callee.params.borrow().clone();
        let ret = callee.ret.get();
        if args.len() != params.len() {
            return Err(format!(
                "`{name}` expects {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        let mut arg_cores = Vec::with_capacity(args.len());
        for (i, a) in args.iter().enumerate() {
            let (ac, at) = self.elab(a, scope, max)?;
            if at != params[i].1 {
                return Err(format!(
                    "`{name}` arg {i} expects {:?}, got {at:?}",
                    params[i].1
                ));
            }
            arg_cores.push(ac);
        }
        Ok((Core::Call(id, arg_cores), ret))
    }

    fn elab_body(
        &self,
        forms: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        let mut last = None;
        for f in forms {
            last = Some(self.elab(f, scope, max)?);
        }
        last.ok_or_else(|| "empty body".to_string())
    }
}

// ---------------------------------------------------------------------------
// Runtime: interpreter and compiler over unboxed u64 words.
// ---------------------------------------------------------------------------

/// Call context: the function table, so calls dispatch through the registry cell.
pub struct Ctx<'a> {
    funcs: &'a [Rc<TypedFn>],
}

impl Ctx<'_> {
    #[inline]
    fn call(&self, id: usize, args: &[u64]) -> u64 {
        self.funcs[id].invoke(args, self)
    }
}

fn int_bin(op: BinOp, x: i64, y: i64) -> i64 {
    match op {
        BinOp::Add => x.wrapping_add(y),
        BinOp::Sub => x.wrapping_sub(y),
        BinOp::Mul => x.wrapping_mul(y),
        BinOp::Div => x.checked_div(y).unwrap_or(0),
        BinOp::Mod => x.checked_rem(y).unwrap_or(0),
    }
}

fn float_bin(op: BinOp, x: f64, y: f64) -> f64 {
    match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div => x / y,
        BinOp::Mod => x % y,
    }
}

fn int_cmp(op: CmpOp, x: i64, y: i64) -> bool {
    match op {
        CmpOp::Lt => x < y,
        CmpOp::Gt => x > y,
        CmpOp::Le => x <= y,
        CmpOp::Ge => x >= y,
        CmpOp::Eq => x == y,
        CmpOp::Ne => x != y,
    }
}

fn float_cmp(op: CmpOp, x: f64, y: f64) -> bool {
    match op {
        CmpOp::Lt => x < y,
        CmpOp::Gt => x > y,
        CmpOp::Le => x <= y,
        CmpOp::Ge => x >= y,
        CmpOp::Eq => x == y,
        CmpOp::Ne => x != y,
    }
}

fn eval_core(core: &Core, env: &mut [u64], ctx: &Ctx) -> u64 {
    match core {
        Core::LitI(n) => from_i(*n),
        Core::LitF(f) => from_f(*f),
        Core::Var(i) => env[*i],
        Core::Bin(k, op, a, b) => {
            let (x, y) = (eval_core(a, env, ctx), eval_core(b, env, ctx));
            match k {
                NumKind::I => from_i(int_bin(*op, as_i(x), as_i(y))),
                NumKind::F => from_f(float_bin(*op, as_f(x), as_f(y))),
            }
        }
        Core::Cmp(k, op, a, b) => {
            let (x, y) = (eval_core(a, env, ctx), eval_core(b, env, ctx));
            let r = match k {
                NumKind::I => int_cmp(*op, as_i(x), as_i(y)),
                NumKind::F => float_cmp(*op, as_f(x), as_f(y)),
            };
            r as u64
        }
        Core::Not(a) => (eval_core(a, env, ctx) == 0) as u64,
        Core::And(a, b) => {
            if eval_core(a, env, ctx) != 0 {
                (eval_core(b, env, ctx) != 0) as u64
            } else {
                0
            }
        }
        Core::Or(a, b) => {
            if eval_core(a, env, ctx) != 0 {
                1
            } else {
                (eval_core(b, env, ctx) != 0) as u64
            }
        }
        Core::If(c, t, e) => {
            if eval_core(c, env, ctx) != 0 {
                eval_core(t, env, ctx)
            } else {
                eval_core(e, env, ctx)
            }
        }
        Core::Let(slot, init, body) => {
            let v = eval_core(init, env, ctx);
            env[*slot] = v;
            eval_core(body, env, ctx)
        }
        Core::Call(id, args) => {
            let vals: Vec<u64> = args.iter().map(|a| eval_core(a, env, ctx)).collect();
            ctx.call(*id, &vals)
        }
    }
}

/// A compiled edition: a closure over an unboxed slot vector and a call context.
pub type Compiled = Rc<dyn Fn(&mut [u64], &Ctx) -> u64>;

/// Lower typed core to a tree of closures. Each node captures its compiled
/// children, so the per-node `match` the interpreter pays is gone at call time.
pub fn compile(core: &Core) -> Compiled {
    match core {
        Core::LitI(n) => {
            let w = from_i(*n);
            Rc::new(move |_e, _c| w)
        }
        Core::LitF(f) => {
            let w = from_f(*f);
            Rc::new(move |_e, _c| w)
        }
        Core::Var(i) => {
            let i = *i;
            Rc::new(move |e, _c| e[i])
        }
        Core::Bin(k, op, a, b) => {
            let (ca, cb, op) = (compile(a), compile(b), *op);
            match k {
                NumKind::I => {
                    Rc::new(move |e, c| from_i(int_bin(op, as_i(ca(e, c)), as_i(cb(e, c)))))
                }
                NumKind::F => {
                    Rc::new(move |e, c| from_f(float_bin(op, as_f(ca(e, c)), as_f(cb(e, c)))))
                }
            }
        }
        Core::Cmp(k, op, a, b) => {
            let (ca, cb, op) = (compile(a), compile(b), *op);
            match k {
                NumKind::I => {
                    Rc::new(move |e, c| (int_cmp(op, as_i(ca(e, c)), as_i(cb(e, c)))) as u64)
                }
                NumKind::F => {
                    Rc::new(move |e, c| (float_cmp(op, as_f(ca(e, c)), as_f(cb(e, c)))) as u64)
                }
            }
        }
        Core::Not(a) => {
            let ca = compile(a);
            Rc::new(move |e, c| (ca(e, c) == 0) as u64)
        }
        Core::And(a, b) => {
            let (ca, cb) = (compile(a), compile(b));
            Rc::new(move |e, c| {
                if ca(e, c) != 0 {
                    (cb(e, c) != 0) as u64
                } else {
                    0
                }
            })
        }
        Core::Or(a, b) => {
            let (ca, cb) = (compile(a), compile(b));
            Rc::new(move |e, c| {
                if ca(e, c) != 0 {
                    1
                } else {
                    (cb(e, c) != 0) as u64
                }
            })
        }
        Core::If(cnd, t, e) => {
            let (cc, ct, ce) = (compile(cnd), compile(t), compile(e));
            Rc::new(move |env, c| {
                if cc(env, c) != 0 {
                    ct(env, c)
                } else {
                    ce(env, c)
                }
            })
        }
        Core::Let(slot, init, body) => {
            let (slot, ci, cb) = (*slot, compile(init), compile(body));
            Rc::new(move |e, c| {
                let v = ci(e, c);
                e[slot] = v;
                cb(e, c)
            })
        }
        Core::Call(id, args) => {
            let (id, cargs): (usize, Vec<Compiled>) = (*id, args.iter().map(compile).collect());
            Rc::new(move |e, c| {
                let mut vals = Vec::with_capacity(cargs.len());
                for ca in &cargs {
                    vals.push(ca(e, c));
                }
                c.call(id, &vals)
            })
        }
    }
}

// ---------------------------------------------------------------------------
// The function cell.
// ---------------------------------------------------------------------------

/// A typed function: its signature (the ABI), the typed core (reference
/// interpreter), and an optional hot-swappable compiled edition.
pub struct TypedFn {
    pub name: String,
    params: RefCell<Vec<(String, Ty)>>,
    ret: Cell<Ty>,
    core: RefCell<Option<Core>>,
    slots: Cell<usize>,
    compiled: RefCell<Option<Compiled>>,
    /// Native (Cranelift) edition. Like `compiled`, a call pins (`Rc`-clones) it,
    /// so a redefinition that swaps it out keeps the old code mapped until
    /// in-flight callers return (the `NativeEdition` owns its `JITModule`).
    #[cfg(feature = "jit")]
    native: RefCell<Option<Rc<native::NativeEdition>>>,
    generation: Cell<u64>,
}

impl std::fmt::Debug for TypedFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedFn")
            .field("name", &self.name)
            .field("params", &self.params.borrow())
            .field("ret", &self.ret.get())
            .field("defined", &self.core.borrow().is_some())
            .field("compiled", &self.compiled.borrow().is_some())
            .field("generation", &self.generation.get())
            .finish()
    }
}

impl TypedFn {
    fn placeholder(name: String, params: Vec<(String, Ty)>, ret: Ty) -> TypedFn {
        let slots = params.len();
        TypedFn {
            name,
            params: RefCell::new(params),
            ret: Cell::new(ret),
            core: RefCell::new(None),
            slots: Cell::new(slots),
            compiled: RefCell::new(None),
            #[cfg(feature = "jit")]
            native: RefCell::new(None),
            generation: Cell::new(0),
        }
    }

    pub fn ret(&self) -> Ty {
        self.ret.get()
    }
    pub fn params(&self) -> Vec<(String, Ty)> {
        self.params.borrow().clone()
    }
    pub fn is_compiled(&self) -> bool {
        self.compiled.borrow().is_some()
    }
    pub fn is_defined(&self) -> bool {
        self.core.borrow().is_some()
    }
    pub fn generation(&self) -> u64 {
        self.generation.get()
    }

    fn compile_now(&self) {
        let c = self.core.borrow();
        if let Some(core) = c.as_ref() {
            *self.compiled.borrow_mut() = Some(compile(core));
            // With the `jit` feature, also build a native edition. If Cranelift
            // codegen fails for any reason, fall back to the closure edition
            // rather than failing the definition.
            #[cfg(feature = "jit")]
            {
                let n_params = self.params.borrow().len();
                match native::compile_native(core, n_params, self.slots.get()) {
                    Ok(ed) => *self.native.borrow_mut() = Some(Rc::new(ed)),
                    Err(_) => *self.native.borrow_mut() = None,
                }
            }
            self.generation.set(self.generation.get() + 1);
        }
    }

    fn deoptimize(&self) {
        *self.compiled.borrow_mut() = None;
        #[cfg(feature = "jit")]
        {
            *self.native.borrow_mut() = None;
        }
    }

    /// Invoke with already-unboxed words. Builds the callee frame, dispatches to
    /// the compiled edition if present (pinning it for the call), else interprets.
    fn invoke(&self, args: &[u64], ctx: &Ctx) -> u64 {
        // Native edition first (pinned for the call so a redefinition can't free
        // the code out from under us). `args` are the parameter words directly;
        // the native function builds its own local frame.
        #[cfg(feature = "jit")]
        {
            let native = self.native.borrow().clone();
            if let Some(ed) = native {
                let ctx_ptr = ctx as *const Ctx as *const core::ffi::c_void;
                return unsafe { ed.call(args, ctx_ptr) };
            }
        }
        let mut env = vec![0u64; self.slots.get()];
        env[..args.len()].copy_from_slice(args);
        let edition = self.compiled.borrow().clone();
        match edition {
            Some(f) => f(&mut env, ctx),
            None => {
                let core = self.core.borrow();
                let core = core.as_ref().unwrap_or_else(|| {
                    panic!(
                        "typed function `{}` called before it was defined",
                        self.name
                    )
                });
                eval_core(core, &mut env, ctx)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The registry / front end.
// ---------------------------------------------------------------------------

/// A registry of typed functions, with the define/declare front end. Functions
/// are addressed by a stable id so calls survive redefinition.
#[derive(Default, Debug)]
pub struct Jit {
    funcs: Vec<Rc<TypedFn>>,
    by_name: HashMap<String, usize>,
}

impl Jit {
    pub fn new() -> Jit {
        Jit::default()
    }

    fn intern(&mut self, name: &str, params: Vec<(String, Ty)>, ret: Ty) -> usize {
        if let Some(&id) = self.by_name.get(name) {
            let f = &self.funcs[id];
            *f.params.borrow_mut() = params;
            f.ret.set(ret);
            id
        } else {
            let id = self.funcs.len();
            self.funcs
                .push(Rc::new(TypedFn::placeholder(name.to_string(), params, ret)));
            self.by_name.insert(name.to_string(), id);
            id
        }
    }

    /// Forward-declare a signature so mutually-recursive functions can reference
    /// each other before their bodies exist.
    pub fn declare(&mut self, name: &str, params: &[(&str, Ty)], ret: Ty) -> usize {
        let params = params.iter().map(|(n, t)| ((*n).to_string(), *t)).collect();
        self.intern(name, params, ret)
    }

    /// Type-check and (eagerly) compile a `(deffun-typed ...)` form. Returns the
    /// stable function id.
    pub fn define(&mut self, form: &LispVal) -> Result<usize, String> {
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

        let sig = list_to_vec(&items[1]);
        let (name, ret) = match sig.as_slice() {
            [LispVal::Symbol(n), LispVal::Symbol(r)] => {
                let ret = Ty::parse(&r.borrow().name)
                    .ok_or_else(|| format!("unknown return type `{}`", r.borrow().name))?;
                (n.borrow().name.clone(), ret)
            }
            _ => return Err("deffun-typed: signature must be (name return-type)".to_string()),
        };

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

        // Register the signature *before* elaborating the body so a function can
        // call itself (and any already-declared peer).
        let id = self.intern(&name, params, ret);

        let mut max_slots = scope.len();
        let (core, body_ty) = {
            let cx = Cx {
                funcs: &self.funcs,
                by_name: &self.by_name,
            };
            cx.elab_body(&items[3..], &mut scope, &mut max_slots)?
        };
        if body_ty != ret {
            return Err(format!(
                "{name}: declared return {ret:?} but body has type {body_ty:?}"
            ));
        }

        let f = &self.funcs[id];
        f.slots.set(max_slots);
        *f.core.borrow_mut() = Some(core);
        f.compile_now();
        Ok(id)
    }

    pub fn id(&self, name: &str) -> Option<usize> {
        // Names are case-normalized to uppercase (matching the reader), so
        // callers may use either case.
        self.by_name.get(&name.to_uppercase()).copied()
    }

    /// The (uppercased) name of the function with the given id.
    pub fn name_of(&self, id: usize) -> Option<String> {
        self.funcs.get(id).map(|f| f.name.clone())
    }
    pub fn get(&self, name: &str) -> Option<&Rc<TypedFn>> {
        self.id(name).map(|i| &self.funcs[i])
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx { funcs: &self.funcs }
    }

    /// Call a function by name with boxed [`Value`]s; type-checks the arguments
    /// against the signature and re-boxes the result. This is the public membrane.
    pub fn call(&self, name: &str, args: &[Value]) -> Result<Value, String> {
        let id = self
            .id(name)
            .ok_or_else(|| format!("unknown function `{name}`"))?;
        let f = &self.funcs[id];
        let params = f.params.borrow();
        if args.len() != params.len() {
            return Err(format!(
                "{name}: expected {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        let mut words = Vec::with_capacity(args.len());
        for (a, (_, ty)) in args.iter().zip(params.iter()) {
            words.push(a.to_word(*ty)?);
        }
        drop(params);
        let w = f.invoke(&words, &self.ctx());
        Ok(Value::from_word(w, f.ret.get()))
    }

    /// Convenience for callers holding `LispVal`s: maps `Number`/`Float` to
    /// [`Value`], calls, and re-boxes to `Number`/`Float`/(`Number 0/1` for bool).
    pub fn call_lisp(&self, name: &str, args: &[LispVal]) -> Result<LispVal, String> {
        let mut vals = Vec::with_capacity(args.len());
        for a in args {
            vals.push(match a {
                LispVal::Number(n) => Value::Int(*n),
                LispVal::Float(f) => Value::Float(*f),
                other => return Err(format!("call_lisp: unsupported argument {other:?}")),
            });
        }
        Ok(match self.call(name, &vals)? {
            Value::Int(n) => LispVal::Number(n),
            Value::Float(f) => LispVal::Float(f),
            Value::Bool(b) => LispVal::Number(b as i64),
        })
    }

    /// Drop every compiled edition (force the interpreter path). Test/diagnostic.
    pub fn deoptimize_all(&self) {
        for f in &self.funcs {
            f.deoptimize();
        }
    }
    /// (Re)compile every defined function.
    pub fn compile_all(&self) {
        for f in &self.funcs {
            f.compile_now();
        }
    }
}

/// Collect a proper list into a vector (improper tails are ignored).
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
mod tests;
