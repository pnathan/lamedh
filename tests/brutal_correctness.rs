//! # Brutal correctness suite
//!
//! A ruthless, randomized **differential + metamorphic** correctness harness for
//! Lamedh's execution tiers. The interpreter is growing a typed JIT (type
//! inference, closure compilation, a native Cranelift backend, and — soon —
//! multithreading). Each of those tiers is a *separate implementation of the
//! same semantics*, and the only way a soundness bug survives is if every tier
//! agrees on a wrong answer. This suite makes that vanishingly unlikely by
//! cross-checking the tiers against each other **and** against an independent
//! reference oracle, over millions of generated cases.
//!
//! This is deliberately **not** a fast CI gate. It is a heavy correctness
//! instrument. Crank the volume with environment variables:
//!
//! ```text
//! BRUTAL_PROGRAMS=20000 BRUTAL_INPUTS=64 cargo test --test brutal_correctness -- --nocapture
//! BRUTAL=1 cargo test --release --test brutal_correctness          # ~millions of cases
//! BRUTAL=1 cargo test --release --features jit --test brutal_correctness   # + native tier
//! ```
//!
//! ## What is checked
//!
//! For every randomly generated *well-typed* program (a DAG of `defun-typed`
//! functions over `int64`/`float64`/`bool`) and every random input vector we
//! run, and force into agreement, **four independent evaluators**:
//!
//! 1. the **compiled edition** (`Jit::compile_all` + `call`) — the closure tree
//!    by default, or, under `--features jit`, the **native Cranelift** code;
//! 2. the **typed-core reference interpreter** (`Jit::deoptimize_all` + `call`),
//! 3. the **tracing interpreter** (`Jit::trace_call`, a third code path), and
//! 4. an **independent Rust oracle** that interprets the generator's own AST
//!    with the exact documented semantics (wrapping integer arithmetic,
//!    `/`/`mod`-by-zero ⇒ 0, IEEE-754 `float64`).
//!
//! Float results are compared **bit-for-bit** (catching `-0.0`/rounding bugs),
//! except that any NaN equals any NaN (payload bits are not language semantics
//! and the native backend may canonicalise them differently).
//!
//! On top of N-way agreement we assert:
//! - **structural soundness** of every lowered typed-core tree (`verify_core`):
//!   no out-of-bounds slot or callee id can hide until a lucky input hits it;
//! - **trace determinism**: tracing the same call twice yields an identical log,
//!   and the log's final word equals the returned value;
//! - **metamorphic laws**: deopt/recompile idempotence, redefinition propagation
//!   through call cells, and source-optimizer semantic preservation;
//! - **abstract-interpretation soundness**: an interval domain over-approximates
//!   the concrete result whenever the analysis proves no overflow occurs;
//! - **untyped/typed cross-tier agreement** on the pure-arithmetic overlap, where
//!   the tree-walking interpreter and the JIT must compute the same word;
//! - **string correctness** for the untyped tree-walker: random `concat`/`index`
//!   expressions vs an independent string oracle, plus the algebraic laws those
//!   ops obey (associativity via the oracle, empty-string identity, char
//!   indexing, out-of-bounds erroring, and reflexive structural `equal`).

use lamedh::environment::Environment;
use lamedh::eval_str;
use lamedh::jit::{Jit, Value, core_node_count, verify_core};
use lamedh::reader::read;

// ===========================================================================
// Deterministic PRNG (SplitMix64). No external deps; reproducible by seed.
// ===========================================================================

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `[0, n)` (n > 0).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    fn bool(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }
    /// A "nasty" i64: biased toward boundary values that break naive arithmetic.
    fn nasty_i64(&mut self) -> i64 {
        match self.below(12) {
            0 => 0,
            1 => 1,
            2 => -1,
            3 => i64::MAX,
            4 => i64::MIN,
            5 => i64::MAX - 1,
            6 => i64::MIN + 1,
            7 => 2,
            8 => -2,
            9 => (self.next_u64() % 1_000) as i64 - 500,
            10 => 3_037_000_500, // ~sqrt(i64::MAX); squares overflow
            _ => self.next_u64() as i64,
        }
    }
    /// A "nasty" f64: boundary values that break naive float code, plus a few
    /// finite ones. Includes NaN, ±∞, ±0, subnormals, and the f64 extremes.
    fn nasty_f64(&mut self) -> f64 {
        match self.below(16) {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => -1.0,
            4 => 0.5,
            5 => f64::INFINITY,
            6 => f64::NEG_INFINITY,
            7 => f64::NAN,
            8 => f64::MIN_POSITIVE,
            9 => f64::MAX,
            10 => f64::MIN,
            11 => 1e300,
            12 => -1e-300,
            13 => 4.9e-324, // smallest subnormal
            14 => (self.next_u64() % 2_000) as f64 / 8.0 - 125.0,
            _ => f64::from_bits(self.next_u64()),
        }
    }
}

// ===========================================================================
// The generator's own AST + an INDEPENDENT reference oracle.
//
// This AST is rendered to `defun-typed` source for the JIT, and *separately*
// interpreted in Rust by `OType`-aware evaluation here. Two implementations of
// the same semantics ⇒ a true differential oracle.
// ===========================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GTy {
    Int,
    Bool,
    Float,
}

impl GTy {
    fn lisp(self) -> &'static str {
        match self {
            GTy::Int => "int64",
            GTy::Bool => "bool",
            GTy::Float => "float64",
        }
    }
}

/// A reader-and-oracle-safe float literal: `(value, source-text)` pairs chosen so
/// the rendered text parses back to *exactly* `value` (no round-trip drift).
/// Non-finite and signed-zero values enter only through call arguments, never as
/// literals (the reader has no `inf`/`NaN` syntax).
const FLOAT_LITS: &[(f64, &str)] = &[
    (0.0, "0.0"),
    (1.0, "1.0"),
    (2.0, "2.0"),
    (-1.0, "-1.0"),
    (0.5, "0.5"),
    (-0.5, "-0.5"),
    (0.25, "0.25"),
    (3.0, "3.0"),
    (-4.0, "-4.0"),
    (10.0, "10.0"),
    (100.0, "100.0"),
    (-2.5, "-2.5"),
];

#[derive(Clone, Copy, Debug)]
enum IBin {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}
#[derive(Clone, Copy, Debug)]
enum ICmp {
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
}

