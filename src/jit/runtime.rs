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
    /// Set by a compiled/interpreted body, instead of performing the call
    /// itself, when it reaches a *cross-function* tail call (issue #133
    /// Tier 2a). `TypedFn::invoke`'s dispatch loop checks this immediately
    /// after every underlying edition call and, if set, clears it and
    /// re-dispatches to the named function with the given argument words —
    /// an explicit, host-driven trampoline (mirrors the tree-walker's
    /// `TcoStep::TailCall`) that keeps native stack usage O(1) for
    /// arbitrarily deep mutual/general tail recursion, without adopting
    /// Cranelift's `tail` calling convention (assessed and deferred — see
    /// the design note above `compile_native` in `native.rs`). A *self*
    /// tail call (Tier 1) never touches this: it loops within the callee's
    /// own edition instead.
    pub(super) pending_tail: RefCell<Option<(usize, Vec<u64>)>>,
    /// Set by `int_bin` when a checked integer operation overflows (issue #228).
    /// The membrane reads this after the call returns and sets the evaluator's
    /// `OVERFLOW` condition flag.
    pub(super) overflow: Cell<bool>,
    /// Set by `int_bin` when integer division or remainder by zero is attempted.
    /// The membrane reads this and raises a `Division by zero` error.
    pub(super) div_by_zero: Cell<bool>,
    /// Non-tail call depth (issue #271). Incremented/decremented by
    /// [`Ctx::enter_call`]/[`Ctx::exit_call`] around every non-tail call
    /// reached through [`Ctx::call`] — the interpreter and closure editions'
    /// own `Core::Call` arms, and any native call that falls through to the
    /// host trampoline — *and*, via the imported `jit_enter_call`/
    /// `jit_exit_call` trampolines a native edition calls directly (see
    /// `native::compile_native`'s `emit_call`), around the direct
    /// native-to-native `call_indirect` fast path that otherwise never
    /// touches `Ctx::call` at all. See [`Ctx::MAX_CALL_DEPTH`] for the limit
    /// and how it was derived.
    pub(super) depth: Cell<usize>,
    /// First reachable-error condition recorded during a call that has no
    /// sensible "keep computing, flag it" resolution the way overflow/div-by-
    /// zero do (issue #271): an oversized array/struct allocation, the
    /// non-tail recursion cap, calling a `declare-typed`d-but-never-defined
    /// function, or a stale-arity call site left over by an arity-changing
    /// redefinition. Every site that would otherwise panic instead records a
    /// message here (first one wins, matching the tree-walker's left-to-right
    /// evaluation order) and returns a memory-safe substitute value (a
    /// zero-length arena buffer — a valid, non-null pointer that is also a
    /// harmless word if the caller treats it as a scalar) so execution can run
    /// to completion; the membrane checks this after the top-level call
    /// returns and raises it as a Lisp error in place of the (meaningless)
    /// result, exactly as `div_by_zero` already does for its one fixed
    /// message.
    pub(super) pending_error: RefCell<Option<String>>,
}

