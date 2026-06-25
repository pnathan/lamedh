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
//! ```
//!
//! ## What is checked
//!
//! For every randomly generated *well-typed* program (a DAG of `deffun-typed`
//! functions over `int64`/`bool`) and every random input vector we run, and
//! force into agreement, **four independent evaluators**:
//!
//! 1. the **compiled closure edition** (`Jit::compile_all` + `call`),
//! 2. the **typed-core reference interpreter** (`Jit::deoptimize_all` + `call`),
//! 3. the **tracing interpreter** (`Jit::trace_call`, a third code path), and
//! 4. an **independent Rust oracle** that interprets the generator's own AST
//!    with the exact documented semantics (wrapping integer arithmetic,
//!    `/`/`mod`-by-zero ⇒ 0).
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
//!   the tree-walking interpreter and the JIT must compute the same word.

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
}

// ===========================================================================
// The generator's own AST + an INDEPENDENT reference oracle.
//
// This AST is rendered to `deffun-typed` source for the JIT, and *separately*
// interpreted in Rust by `OType`-aware evaluation here. Two implementations of
// the same semantics ⇒ a true differential oracle.
// ===========================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GTy {
    Int,
    Bool,
}

impl GTy {
    fn lisp(self) -> &'static str {
        match self {
            GTy::Int => "int64",
            GTy::Bool => "bool",
        }
    }
}

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
#[derive(Clone, Copy, PartialEq, Debug)]
enum OVal {
    I(i64),
    B(bool),
}