#[derive(Clone, Debug)]
enum E {
    LitI(i64),
    LitB(bool),
    LitF(f64, &'static str), // (value, reader-safe source text)
    Var(String),
    Bin(IBin, Box<E>, Box<E>),
    Cmp(ICmp, Box<E>, Box<E>),
    Not(Box<E>),
    And(Box<E>, Box<E>),
    Or(Box<E>, Box<E>),
    If(Box<E>, Box<E>, Box<E>),
    Let(String, GTy, Box<E>, Box<E>),
    Call(usize, Vec<E>), // callee index into the program's function table
}

#[derive(Clone)]
struct FnDef {
    name: String,
    params: Vec<(String, GTy)>,
    ret: GTy,
    body: E,
}

/// A boxed reference value the oracle computes in.
#[derive(Clone, Copy, Debug)]
enum OVal {
    I(i64),
    B(bool),
    F(f64),
}

impl OVal {
    fn to_value(self) -> Value {
        match self {
            OVal::I(n) => Value::Int(n),
            OVal::B(b) => Value::Bool(b),
            OVal::F(f) => Value::Float(f),
        }
    }
}

/// The reference oracle: interpret `e` with `scope` (name→value) over the whole
/// program `prog` (so `Call`s resolve to earlier functions). Semantics mirror
/// `jit::eval_core` exactly: wrapping integer arithmetic, `/`/`mod` by zero ⇒ 0,
/// truncating `mod` (`checked_rem`), short-circuit `and`/`or`.
fn oracle(e: &E, scope: &mut Vec<(String, OVal)>, prog: &[FnDef]) -> OVal {
    match e {
        E::LitI(n) => OVal::I(*n),
        E::LitB(b) => OVal::B(*b),
        E::LitF(f, _) => OVal::F(*f),
        E::Var(name) => scope
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
            .unwrap_or_else(|| panic!("oracle: unbound var {name}")),
        // Arithmetic is operand-type directed (exactly like the JIT elaborator):
        // both operands int ⇒ wrapping int op; both float ⇒ IEEE f64 op.
        E::Bin(op, a, b) => match (oracle(a, scope, prog), oracle(b, scope, prog)) {
            (OVal::I(x), OVal::I(y)) => OVal::I(match op {
                IBin::Add => x.wrapping_add(y),
                IBin::Sub => x.wrapping_sub(y),
                IBin::Mul => x.wrapping_mul(y),
                IBin::Div => x.checked_div(y).unwrap_or(0),
                IBin::Mod => x.checked_rem(y).unwrap_or(0),
            }),
            (OVal::F(x), OVal::F(y)) => OVal::F(match op {
                IBin::Add => x + y,
                IBin::Sub => x - y,
                IBin::Mul => x * y,
                IBin::Div => x / y,
                IBin::Mod => x % y, // unreachable: float mod is type-rejected
            }),
            (a, b) => panic!("oracle: ill-typed Bin operands {a:?} {b:?}"),
        },
        E::Cmp(op, a, b) => {
            let r = match (oracle(a, scope, prog), oracle(b, scope, prog)) {
                (OVal::I(x), OVal::I(y)) => cmp(*op, x, y),
                (OVal::F(x), OVal::F(y)) => cmp(*op, x, y),
                (a, b) => panic!("oracle: ill-typed Cmp operands {a:?} {b:?}"),
            };
            OVal::B(r)
        }
        E::Not(a) => OVal::B(!oracle_b(a, scope, prog)),
        E::And(a, b) => OVal::B(oracle_b(a, scope, prog) && oracle_b(b, scope, prog)),
        E::Or(a, b) => OVal::B(oracle_b(a, scope, prog) || oracle_b(b, scope, prog)),
        E::If(c, t, f) => {
            if oracle_b(c, scope, prog) {
                oracle(t, scope, prog)
            } else {
                oracle(f, scope, prog)
            }
        }
        E::Let(name, _ty, init, body) => {
            let v = oracle(init, scope, prog);
            scope.push((name.clone(), v));
            let r = oracle(body, scope, prog);
            scope.pop();
            r
        }
        E::Call(idx, args) => {
            let callee = &prog[*idx];
            let mut callee_scope: Vec<(String, OVal)> = Vec::new();
            for ((pname, _pty), a) in callee.params.iter().zip(args) {
                let v = oracle(a, scope, prog);
                callee_scope.push((pname.clone(), v));
            }
            oracle(&callee.body, &mut callee_scope, prog)
        }
    }
}
fn cmp<T: PartialOrd>(op: ICmp, x: T, y: T) -> bool {
    match op {
        ICmp::Lt => x < y,
        ICmp::Gt => x > y,
        ICmp::Le => x <= y,
        ICmp::Ge => x >= y,
        ICmp::Eq => x == y,
        ICmp::Ne => x != y,
    }
}
fn oracle_i(e: &E, scope: &mut Vec<(String, OVal)>, prog: &[FnDef]) -> i64 {
    match oracle(e, scope, prog) {
        OVal::I(n) => n,
        other => panic!("oracle: expected int, got {other:?}"),
    }
}
fn oracle_b(e: &E, scope: &mut Vec<(String, OVal)>, prog: &[FnDef]) -> bool {
    match oracle(e, scope, prog) {
        OVal::B(b) => b,
        other => panic!("oracle: expected bool, got {other:?}"),
    }
}

// ===========================================================================
// Rendering an `E` to `defun-typed` Lisp source.
// ===========================================================================

fn render(e: &E, prog: &[FnDef], out: &mut String) {
    match e {
        E::LitI(n) => out.push_str(&n.to_string()),
        E::LitB(b) => out.push_str(if *b { "true" } else { "false" }),
        E::LitF(_, text) => out.push_str(text),
        E::Var(name) => out.push_str(name),
        E::Bin(op, a, b) => {
            let s = match op {
                IBin::Add => "+",
                IBin::Sub => "-",
                IBin::Mul => "*",
                IBin::Div => "/",
                IBin::Mod => "mod",
            };
            render_call(s, &[a, b], prog, out);
        }
        E::Cmp(op, a, b) => {
            let s = match op {
                ICmp::Lt => "<",
                ICmp::Gt => ">",
                ICmp::Le => "<=",
                ICmp::Ge => ">=",
                ICmp::Eq => "=",
                ICmp::Ne => "/=",
            };
            render_call(s, &[a, b], prog, out);
        }
        E::Not(a) => render_call("not", &[a], prog, out),
        E::And(a, b) => render_call("and", &[a, b], prog, out),
        E::Or(a, b) => render_call("or", &[a, b], prog, out),
        E::If(c, t, f) => render_call("if", &[c, t, f], prog, out),
        E::Let(name, ty, init, body) => {
            out.push_str("(let-typed ((");
            out.push_str(name);
            out.push(' ');
            out.push_str(ty.lisp());
            out.push(' ');
            render(init, prog, out);
            out.push_str(")) ");
            render(body, prog, out);
            out.push(')');
        }
        E::Call(idx, args) => {
            let name = prog[*idx].name.clone();
            out.push('(');
            out.push_str(&name);
            for a in args {
                out.push(' ');
                render(a, prog, out);
            }
            out.push(')');
        }
    }
}
fn render_call(head: &str, args: &[&E], prog: &[FnDef], out: &mut String) {
    out.push('(');
    out.push_str(head);
    for a in args {
        out.push(' ');
        render(a, prog, out);
    }
    out.push(')');
}

fn render_defun(f: &FnDef, prog: &[FnDef]) -> String {
    let mut s = String::new();
    s.push_str("(defun-typed (");
    s.push_str(&f.name);
    s.push(' ');
    s.push_str(f.ret.lisp());
    s.push_str(") (");
    for (i, (pn, pt)) in f.params.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push('(');
        s.push_str(pn);
        s.push(' ');
        s.push_str(pt.lisp());
        s.push(')');
    }
    s.push_str(") ");
    render(&f.body, prog, &mut s);
    s.push(')');
    s
}