impl Ctx<'_> {
    /// Byte offset of the `overflow` field from the start of `Ctx`.
    /// Used by native codegen to store the flag without branches.
    pub(super) const OVERFLOW_OFFSET: usize = std::mem::offset_of!(Ctx<'_>, overflow);
    /// Byte offset of the `div_by_zero` field from the start of `Ctx`.
    pub(super) const DIV_BY_ZERO_OFFSET: usize = std::mem::offset_of!(Ctx<'_>, div_by_zero);

    /// Maximum non-tail call depth before a typed call is refused with a
    /// recursion-limit error instead of growing the call stack further
    /// (issue #271).
    ///
    /// Arithmetic: the evaluator thread runs under [`crate::with_large_stack`],
    /// a 512 MiB stack (`INTERPRETER_STACK_SIZE`). The interpreter/closure
    /// editions reached through [`Ctx::call`] recurse as ordinary Rust calls —
    /// `eval_core`/`eval_core_nontail`'s own frame plus the per-call `Vec<u64>`
    /// argument buffer and the callee's fresh `env` `Vec` — call it a generous
    /// 2 KiB/frame upper bound (smaller than, but the same order of magnitude
    /// as, the tree-walker's own per-`eval`-frame budget behind
    /// `DEFAULT_EVAL_DEPTH_LIMIT`, since typed-core frames do far less
    /// per-node bookkeeping). A native frame guarded via
    /// `jit_enter_call`/`jit_exit_call` is far smaller still (a handful of
    /// 8-byte stack slots), so the same cap is comfortably conservative
    /// there too. `MAX_CALL_DEPTH * 2 KiB` ≈ 100 MiB, leaving well over
    /// 400 MiB of headroom in the 512 MiB budget for the rest of the call
    /// chain that led to this typed call (evaluator frames, other typed calls
    /// already on the stack, …) — generous enough not to trip on realistic
    /// recursive typed programs, small enough to trip long before the stack
    /// is actually exhausted.
    pub(super) const MAX_CALL_DEPTH: usize = 50_000;

    /// Record `msg` as the pending error for this call, unless one is already
    /// recorded — first error wins, matching the tree-walker's left-to-right
    /// evaluation order (and the existing overflow-before-div-by-zero
    /// ordering, issue #228).
    pub(super) fn set_pending_error(&self, msg: String) {
        let mut slot = self.pending_error.borrow_mut();
        if slot.is_none() {
            *slot = Some(msg);
        }
    }

    /// Enter a non-tail call: bump the depth counter, or refuse — recording a
    /// recursion-limit [`pending_error`](Ctx::pending_error) — if already at
    /// [`Ctx::MAX_CALL_DEPTH`]. Returns `true` on success; a successful
    /// `enter_call` must be paired with exactly one [`Ctx::exit_call`] once
    /// the call returns.
    pub(super) fn enter_call(&self) -> bool {
        let d = self.depth.get();
        if d >= Self::MAX_CALL_DEPTH {
            self.set_pending_error(format!(
                "recursion limit exceeded ({} non-tail typed calls); rewrite as a tail call or iteratively",
                Self::MAX_CALL_DEPTH
            ));
            false
        } else {
            self.depth.set(d + 1);
            true
        }
    }

    /// Leave a non-tail call entered via [`Ctx::enter_call`].
    pub(super) fn exit_call(&self) {
        self.depth.set(self.depth.get().saturating_sub(1));
    }

    #[inline]
    pub(super) fn call(&self, id: usize, args: &[u64]) -> u64 {
        if !self.enter_call() {
            // Memory-safe substitute: a valid (non-null) pointer to a
            // zero-length buffer, harmless whether the caller treats the
            // result as a scalar word or as an array/struct pointer. The
            // membrane discards it once it sees `pending_error` set.
            return self.alloc_buffer(0) as u64;
        }
        let r = self.funcs[id].invoke(args, self);
        self.exit_call();
        r
    }

    /// Allocate an `n`-element buffer `[n, 0, 0, …]` in the arena and return a
    /// raw pointer to its header word. The arena owns the `Box`, keeping the
    /// data pointer valid (and stable) until the call returns.
    ///
    /// `n` over the limit (issue #271; reachable from `jit_alloc` with a
    /// Cranelift-computed `n` — a negative array-size expression reinterprets
    /// as a huge `usize`) no longer asserts: it records a
    /// [`pending_error`](Ctx::pending_error) mirroring the evaluator's own
    /// `MakeArray` over-limit message (`src/evaluator/apply.rs`) and falls
    /// back to a zero-length allocation, which keeps every bounds-checked
    /// buffer access (`buf_get`/`buf_set`) safe for the rest of the call; the
    /// membrane raises the recorded error once the call returns.
    pub(super) fn alloc_buffer(&self, n: usize) -> *mut u64 {
        const MAX_ELEMENTS: usize = 16 * 1024 * 1024; // 16 M — same cap as interpreted (array)
        let n = if n > MAX_ELEMENTS {
            self.set_pending_error(format!("array: size {n} exceeds maximum of {MAX_ELEMENTS}"));
            0
        } else {
            n
        };
        let mut buf = vec![0u64; n + 1].into_boxed_slice();
        buf[0] = n as u64;
        let ptr = buf.as_mut_ptr();
        self.arena.borrow_mut().push(buf);
        ptr
    }

    /// [`Ctx::alloc_buffer`] for a *signed* element count, as computed by
    /// typed code. A negative size records the evaluator's own
    /// "non-negative integer" `ARRAY` error (issue #271 — the interpreter and
    /// tracing tiers used to clamp negatives to 0 silently, while the native
    /// tier reinterpreted them as huge unsigned sizes; all three now agree
    /// with the tree-walker) and falls back to a zero-length allocation.
    pub(super) fn alloc_buffer_signed(&self, n: i64) -> *mut u64 {
        if n < 0 {
            self.set_pending_error(format!(
                "ARRAY: size must be a non-negative integer, got {n}"
            ));
            return self.alloc_buffer(0);
        }
        self.alloc_buffer(n as usize)
    }

    /// Record the evaluator's own out-of-range array-index error for a
    /// bounds-check that failed on `FETCH`/`STORE` (issue #282). The typed
    /// tiers used to bounds-check and then silently substitute (fetch → 0,
    /// store → no-op); the tree-walker (`src/evaluator/apply.rs`, `ArrayFetch`/
    /// `ArrayStore`) instead errors, distinguishing a negative index from an
    /// in-range-sign-but-past-the-end index. This records the matching message
    /// (`is_store` picks the `STORE`/`store` vs `FETCH`/`fetch` wording exactly
    /// as the evaluator spells it) as a [`pending_error`](Ctx::pending_error);
    /// the caller still returns the memory-safe substitute so the rest of the
    /// call stays panic-free, and the membrane raises the recorded error once
    /// the call returns.
    pub(super) fn record_index_error(&self, idx: i64, len: i64, is_store: bool) {
        if idx < 0 {
            let who = if is_store { "STORE" } else { "FETCH" };
            self.set_pending_error(format!(
                "{who}: index must be a non-negative integer, got {idx}"
            ));
        } else {
            let who = if is_store { "store" } else { "fetch" };
            self.set_pending_error(format!("{who}: index {idx} out of bounds (length {len})"));
        }
    }

    /// Record a pending cross-function tail call for `TypedFn::invoke`'s
    /// dispatch loop to pick up, instead of performing it here. `args` is
    /// copied immediately, so the caller's argument buffer (a Cranelift
    /// stack slot, or a `Vec` about to be dropped) need not outlive this call.
    pub(super) fn set_pending_tail(&self, id: usize, args: &[u64]) {
        *self.pending_tail.borrow_mut() = Some((id, args.to_vec()));
    }

    /// Take (clear) any pending tail call recorded since the last check.
    pub(super) fn take_pending_tail(&self) -> Option<(usize, Vec<u64>)> {
        self.pending_tail.borrow_mut().take()
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
    // `n` is a typed-code (signed) element count that crossed the ABI as a
    // word; reinterpret it back so a negative size gets the evaluator's
    // "non-negative integer" error instead of becoming a huge unsigned size.
    ctx.alloc_buffer_signed(n as i64)
}

/// Host trampoline the native edition calls from the out-of-bounds arm of a
/// guarded `FETCH`/`STORE` (issue #282): records the evaluator's own index
/// error on `Ctx` so the membrane raises it after the call returns, mirroring
/// the interpreter tiers' `buf_get`/`buf_set`. The native code still produces
/// its memory-safe substitute (fetch → 0, store → no-op); only the else-arm
/// grew this call. `is_store` is a word carrying a bool (0 = fetch, 1 = store).
///
/// # Safety
/// Called only from Cranelift-generated code with the `ctx` pointer threaded
/// from the native entry.
#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn jit_array_oob(
    ctx: *const core::ffi::c_void,
    idx: i64,
    len: i64,
    is_store: u64,
) {
    let ctx = unsafe { &*(ctx as *const Ctx) };
    ctx.record_index_error(idx, len, is_store != 0);
}

/// Host trampoline a native edition calls immediately before making a
/// non-tail call (issue #271): mirrors [`Ctx::enter_call`] for the direct
/// native-to-native `call_indirect` fast path, which otherwise never reaches
/// [`Ctx::call`] and so would recurse with no depth guard at all. Returns
/// nonzero (call is allowed; the depth counter has been bumped) or zero (the
/// depth cap was hit; a recursion-limit error is now pending and the native
/// caller must skip the call, substituting `jit_alloc(ctx, 0)` for its value).
///
/// # Safety
/// Called only from Cranelift-generated code with the `ctx` pointer threaded
/// from the native entry.
#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn jit_enter_call(ctx: *const core::ffi::c_void) -> u64 {
    let ctx = unsafe { &*(ctx as *const Ctx) };
    ctx.enter_call() as u64
}

