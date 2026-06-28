use super::*;
// ---------------------------------------------------------------------------
// Runtime: interpreter and compiler over unboxed u64 words.
// ---------------------------------------------------------------------------

/// Call context: the function table (so calls dispatch through the registry
/// cell) plus the **call arena** that roots every array/struct buffer allocated
/// during the call. Compound values live as pointers into these `Box<[u64]>`
/// buffers; the arena (and therefore the buffers) is dropped when the top-level
/// membrane call returns — after any compound result has been copied out. A
/// `Box<[u64]>`'s heap data pointer is stable across arena `Vec` growth, so
/// native code may hold a raw `base` for the duration of a call.
pub struct Ctx<'a> {
    pub(super) funcs: &'a [Rc<TypedFn>],
    pub(super) arena: RefCell<Vec<Box<[u64]>>>,
}

impl Ctx<'_> {
    #[inline]
    pub(super) fn call(&self, id: usize, args: &[u64]) -> u64 {
        self.funcs[id].invoke(args, self)
    }

    /// Allocate an `n`-element buffer `[n, 0, 0, …]` in the arena and return a
    /// raw pointer to its header word. The arena owns the `Box`, keeping the
    /// data pointer valid (and stable) until the call returns.
    pub(super) fn alloc_buffer(&self, n: usize) -> *mut u64 {
        let mut buf = vec![0u64; n + 1].into_boxed_slice();
        buf[0] = n as u64;
        let ptr = buf.as_mut_ptr();
        self.arena.borrow_mut().push(buf);
        ptr
    }
}

/// Host trampoline for in-native array/struct allocation: allocate an
/// `n`-element buffer in the call arena and return its header pointer.
///
/// # Safety
/// Called only from Cranelift-generated code with the `ctx` pointer threaded
/// from the native entry; `ctx` must point to the live [`Ctx`] for the call.
#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn jit_alloc(ctx: *const core::ffi::c_void, n: u64) -> *mut u64 {
    let ctx = unsafe { &*(ctx as *const Ctx) };
    ctx.alloc_buffer(n as usize)
}

pub(super) fn int_bin(op: BinOp, x: i64, y: i64) -> i64 {
    match op {
        BinOp::Add => x.wrapping_add(y),
        BinOp::Sub => x.wrapping_sub(y),
        BinOp::Mul => x.wrapping_mul(y),
        BinOp::Div => x.checked_div(y).unwrap_or(0),
        BinOp::Mod => x.checked_rem(y).unwrap_or(0),
    }
}

pub(super) fn float_bin(op: BinOp, x: f64, y: f64) -> f64 {
    match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div => x / y,
        BinOp::Mod => x % y,
    }
}

pub(super) fn int_cmp(op: CmpOp, x: i64, y: i64) -> bool {
    match op {
        CmpOp::Lt => x < y,
        CmpOp::Gt => x > y,
        CmpOp::Le => x <= y,
        CmpOp::Ge => x >= y,
        CmpOp::Eq => x == y,
        CmpOp::Ne => x != y,
    }
}

pub(super) fn float_cmp(op: CmpOp, x: f64, y: f64) -> bool {
    match op {
        CmpOp::Lt => x < y,
        CmpOp::Gt => x > y,
        CmpOp::Le => x <= y,
        CmpOp::Ge => x >= y,
        CmpOp::Eq => x == y,
        CmpOp::Ne => x != y,
    }
}

// --- flat-buffer access shared by the interpreter and closure backends ------
// All compound values are a pointer to a `[len, e0, e1, …]` buffer. Access is
// bounds-checked (out-of-range load → 0, store → no-op) to stay panic-free and
// to agree with the native edition's guarded loads/stores.

/// # Safety: `base` must be a live buffer pointer from [`Ctx::alloc_buffer`].
#[inline]
unsafe fn buf_get(base: u64, idx: i64) -> u64 {
    let p = base as *const u64;
    let len = unsafe { *p } as i64;
    if idx < 0 || idx >= len {
        0
    } else {
        unsafe { *p.add(idx as usize + 1) }
    }
}
/// # Safety: as [`buf_get`].
#[inline]
unsafe fn buf_set(base: u64, idx: i64, val: u64) {
    let p = base as *mut u64;
    let len = unsafe { *p } as i64;
    if idx >= 0 && idx < len {
        unsafe { *p.add(idx as usize + 1) = val }
    }
}
/// # Safety: as [`buf_get`].
#[inline]
unsafe fn field_get(base: u64, idx: usize) -> u64 {
    unsafe { *(base as *const u64).add(idx + 1) }
}
/// # Safety: as [`buf_get`].
#[inline]
unsafe fn field_set(base: u64, idx: usize, val: u64) {
    unsafe { *(base as *mut u64).add(idx + 1) = val }
}