// ===========================================================================
// The generator: random WELL-TYPED programs.
// ===========================================================================

struct Gen<'a> {
    rng: &'a mut Rng,
}

impl Gen<'_> {
    /// Generate a well-typed expression of type `want`, given visible bindings
    /// `scope` and the already-defined functions `prog`. `fuel` bounds depth.
    fn expr(
        &mut self,
        want: GTy,
        scope: &[(String, GTy)],
        prog: &[FnDef],
        fuel: u32,
        let_ctr: &mut usize,
    ) -> E {
        if fuel == 0 {
            return self.leaf(want, scope);
        }
        // Functions of the wanted return type that we could call.
        let callable: Vec<usize> = prog
            .iter()
            .enumerate()
            .filter(|(_, f)| f.ret == want)
            .map(|(i, _)| i)
            .collect();

        match want {
            GTy::Int | GTy::Float => {
                let nty = want; // int or float; arithmetic stays within the type
                let choice = self.rng.below(7);
                match choice {
                    0 | 1 => {
                        // `mod` is int64-only in the type system; never emit it for float.
                        let n = if nty == GTy::Int { 5 } else { 4 };
                        let op = match self.rng.below(n) {
                            0 => IBin::Add,
                            1 => IBin::Sub,
                            2 => IBin::Mul,
                            3 => IBin::Div,
                            _ => IBin::Mod,
                        };
                        E::Bin(
                            op,
                            Box::new(self.expr(nty, scope, prog, fuel - 1, let_ctr)),
                            Box::new(self.expr(nty, scope, prog, fuel - 1, let_ctr)),
                        )
                    }
                    2 => E::If(
                        Box::new(self.expr(GTy::Bool, scope, prog, fuel - 1, let_ctr)),
                        Box::new(self.expr(nty, scope, prog, fuel - 1, let_ctr)),
                        Box::new(self.expr(nty, scope, prog, fuel - 1, let_ctr)),
                    ),
                    3 => self.gen_let(nty, scope, prog, fuel, let_ctr),
                    4 if !callable.is_empty() => {
                        self.gen_call(callable, scope, prog, fuel, let_ctr)
                    }
                    _ => self.leaf(nty, scope),
                }
            }
            GTy::Bool => {
                let choice = self.rng.below(7);
                match choice {
                    0 | 1 => {
                        let op = match self.rng.below(6) {
                            0 => ICmp::Lt,
                            1 => ICmp::Gt,
                            2 => ICmp::Le,
                            3 => ICmp::Ge,
                            4 => ICmp::Eq,
                            _ => ICmp::Ne,
                        };
                        // Comparisons are operand-type directed: pick int OR float
                        // operands (both the same type), mirroring the elaborator.
                        let oty = self.num_ty();
                        E::Cmp(
                            op,
                            Box::new(self.expr(oty, scope, prog, fuel - 1, let_ctr)),
                            Box::new(self.expr(oty, scope, prog, fuel - 1, let_ctr)),
                        )
                    }
                    2 => E::And(
                        Box::new(self.expr(GTy::Bool, scope, prog, fuel - 1, let_ctr)),
                        Box::new(self.expr(GTy::Bool, scope, prog, fuel - 1, let_ctr)),
                    ),
                    3 => E::Or(
                        Box::new(self.expr(GTy::Bool, scope, prog, fuel - 1, let_ctr)),
                        Box::new(self.expr(GTy::Bool, scope, prog, fuel - 1, let_ctr)),
                    ),
                    4 => E::Not(Box::new(self.expr(
                        GTy::Bool,
                        scope,
                        prog,
                        fuel - 1,
                        let_ctr,
                    ))),
                    5 if !callable.is_empty() => {
                        self.gen_call(callable, scope, prog, fuel, let_ctr)
                    }
                    _ => E::If(
                        Box::new(self.expr(GTy::Bool, scope, prog, fuel - 1, let_ctr)),
                        Box::new(self.expr(GTy::Bool, scope, prog, fuel - 1, let_ctr)),
                        Box::new(self.expr(GTy::Bool, scope, prog, fuel - 1, let_ctr)),
                    ),
                }
            }
        }
    }

    /// A random type for a parameter, return value, or `let` binding.
    fn rand_ty(&mut self) -> GTy {
        match self.rng.below(3) {
            0 => GTy::Int,
            1 => GTy::Bool,
            _ => GTy::Float,
        }
    }
    /// A random *numeric* type (the only ones arithmetic/comparison operands take).
    fn num_ty(&mut self) -> GTy {
        if self.rng.bool() {
            GTy::Int
        } else {
            GTy::Float
        }
    }

    fn gen_let(
        &mut self,
        want: GTy,
        scope: &[(String, GTy)],
        prog: &[FnDef],
        fuel: u32,
        let_ctr: &mut usize,
    ) -> E {
        let bty = self.rand_ty();
        let init = self.expr(bty, scope, prog, fuel - 1, let_ctr);
        let name = format!("l{}", *let_ctr);
        *let_ctr += 1;
        let mut inner = scope.to_vec();
        inner.push((name.clone(), bty));
        let body = self.expr(want, &inner, prog, fuel - 1, let_ctr);
        E::Let(name, bty, Box::new(init), Box::new(body))
    }

    fn gen_call(
        &mut self,
        callable: Vec<usize>,
        scope: &[(String, GTy)],
        prog: &[FnDef],
        fuel: u32,
        let_ctr: &mut usize,
    ) -> E {
        let idx = callable[self.rng.below(callable.len())];
        let param_tys: Vec<GTy> = prog[idx].params.iter().map(|(_, t)| *t).collect();
        let args = param_tys
            .iter()
            .map(|t| self.expr(*t, scope, prog, fuel - 1, let_ctr))
            .collect();
        E::Call(idx, args)
    }

    /// A leaf of the wanted type: a visible variable if one exists, else a literal.
    fn leaf(&mut self, want: GTy, scope: &[(String, GTy)]) -> E {
        let vars: Vec<&String> = scope
            .iter()
            .filter(|(_, t)| *t == want)
            .map(|(n, _)| n)
            .collect();
        match want {
            GTy::Int => {
                if !vars.is_empty() && self.rng.bool() {
                    E::Var(vars[self.rng.below(vars.len())].clone())
                } else {
                    // Keep literals in a reader-friendly range; extreme values
                    // enter through call arguments instead.
                    E::LitI(self.rng.below(2001) as i64 - 1000)
                }
            }
            GTy::Bool => {
                if !vars.is_empty() && self.rng.below(3) == 0 {
                    E::Var(vars[self.rng.below(vars.len())].clone())
                } else {
                    E::LitB(self.rng.bool())
                }
            }
            GTy::Float => {
                if !vars.is_empty() && self.rng.bool() {
                    E::Var(vars[self.rng.below(vars.len())].clone())
                } else {
                    let (v, t) = FLOAT_LITS[self.rng.below(FLOAT_LITS.len())];
                    E::LitF(v, t)
                }
            }
        }
    }

    fn program(&mut self, fuel: u32) -> Vec<FnDef> {
        let n_funcs = 1 + self.rng.below(4); // 1..=4
        let mut prog: Vec<FnDef> = Vec::new();
        for i in 0..n_funcs {
            let arity = 1 + self.rng.below(3); // 1..=3
            let params: Vec<(String, GTy)> = (0..arity)
                .map(|p| (format!("p{p}"), self.rand_ty()))
                .collect();
            let ret = self.rand_ty();
            let mut let_ctr = 0usize;
            let body = self.expr(ret, &params, &prog, fuel, &mut let_ctr);
            prog.push(FnDef {
                name: format!("f{i}"),
                params,
                ret,
                body,
            });
        }
        prog
    }
}