/// Host trampoline a native edition calls after a non-tail call it guarded
/// with [`jit_enter_call`] returns — the native-side counterpart of
/// [`Ctx::exit_call`].
///
/// # Safety
/// As [`jit_enter_call`].
#[cfg(feature = "jit")]
pub(crate) unsafe extern "C" fn jit_exit_call(ctx: *const core::ffi::c_void) {
    let ctx = unsafe { &*(ctx as *const Ctx) };
    ctx.exit_call();
}

pub(super) fn int_bin(op: BinOp, x: i64, y: i64, ctx: &Ctx) -> i64 {
    match op {
        BinOp::Add => match x.checked_add(y) {
            Some(v) => v,
            None => {
                ctx.overflow.set(true);
                x.wrapping_add(y)
            }
        },
        BinOp::Sub => match x.checked_sub(y) {
            Some(v) => v,
            None => {
                ctx.overflow.set(true);
                x.wrapping_sub(y)
            }
        },
        BinOp::Mul => match x.checked_mul(y) {
            Some(v) => v,
            None => {
                ctx.overflow.set(true);
                x.wrapping_mul(y)
            }
        },
        BinOp::Div => {
            if y == 0 {
                ctx.div_by_zero.set(true);
                0
            } else if x == i64::MIN && y == -1 {
                ctx.overflow.set(true);
                x.wrapping_div(y)
            } else {
                x / y
            }
        }
        BinOp::Mod => {
            if y == 0 {
                ctx.div_by_zero.set(true);
                0
            } else {
                // Euclidean modulo, matching the evaluator's MOD — a
                // deliberate PR #112 design (REMAINDER is the truncated op):
                // (mod -7 3) = 2, and the result sign follows the divisor's
                // magnitude space, never the dividend. checked_rem_euclid
                // handles MIN % -1 (mathematically 0; the intermediate
                // overflows, so it returns None): the evaluator answers 0
                // with NO flag, so the typed tiers must too — this
                // supersedes the OVERFLOW flag #228/#268 set here on the
                // mistaken assumption that MOD was the truncated op (#280).
                x.checked_rem_euclid(y).unwrap_or(0)
            }
        }
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
// bounds-checked to stay panic-free and to agree with the native edition's
// guarded loads/stores. An out-of-range index records the evaluator's own
// index error (issue #282) and still returns a memory-safe substitute (fetch →
// 0, store → no-op) so the rest of the call runs; the membrane raises the
// recorded error once the call returns.

/// # Safety: `base` must be a live buffer pointer from [`Ctx::alloc_buffer`].
#[inline]
unsafe fn buf_get(base: u64, idx: i64, ctx: &Ctx) -> u64 {
    let p = base as *const u64;
    let len = unsafe { *p } as i64;
    if idx < 0 || idx >= len {
        ctx.record_index_error(idx, len, false);
        0
    } else {
        unsafe { *p.add(idx as usize + 1) }
    }
}
/// # Safety: as [`buf_get`].
#[inline]
unsafe fn buf_set(base: u64, idx: i64, val: u64, ctx: &Ctx) {
    let p = base as *mut u64;
    let len = unsafe { *p } as i64;
    if idx >= 0 && idx < len {
        unsafe { *p.add(idx as usize + 1) = val }
    } else {
        ctx.record_index_error(idx, len, true);
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

/// Evaluate `core` in **tail position** of the function identified by
/// `self_id`. A `Core::Call(id, ..)` reached here with `id == self_id` is a
/// **self tail call** (issue #133 Tier 1): instead of recursing through
/// [`Ctx::call`] (which would grow the native Rust stack once per
/// iteration), the loop evaluates the new argument values, overwrites
/// `env`'s parameter slots with them, and restarts from the top of the
/// body — O(1) native stack for an arbitrarily deep tail-recursive typed
/// function.
///
/// `self_id` exists so a *non-self* call, or a self call reached in a
/// *non-tail* position (e.g. `fib`'s two recursive calls, or a self-call
/// used as a `bin`/`cmp` operand or another call's argument), is never
/// mistaken for a loop: only the tail-preserving constructs this loop
/// switches on (`if`/`let`/`and`/`or`/`seq`) ever hand a `Call` node to this
/// function with tail status still in effect; every definitely-non-tail
/// sub-expression is evaluated via [`eval_core_nontail`], which always
/// performs an ordinary (depth-bounded) call through the registry, even for
/// a self-referencing id.
pub(super) fn eval_core(core: &Core, env: &mut [u64], ctx: &Ctx, self_id: usize) -> u64 {
    let top = core;
    let mut current = core;
    loop {
        match current {
            Core::If(c, t, e) => {
                current = if eval_core_nontail(c, env, ctx) != 0 {
                    t
                } else {
                    e
                };
            }
            Core::Let(slot, init, body) => {
                let v = eval_core_nontail(init, env, ctx);
                env[*slot] = v;
                current = body;
            }
            Core::And(a, b) => {
                if eval_core_nontail(a, env, ctx) == 0 {
                    return 0;
                }
                current = b;
            }
            Core::Or(a, b) => {
                if eval_core_nontail(a, env, ctx) != 0 {
                    return 1;
                }
                current = b;
            }
            Core::Seq(forms) => match forms.split_last() {
                Some((last, init)) => {
                    for f in init {
                        eval_core_nontail(f, env, ctx);
                    }
                    current = last;
                }
                None => return 0,
            },
            Core::Call(id, args) if *id == self_id => {
                // Parallel assignment: evaluate every new argument (which may
                // read *old* slot values that a sibling argument is about to
                // overwrite, e.g. `(sum (- n 1) (+ acc n))`) before storing
                // any of them.
                let vals: Vec<u64> = args
                    .iter()
                    .map(|a| eval_core_nontail(a, env, ctx))
                    .collect();
                env[..vals.len()].copy_from_slice(&vals);
                current = top;
            }
            Core::Call(id, args) => {
                // Tier 2a (issue #133): a *cross*-function tail call. Hand it
                // off to the host trampoline (`Ctx::set_pending_tail`, drained
                // by `TypedFn::invoke`) instead of recursing through
                // `Ctx::call` — O(1) native Rust stack for arbitrarily deep
                // mutual/general tail recursion (e.g. `even?`/`odd?`). The
                // returned value is a placeholder; `invoke`'s trampoline loop
                // overwrites it with the real result once the chain bottoms out.
                let vals: Vec<u64> = args
                    .iter()
                    .map(|a| eval_core_nontail(a, env, ctx))
                    .collect();
                ctx.set_pending_tail(*id, &vals);
                return 0;
            }
            other => return eval_core_nontail(other, env, ctx),
        }
    }
}

/// Evaluate `core` as a definitely-non-tail sub-expression: an ordinary,
/// depth-bounded recursive walk. A `Call` here — even to `self_id` — is a
/// real function call through the registry, never a tail loop.
fn eval_core_nontail(core: &Core, env: &mut [u64], ctx: &Ctx) -> u64 {
    match core {
        Core::LitI(n) => from_i(*n),
        Core::LitF(f) => from_f(*f),
        Core::Var(i) => env[*i],
        Core::Bin(k, op, a, b) => {
            let (x, y) = (
                eval_core_nontail(a, env, ctx),
                eval_core_nontail(b, env, ctx),
            );
            match k {
                NumKind::I => from_i(int_bin(*op, as_i(x), as_i(y), ctx)),
                NumKind::F => from_f(float_bin(*op, as_f(x), as_f(y))),
            }
        }
        Core::Cmp(k, op, a, b) => {
            let (x, y) = (
                eval_core_nontail(a, env, ctx),
                eval_core_nontail(b, env, ctx),
            );
            let r = match k {
                NumKind::I => int_cmp(*op, as_i(x), as_i(y)),
                NumKind::F => float_cmp(*op, as_f(x), as_f(y)),
            };
            r as u64
        }
        Core::Not(a) => (eval_core_nontail(a, env, ctx) == 0) as u64,
        Core::And(a, b) => {
            if eval_core_nontail(a, env, ctx) != 0 {
                (eval_core_nontail(b, env, ctx) != 0) as u64
            } else {
                0
            }
        }
        Core::Or(a, b) => {
            if eval_core_nontail(a, env, ctx) != 0 {
                1
            } else {
                (eval_core_nontail(b, env, ctx) != 0) as u64
            }
        }
        Core::If(c, t, e) => {
            if eval_core_nontail(c, env, ctx) != 0 {
                eval_core_nontail(t, env, ctx)
            } else {
                eval_core_nontail(e, env, ctx)
            }
        }
        Core::Let(slot, init, body) => {
            let v = eval_core_nontail(init, env, ctx);
            env[*slot] = v;
            eval_core_nontail(body, env, ctx)
        }
        Core::Call(id, args) => {
            let vals: Vec<u64> = args
                .iter()
                .map(|a| eval_core_nontail(a, env, ctx))
                .collect();
            ctx.call(*id, &vals)
        }
        Core::ToChar(a) => eval_core_nontail(a, env, ctx) & 0xff,
        Core::ArrayNew(n) => {
            let len = as_i(eval_core_nontail(n, env, ctx));
            ctx.alloc_buffer_signed(len) as u64
        }
        Core::ArrayGet(a, i) => {
            let base = eval_core_nontail(a, env, ctx);
            let idx = as_i(eval_core_nontail(i, env, ctx));
            unsafe { buf_get(base, idx, ctx) }
        }
        Core::ArraySet(a, i, v) => {
            let base = eval_core_nontail(a, env, ctx);
            let idx = as_i(eval_core_nontail(i, env, ctx));
            let val = eval_core_nontail(v, env, ctx);
            unsafe { buf_set(base, idx, val, ctx) };
            val
        }
        Core::ArrayLen(a) => {
            let base = eval_core_nontail(a, env, ctx);
            unsafe { *(base as *const u64) }
        }
        Core::StructNew(inits) => {
            let vals: Vec<u64> = inits
                .iter()
                .map(|c| eval_core_nontail(c, env, ctx))
                .collect();
            let base = ctx.alloc_buffer(vals.len());
            for (i, v) in vals.iter().enumerate() {
                unsafe { *base.add(i + 1) = *v };
            }
            base as u64
        }
        Core::FieldGet(s, idx) => {
            let base = eval_core_nontail(s, env, ctx);
            unsafe { field_get(base, *idx) }
        }
        Core::FieldSet(s, idx, v) => {
            let base = eval_core_nontail(s, env, ctx);
            let val = eval_core_nontail(v, env, ctx);
            unsafe { field_set(base, *idx, val) };
            val
        }
        Core::Seq(forms) => {
            let mut r = 0;
            for f in forms {
                r = eval_core_nontail(f, env, ctx);
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
                NumKind::I => from_i(int_bin(*op, as_i(x), as_i(y), ctx)),
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
            let len = as_i(eval_core_traced(n, env, ctx, depth + 1, log));
            step!(
                "arraynew",
                ctx.alloc_buffer_signed(len) as u64,
                NO_SLOT,
                NO_CALLEE
            )
        }
        Core::ArrayGet(a, i) => {
            let base = eval_core_traced(a, env, ctx, depth + 1, log);
            let idx = as_i(eval_core_traced(i, env, ctx, depth + 1, log));
            step!(
                "arrayget",
                unsafe { buf_get(base, idx, ctx) },
                NO_SLOT,
                NO_CALLEE
            )
        }
        Core::ArraySet(a, i, v) => {
            let base = eval_core_traced(a, env, ctx, depth + 1, log);
            let idx = as_i(eval_core_traced(i, env, ctx, depth + 1, log));
            let val = eval_core_traced(v, env, ctx, depth + 1, log);
            unsafe { buf_set(base, idx, val, ctx) };
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
                    Rc::new(move |e, c| from_i(int_bin(op, as_i(ca(e, c)), as_i(cb(e, c)), c)))
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
                let len = as_i(cn(e, c));
                c.alloc_buffer_signed(len) as u64
            })
        }
        Core::ArrayGet(a, i) => {
            let (ca, ci) = (compile(a), compile(i));
            Rc::new(move |e, c| {
                let base = ca(e, c);
                let idx = as_i(ci(e, c));
                unsafe { buf_get(base, idx, c) }
            })
        }
        Core::ArraySet(a, i, v) => {
            let (ca, ci, cv) = (compile(a), compile(i), compile(v));
            Rc::new(move |e, c| {
                let base = ca(e, c);
                let idx = as_i(ci(e, c));
                let val = cv(e, c);
                unsafe { buf_set(base, idx, val, c) };
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

// ---------------------------------------------------------------------------
// Closure-backend tail-call optimization (issue #133 Tier 1).
// ---------------------------------------------------------------------------

/// Outcome of one step of a tail-position compiled closure: either the final
/// value, or a signal that a self tail call just overwrote `env`'s parameter
/// slots and the top-level loop in [`compile_with_tco`] should run the body
/// again.
pub(super) enum TailStep {
    Done(u64),
    Loop,
}

/// A tail-position compiled closure — the closure-backend counterpart of
/// [`eval_core`]'s tail loop.
pub(super) type CompiledTail = Rc<dyn Fn(&mut [u64], &Ctx) -> TailStep>;

/// Compile `core`, the body of the function identified by `self_id`, to a
/// [`Compiled`] closure with tail-call optimization: a `Core::Call(id, ..)`
/// reached in tail position with `id == self_id` loops in place instead of
/// recursing through [`Ctx::call`] — O(1) native (Rust) stack for an
/// arbitrarily deep tail-recursive typed function, mirroring [`eval_core`]'s
/// discipline for the closure backend that `TypedFn::compile_now` actually
/// installs as the default-build hot path.
pub fn compile_with_tco(core: &Core, self_id: usize) -> Compiled {
    let tail = compile_tail(core, self_id);
    Rc::new(move |env, ctx| {
        loop {
            match tail(env, ctx) {
                TailStep::Done(v) => return v,
                TailStep::Loop => {}
            }
        }
    })
}

/// Compile `core` as a tail-position node of the function `self_id`. Tail
/// status only propagates through the constructs whose value is *exactly*
/// their tail sub-node's value with no further transformation
/// (`if`/`let`/`and`/`or`/`seq`); everything else compiles via the ordinary
/// (non-tail) [`compile`] and is wrapped as an already-`Done` step.
fn compile_tail(core: &Core, self_id: usize) -> CompiledTail {
    match core {
        Core::If(c, t, e) => {
            let cc = compile(c);
            let ct = compile_tail(t, self_id);
            let ce = compile_tail(e, self_id);
            Rc::new(move |env, ctx| {
                if cc(env, ctx) != 0 {
                    ct(env, ctx)
                } else {
                    ce(env, ctx)
                }
            })
        }
        Core::Let(slot, init, body) => {
            let slot = *slot;
            let ci = compile(init);
            let cb = compile_tail(body, self_id);
            Rc::new(move |env, ctx| {
                let v = ci(env, ctx);
                env[slot] = v;
                cb(env, ctx)
            })
        }
        Core::And(a, b) => {
            let ca = compile(a);
            let cb = compile_tail(b, self_id);
            Rc::new(move |env, ctx| {
                if ca(env, ctx) != 0 {
                    cb(env, ctx)
                } else {
                    TailStep::Done(0)
                }
            })
        }
        Core::Or(a, b) => {
            let ca = compile(a);
            let cb = compile_tail(b, self_id);
            Rc::new(move |env, ctx| {
                if ca(env, ctx) != 0 {
                    TailStep::Done(1)
                } else {
                    cb(env, ctx)
                }
            })
        }
        Core::Seq(forms) => match forms.split_last() {
            Some((last, init)) => {
                let cinit: Vec<Compiled> = init.iter().map(compile).collect();
                let clast = compile_tail(last, self_id);
                Rc::new(move |env, ctx| {
                    for f in &cinit {
                        f(env, ctx);
                    }
                    clast(env, ctx)
                })
            }
            None => Rc::new(|_e, _c| TailStep::Done(0)),
        },
        Core::Call(id, args) if *id == self_id => {
            let cargs: Vec<Compiled> = args.iter().map(compile).collect();
            Rc::new(move |env, ctx| {
                // Parallel assignment: see eval_core's identical comment.
                let vals: Vec<u64> = cargs.iter().map(|ca| ca(env, ctx)).collect();
                env[..vals.len()].copy_from_slice(&vals);
                TailStep::Loop
            })
        }
        Core::Call(id, args) => {
            // Tier 2a (issue #133): cross-function tail call -> host
            // trampoline. See eval_core's identical arm for the rationale;
            // `TailStep::Done`'s value is a placeholder that `invoke`'s
            // trampoline loop overwrites once the chain bottoms out.
            let id = *id;
            let cargs: Vec<Compiled> = args.iter().map(compile).collect();
            Rc::new(move |env, ctx| {
                let vals: Vec<u64> = cargs.iter().map(|ca| ca(env, ctx)).collect();
                ctx.set_pending_tail(id, &vals);
                TailStep::Done(0)
            })
        }
        other => {
            let cv = compile(other);
            Rc::new(move |env, ctx| TailStep::Done(cv(env, ctx)))
        }
    }
}