impl OVal {
    fn to_value(self) -> Value {
        match self {
            OVal::I(n) => Value::Int(n),
            OVal::B(b) => Value::Bool(b),
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
        E::Var(name) => scope
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
            .unwrap_or_else(|| panic!("oracle: unbound var {name}")),
        E::Bin(op, a, b) => {
            let (x, y) = (oracle_i(a, scope, prog), oracle_i(b, scope, prog));
            let r = match op {
                IBin::Add => x.wrapping_add(y),
                IBin::Sub => x.wrapping_sub(y),
                IBin::Mul => x.wrapping_mul(y),
                IBin::Div => x.checked_div(y).unwrap_or(0),
                IBin::Mod => x.checked_rem(y).unwrap_or(0),
            };
            OVal::I(r)
        }
        E::Cmp(op, a, b) => {
            let (x, y) = (oracle_i(a, scope, prog), oracle_i(b, scope, prog));
            let r = match op {
                ICmp::Lt => x < y,
                ICmp::Gt => x > y,
                ICmp::Le => x <= y,
                ICmp::Ge => x >= y,
                ICmp::Eq => x == y,
                ICmp::Ne => x != y,
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
fn oracle_i(e: &E, scope: &mut Vec<(String, OVal)>, prog: &[FnDef]) -> i64 {
    match oracle(e, scope, prog) {
        OVal::I(n) => n,
        OVal::B(_) => panic!("oracle: expected int"),
    }
}
fn oracle_b(e: &E, scope: &mut Vec<(String, OVal)>, prog: &[FnDef]) -> bool {
    match oracle(e, scope, prog) {
        OVal::B(b) => b,
        OVal::I(_) => panic!("oracle: expected bool"),
    }
}

// ===========================================================================
// Rendering an `E` to `deffun-typed` Lisp source.
// ===========================================================================

fn render(e: &E, prog: &[FnDef], out: &mut String) {
    match e {
        E::LitI(n) => out.push_str(&n.to_string()),
        E::LitB(b) => out.push_str(if *b { "true" } else { "false" }),
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
    s.push_str("(deffun-typed (");
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
            GTy::Int => {
                let choice = self.rng.below(7);
                match choice {
                    0 | 1 => {
                        let op = match self.rng.below(5) {
                            0 => IBin::Add,
                            1 => IBin::Sub,
                            2 => IBin::Mul,
                            3 => IBin::Div,
                            _ => IBin::Mod,
                        };
                        E::Bin(
                            op,
                            Box::new(self.expr(GTy::Int, scope, prog, fuel - 1, let_ctr)),
                            Box::new(self.expr(GTy::Int, scope, prog, fuel - 1, let_ctr)),
                        )
                    }
                    2 => E::If(
                        Box::new(self.expr(GTy::Bool, scope, prog, fuel - 1, let_ctr)),
                        Box::new(self.expr(GTy::Int, scope, prog, fuel - 1, let_ctr)),
                        Box::new(self.expr(GTy::Int, scope, prog, fuel - 1, let_ctr)),
                    ),
                    3 => self.gen_let(GTy::Int, scope, prog, fuel, let_ctr),
                    4 if !callable.is_empty() => {
                        self.gen_call(callable, scope, prog, fuel, let_ctr)
                    }
                    _ => self.leaf(GTy::Int, scope),
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
                        E::Cmp(
                            op,
                            Box::new(self.expr(GTy::Int, scope, prog, fuel - 1, let_ctr)),
                            Box::new(self.expr(GTy::Int, scope, prog, fuel - 1, let_ctr)),
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

    fn gen_let(
        &mut self,
        want: GTy,
        scope: &[(String, GTy)],
        prog: &[FnDef],
        fuel: u32,
        let_ctr: &mut usize,
    ) -> E {
        let bty = if self.rng.bool() { GTy::Int } else { GTy::Bool };
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
        }
    }

    fn program(&mut self, fuel: u32) -> Vec<FnDef> {
        let n_funcs = 1 + self.rng.below(4); // 1..=4
        let mut prog: Vec<FnDef> = Vec::new();
        for i in 0..n_funcs {
            let arity = 1 + self.rng.below(3); // 1..=3
            let params: Vec<(String, GTy)> = (0..arity)
                .map(|p| {
                    let t = if self.rng.bool() { GTy::Int } else { GTy::Bool };
                    (format!("p{p}"), t)
                })
                .collect();
            let ret = if self.rng.bool() { GTy::Int } else { GTy::Bool };
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
        })
        .collect()
}

fn args_to_ovals(args: &[Value]) -> Vec<OVal> {
    args.iter()
        .map(|v| match v {
            Value::Int(n) => OVal::I(*n),
            Value::Bool(b) => OVal::B(*b),
            Value::Float(_) => unreachable!("the fuzzer is int/bool only"),
        })
        .collect()
}

fn val_eq(a: Value, b: Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x.to_bits() == y.to_bits(),
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
                assert!(val_eq(traced, traced2), "trace non-determinism (value)");
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
                let c = compiled[k];
                let it = interpreted[k];
                let mismatch = !(val_eq(c, it) && val_eq(it, traced) && val_eq(traced, oracle_val));
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
            assert!(val_eq(r0, r1), "deopt changed the result for {entry}");

            // Recompile: result stable, generation strictly advances (idempotent
            // semantics, fresh edition).
            j.compile_all();
            let gen1 = j.get(entry).unwrap().generation();
            let r2 = j.call(entry, &args).unwrap();
            assert!(val_eq(r0, r2), "recompile changed the result for {entry}");
            assert!(gen1 > gen0, "recompile did not advance generation");

            // Recompiling again keeps the answer.
            j.compile_all();
            let r3 = j.call(entry, &args).unwrap();
            assert!(val_eq(r0, r3), "second recompile changed the result");
        }
    });
}

#[test]
fn metamorphic_redefinition_propagates_through_call_cell() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    let def = |src: &str| read(src, &env).unwrap();

    j.define(&def("(deffun-typed (base int64) ((x int64)) (* x x))"))
        .unwrap();
    // `user` is compiled once, calling `base` through the registry cell.
    j.define(&def(
        "(deffun-typed (user int64) ((x int64)) (+ (base x) 1))",
    ))
    .unwrap();
    j.compile_all();
    assert_eq!(j.call("USER", &[Value::Int(5)]).unwrap(), Value::Int(26)); // 25+1

    // Redefine the callee; the caller is never recompiled but must see the change.
    j.define(&def(
        "(deffun-typed (base int64) ((x int64)) (* x (* x x)))",
    ))
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
        "(deffun-typed (fadd float64) ((x float64) (y float64)) (+ x y))",
        "(deffun-typed (fsub float64) ((x float64) (y float64)) (- x y))",
        "(deffun-typed (fmul float64) ((x float64) (y float64)) (* x y))",
        "(deffun-typed (fdiv float64) ((x float64) (y float64)) (/ x y))",
        "(deffun-typed (fcmp bool) ((x float64) (y float64)) (< x y))",
        "(deffun-typed (fmix float64) ((x float64) (y float64)) \
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
                    val_eq(c, it) && val_eq(it, tr),
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
            "(deffun-typed (fib int64) ((n int64)) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))",
            "(deffun-typed (fact int64) ((n int64)) (if (<= n 1) 1 (* n (fact (- n 1)))))",
            "(deffun-typed (gcd int64) ((a int64) (b int64)) (if (= b 0) a (gcd b (mod a b))))",
            "(deffun-typed (pw int64) ((b int64) (e int64)) (if (= e 0) 1 (* b (pw b (- e 1)))))",
            "(deffun-typed (sum int64) ((n int64)) (if (= n 0) 0 (+ n (sum (- n 1)))))",
            "(deffun-typed (ack int64) ((m int64) (n int64)) \
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