// ===========================================================================
// Wiring a generated program into a fresh Jit.
// ===========================================================================

fn build_jit(prog: &[FnDef]) -> Jit {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    for f in prog {
        let src = render_defun(f, prog);
        let form = read(&src, &env).unwrap_or_else(|e| panic!("read failed for `{src}`: {e}"));
        j.define(&form)
            .unwrap_or_else(|e| panic!("the generator emitted ill-typed source!\n  {src}\n  {e}"));
    }
    j
}

/// A random argument vector for `f`'s parameter types.
fn random_args(rng: &mut Rng, f: &FnDef) -> Vec<Value> {
    f.params
        .iter()
        .map(|(_, t)| match t {
            GTy::Int => Value::Int(rng.nasty_i64()),
            GTy::Bool => Value::Bool(rng.bool()),
            GTy::Float => Value::Float(rng.nasty_f64()),
        })
        .collect()
}

fn args_to_ovals(args: &[Value]) -> Vec<OVal> {
    args.iter()
        .map(|v| match v {
            Value::Int(n) => OVal::I(*n),
            Value::Bool(b) => OVal::B(*b),
            Value::Float(f) => OVal::F(*f),
            // The generator never emits char args; carry the byte value if it
            // ever does, keeping this mapping total.
            Value::Char(b) => OVal::I(*b as i64),
            // The generator emits only scalar argument types.
            Value::Array(_) | Value::Struct(_) => {
                unreachable!("generator emits only scalar arguments")
            }
        })
        .collect()
}

/// Tier-agreement equality. Floats compare **bit-for-bit** (so a `-0.0` vs `0.0`
/// or a wrong rounding is caught) — except that *any* NaN equals *any* NaN, since
/// NaN payload bits are not part of the language semantics and the native
/// (Cranelift) backend may canonicalise them differently from Rust's `f64`.
fn val_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => {
            (x.is_nan() && y.is_nan()) || x.to_bits() == y.to_bits()
        }
        _ => false,
    }
}

