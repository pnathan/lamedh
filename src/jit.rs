//! Typed JIT prototype: pre-runtime monomorphic type checking + closure compilation.
//!
//! Working slice of `docs/typed-jit-design.md` that needs no native-code backend
//! (no external deps; Cranelift slots in behind the same [`TypedFn`] interface
//! later, as a `jit` cargo feature).
//!
//! ## What works
//! - **Type membrane + inference.** `(deffun-typed (name ret) ((arg ty)...)
//!   body...)` is elaborated by a bidirectional checker that runs *before*
//!   runtime and rejects ill-typed definitions. Elaboration *is* type checking
//!   (Turnstile-style): [`Cx::elab`] returns the typed [`Core`] and its [`Ty`].
//!   Type agreement is decided by HM-lite **unification** ([`infer`]): explicit
//!   annotations are principal-type pins, and a `let-typed` binding may omit its
//!   type to have it **inferred** from the initializer. Every type is `resolve`d
//!   to a concrete scalar before a definition is accepted (issue #135), the
//!   substrate the array/string element types (#137/#138) monomorphize on.
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
//! Core: `int64`/`float64`/`bool`/`char` (= `u8`/`byte`); `+ - * / mod` and
//! comparisons `< > <= >= = /=` (operand-type directed), `and`/`or`/`not`, `if`,
//! `let-typed`, and calls. `char` is an unboxed byte (`0..=255` in a `u64`):
//! it compares as an integer, converts to/from `int64` via `char-code` /
//! `code-char` (narrowing masks to a byte), and crosses the membrane as a
//! `LispVal::Number` (issue #136).
//! Integer arithmetic wraps and integer `/`,`mod` by zero yield `0` (no panics);
//! this diverges from the checked tree-walker and is revisited with #67.

use crate::LispVal;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

#[cfg(feature = "jit")]
mod native;

mod infer;
use infer::Infer;

// ---------------------------------------------------------------------------
// Types and runtime values.
// ---------------------------------------------------------------------------

/// A monomorphic type in the typed core.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ty {
    Int64,
    Float64,
    Bool,
    /// A byte / `u8` scalar (issue #136). Runtime representation is the byte
    /// value (`0..=255`) held in the low bits of the `u64` word; it slots into
    /// the unboxed scalar tier alongside `int64`/`float64`/`bool`.
    Char,
    /// A type variable (issue #135): an as-yet-undetermined type, identified by
    /// a fresh id. Variables exist only *during* elaboration; [`Infer::resolve`]
    /// must drive every one to a concrete scalar before a definition is
    /// accepted, so no `Var` is ever stored in a function signature or reaches
    /// the runtime membrane.
    Var(u32),
}

impl Ty {
    fn parse(name: &str) -> Option<Ty> {
        match name {
            "INT64" => Some(Ty::Int64),
            "FLOAT64" => Some(Ty::Float64),
            "BOOL" => Some(Ty::Bool),
            "CHAR" | "U8" | "BYTE" => Some(Ty::Char),
            _ => None,
        }
    }

    /// Arithmetic interpretation (`+ - * /`). `char` is intentionally excluded:
    /// byte math is done by widening to `int64` (`char-code`) and narrowing back
    /// (`code-char`).
    fn as_num(self) -> Option<NumTy> {
        match self {
            Ty::Int64 => Some(NumTy::I),
            Ty::Float64 => Some(NumTy::F),
            Ty::Bool | Ty::Char | Ty::Var(_) => None,
        }
    }

    /// Comparison interpretation. `char` compares as an unsigned-small integer
    /// (its word holds a `0..=255` byte value), so it reuses the integer path.
    fn cmp_num(self) -> Option<NumTy> {
        match self {
            Ty::Int64 | Ty::Char => Some(NumTy::I),
            Ty::Float64 => Some(NumTy::F),
            Ty::Bool | Ty::Var(_) => None,
        }
    }
}

/// Lower-case surface name of a [`Ty`], matching the `deffun-typed` syntax
/// (`int64`, `float64`, `bool`, `char`). Used by introspection (`describe`,
/// `disassemble`).
pub fn ty_name(t: Ty) -> &'static str {
    match t {
        Ty::Int64 => "int64",
        Ty::Float64 => "float64",
        Ty::Bool => "bool",
        Ty::Char => "char",
        // A variable should never survive to a signature/introspection site; "?"
        // is a defensive placeholder (the id is omitted to keep this `&'static`).
        Ty::Var(_) => "?",
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
    /// A byte value (`0..=255`) at the boundary. Boxes to/from `LispVal::Number`
    /// since there is no `LispVal::Char` (issue #136 membrane decision).
    Char(u8),
}