pub(super) fn eval_core(core: &Core, env: &mut [u64], ctx: &Ctx) -> u64 {
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
        Core::ArrayNew(n) => {
            let len = as_i(eval_core(n, env, ctx)).max(0) as usize;
            ctx.alloc_buffer(len) as u64
        }
        Core::ArrayGet(a, i) => {
            let base = eval_core(a, env, ctx);
            let idx = as_i(eval_core(i, env, ctx));
            unsafe { buf_get(base, idx) }
        }
        Core::ArraySet(a, i, v) => {
            let base = eval_core(a, env, ctx);
            let idx = as_i(eval_core(i, env, ctx));
            let val = eval_core(v, env, ctx);
            unsafe { buf_set(base, idx, val) };
            val
        }
        Core::ArrayLen(a) => {
            let base = eval_core(a, env, ctx);
            unsafe { *(base as *const u64) }
        }
        Core::StructNew(inits) => {
            let vals: Vec<u64> = inits.iter().map(|c| eval_core(c, env, ctx)).collect();
            let base = ctx.alloc_buffer(vals.len());
            for (i, v) in vals.iter().enumerate() {
                unsafe { *base.add(i + 1) = *v };
            }
            base as u64
        }
        Core::FieldGet(s, idx) => {
            let base = eval_core(s, env, ctx);
            unsafe { field_get(base, *idx) }
        }
        Core::FieldSet(s, idx, v) => {
            let base = eval_core(s, env, ctx);
            let val = eval_core(v, env, ctx);
            unsafe { field_set(base, *idx, val) };
            val
        }
        Core::Seq(forms) => {
            let mut r = 0;
            for f in forms {
                r = eval_core(f, env, ctx);
            }
            r
        }
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

pub(super) const NO_SLOT: usize = usize::MAX;
pub(super) const NO_CALLEE: usize = usize::MAX;

/// Tracing twin of [`eval_core`]. Pushes a [`TraceStep`] for every node it
/// actually evaluates (so short-circuited `and`/`or`/`if` branches leave no
/// step, exactly mirroring the evaluation the interpreter performs). It must
/// stay byte-for-byte semantically identical to [`eval_core`]; the two are
/// differential-tested against each other in the suite.
pub(super) fn eval_core_traced(
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
        Core::ArrayNew(n) => {
            let len = as_i(eval_core_traced(n, env, ctx, depth + 1, log)).max(0) as usize;
            step!("arraynew", ctx.alloc_buffer(len) as u64, NO_SLOT, NO_CALLEE)
        }
        Core::ArrayGet(a, i) => {
            let base = eval_core_traced(a, env, ctx, depth + 1, log);
            let idx = as_i(eval_core_traced(i, env, ctx, depth + 1, log));
            step!(
                "arrayget",
                unsafe { buf_get(base, idx) },
                NO_SLOT,
                NO_CALLEE
            )
        }
        Core::ArraySet(a, i, v) => {
            let base = eval_core_traced(a, env, ctx, depth + 1, log);
            let idx = as_i(eval_core_traced(i, env, ctx, depth + 1, log));
            let val = eval_core_traced(v, env, ctx, depth + 1, log);
            unsafe { buf_set(base, idx, val) };
            step!("arrayset", val, NO_SLOT, NO_CALLEE)
        }
        Core::ArrayLen(a) => {
            let base = eval_core_traced(a, env, ctx, depth + 1, log);
            step!(
                "arraylen",
                unsafe { *(base as *const u64) },
                NO_SLOT,
                NO_CALLEE
            )
        }
        Core::StructNew(inits) => {
            let vals: Vec<u64> = inits
                .iter()
                .map(|c| eval_core_traced(c, env, ctx, depth + 1, log))
                .collect();
            let base = ctx.alloc_buffer(vals.len());
            for (i, v) in vals.iter().enumerate() {
                unsafe { *base.add(i + 1) = *v };
            }
            step!("structnew", base as u64, NO_SLOT, NO_CALLEE)
        }
        Core::FieldGet(s, idx) => {
            let base = eval_core_traced(s, env, ctx, depth + 1, log);
            step!(
                "fieldget",
                unsafe { field_get(base, *idx) },
                NO_SLOT,
                NO_CALLEE
            )
        }
        Core::FieldSet(s, idx, v) => {
            let base = eval_core_traced(s, env, ctx, depth + 1, log);
            let val = eval_core_traced(v, env, ctx, depth + 1, log);
            unsafe { field_set(base, *idx, val) };
            step!("fieldset", val, NO_SLOT, NO_CALLEE)
        }
        Core::Seq(forms) => {
            let mut r = 0;
            for f in forms {
                r = eval_core_traced(f, env, ctx, depth + 1, log);
            }
            step!("seq", r, NO_SLOT, NO_CALLEE)
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
        Core::Call(_, args) | Core::StructNew(args) | Core::Seq(args) => {
            args.iter().map(core_node_count).sum()
        }
        Core::ArrayNew(a) | Core::ArrayLen(a) | Core::FieldGet(a, _) => core_node_count(a),
        Core::ArrayGet(a, b) | Core::FieldSet(a, _, b) => core_node_count(a) + core_node_count(b),
        Core::ArraySet(a, b, c) => core_node_count(a) + core_node_count(b) + core_node_count(c),
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
        Core::ArrayNew(a) | Core::ArrayLen(a) | Core::FieldGet(a, _) => {
            verify_core(a, n_slots, n_funcs)
        }
        Core::ArrayGet(a, b) | Core::FieldSet(a, _, b) => {
            verify_core(a, n_slots, n_funcs)?;
            verify_core(b, n_slots, n_funcs)
        }
        Core::ArraySet(a, b, c) => {
            verify_core(a, n_slots, n_funcs)?;
            verify_core(b, n_slots, n_funcs)?;
            verify_core(c, n_slots, n_funcs)
        }
        Core::StructNew(inits) => {
            for c in inits {
                verify_core(c, n_slots, n_funcs)?;
            }
            Ok(())
        }
        Core::Seq(forms) => {
            for c in forms {
                verify_core(c, n_slots, n_funcs)?;
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
        Core::ArrayNew(n) => {
            let cn = compile(n);
            Rc::new(move |e, c| {
                let len = as_i(cn(e, c)).max(0) as usize;
                c.alloc_buffer(len) as u64
            })
        }
        Core::ArrayGet(a, i) => {
            let (ca, ci) = (compile(a), compile(i));
            Rc::new(move |e, c| {
                let base = ca(e, c);
                let idx = as_i(ci(e, c));
                unsafe { buf_get(base, idx) }
            })
        }
        Core::ArraySet(a, i, v) => {
            let (ca, ci, cv) = (compile(a), compile(i), compile(v));
            Rc::new(move |e, c| {
                let base = ca(e, c);
                let idx = as_i(ci(e, c));
                let val = cv(e, c);
                unsafe { buf_set(base, idx, val) };
                val
            })
        }
        Core::ArrayLen(a) => {
            let ca = compile(a);
            Rc::new(move |e, c| {
                let base = ca(e, c);
                unsafe { *(base as *const u64) }
            })
        }
        Core::StructNew(inits) => {
            let cinits: Vec<Compiled> = inits.iter().map(compile).collect();
            Rc::new(move |e, c| {
                let base = c.alloc_buffer(cinits.len());
                for (i, ci) in cinits.iter().enumerate() {
                    let v = ci(e, c);
                    unsafe { *base.add(i + 1) = v };
                }
                base as u64
            })
        }
        Core::FieldGet(s, idx) => {
            let (cs, idx) = (compile(s), *idx);
            Rc::new(move |e, c| {
                let base = cs(e, c);
                unsafe { field_get(base, idx) }
            })
        }
        Core::FieldSet(s, idx, v) => {
            let (cs, idx, cv) = (compile(s), *idx, compile(v));
            Rc::new(move |e, c| {
                let base = cs(e, c);
                let val = cv(e, c);
                unsafe { field_set(base, idx, val) };
                val
            })
        }
        Core::Seq(forms) => {
            let cforms: Vec<Compiled> = forms.iter().map(compile).collect();
            Rc::new(move |e, c| {
                let mut r = 0;
                for cf in &cforms {
                    r = cf(e, c);
                }
                r
            })
        }
    }
}