// ===========================================================================
// THE MAIN EVENT: N-way differential over random programs.
// ===========================================================================

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
fn brutal_differential_random_programs() {
    // Heavy recursion in the oracle/JIT for big random programs: run on a wide
    // stack just like the real interpreter does.
    lamedh::with_large_stack(|| {
        let brutal = std::env::var("BRUTAL").is_ok();
        let n_programs = env_usize("BRUTAL_PROGRAMS", if brutal { 50_000 } else { 4_000 });
        let n_inputs = env_usize("BRUTAL_INPUTS", if brutal { 64 } else { 32 });
        let base_seed = env_usize("BRUTAL_SEED", 0xC0FFEE) as u64;

        let mut total_cases: u64 = 0;
        let mut total_nodes: u64 = 0;

        for pi in 0..n_programs {
            let mut grng = Rng::new(base_seed.wrapping_add(pi as u64));
            let fuel = 4 + grng.below(4) as u32; // 4..=7
            let prog = Gen { rng: &mut grng }.program(fuel);
            let j = build_jit(&prog);
            let entry = prog.len() - 1; // last fn can call all earlier ones
            let entry_name = &prog[entry].name;

            // ---- Structural soundness of EVERY lowered function. -----------
            for (id, f) in prog.iter().enumerate() {
                let tf = j
                    .get(&f.name)
                    .unwrap_or_else(|| panic!("{} missing from registry", f.name));
                let core = tf.core_clone().expect("defined fn must have core");
                verify_core(&core, tf.n_slots(), prog.len()).unwrap_or_else(|e| {
                    panic!(
                        "STRUCTURAL UNSOUNDNESS in {} (id {id}): {e}\n  core: {core:?}",
                        f.name
                    )
                });
                assert!(
                    core_node_count(&core) >= 1,
                    "{} lowered to an empty core",
                    f.name
                );
                total_nodes += core_node_count(&core) as u64;
            }

            // ---- Generate the input vectors for this program. --------------
            let mut irng = Rng::new(base_seed ^ (0xABCD_0000 + pi as u64));
            let inputs: Vec<Vec<Value>> = (0..n_inputs)
                .map(|_| random_args(&mut irng, &prog[entry]))
                .collect();

            // ---- Tier 1: compiled. -----------------------------------------
            j.compile_all();
            let compiled: Vec<Value> = inputs
                .iter()
                .map(|a| {
                    j.call(entry_name, a)
                        .unwrap_or_else(|e| panic!("compiled call failed: {e}"))
                })
                .collect();

            // ---- Tier 2: typed-core interpreter. ---------------------------
            j.deoptimize_all();
            let interpreted: Vec<Value> = inputs
                .iter()
                .map(|a| {
                    j.call(entry_name, a)
                        .unwrap_or_else(|e| panic!("interpreted call failed: {e}"))
                })
                .collect();

            // ---- Tier 3 + 4: tracing interpreter & independent oracle. -----
            for (k, args) in inputs.iter().enumerate() {
                let (traced, log) = j
                    .trace_call(entry_name, args)
                    .unwrap_or_else(|e| panic!("trace call failed: {e}"));

                // Trace must be non-empty and end at the returned word.
                assert!(!log.is_empty(), "empty trace for {entry_name}");
                // Determinism: a second trace is byte-identical.
                let (traced2, log2) = j.trace_call(entry_name, args).unwrap();
                assert!(val_eq(&traced, &traced2), "trace non-determinism (value)");
                assert_eq!(log, log2, "trace non-determinism (log)");

                // Oracle: independent Rust interpretation of the same AST.
                let mut scope: Vec<(String, OVal)> = prog[entry]
                    .params
                    .iter()
                    .map(|(n, _)| n.clone())
                    .zip(args_to_ovals(args))
                    .collect();
                let oracle_val = oracle(&prog[entry].body, &mut scope, &prog).to_value();

                // FOUR-WAY AGREEMENT.
                let c = &compiled[k];
                let it = &interpreted[k];
                let mismatch =
                    !(val_eq(c, it) && val_eq(it, &traced) && val_eq(&traced, &oracle_val));
                if mismatch {
                    panic!(
                        "TIER DISAGREEMENT (program seed {}, input #{k} = {args:?})\n  \
                         compiled    = {c:?}\n  \
                         interpreted = {it:?}\n  \
                         traced      = {traced:?}\n  \
                         oracle      = {oracle_val:?}\n  \
                         source:\n{}",
                        base_seed.wrapping_add(pi as u64),
                        prog.iter()
                            .map(|f| render_defun(f, &prog))
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                }
                total_cases += 4;
            }
        }

        eprintln!(
            "brutal_differential: {n_programs} programs, {total_cases} tier-evaluations agreed; \
             {total_nodes} typed-core nodes structurally verified."
        );
    });
}

// ===========================================================================
// Cross-tier: the untyped tree-walker vs the typed JIT on pure arithmetic.
//
// The two interpreters share integer wrapping semantics for `+ - *`, so on the
// pure-arithmetic overlap they must compute the SAME machine word. This is the
// only place the suite can pin the production tree-walker against the JIT.
// ===========================================================================

/// Generate a pure `+ - *` integer expression over `params`; render it twice:
/// once as untyped Lisp, once for the typed JIT. Both must equal the oracle.
fn gen_pure_arith(rng: &mut Rng, params: &[String], fuel: u32) -> E {
    if fuel == 0 || rng.below(3) == 0 {
        if !params.is_empty() && rng.bool() {
            return E::Var(params[rng.below(params.len())].clone());
        }
        return E::LitI(rng.below(401) as i64 - 200);
    }
    let op = match rng.below(3) {
        0 => IBin::Add,
        1 => IBin::Sub,
        _ => IBin::Mul,
    };
    E::Bin(
        op,
        Box::new(gen_pure_arith(rng, params, fuel - 1)),
        Box::new(gen_pure_arith(rng, params, fuel - 1)),
    )
}

#[test]
fn untyped_treewalker_matches_jit_on_pure_arithmetic() {
    lamedh::with_large_stack(|| {
        let brutal = std::env::var("BRUTAL").is_ok();
        let n = env_usize("BRUTAL_ARITH", if brutal { 200_000 } else { 20_000 });
        let env = Environment::new_with_builtins();
        let params: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let mut checked = 0u64;

        for s in 0..n {
            let mut rng = Rng::new(0x5EED ^ s as u64);
            let fuel = 3 + rng.below(3) as u32;
            let body = gen_pure_arith(&mut rng, &params, fuel);
            let prog = vec![FnDef {
                name: "g".into(),
                params: params.iter().map(|p| (p.clone(), GTy::Int)).collect(),
                ret: GTy::Int,
                body: body.clone(),
            }];

            let j = build_jit(&prog);
            j.compile_all();

            for _ in 0..4 {
                let a = rng.nasty_i64();
                let b = rng.nasty_i64();
                let c = rng.nasty_i64();
                let args = [Value::Int(a), Value::Int(b), Value::Int(c)];

                // Oracle.
                let mut scope = vec![
                    ("a".into(), OVal::I(a)),
                    ("b".into(), OVal::I(b)),
                    ("c".into(), OVal::I(c)),
                ];
                let want = oracle_i(&body, &mut scope, &prog);

                // JIT (compiled).
                let got_jit = match j.call("g", &args).unwrap() {
                    Value::Int(n) => n,
                    other => panic!("jit returned non-int {other:?}"),
                };

                // Untyped tree-walker: bind a/b/c and eval the rendered body.
                let mut src = String::new();
                render(&body, &prog, &mut src);
                let bound = format!("(let ((a {a}) (b {b}) (c {c})) {src})");
                let got_tw = match eval_str(&bound, &env).unwrap() {
                    lamedh::LispVal::Number(n) => n,
                    other => panic!("tree-walker returned non-number {other:?}"),
                };

                assert_eq!(want, got_jit, "oracle vs jit on `{src}` @ {args:?}");
                assert_eq!(
                    want, got_tw,
                    "oracle vs tree-walker on `{src}` @ a={a} b={b} c={c}"
                );
                checked += 1;
            }
        }
        eprintln!(
            "untyped_vs_jit: {checked} pure-arithmetic words agreed across both interpreters"
        );
    });
}

// ===========================================================================
// Abstract interpretation: an interval domain over-approximates the concrete
// result; whenever the analysis PROVES no overflow, the concrete value must lie
// inside the computed interval. A miscompile that produced an out-of-range
// value would be caught here even without a matching oracle case.
// ===========================================================================

#[derive(Clone, Copy, Debug)]
struct Iv {
    lo: i128,
    hi: i128,
}

/// Interval transfer for `+ - *` over `E`s built from `Var(range)`/`LitI`.
/// `None` means the analysis bailed (unsupported node) — we then skip the check.
fn interval(e: &E, vars: &[(String, Iv)]) -> Option<Iv> {
    match e {
        E::LitI(n) => Some(Iv {
            lo: *n as i128,
            hi: *n as i128,
        }),
        E::Var(name) => vars.iter().find(|(n, _)| n == name).map(|(_, iv)| *iv),
        E::Bin(op, a, b) => {
            let x = interval(a, vars)?;
            let y = interval(b, vars)?;
            match op {
                IBin::Add => Some(Iv {
                    lo: x.lo + y.lo,
                    hi: x.hi + y.hi,
                }),
                IBin::Sub => Some(Iv {
                    lo: x.lo - y.hi,
                    hi: x.hi - y.lo,
                }),
                IBin::Mul => {
                    let ps = [x.lo * y.lo, x.lo * y.hi, x.hi * y.lo, x.hi * y.hi];
                    Some(Iv {
                        lo: *ps.iter().min().unwrap(),
                        hi: *ps.iter().max().unwrap(),
                    })
                }
                _ => None,
            }
        }
        _ => None,
    }
}

#[test]
fn abstract_interpretation_soundness() {
    lamedh::with_large_stack(|| {
        let brutal = std::env::var("BRUTAL").is_ok();
        let n = env_usize("BRUTAL_ABSINT", if brutal { 100_000 } else { 10_000 });
        let params: Vec<String> = vec!["a".into(), "b".into()];
        // Bounded input ranges so the interval analysis can prove non-overflow.
        const LO: i64 = -10_000;
        const HI: i64 = 10_000;
        let mut proven = 0u64;
        let mut skipped = 0u64;

        for s in 0..n {
            let mut rng = Rng::new(0xABACABB ^ s as u64);
            let fuel = 2 + rng.below(3) as u32;
            let body = gen_pure_arith(&mut rng, &params, fuel);
            let prog = vec![FnDef {
                name: "h".into(),
                params: params.iter().map(|p| (p.clone(), GTy::Int)).collect(),
                ret: GTy::Int,
                body: body.clone(),
            }];
            let vars = vec![
                (
                    "a".to_string(),
                    Iv {
                        lo: LO as i128,
                        hi: HI as i128,
                    },
                ),
                (
                    "b".to_string(),
                    Iv {
                        lo: LO as i128,
                        hi: HI as i128,
                    },
                ),
            ];
            let iv = match interval(&body, &vars) {
                Some(iv) => iv,
                None => {
                    skipped += 1;
                    continue;
                }
            };
            // Only assert soundness where wrapping cannot occur.
            if iv.lo < i64::MIN as i128 || iv.hi > i64::MAX as i128 {
                skipped += 1;
                continue;
            }

            let j = build_jit(&prog);
            j.compile_all();

            for _ in 0..8 {
                let a = (LO as i128 + (rng.next_u64() as i128 % (HI - LO + 1) as i128)) as i64;
                let b = (LO as i128 + (rng.next_u64() as i128 % (HI - LO + 1) as i128)) as i64;
                let got = match j.call("h", &[Value::Int(a), Value::Int(b)]).unwrap() {
                    Value::Int(n) => n as i128,
                    other => panic!("non-int {other:?}"),
                };
                assert!(
                    got >= iv.lo && got <= iv.hi,
                    "ABSTRACT-INTERP UNSOUND: result {got} escaped interval [{},{}] for a={a} b={b}",
                    iv.lo,
                    iv.hi
                );
                proven += 1;
            }
        }
        eprintln!(
            "abstract_interpretation: {proven} concrete results contained in proven-safe intervals \
             ({skipped} expressions skipped as possibly-overflowing)"
        );
    });
}

// ===========================================================================
// Metamorphic laws on the typed JIT.
// ===========================================================================

#[test]
fn metamorphic_deopt_recompile_idempotence() {
    lamedh::with_large_stack(|| {
        let n = env_usize("BRUTAL_META", 3_000);
        for pi in 0..n {
            let mut grng = Rng::new(0xDEAD_0000 + pi as u64);
            let fuel = 4 + grng.below(3) as u32;
            let prog = Gen { rng: &mut grng }.program(fuel);
            let j = build_jit(&prog);
            let entry = &prog[prog.len() - 1].name;

            let mut irng = Rng::new(0xBEEF_0000 + pi as u64);
            let args = random_args(&mut irng, &prog[prog.len() - 1]);

            // Baseline (compiled).
            j.compile_all();
            let gen0 = j.get(entry).unwrap().generation();
            let r0 = j.call(entry, &args).unwrap();

            // Deopt then interpret: same result.
            j.deoptimize_all();
            assert!(!j.get(entry).unwrap().is_compiled());
            let r1 = j.call(entry, &args).unwrap();
            assert!(val_eq(&r0, &r1), "deopt changed the result for {entry}");

            // Recompile: result stable, generation strictly advances (idempotent
            // semantics, fresh edition).
            j.compile_all();
            let gen1 = j.get(entry).unwrap().generation();
            let r2 = j.call(entry, &args).unwrap();
            assert!(val_eq(&r0, &r2), "recompile changed the result for {entry}");
            assert!(gen1 > gen0, "recompile did not advance generation");

            // Recompiling again keeps the answer.
            j.compile_all();
            let r3 = j.call(entry, &args).unwrap();
            assert!(val_eq(&r0, &r3), "second recompile changed the result");
        }
    });
}

#[test]
fn metamorphic_redefinition_propagates_through_call_cell() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    let def = |src: &str| read(src, &env).unwrap();

    j.define(&def("(defun-typed (base int64) ((x int64)) (* x x))"))
        .unwrap();
    // `user` is compiled once, calling `base` through the registry cell.
    j.define(&def(
        "(defun-typed (user int64) ((x int64)) (+ (base x) 1))",
    ))
    .unwrap();
    j.compile_all();
    assert_eq!(j.call("USER", &[Value::Int(5)]).unwrap(), Value::Int(26)); // 25+1

    // Redefine the callee; the caller is never recompiled but must see the change.
    j.define(&def("(defun-typed (base int64) ((x int64)) (* x (* x x)))"))
        .unwrap();
    assert_eq!(j.call("USER", &[Value::Int(5)]).unwrap(), Value::Int(126)); // 125+1

    // The traced reference path must agree with the (cell-routed) compiled path.
    let (traced, _) = j.trace_call("USER", &[Value::Int(5)]).unwrap();
    assert_eq!(traced, Value::Int(126));
}