impl Value {
    fn to_word(self, ty: Ty) -> Result<u64, String> {
        match (self, ty) {
            (Value::Int(n), Ty::Int64) => Ok(n as u64),
            (Value::Float(f), Ty::Float64) => Ok(f.to_bits()),
            (Value::Bool(b), Ty::Bool) => Ok(b as u64),
            (Value::Char(b), Ty::Char) => Ok(b as u64),
            // Membrane coercion: an untyped Number flowing into a `char`
            // parameter is masked to a byte (the byte value).
            (Value::Int(n), Ty::Char) => Ok((n as u8) as u64),
            _ => Err(format!("value {self:?} does not match type {ty:?}")),
        }
    }

    fn from_word(w: u64, ty: Ty) -> Value {
        match ty {
            Ty::Int64 => Value::Int(w as i64),
            Ty::Float64 => Value::Float(f64::from_bits(w)),
            Ty::Bool => Value::Bool(w != 0),
            Ty::Char => Value::Char(w as u8),
            // Signatures are fully resolved before storage (see [`Ty::Var`]), so
            // a variable never crosses the membrane.
            Ty::Var(_) => unreachable!("from_word on an unresolved type variable"),
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
    /// Narrow an `int64` word to a `char` by masking to a byte (`code-char`).
    /// The widening direction (`char-code`) needs no node: a `char` word already
    /// holds the byte value, so it is reused unchanged at type `int64`.
    ToChar(Box<Core>),
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
    /// The inference state for this definition: fresh variables + substitution
    /// (issue #135). Held behind a `RefCell` so the elaboration methods keep
    /// their `&self` signatures while still threading one shared substitution.
    infer: RefCell<Infer>,
}

impl Cx<'_> {
    /// A fresh type variable from this definition's inference state.
    fn fresh(&self) -> Ty {
        self.infer.borrow_mut().fresh()
    }
    /// Unify two types, extending the substitution (or report the clash).
    fn unify(&self, a: Ty, b: Ty) -> Result<(), String> {
        self.infer.borrow_mut().unify(a, b)
    }
    /// Read a type's current representative under the substitution (for
    /// diagnostics; may still be a variable).
    fn walk(&self, t: Ty) -> Ty {
        self.infer.borrow().walk(t)
    }
    /// Resolve a type to a concrete scalar, erroring if it is still ambiguous.
    fn resolve(&self, t: Ty) -> Result<Ty, String> {
        self.infer.borrow().resolve(t)
    }

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
                    "CHAR-CODE" => self.elab_char_code(args, scope, max),
                    "CODE-CHAR" => self.elab_code_char(args, scope, max),
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
        if self.unify(ta, tb).is_err() {
            return Err(format!(
                "`{op}` operands disagree: {:?} vs {:?}",
                self.walk(ta),
                self.walk(tb)
            ));
        }
        let rt = self
            .resolve(ta)
            .map_err(|_| format!("`{op}`: cannot infer operand type"))?;
        let num = rt
            .as_num()
            .ok_or_else(|| format!("`{op}` expects numeric operands, got {rt:?}"))?;
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
        Ok((Core::Bin(num.into(), bop, Box::new(a), Box::new(b)), rt))
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
        if self.unify(ta, tb).is_err() {
            return Err(format!(
                "`{op}` operands disagree: {:?} vs {:?}",
                self.walk(ta),
                self.walk(tb)
            ));
        }
        let rt = self
            .resolve(ta)
            .map_err(|_| format!("`{op}`: cannot infer operand type"))?;
        let num = rt
            .cmp_num()
            .ok_or_else(|| format!("`{op}` expects comparable operands, got {rt:?}"))?;
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
        if self.unify(ta, Ty::Bool).is_err() {
            return Err(format!("`not` expects bool, got {:?}", self.walk(ta)));
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
        if self.unify(ta, Ty::Bool).is_err() || self.unify(tb, Ty::Bool).is_err() {
            return Err(format!(
                "`{op}` expects bool operands, got {:?} and {:?}",
                self.walk(ta),
                self.walk(tb)
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
        if self.unify(tc, Ty::Bool).is_err() {
            return Err(format!(
                "`if` condition must be bool, got {:?}",
                self.walk(tc)
            ));
        }
        let saved = scope.len();
        let (t, tt) = self.elab(&args[1], scope, max)?;
        scope.truncate(saved);
        let (e, te) = self.elab(&args[2], scope, max)?;
        scope.truncate(saved);
        if self.unify(tt, te).is_err() {
            return Err(format!(
                "`if` branches disagree: {:?} vs {:?}",
                self.walk(tt),
                self.walk(te)
            ));
        }
        Ok((
            Core::If(Box::new(c), Box::new(t), Box::new(e)),
            self.walk(tt),
        ))
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
            // Two binding shapes: `(name type init)` pins the type explicitly,
            // `(name init)` leaves it to be inferred from the initializer
            // (issue #135 — the one surface-compatible inferable position).
            let (name, declared, init) = match parts.as_slice() {
                [LispVal::Symbol(n), LispVal::Symbol(t), init] => {
                    let ty = Ty::parse(&t.borrow().name)
                        .ok_or_else(|| format!("unknown type `{}`", t.borrow().name))?;
                    (n.borrow().name.clone(), Some(ty), init)
                }
                [LispVal::Symbol(n), init] => (n.borrow().name.clone(), None, init),
                _ => {
                    return Err(
                        "`let-typed` binding must be (name type init) or (name init)".to_string(),
                    );
                }
            };
            let (init_core, init_ty) = self.elab(init, scope, max)?;
            // An explicit annotation is a principal-type pin: unify it with the
            // inferred initializer type (must agree). Omitting it makes a fresh
            // variable that the initializer constrains — the inference path.
            let ty = match declared {
                Some(d) => {
                    if self.unify(d, init_ty).is_err() {
                        return Err(format!(
                            "binding `{name}` declared {d:?} but init is {:?}",
                            self.walk(init_ty)
                        ));
                    }
                    d
                }
                None => {
                    let v = self.fresh();
                    self.unify(v, init_ty)
                        .expect("fresh variable always unifies");
                    // Resolve eagerly so the binding carries a concrete type for
                    // any references in the body; an initializer whose type stays
                    // ambiguous is rejected here rather than deferred.
                    self.resolve(v)
                        .map_err(|_| format!("cannot infer type for binding `{name}`"))?
                }
            };
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
            if self.unify(at, params[i].1).is_err() {
                return Err(format!(
                    "`{name}` arg {i} expects {:?}, got {:?}",
                    params[i].1,
                    self.walk(at)
                ));
            }
            arg_cores.push(ac);
        }
        Ok((Core::Call(id, arg_cores), ret))
    }

    /// `(char-code c)` : char -> int64. Widening: the char word already holds
    /// the byte value, so the core is reused unchanged, only its type changes.
    fn elab_char_code(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`char-code` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        if self.unify(ta, Ty::Char).is_err() {
            return Err(format!("`char-code` expects char, got {:?}", self.walk(ta)));
        }
        Ok((a, Ty::Int64))
    }

    /// `(code-char n)` : int64 -> char. Narrowing: mask the word to a byte.
    fn elab_code_char(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`code-char` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        if self.unify(ta, Ty::Int64).is_err() {
            return Err(format!(
                "`code-char` expects int64, got {:?}",
                self.walk(ta)
            ));
        }
        Ok((Core::ToChar(Box::new(a)), Ty::Char))
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
        Core::ToChar(a) => eval_core(a, env, ctx) & 0xff,
    }
}

// ---------------------------------------------------------------------------
// Debug trace: a stepping interpreter over the typed core.
// ---------------------------------------------------------------------------

/// One recorded step of the tracing interpreter ([`Jit::trace_call`]).
///
/// The trace is a pre-order-ish log of node *completions*: a node's step is
/// pushed once its sub-evaluations are done and its result word is known. This
/// is enough to drive a stepper/examiner and to assert structural correctness
/// properties (determinism, result-word agreement, slot-bound safety) over the
/// reference interpreter without touching the hot path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceStep {
    /// Nesting depth of this node in the body's syntax tree (root = 0).
    pub depth: usize,
    /// A stable tag for the node kind (`"litI"`, `"bin"`, `"if"`, `"call"`, …).
    pub op: &'static str,
    /// The raw machine word this node evaluated to (interpret via the static type).
    pub result: u64,
    /// For `Var`/`Let` nodes, the slot index touched; `usize::MAX` otherwise.
    pub slot: usize,
    /// For `Call` nodes, the callee function id; `usize::MAX` otherwise.
    pub callee: usize,
}

const NO_SLOT: usize = usize::MAX;
const NO_CALLEE: usize = usize::MAX;

/// Tracing twin of [`eval_core`]. Pushes a [`TraceStep`] for every node it
/// actually evaluates (so short-circuited `and`/`or`/`if` branches leave no
/// step, exactly mirroring the evaluation the interpreter performs). It must
/// stay byte-for-byte semantically identical to [`eval_core`]; the two are
/// differential-tested against each other in the suite.
fn eval_core_traced(
    core: &Core,
    env: &mut [u64],
    ctx: &Ctx,
    depth: usize,
    log: &mut Vec<TraceStep>,
) -> u64 {
    macro_rules! step {
        ($op:expr, $result:expr, $slot:expr, $callee:expr) => {{
            let r = $result;
            log.push(TraceStep {
                depth,
                op: $op,
                result: r,
                slot: $slot,
                callee: $callee,
            });
            r
        }};
    }
    match core {
        Core::LitI(n) => step!("litI", from_i(*n), NO_SLOT, NO_CALLEE),
        Core::LitF(f) => step!("litF", from_f(*f), NO_SLOT, NO_CALLEE),
        Core::Var(i) => step!("var", env[*i], *i, NO_CALLEE),
        Core::Bin(k, op, a, b) => {
            let x = eval_core_traced(a, env, ctx, depth + 1, log);
            let y = eval_core_traced(b, env, ctx, depth + 1, log);
            let r = match k {
                NumKind::I => from_i(int_bin(*op, as_i(x), as_i(y))),
                NumKind::F => from_f(float_bin(*op, as_f(x), as_f(y))),
            };
            step!("bin", r, NO_SLOT, NO_CALLEE)
        }
        Core::Cmp(k, op, a, b) => {
            let x = eval_core_traced(a, env, ctx, depth + 1, log);
            let y = eval_core_traced(b, env, ctx, depth + 1, log);
            let r = match k {
                NumKind::I => int_cmp(*op, as_i(x), as_i(y)),
                NumKind::F => float_cmp(*op, as_f(x), as_f(y)),
            } as u64;
            step!("cmp", r, NO_SLOT, NO_CALLEE)
        }
        Core::Not(a) => {
            let v = eval_core_traced(a, env, ctx, depth + 1, log);
            step!("not", (v == 0) as u64, NO_SLOT, NO_CALLEE)
        }
        Core::And(a, b) => {
            let r = if eval_core_traced(a, env, ctx, depth + 1, log) != 0 {
                (eval_core_traced(b, env, ctx, depth + 1, log) != 0) as u64
            } else {
                0
            };
            step!("and", r, NO_SLOT, NO_CALLEE)
        }
        Core::Or(a, b) => {
            let r = if eval_core_traced(a, env, ctx, depth + 1, log) != 0 {
                1
            } else {
                (eval_core_traced(b, env, ctx, depth + 1, log) != 0) as u64
            };
            step!("or", r, NO_SLOT, NO_CALLEE)
        }
        Core::If(c, t, e) => {
            let r = if eval_core_traced(c, env, ctx, depth + 1, log) != 0 {
                eval_core_traced(t, env, ctx, depth + 1, log)
            } else {
                eval_core_traced(e, env, ctx, depth + 1, log)
            };
            step!("if", r, NO_SLOT, NO_CALLEE)
        }
        Core::Let(slot, init, body) => {
            let v = eval_core_traced(init, env, ctx, depth + 1, log);
            env[*slot] = v;
            let r = eval_core_traced(body, env, ctx, depth + 1, log);
            step!("let", r, *slot, NO_CALLEE)
        }
        Core::Call(id, args) => {
            let vals: Vec<u64> = args
                .iter()
                .map(|a| eval_core_traced(a, env, ctx, depth + 1, log))
                .collect();
            step!("call", ctx.call(*id, &vals), NO_SLOT, *id)
        }
        Core::ToChar(a) => {
            let v = eval_core_traced(a, env, ctx, depth + 1, log);
            step!("tochar", v & 0xff, NO_SLOT, NO_CALLEE)
        }
    }
}

/// Number of nodes in a typed-core tree (structural size, for invariants).
pub fn core_node_count(core: &Core) -> usize {
    1 + match core {
        Core::LitI(_) | Core::LitF(_) | Core::Var(_) => 0,
        Core::Not(a) | Core::ToChar(a) => core_node_count(a),
        Core::Bin(_, _, a, b)
        | Core::Cmp(_, _, a, b)
        | Core::And(a, b)
        | Core::Or(a, b)
        | Core::Let(_, a, b) => core_node_count(a) + core_node_count(b),
        Core::If(c, t, e) => core_node_count(c) + core_node_count(t) + core_node_count(e),
        Core::Call(_, args) => args.iter().map(core_node_count).sum(),
    }
}

/// Verify a typed-core tree is *well-formed* against a frame of `n_slots`:
/// every `Var`/`Let` slot index is in bounds, and every `Call` id is in
/// `0..n_funcs`. This is a cheap subject-reduction-style structural check the
/// suite runs on every defined function to catch lowering bugs that would
/// otherwise corrupt memory or panic only on a lucky input.
pub fn verify_core(core: &Core, n_slots: usize, n_funcs: usize) -> Result<(), String> {
    match core {
        Core::LitI(_) | Core::LitF(_) => Ok(()),
        Core::Var(i) => {
            if *i < n_slots {
                Ok(())
            } else {
                Err(format!("Var slot {i} out of bounds (n_slots={n_slots})"))
            }
        }
        Core::Not(a) | Core::ToChar(a) => verify_core(a, n_slots, n_funcs),
        Core::Bin(_, _, a, b) | Core::Cmp(_, _, a, b) | Core::And(a, b) | Core::Or(a, b) => {
            verify_core(a, n_slots, n_funcs)?;
            verify_core(b, n_slots, n_funcs)
        }
        Core::Let(slot, init, body) => {
            if *slot >= n_slots {
                return Err(format!("Let slot {slot} out of bounds (n_slots={n_slots})"));
            }
            verify_core(init, n_slots, n_funcs)?;
            verify_core(body, n_slots, n_funcs)
        }
        Core::If(c, t, e) => {
            verify_core(c, n_slots, n_funcs)?;
            verify_core(t, n_slots, n_funcs)?;
            verify_core(e, n_slots, n_funcs)
        }
        Core::Call(id, args) => {
            if *id >= n_funcs {
                return Err(format!("Call id {id} out of bounds (n_funcs={n_funcs})"));
            }
            for a in args {
                verify_core(a, n_slots, n_funcs)?;
            }
            Ok(())
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
        Core::ToChar(a) => {
            let ca = compile(a);
            Rc::new(move |e, c| ca(e, c) & 0xff)
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
    /// Stable heap word holding this function's current native entry pointer (or
    /// `0`). Other compiled functions bake this cell's *address* and load it to
    /// make direct calls; it is updated on (re)compile and cleared on deopt. A
    /// heap `Box` so the address is stable across registry `Vec` growth.
    #[cfg(feature = "jit")]
    entry: Box<Cell<usize>>,
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
            #[cfg(feature = "jit")]
            entry: Box::new(Cell::new(0)),
            generation: Cell::new(0),
        }
    }

    /// Address of this function's native-entry cell (stable; baked into other
    /// functions' compiled code for direct calls).
    #[cfg(feature = "jit")]
    fn entry_cell_addr(&self) -> usize {
        &*self.entry as *const Cell<usize> as usize
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
    /// The number of slots in this function's activation frame (params + lets).
    pub fn n_slots(&self) -> usize {
        self.slots.get()
    }
    /// A clone of this function's typed-core IR, for structural inspection/verification.
    pub fn core_clone(&self) -> Option<Core> {
        self.core.borrow().clone()
    }

    #[cfg_attr(not(feature = "jit"), allow(unused_variables))]
    fn compile_now(&self, funcs: &[Rc<TypedFn>]) {
        let c = self.core.borrow();
        if let Some(core) = c.as_ref() {
            *self.compiled.borrow_mut() = Some(compile(core));
            // With the `jit` feature, also build a native edition. If Cranelift
            // codegen fails for any reason, fall back to the closure edition
            // rather than failing the definition. The entry cell is updated so
            // other compiled functions call this one's native code directly.
            #[cfg(feature = "jit")]
            {
                let n_params = self.params.borrow().len();
                let cell_addrs: Vec<usize> = funcs.iter().map(|f| f.entry_cell_addr()).collect();
                match native::compile_native(core, n_params, self.slots.get(), &cell_addrs) {
                    Ok(ed) => {
                        self.entry.set(ed.entry_addr());
                        *self.native.borrow_mut() = Some(Rc::new(ed));
                    }
                    Err(_) => {
                        self.entry.set(0);
                        *self.native.borrow_mut() = None;
                    }
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
            self.entry.set(0);
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

    /// Forward-declare from a `(declare-typed (name ret) ((arg ty)...))` form.
    /// Returns the (uppercased) name.
    pub fn declare_form(&mut self, form: &LispVal) -> Result<String, String> {
        let items = list_to_vec(form);
        match items.first() {
            Some(LispVal::Symbol(s)) if s.borrow().name == "DECLARE-TYPED" => {}
            _ => return Err("expected a (declare-typed ...) form".to_string()),
        }
        if items.len() != 3 {
            return Err("declare-typed: (declare-typed (name ret) ((arg ty)...))".to_string());
        }
        let (name, ret, params) = parse_signature(&items)?;
        self.intern(&name, params, ret);
        Ok(name)
    }

    /// The (parameter types, return type) of a registered function.
    pub fn signature(&self, name: &str) -> Option<(Vec<Ty>, Ty)> {
        let id = self.id(name)?;
        let f = &self.funcs[id];
        let ptys = f.params.borrow().iter().map(|(_, t)| *t).collect();
        Some((ptys, f.ret.get()))
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

        let (name, ret, params) = parse_signature(&items)?;
        let mut scope: Scope = params.clone();

        // Register the signature *before* elaborating the body so a function can
        // call itself (and any already-declared peer).
        let id = self.intern(&name, params, ret);

        let mut max_slots = scope.len();
        let core = {
            let cx = Cx {
                funcs: &self.funcs,
                by_name: &self.by_name,
                infer: RefCell::new(Infer::new()),
            };
            let (core, body_ty) = cx.elab_body(&items[3..], &mut scope, &mut max_slots)?;
            // The declared return type is a principal-type pin: the body's
            // inferred type must unify with it.
            if cx.unify(body_ty, ret).is_err() {
                return Err(format!(
                    "{name}: declared return {ret:?} but body has type {:?}",
                    cx.walk(body_ty)
                ));
            }
            // Final resolve: with `ret` concrete the body type is pinned, but
            // reject any definition whose body type is left ambiguous.
            cx.resolve(body_ty).map_err(|e| format!("{name}: {e}"))?;
            core
        };

        let f = self.funcs[id].clone();
        f.slots.set(max_slots);
        *f.core.borrow_mut() = Some(core);
        f.compile_now(&self.funcs);
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
        if !f.is_defined() {
            return Err(format!("{name}: declared but not defined"));
        }
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
                LispVal::Char(b) => Value::Char(*b),
                other => return Err(format!("call_lisp: unsupported argument {other:?}")),
            });
        }
        Ok(match self.call(name, &vals)? {
            Value::Int(n) => LispVal::Number(n),
            Value::Float(f) => LispVal::Float(f),
            Value::Bool(b) => LispVal::Number(b as i64),
            Value::Char(b) => LispVal::Char(b),
        })
    }

    /// Trace a call through the **reference (typed-core) interpreter**, returning
    /// the boxed result alongside a step-by-step [`TraceStep`] log of the callee's
    /// own body (nested calls run normally and are summarised by a single `call`
    /// step). This is the debug stepper/examiner: it lets a correctness suite
    /// inspect *how* a typed function computed its result, and assert invariants
    /// (determinism, result agreement, slot-bound safety) over the trace.
    ///
    /// Independent of whether a compiled edition exists — it always interprets so
    /// the trace reflects the reference semantics that compiled editions must match.
    pub fn trace_call(
        &self,
        name: &str,
        args: &[Value],
    ) -> Result<(Value, Vec<TraceStep>), String> {
        let id = self
            .id(name)
            .ok_or_else(|| format!("unknown function `{name}`"))?;
        let f = &self.funcs[id];
        let core = f
            .core_clone()
            .ok_or_else(|| format!("{name}: declared but not defined"))?;
        let params = f.params.borrow().clone();
        if args.len() != params.len() {
            return Err(format!(
                "{name}: expected {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        let mut frame = vec![0u64; f.slots.get()];
        for (i, (a, (_, ty))) in args.iter().zip(params.iter()).enumerate() {
            frame[i] = a.to_word(*ty)?;
        }
        let ctx = self.ctx();
        let mut log = Vec::new();
        let w = eval_core_traced(&core, &mut frame, &ctx, 0, &mut log);
        Ok((Value::from_word(w, f.ret.get()), log))
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
            f.compile_now(&self.funcs);
        }
    }

    /// Render the typed-core IR of `name` as a flat pseudo-assembly listing.
    ///
    /// This is the "what is actually being run" view for a jotted (typed)
    /// function: the typed core is the reference program every compiled edition
    /// must match, lowered here to a linear, register-and-label instruction
    /// stream. Returns `None` if no typed function by that name is registered.
    pub fn disassemble(&self, name: &str) -> Option<String> {
        let f = self.get(name)?;
        let params = f.params();
        let ret = f.ret();
        let mut s = String::new();
        let sig = params
            .iter()
            .map(|(_, t)| ty_name(*t))
            .collect::<Vec<_>>()
            .join(" ");
        s.push_str(&format!(
            "; typed function {} ({sig}) -> {}\n",
            f.name,
            ty_name(ret)
        ));
        if params.is_empty() {
            s.push_str("; slots: (none)\n");
        } else {
            s.push_str("; slots:\n");
            for (i, (pname, ty)) in params.iter().enumerate() {
                s.push_str(&format!(";   slot{i} = {pname} : {}\n", ty_name(*ty)));
            }
        }
        s.push_str(&format!(
            "; compiled edition: {}\n",
            if f.is_compiled() {
                "yes"
            } else {
                "no (interpreted)"
            }
        ));
        match f.core_clone() {
            None => s.push_str("    ; declared but not yet defined\n"),
            Some(core) => {
                let mut out = Vec::new();
                let mut reg = 0usize;
                let mut lab = 0usize;
                self.dis_emit(&core, "rv", &mut out, &mut reg, &mut lab);
                for line in out {
                    s.push_str(&line);
                    s.push('\n');
                }
                s.push_str("    ret rv\n");
            }
        }
        Some(s)
    }

    /// Linearize `core` into instructions writing their result into register
    /// `dst`, appending textual lines to `out`. `reg`/`lab` are monotonic
    /// counters for fresh temporaries and branch labels.
    fn dis_emit(
        &self,
        core: &Core,
        dst: &str,
        out: &mut Vec<String>,
        reg: &mut usize,
        lab: &mut usize,
    ) {
        let fresh = |reg: &mut usize| {
            let r = format!("r{}", *reg);
            *reg += 1;
            r
        };
        let fresh_lab = |lab: &mut usize, base: &str| {
            let l = format!(".{base}{}", *lab);
            *lab += 1;
            l
        };
        match core {
            Core::LitI(n) => out.push(format!("    {dst} = li   {n}")),
            Core::LitF(x) => out.push(format!("    {dst} = lf   {x}")),
            Core::Var(i) => out.push(format!("    {dst} = ld   slot{i}")),
            Core::ToChar(a) => {
                self.dis_emit(a, dst, out, reg, lab);
                out.push(format!("    {dst} = and  {dst}, 0xff        ; code-char"));
            }
            Core::Not(a) => {
                self.dis_emit(a, dst, out, reg, lab);
                out.push(format!("    {dst} = not  {dst}"));
            }
            Core::Bin(k, op, a, b) => {
                let t1 = fresh(reg);
                let t2 = fresh(reg);
                self.dis_emit(a, &t1, out, reg, lab);
                self.dis_emit(b, &t2, out, reg, lab);
                out.push(format!("    {dst} = {} {t1}, {t2}", bin_mnemonic(*k, *op)));
            }
            Core::Cmp(k, op, a, b) => {
                let t1 = fresh(reg);
                let t2 = fresh(reg);
                self.dis_emit(a, &t1, out, reg, lab);
                self.dis_emit(b, &t2, out, reg, lab);
                out.push(format!("    {dst} = {} {t1}, {t2}", cmp_mnemonic(*k, *op)));
            }
            Core::And(a, b) => {
                self.dis_emit(a, dst, out, reg, lab);
                let end = fresh_lab(lab, "and_end");
                out.push(format!("    brz  {dst}, {end}          ; short-circuit"));
                self.dis_emit(b, dst, out, reg, lab);
                out.push(format!("{end}:"));
            }
            Core::Or(a, b) => {
                self.dis_emit(a, dst, out, reg, lab);
                let end = fresh_lab(lab, "or_end");
                out.push(format!("    brnz {dst}, {end}          ; short-circuit"));
                self.dis_emit(b, dst, out, reg, lab);
                out.push(format!("{end}:"));
            }
            Core::If(c, t, e) => {
                let tc = fresh(reg);
                self.dis_emit(c, &tc, out, reg, lab);
                let l_else = fresh_lab(lab, "else");
                let l_end = fresh_lab(lab, "endif");
                out.push(format!("    brz  {tc}, {l_else}"));
                self.dis_emit(t, dst, out, reg, lab);
                out.push(format!("    br   {l_end}"));
                out.push(format!("{l_else}:"));
                self.dis_emit(e, dst, out, reg, lab);
                out.push(format!("{l_end}:"));
            }
            Core::Let(slot, val, body) => {
                let tv = fresh(reg);
                self.dis_emit(val, &tv, out, reg, lab);
                out.push(format!("    st   slot{slot}, {tv}"));
                self.dis_emit(body, dst, out, reg, lab);
            }
            Core::Call(id, args) => {
                let mut argregs = Vec::with_capacity(args.len());
                for a in args {
                    let t = fresh(reg);
                    self.dis_emit(a, &t, out, reg, lab);
                    argregs.push(t);
                }
                let callee = self.name_of(*id).unwrap_or_else(|| format!("fn#{id}"));
                out.push(format!("    {dst} = call {callee}({})", argregs.join(", ")));
            }
        }
    }
}

/// Mnemonic for an arithmetic [`BinOp`] at numeric kind `k` (`i*` vs `f*`).
fn bin_mnemonic(k: NumKind, op: BinOp) -> &'static str {
    let p = matches!(k, NumKind::I);
    match (op, p) {
        (BinOp::Add, true) => "iadd",
        (BinOp::Add, false) => "fadd",
        (BinOp::Sub, true) => "isub",
        (BinOp::Sub, false) => "fsub",
        (BinOp::Mul, true) => "imul",
        (BinOp::Mul, false) => "fmul",
        (BinOp::Div, true) => "idiv",
        (BinOp::Div, false) => "fdiv",
        (BinOp::Mod, true) => "imod",
        (BinOp::Mod, false) => "fmod",
    }
}

/// Mnemonic for a comparison [`CmpOp`] at numeric kind `k` (`icmp.*`/`fcmp.*`).
fn cmp_mnemonic(k: NumKind, op: CmpOp) -> &'static str {
    let p = matches!(k, NumKind::I);
    match (op, p) {
        (CmpOp::Lt, true) => "icmp.lt",
        (CmpOp::Lt, false) => "fcmp.lt",
        (CmpOp::Gt, true) => "icmp.gt",
        (CmpOp::Gt, false) => "fcmp.gt",
        (CmpOp::Le, true) => "icmp.le",
        (CmpOp::Le, false) => "fcmp.le",
        (CmpOp::Ge, true) => "icmp.ge",
        (CmpOp::Ge, false) => "fcmp.ge",
        (CmpOp::Eq, true) => "icmp.eq",
        (CmpOp::Eq, false) => "fcmp.eq",
        (CmpOp::Ne, true) => "icmp.ne",
        (CmpOp::Ne, false) => "fcmp.ne",
    }
}

/// A parsed typed signature: function name, return type, parameter `(name, type)`s.
type ParsedSig = (String, Ty, Vec<(String, Ty)>);

/// Parse `items[1]` = `(name ret)` and `items[2]` = `((arg ty)...)` shared by
/// `deffun-typed` and `declare-typed`.
fn parse_signature(items: &[LispVal]) -> Result<ParsedSig, String> {
    let sig = list_to_vec(&items[1]);
    let (name, ret) = match sig.as_slice() {
        [LispVal::Symbol(n), LispVal::Symbol(r)] => {
            let ret = Ty::parse(&r.borrow().name)
                .ok_or_else(|| format!("unknown return type `{}`", r.borrow().name))?;
            (n.borrow().name.clone(), ret)
        }
        _ => return Err("typed signature must be (name return-type)".to_string()),
    };
    let mut params = Vec::new();
    for p in list_to_vec(&items[2]) {
        let parts = list_to_vec(&p);
        match parts.as_slice() {
            [LispVal::Symbol(a), LispVal::Symbol(t)] => {
                let ty = Ty::parse(&t.borrow().name)
                    .ok_or_else(|| format!("unknown param type `{}`", t.borrow().name))?;
                params.push((a.borrow().name.clone(), ty));
            }
            _ => return Err("each parameter must be (name type)".to_string()),
        }
    }
    Ok((name, ret, params))
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