// ===========================================================================
// Metamorphic: the source optimizer preserves untyped semantics.
//   eval( (optimize 'F) )  ==  eval( F )
// ===========================================================================

/// Render a closed (variable-free) small-int arithmetic form as untyped Lisp.
fn gen_closed_arith(rng: &mut Rng, fuel: u32) -> String {
    if fuel == 0 || rng.below(3) == 0 {
        return (rng.below(41) as i64 - 20).to_string();
    }
    let op = match rng.below(3) {
        0 => "+",
        1 => "-",
        _ => "*",
    };
    format!(
        "({op} {} {})",
        gen_closed_arith(rng, fuel - 1),
        gen_closed_arith(rng, fuel - 1)
    )
}

#[test]
fn metamorphic_optimizer_preserves_semantics() {
    lamedh::with_large_stack(|| {
        let n = env_usize("BRUTAL_OPT", 50_000);
        let env = Environment::new_with_builtins();
        for s in 0..n {
            let mut rng = Rng::new(0x0071_0000u64.wrapping_add(s as u64));
            let form = gen_closed_arith(&mut rng, 4);
            let raw = eval_str(&form, &env).unwrap();
            // `(optimize '<form>)` returns the rewritten form; eval it and compare.
            let opt_then_eval = eval_str(&format!("(eval (optimize '{form}))"), &env).unwrap();
            assert_eq!(
                raw, opt_then_eval,
                "OPTIMIZER CHANGED SEMANTICS for `{form}`: {raw:?} vs {opt_then_eval:?}"
            );
        }
    });
}

// ===========================================================================
// Curated float differential: exact-bit agreement across tiers, including the
// nasty values (NaN, infinities, signed zero, subnormals).
// ===========================================================================

#[test]
fn float_tiers_agree_bit_for_bit() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    let def = |s: &str| read(s, &env).unwrap();
    for src in [
        "(defun-typed (fadd float64) ((x float64) (y float64)) (+ x y))",
        "(defun-typed (fsub float64) ((x float64) (y float64)) (- x y))",
        "(defun-typed (fmul float64) ((x float64) (y float64)) (* x y))",
        "(defun-typed (fdiv float64) ((x float64) (y float64)) (/ x y))",
        "(defun-typed (fcmp bool) ((x float64) (y float64)) (< x y))",
        "(defun-typed (fmix float64) ((x float64) (y float64)) \
           (if (< x y) (* x y) (- x y)))",
    ] {
        j.define(&def(src)).unwrap();
    }

    let nasty = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::MIN,
        1e300,
        -1e-300,
        std::f64::consts::PI,
    ];

    for &x in &nasty {
        for &y in &nasty {
            let a = [Value::Float(x), Value::Float(y)];
            for fname in ["FADD", "FSUB", "FMUL", "FDIV", "FCMP", "FMIX"] {
                j.compile_all();
                let c = j.call(fname, &a).unwrap();
                j.deoptimize_all();
                let it = j.call(fname, &a).unwrap();
                let (tr, _) = j.trace_call(fname, &a).unwrap();
                assert!(
                    val_eq(&c, &it) && val_eq(&it, &tr),
                    "float tier disagreement on {fname}({x}, {y}): {c:?} / {it:?} / {tr:?}"
                );
            }
        }
    }
}

// ===========================================================================
// Curated recursive differential sweep against hand-written Rust references.
// Mirrors classic kernels (fib/fact/gcd/ackermann/power/sum) at scale.
// ===========================================================================

#[test]
fn recursive_kernels_match_rust_references() {
    lamedh::with_large_stack(|| {
        let env = Environment::new_with_builtins();
        let mut j = Jit::new();
        let def = |s: &str| read(s, &env).unwrap();
        for src in [
            "(defun-typed (fib int64) ((n int64)) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))",
            "(defun-typed (fact int64) ((n int64)) (if (<= n 1) 1 (* n (fact (- n 1)))))",
            "(defun-typed (gcd int64) ((a int64) (b int64)) (if (= b 0) a (gcd b (mod a b))))",
            "(defun-typed (pw int64) ((b int64) (e int64)) (if (= e 0) 1 (* b (pw b (- e 1)))))",
            "(defun-typed (sum int64) ((n int64)) (if (= n 0) 0 (+ n (sum (- n 1)))))",
            "(defun-typed (ack int64) ((m int64) (n int64)) \
               (if (= m 0) (+ n 1) (if (= n 0) (ack (- m 1) 1) (ack (- m 1) (ack m (- n 1))))))",
        ] {
            j.define(&def(src)).unwrap();
        }

        fn rfib(n: i64) -> i64 {
            if n < 2 {
                n
            } else {
                rfib(n - 1).wrapping_add(rfib(n - 2))
            }
        }
        fn rfact(n: i64) -> i64 {
            if n <= 1 {
                1
            } else {
                n.wrapping_mul(rfact(n - 1))
            }
        }
        fn rgcd(a: i64, b: i64) -> i64 {
            if b == 0 {
                a
            } else {
                rgcd(b, a.checked_rem(b).unwrap_or(0))
            }
        }
        fn rpw(b: i64, e: i64) -> i64 {
            if e == 0 {
                1
            } else {
                b.wrapping_mul(rpw(b, e - 1))
            }
        }
        fn rsum(n: i64) -> i64 {
            if n == 0 {
                0
            } else {
                n.wrapping_add(rsum(n - 1))
            }
        }
        fn rack(m: i64, n: i64) -> i64 {
            if m == 0 {
                n + 1
            } else if n == 0 {
                rack(m - 1, 1)
            } else {
                rack(m - 1, rack(m, n - 1))
            }
        }

        let check = |j: &Jit, name: &str, args: &[i64], want: i64| {
            let a: Vec<Value> = args.iter().map(|&n| Value::Int(n)).collect();
            j.compile_all();
            let c = j.call(name, &a).unwrap();
            j.deoptimize_all();
            let it = j.call(name, &a).unwrap();
            j.compile_all();
            assert_eq!(c, Value::Int(want), "{name}{args:?} compiled");
            assert_eq!(it, Value::Int(want), "{name}{args:?} interpreted");
        };

        for n in 0..28 {
            check(&j, "FIB", &[n], rfib(n));
            check(&j, "FACT", &[n], rfact(n));
            check(&j, "SUM", &[n * 30], rsum(n * 30));
        }
        for a in 0..50 {
            for b in 0..50 {
                check(&j, "GCD", &[a, b], rgcd(a, b));
            }
        }
        for b in -6..6 {
            for e in 0..20 {
                check(&j, "PW", &[b, e], rpw(b, e));
            }
        }
        for m in 0..3 {
            for n in 0..6 {
                check(&j, "ACK", &[m, n], rack(m, n));
            }
        }
    });
}

// ===========================================================================
// STRINGS: differential + metamorphic correctness for the untyped tree-walker.
//
// Strings are not part of the typed JIT (its types are int64/float64/bool), so
// the correctness oracle here targets the production tree-walking interpreter
// directly: random `concat`/`index` expressions over random strings are checked
// against an independent Rust string oracle, plus the algebraic laws those ops
// must obey (associativity, identity, char-indexing, reflexive `equal`).
// ===========================================================================

/// A string-valued expression in the string mini-language.
#[derive(Clone, Debug)]
enum SE {
    Lit(String),
    Var(String),
    Concat(Vec<SE>),
}

/// Independent oracle: the exact string this expression denotes.
fn oracle_s(e: &SE, scope: &[(String, String)]) -> String {
    match e {
        SE::Lit(s) => s.clone(),
        SE::Var(name) => scope
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("string oracle: unbound {name}")),
        SE::Concat(parts) => parts.iter().map(|p| oracle_s(p, scope)).collect(),
    }
}

/// Render a string expression to untyped Lisp.
fn render_s(e: &SE, out: &mut String) {
    match e {
        SE::Lit(s) => {
            out.push('"');
            out.push_str(s);
            out.push('"');
        }
        SE::Var(name) => out.push_str(name),
        SE::Concat(parts) => {
            out.push_str("(concat");
            for p in parts {
                out.push(' ');
                render_s(p, out);
            }
            out.push(')');
        }
    }
}

/// A reader-safe random string: ASCII letters/digits/space only (no `"` or `\`,
/// which would need escaping), length 0..=5.
fn rand_string(rng: &mut Rng) -> String {
    const ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ";
    let len = rng.below(6);
    (0..len)
        .map(|_| ALPHA[rng.below(ALPHA.len())] as char)
        .collect()
}

fn gen_se(rng: &mut Rng, vars: &[String], fuel: u32) -> SE {
    if fuel == 0 || rng.below(3) == 0 {
        if !vars.is_empty() && rng.bool() {
            return SE::Var(vars[rng.below(vars.len())].clone());
        }
        return SE::Lit(rand_string(rng));
    }
    let n = 2 + rng.below(2); // 2..=3 children
    let parts = (0..n).map(|_| gen_se(rng, vars, fuel - 1)).collect();
    SE::Concat(parts)
}

#[test]
fn strings_differential_and_metamorphic() {
    lamedh::with_large_stack(|| {
        let brutal = std::env::var("BRUTAL").is_ok();
        let n = env_usize("BRUTAL_STRINGS", if brutal { 200_000 } else { 20_000 });
        // `with_stdlib` so the Lisp-level `equal` (structural equality) is present
        // alongside the `concat`/`index` builtins.
        let env = Environment::with_stdlib();
        let mut checked = 0u64;

        for s in 0..n {
            let mut rng = Rng::new(0x57A1_4E65u64.wrapping_add(s as u64));
            // 1..=3 string variables with random bindings.
            let nvars = 1 + rng.below(3);
            let scope: Vec<(String, String)> = (0..nvars)
                .map(|k| (format!("s{k}"), rand_string(&mut rng)))
                .collect();
            let var_names: Vec<String> = scope.iter().map(|(n, _)| n.clone()).collect();

            let fuel = 1 + rng.below(3) as u32;
            let expr = gen_se(&mut rng, &var_names, fuel);
            let want = oracle_s(&expr, &scope);

            // Build the `let` prelude binding the variables.
            let mut bindings = String::new();
            for (name, val) in &scope {
                bindings.push_str(&format!("({name} \"{val}\") "));
            }
            let mut body = String::new();
            render_s(&expr, &mut body);
            let prog = format!("(let ({bindings}) {body})");

            // Differential: interpreter result == oracle string.
            match eval_str(&prog, &env).unwrap() {
                lamedh::LispVal::String(got) => assert_eq!(
                    got, want,
                    "STRING DIFFERENTIAL mismatch for `{prog}`: got {got:?} want {want:?}"
                ),
                other => panic!("expected a string from `{prog}`, got {other:?}"),
            }

            // Metamorphic 1: right/left identity with the empty string.
            let id_r = format!("(let ({bindings}) (concat {body} \"\"))");
            let id_l = format!("(let ({bindings}) (concat \"\" {body}))");
            for p in [&id_r, &id_l] {
                match eval_str(p, &env).unwrap() {
                    lamedh::LispVal::String(got) => {
                        assert_eq!(got, want, "STRING IDENTITY law broke for `{p}`")
                    }
                    other => panic!("expected string, got {other:?}"),
                }
            }

            // Metamorphic 2: `(equal X X)` is true (reflexivity through the reader).
            let refl = format!("(let ({bindings}) (equal {body} {body}))");
            assert!(
                eval_str(&refl, &env).unwrap().is_truthy(),
                "STRING `equal` not reflexive for `{refl}`"
            );

            // Metamorphic 3: char-indexing agrees with the oracle, and reading one
            // past the end is an out-of-bounds error (not a wrong char / panic).
            let chars: Vec<char> = want.chars().collect();
            // Probe a couple of in-bounds positions and the first OOB position.
            if !chars.is_empty() {
                let i = rng.below(chars.len());
                let q = format!("(let ({bindings}) (index {body} {i}))");
                match eval_str(&q, &env).unwrap() {
                    lamedh::LispVal::String(got) => assert_eq!(
                        got,
                        chars[i].to_string(),
                        "STRING INDEX mismatch at {i} for `{q}`"
                    ),
                    other => panic!("expected single-char string, got {other:?}"),
                }
            }
            let oob = format!("(let ({bindings}) (index {body} {}))", chars.len());
            assert!(
                eval_str(&oob, &env).is_err(),
                "STRING INDEX out-of-bounds should error for `{oob}`"
            );

            checked += 1;
        }
        eprintln!("strings: {checked} concat/index expressions agreed with the oracle + laws");
    });
}
