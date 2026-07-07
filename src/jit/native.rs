//! Native-code backend for the typed JIT, via Cranelift. Compiled in only with
//! `--features jit`.
//!
//! Each typed function is lowered to a native function with the ABI
//! `extern "C" fn(args: *const u64, ctx: *const c_void) -> u64`, where `args`
//! points to the (unboxed) parameter words and `ctx` is an opaque pointer to the
//! Rust [`Ctx`]. Leaf work — arithmetic, comparisons, `if`/`and`/`or`, `let` —
//! is fully native on unboxed machine words (`int64` in registers, `float64` via
//! bitcast, `bool` as `0`/`1`).
//!
//! **Calls** load the callee's current native entry from its heap-stable *entry
//! cell* and, if present, call it directly (`call_indirect`); otherwise they fall
//! back to a host trampoline [`jit_trampoline`] that re-enters `Ctx::call` (the
//! design doc's `universal_call`) to reach the callee's closure/interpreter
//! edition. Because the call goes through the cell — not a baked code address —
//! recursion and redefinition need no Cranelift relocation or hot-patching: a
//! redefined callee just updates its cell, and the cell address survives registry
//! `Vec` growth (it is a heap `Box`).
//!
//! `if`/`and`/`or` use a per-node result stack slot (rather than block
//! parameters) so the lowering does not depend on the block-argument API, which
//! has churned across Cranelift releases.

use super::{BinOp, CmpOp, Core, Ctx, NumKind};
use core::ffi::c_void;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    AbiParam, InstBuilder, MemFlagsData, SigRef, Signature, StackSlotData, StackSlotKind, Value,
    types,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, default_libcall_names};

/// The native calling convention for a compiled typed function.
type NativeEntry = unsafe extern "C" fn(*const u64, *const c_void) -> u64;

/// A finalized native edition. Owns its `JITModule`, whose `Drop` frees the
/// executable memory — so keeping this alive (e.g. behind an `Rc` that an
/// in-flight call has cloned) keeps the code mapped.
pub struct NativeEdition {
    // Field order matters for drop: `entry` is a pointer into `module`'s code.
    entry: NativeEntry,
    _module: JITModule,
}

impl std::fmt::Debug for NativeEdition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeEdition")
    }
}

impl NativeEdition {
    /// Invoke the native function. `args` must have exactly the parameter count;
    /// `ctx` is the live call context (passed opaque for re-entrant calls).
    ///
    /// # Safety
    /// `ctx` must point to the `Ctx` that owns the function table for the whole
    /// duration of the call (the trampoline dereferences it).
    pub unsafe fn call(&self, args: &[u64], ctx: *const c_void) -> u64 {
        unsafe { (self.entry)(args.as_ptr(), ctx) }
    }

    /// The native entry pointer as an integer, to publish into the function's
    /// entry cell for direct calls from other compiled functions.
    pub fn entry_addr(&self) -> usize {
        self.entry as *const () as usize
    }
}

/// The host trampoline every native call routes through. Re-enters the registry
/// so self/cross/mutual calls and redefinition all work without native-level
/// relocation.
///
/// # Safety
/// Called only from Cranelift-generated code with arguments it constructed:
/// `ctx` is the pointer threaded from the entry, `args`/`argc` describe a buffer
/// of `argc` `u64` words.
unsafe extern "C" fn jit_trampoline(
    ctx: *const c_void,
    id: u64,
    args: *const u64,
    argc: u64,
) -> u64 {
    let ctx = unsafe { &*(ctx as *const Ctx) };
    let slice = unsafe { std::slice::from_raw_parts(args, argc as usize) };
    ctx.call(id as usize, slice)
}

/// Host trampoline for a *cross*-function tail call (issue #133 Tier 2a).
/// Unlike [`jit_trampoline`] (which re-enters `Ctx::call` and so grows the
/// native Rust stack by one frame per call — fine for an ordinary,
/// non-tail-position call, unsound for a tail-recursive chain of unbounded
/// depth), this records the target and its argument words on `Ctx` and
/// returns immediately: the native function that called this then returns
/// normally (ordinary SystemV return, no calling-convention change), and
/// `TypedFn::invoke`'s dispatch loop (not this function, and not any native
/// code) picks up the pending call and re-dispatches — a fresh top-level
/// call each time, so native stack usage is O(1) regardless of chain depth.
/// Cranelift's `tail` calling convention / `return_call_indirect` was
/// evaluated and rejected for this (Tier 2b, deferred): adopting it would
/// require every typed function to also expose a distinct SystemV entry
/// thunk (Rust cannot call a `Tail`-CC function directly), plus reworking
/// the entry-cell Rc-pinning discipline, since `return_call` eliminates the
/// caller's frame and so invalidates the "pin the callee's edition for the
/// duration of the (still-existing) caller frame" invariant the redefinition
/// story depends on. This trampoline sidesteps both problems entirely by
/// never changing any function's calling convention.
///
/// # Safety
/// Called only from Cranelift-generated code with arguments it constructed:
/// `ctx` is the pointer threaded from the entry, `args`/`argc` describe a
/// buffer of `argc` `u64` words that are copied out immediately (need not
/// outlive this call).
unsafe extern "C" fn jit_set_pending_tail(
    ctx: *const c_void,
    id: u64,
    args: *const u64,
    argc: u64,
) {
    let ctx = unsafe { &*(ctx as *const Ctx) };
    let slice = unsafe { std::slice::from_raw_parts(args, argc as usize) };
    ctx.set_pending_tail(id as usize, slice);
}

/// Compile `core` (a function body with `n_params` parameters and `n_slots`
/// total local slots) to a native edition.
///
/// `cell_addrs[id]` is the address of function `id`'s entry cell — a stable
/// heap word holding that function's current native entry pointer (or 0). A
/// `Call(id, ..)` loads that word and, if non-zero, calls the callee's native
/// code **directly**; otherwise it falls back to the host trampoline. The cell
/// is a heap `Box`, so its address is stable across registry growth (Vec
/// reallocation) and across redefinition (the word is updated in place).
pub fn compile_native(
    core: &Core,
    self_id: usize,
    n_params: usize,
    n_slots: usize,
    cell_addrs: &[usize],
    param_counts: &[usize],
) -> Result<NativeEdition, String> {
    let mut jb = JITBuilder::new(default_libcall_names()).map_err(|e| e.to_string())?;
    jb.symbol("jit_trampoline", jit_trampoline as *const u8);
    jb.symbol("jit_set_pending_tail", jit_set_pending_tail as *const u8);
    jb.symbol("jit_alloc", super::jit_alloc as *const u8);
    jb.symbol("jit_enter_call", super::jit_enter_call as *const u8);
    jb.symbol("jit_exit_call", super::jit_exit_call as *const u8);
    let mut module = JITModule::new(jb);
    let ptr = module.target_config().pointer_type();

    // Signature of a compiled typed function, for direct `call_indirect`.
    let mut callee_sig = Signature::new(module.target_config().default_call_conv);
    callee_sig.params.push(AbiParam::new(ptr));
    callee_sig.params.push(AbiParam::new(ptr));
    callee_sig.returns.push(AbiParam::new(types::I64));

    // Imported trampoline signature: (ctx, id, args*, argc) -> u64.
    let mut tsig = module.make_signature();
    tsig.params.push(AbiParam::new(ptr));
    tsig.params.push(AbiParam::new(types::I64));
    tsig.params.push(AbiParam::new(ptr));
    tsig.params.push(AbiParam::new(types::I64));
    tsig.returns.push(AbiParam::new(types::I64));
    let tramp_id = module
        .declare_function("jit_trampoline", Linkage::Import, &tsig)
        .map_err(|e| e.to_string())?;

    // Imported cross-function tail-call trampoline signature (issue #133
    // Tier 2a): (ctx, id, args*, argc) -> (), no return value.
    let mut ptsig = module.make_signature();
    ptsig.params.push(AbiParam::new(ptr));
    ptsig.params.push(AbiParam::new(types::I64));
    ptsig.params.push(AbiParam::new(ptr));
    ptsig.params.push(AbiParam::new(types::I64));
    let pending_tail_id = module
        .declare_function("jit_set_pending_tail", Linkage::Import, &ptsig)
        .map_err(|e| e.to_string())?;

    // Imported allocator: (ctx, n) -> *mut u64, for array/struct allocation.
    let mut asig = module.make_signature();
    asig.params.push(AbiParam::new(ptr));
    asig.params.push(AbiParam::new(types::I64));
    asig.returns.push(AbiParam::new(ptr));
    let alloc_id = module
        .declare_function("jit_alloc", Linkage::Import, &asig)
        .map_err(|e| e.to_string())?;

    // Imported non-tail-call depth guard (issue #271): `jit_enter_call`
    // (ctx) -> bool-as-i64 (nonzero = ok, depth bumped; zero = at the cap, a
    // recursion-limit error is now pending) and `jit_exit_call` (ctx) -> (),
    // its counterpart. Wraps every non-tail `Call` (`emit_call`) so the
    // direct native-to-native `call_indirect` fast path — which never
    // otherwise reaches `Ctx::call` — is guarded too, not just calls that
    // fall through to the host trampoline.
    let mut esig = module.make_signature();
    esig.params.push(AbiParam::new(ptr));
    esig.returns.push(AbiParam::new(types::I64));
    let enter_call_id = module
        .declare_function("jit_enter_call", Linkage::Import, &esig)
        .map_err(|e| e.to_string())?;

    let mut xsig = module.make_signature();
    xsig.params.push(AbiParam::new(ptr));
    let exit_call_id = module
        .declare_function("jit_exit_call", Linkage::Import, &xsig)
        .map_err(|e| e.to_string())?;

    // The function we are building: (args*, ctx*) -> u64.
    let mut ctx_codegen = module.make_context();
    ctx_codegen.func.signature.params.push(AbiParam::new(ptr));
    ctx_codegen.func.signature.params.push(AbiParam::new(ptr));
    ctx_codegen
        .func
        .signature
        .returns
        .push(AbiParam::new(types::I64));

    let mut fbctx = FunctionBuilderContext::new();
    {
        let mut b = FunctionBuilder::new(&mut ctx_codegen.func, &mut fbctx);
        let tramp_ref = module.declare_func_in_func(tramp_id, b.func);
        let pending_tail_ref = module.declare_func_in_func(pending_tail_id, b.func);
        let alloc_ref = module.declare_func_in_func(alloc_id, b.func);
        let enter_call_ref = module.declare_func_in_func(enter_call_id, b.func);
        let exit_call_ref = module.declare_func_in_func(exit_call_id, b.func);
        let callee_sig = b.import_signature(callee_sig);

        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);
        b.seal_block(entry);
        let args_ptr = b.block_params(entry)[0];
        let ctx_ptr = b.block_params(entry)[1];

        // Local slot frame (params + let bindings).
        let env_slot = b.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            (n_slots.max(1) * 8) as u32,
            3,
        ));
        for i in 0..n_params {
            let v = b.ins().load(
                types::I64,
                MemFlagsData::trusted(),
                args_ptr,
                (i * 8) as i32,
            );
            b.ins().stack_store(v, env_slot, (i * 8) as i32);
        }

        // Loop header for self tail calls (issue #133 Tier 1): the prologue
        // falls through to it once; a self tail call elsewhere in the body
        // stores its new argument values into `env_slot` and jumps back here
        // instead of recursing. Not sealed until the whole body is emitted —
        // every back-edge into it must be known first.
        let header = b.create_block();
        b.ins().jump(header, &[]);
        b.switch_to_block(header);

        let mut e = Emitter {
            b: &mut b,
            ptr,
            env_slot,
            ctx_ptr,
            tramp_ref,
            pending_tail_ref,
            alloc_ref,
            enter_call_ref,
            exit_call_ref,
            callee_sig,
            cell_addrs,
            param_counts,
            self_id,
            header,
        };
        match e.emit(core, true) {
            Emitted::Value(result) => {
                b.ins().return_(&[result]);
            }
            Emitted::TailLooped => {
                // The whole body is an unconditional self tail call with no
                // reachable base case (e.g. `(defun-typed (f int64) () (f))`)
                // — every path already jumped back to `header`; there is no
                // value to return from here. A user-level infinite loop, not
                // a compiler error.
            }
        }
        b.seal_block(header);
        b.finalize();
    }

    let func_id = module
        .declare_function("typed_fn", Linkage::Export, &ctx_codegen.func.signature)
        .map_err(|e| e.to_string())?;
    module
        .define_function(func_id, &mut ctx_codegen)
        .map_err(|e| e.to_string())?;
    module.clear_context(&mut ctx_codegen);
    module.finalize_definitions().map_err(|e| e.to_string())?;

    let code = module.get_finalized_function(func_id);
    let entry = unsafe { std::mem::transmute::<*const u8, NativeEntry>(code) };
    Ok(NativeEdition {
        entry,
        _module: module,
    })
}

struct Emitter<'a, 'b, 'c> {
    b: &'a mut FunctionBuilder<'b>,
    ptr: types::Type,
    env_slot: cranelift_codegen::ir::StackSlot,
    ctx_ptr: Value,
    tramp_ref: cranelift_codegen::ir::FuncRef,
    /// Imported [`jit_set_pending_tail`] (issue #133 Tier 2a): records a
    /// cross-function tail call on `Ctx` instead of performing it natively.
    pending_tail_ref: cranelift_codegen::ir::FuncRef,
    alloc_ref: cranelift_codegen::ir::FuncRef,
    /// Imported [`super::jit_enter_call`]/[`super::jit_exit_call`] (issue
    /// #271): the non-tail call depth guard `emit_call` wraps every call in.
    enter_call_ref: cranelift_codegen::ir::FuncRef,
    exit_call_ref: cranelift_codegen::ir::FuncRef,
    callee_sig: SigRef,
    cell_addrs: &'c [usize],
    /// Expected parameter count for each callee, parallel to `cell_addrs`.
    /// Used in `emit_call` to detect call-site/callee arity mismatches at
    /// native-compile time (arising from redefinitions that changed arity after
    /// callers were elaborated but before they were recompiled).
    param_counts: &'c [usize],
    /// This function's own registry id — a `Call(id, ..)` reached in tail
    /// position with `id == self_id` is a self tail call (issue #133 Tier 1).
    self_id: usize,
    /// The loop header block a self tail call jumps back to, after storing
    /// its new argument values into `env_slot`. Not sealed until the whole
    /// body has been emitted (every back-edge must be known first).
    header: cranelift_codegen::ir::Block,
}

/// Outcome of emitting one [`Core`] node. Mirrors [`crate::jit::runtime::TailStep`]
/// for the native backend: a self tail call terminates its current block
/// with a jump to the loop header instead of producing a value, so callers
/// in a tail-propagating position must check for this and skip any further
/// store/jump they would otherwise add to that (now-terminated) block.
enum Emitted {
    Value(Value),
    TailLooped,
}

impl Emitter<'_, '_, '_> {
    fn iconst(&mut self, n: i64) -> Value {
        self.b.ins().iconst(types::I64, n)
    }

    /// `word != 0` as an I8 predicate.
    fn truthy(&mut self, w: Value) -> Value {
        let z = self.iconst(0);
        self.b.ins().icmp(IntCC::NotEqual, w, z)
    }

    /// Emit `core` in a definitely-non-tail context: always produces a
    /// value (a self tail call can only arise from a tail-position `emit`
    /// call, never from here).
    fn emit_value(&mut self, core: &Core) -> Value {
        match self.emit(core, false) {
            Emitted::Value(v) => v,
            Emitted::TailLooped => unreachable!("non-tail context cannot tail-loop"),
        }
    }

    /// Emit `core`, which is in tail position of the function body iff
    /// `tail` is set. Tail status only propagates through the constructs
    /// whose value is *exactly* their tail sub-node's value with no further
    /// transformation (`if`/`let`/`and`/`or`/`seq`); everything else is
    /// evaluated via [`Self::emit_value`] and wrapped as an already-produced
    /// value.
    fn emit(&mut self, core: &Core, tail: bool) -> Emitted {
        match core {
            Core::LitI(n) => Emitted::Value(self.iconst(*n)),
            Core::LitF(f) => Emitted::Value(self.iconst(f.to_bits() as i64)),
            Core::Var(i) => Emitted::Value(self.b.ins().stack_load(
                types::I64,
                self.env_slot,
                (*i * 8) as i32,
            )),
            Core::Bin(k, op, a, b) => {
                let (x, y) = (self.emit_value(a), self.emit_value(b));
                Emitted::Value(match k {
                    NumKind::I => self.int_bin(*op, x, y),
                    NumKind::F => self.float_bin(*op, x, y),
                })
            }
            Core::Cmp(k, op, a, b) => {
                let (x, y) = (self.emit_value(a), self.emit_value(b));
                let cmp = match k {
                    NumKind::I => self.b.ins().icmp(int_cc(*op), x, y),
                    NumKind::F => {
                        let (xf, yf) = (self.as_f(x), self.as_f(y));
                        self.b.ins().fcmp(float_cc(*op), xf, yf)
                    }
                };
                Emitted::Value(self.b.ins().uextend(types::I64, cmp))
            }
            Core::Not(a) => {
                let v = self.emit_value(a);
                let z = self.iconst(0);
                let c = self.b.ins().icmp(IntCC::Equal, v, z);
                Emitted::Value(self.b.ins().uextend(types::I64, c))
            }
            Core::And(a, b) => {
                // a ? (b != 0) : 0  — short-circuits. `b` is bool-typed (the
                // elaborator requires it), so truthy-normalizing it is the
                // identity on any value that isn't itself a tail-loop.
                let av = self.emit_value(a);
                let cond = self.truthy(av);
                self.branch_merge(
                    cond,
                    |s| s.emit_truthy(b, tail),
                    |s| Emitted::Value(s.iconst(0)),
                )
            }
            Core::Or(a, b) => {
                // a ? 1 : (b != 0)  — short-circuits.
                let av = self.emit_value(a);
                let cond = self.truthy(av);
                self.branch_merge(
                    cond,
                    |s| Emitted::Value(s.iconst(1)),
                    |s| s.emit_truthy(b, tail),
                )
            }
            Core::If(c, t, e) => {
                let cv = self.emit_value(c);
                let cond = self.truthy(cv);
                self.branch_merge(cond, |s| s.emit(t, tail), |s| s.emit(e, tail))
            }
            Core::Let(slot, init, body) => {
                let v = self.emit_value(init);
                self.b
                    .ins()
                    .stack_store(v, self.env_slot, (*slot * 8) as i32);
                self.emit(body, tail)
            }
            Core::Call(id, args) if tail && *id == self.self_id => self.emit_self_tail_call(args),
            Core::Call(id, args) if tail => Emitted::Value(self.emit_cross_tail_call(*id, args)),
            Core::Call(id, args) => Emitted::Value(self.emit_call(*id, args)),
            Core::ToChar(a) => {
                // Narrow int64 -> char by masking to a byte (issue #136).
                let v = self.emit_value(a);
                Emitted::Value(self.b.ins().band_imm(v, 0xff))
            }
            Core::ArrayNew(n) => {
                let n = self.emit_value(n);
                // Pass the signed length through unchanged: `jit_alloc`
                // routes it to `Ctx::alloc_buffer_signed`, which records the
                // evaluator's "non-negative integer" error for a negative
                // size (issue #271) instead of silently clamping to 0.
                Emitted::Value(self.alloc(n))
            }
            Core::ArrayGet(a, i) => {
                let base = self.emit_value(a);
                let idx = self.emit_value(i);
                let in_range = self.in_bounds(base, idx);
                Emitted::Value(self.branch_to_slot(
                    in_range,
                    |s| {
                        let addr = s.elem_addr(base, idx);
                        s.b.ins().load(types::I64, MemFlagsData::trusted(), addr, 0)
                    },
                    |s| s.iconst(0),
                ))
            }
            Core::ArraySet(a, i, v) => {
                let base = self.emit_value(a);
                let idx = self.emit_value(i);
                let val = self.emit_value(v);
                let in_range = self.in_bounds(base, idx);
                Emitted::Value(self.branch_to_slot(
                    in_range,
                    |s| {
                        let addr = s.elem_addr(base, idx);
                        s.b.ins().store(MemFlagsData::trusted(), val, addr, 0);
                        val
                    },
                    |_s| val,
                ))
            }
            Core::ArrayLen(a) => {
                let base = self.emit_value(a);
                Emitted::Value(
                    self.b
                        .ins()
                        .load(types::I64, MemFlagsData::trusted(), base, 0),
                )
            }
            Core::StructNew(inits) => {
                let vals: Vec<Value> = inits.iter().map(|c| self.emit_value(c)).collect();
                let n = self.iconst(vals.len() as i64);
                let base = self.alloc(n);
                for (i, v) in vals.iter().enumerate() {
                    self.b
                        .ins()
                        .store(MemFlagsData::trusted(), *v, base, ((i + 1) * 8) as i32);
                }
                Emitted::Value(base)
            }
            Core::FieldGet(s, idx) => {
                let base = self.emit_value(s);
                Emitted::Value(self.b.ins().load(
                    types::I64,
                    MemFlagsData::trusted(),
                    base,
                    ((*idx + 1) * 8) as i32,
                ))
            }
            Core::FieldSet(s, idx, v) => {
                let base = self.emit_value(s);
                let val = self.emit_value(v);
                self.b
                    .ins()
                    .store(MemFlagsData::trusted(), val, base, ((*idx + 1) * 8) as i32);
                Emitted::Value(val)
            }
            Core::Seq(forms) => match forms.split_last() {
                Some((last, init)) => {
                    for f in init {
                        self.emit_value(f);
                    }
                    self.emit(last, tail)
                }
                None => Emitted::Value(self.iconst(0)),
            },
        }
    }

    /// Evaluate `core` and normalize to the boolean word `(value != 0)`,
    /// tail-aware: if `core` tail-loops, there is no value to normalize —
    /// propagate the tail-loop unchanged.
    fn emit_truthy(&mut self, core: &Core, tail: bool) -> Emitted {
        match self.emit(core, tail) {
            Emitted::Value(v) => {
                let t = self.truthy(v);
                Emitted::Value(self.b.ins().uextend(types::I64, t))
            }
            Emitted::TailLooped => Emitted::TailLooped,
        }
    }

    /// A self tail call in tail position (issue #133 Tier 1): evaluate every
    /// new argument value in the *current* env — before any parameter slot
    /// is overwritten, exactly mirroring `eval_core`'s parallel-assignment
    /// ordering, since a new argument may read an old parameter a sibling
    /// argument is about to clobber (`(sum (- n 1) (+ acc n))`) — store them
    /// into the parameter slots, then jump back to the loop header instead
    /// of emitting a call. This terminates the current block; the caller
    /// must not add further instructions to it.
    fn emit_self_tail_call(&mut self, args: &[Core]) -> Emitted {
        let vals: Vec<Value> = args.iter().map(|a| self.emit_value(a)).collect();
        for (i, v) in vals.iter().enumerate() {
            self.b.ins().stack_store(*v, self.env_slot, (i * 8) as i32);
        }
        self.b.ins().jump(self.header, &[]);
        Emitted::TailLooped
    }

    /// A *cross*-function tail call in tail position (issue #133 Tier 2a):
    /// evaluate the arguments, hand the target id and argument words to the
    /// host via [`jit_set_pending_tail`], and produce a placeholder value.
    /// Unlike [`Self::emit_self_tail_call`], this does *not* terminate the
    /// current block with a jump — the function returns normally (its own
    /// `return_`, emitted by the ordinary tail-value machinery this value
    /// flows through), and `TypedFn::invoke`'s Rust-level dispatch loop picks
    /// up the pending call from there. No calling-convention change, no
    /// native-to-native jump: this is what keeps native stack usage O(1) for
    /// mutual/general tail recursion without Cranelift's `tail` CC (see the
    /// design note on [`jit_set_pending_tail`]).
    fn emit_cross_tail_call(&mut self, id: usize, args: &[Core]) -> Value {
        let vals: Vec<Value> = args.iter().map(|a| self.emit_value(a)).collect();
        let argc = vals.len();
        let buf = self.b.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            (argc.max(1) * 8) as u32,
            3,
        ));
        for (i, v) in vals.iter().enumerate() {
            self.b.ins().stack_store(*v, buf, (i * 8) as i32);
        }
        let buf_addr = self.b.ins().stack_addr(self.ptr, buf, 0);
        let id_v = self.iconst(id as i64);
        let argc_v = self.iconst(argc as i64);
        self.b.ins().call(
            self.pending_tail_ref,
            &[self.ctx_ptr, id_v, buf_addr, argc_v],
        );
        self.iconst(0)
    }

    /// Evaluate a two-way branch, tail-aware. Mirrors [`Self::branch_to_slot`]
    /// (store each branch's value into a shared stack slot and merge) but
    /// skips the store+jump for a branch that already terminated its block
    /// via [`Self::emit_self_tail_call`]'s jump to the loop header. If *both*
    /// branches tail-loop, the merge block is unreachable — report the whole
    /// node as tail-looping too rather than switching to a block with no
    /// predecessors (`create_block` alone leaves it "pristine," which
    /// `FunctionBuilder::finalize` explicitly exempts from the
    /// sealed/filled check — verified against `cranelift-frontend`'s
    /// `finalize()`, so an unused `merge_b` is sound to simply never visit).
    fn branch_merge(
        &mut self,
        cond: Value,
        then_f: impl FnOnce(&mut Self) -> Emitted,
        else_f: impl FnOnce(&mut Self) -> Emitted,
    ) -> Emitted {
        let then_b = self.b.create_block();
        let else_b = self.b.create_block();
        let merge_b = self.b.create_block();
        let res =
            self.b
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));

        self.b.ins().brif(cond, then_b, &[], else_b, &[]);

        self.b.switch_to_block(then_b);
        self.b.seal_block(then_b);
        let tr = then_f(self);
        if let Emitted::Value(v) = tr {
            self.b.ins().stack_store(v, res, 0);
            self.b.ins().jump(merge_b, &[]);
        }
        // else: then's terminal block already ended with a jump to `header`.

        self.b.switch_to_block(else_b);
        self.b.seal_block(else_b);
        let er = else_f(self);
        if let Emitted::Value(v) = er {
            self.b.ins().stack_store(v, res, 0);
            self.b.ins().jump(merge_b, &[]);
        }

        if matches!(tr, Emitted::TailLooped) && matches!(er, Emitted::TailLooped) {
            return Emitted::TailLooped;
        }

        self.b.switch_to_block(merge_b);
        self.b.seal_block(merge_b);
        Emitted::Value(self.b.ins().stack_load(types::I64, res, 0))
    }

    /// Call the host allocator for an `len`-element buffer; returns the header
    /// pointer as an `I64` word.
    fn alloc(&mut self, len: Value) -> Value {
        let call = self.b.ins().call(self.alloc_ref, &[self.ctx_ptr, len]);
        self.b.inst_results(call)[0]
    }

    /// Address of element `idx` in buffer `base`: `base + 8*(idx+1)`.
    fn elem_addr(&mut self, base: Value, idx: Value) -> Value {
        let off = self.b.ins().iadd_imm(idx, 1);
        let byte_off = self.b.ins().imul_imm(off, 8);
        self.b.ins().iadd(base, byte_off)
    }

    /// Predicate `0 <= idx < len(base)` for bounds-checked access.
    fn in_bounds(&mut self, base: Value, idx: Value) -> Value {
        let len = self
            .b
            .ins()
            .load(types::I64, MemFlagsData::trusted(), base, 0);
        let zero = self.iconst(0);
        let ge0 = self
            .b
            .ins()
            .icmp(IntCC::SignedGreaterThanOrEqual, idx, zero);
        let lt = self.b.ins().icmp(IntCC::SignedLessThan, idx, len);
        self.b.ins().band(ge0, lt)
    }

    /// Set a flag byte in `Ctx` to 1 (branchless OR into the existing value).
    fn set_ctx_flag(&mut self, offset: usize, flag_i8: Value) {
        let addr = self.b.ins().iadd_imm(self.ctx_ptr, offset as i64);
        let old = self
            .b
            .ins()
            .load(types::I8, MemFlagsData::trusted(), addr, 0);
        let new = self.b.ins().bor(old, flag_i8);
        self.b.ins().store(MemFlagsData::trusted(), new, addr, 0);
    }

    fn int_bin(&mut self, op: BinOp, x: Value, y: Value) -> Value {
        use super::runtime::Ctx;
        match op {
            BinOp::Add => {
                let (result, of) = self.b.ins().sadd_overflow(x, y);
                self.set_ctx_flag(Ctx::OVERFLOW_OFFSET, of);
                result
            }
            BinOp::Sub => {
                let (result, of) = self.b.ins().ssub_overflow(x, y);
                self.set_ctx_flag(Ctx::OVERFLOW_OFFSET, of);
                result
            }
            BinOp::Mul => {
                let (result, of) = self.b.ins().smul_overflow(x, y);
                self.set_ctx_flag(Ctx::OVERFLOW_OFFSET, of);
                result
            }
            BinOp::Div | BinOp::Mod => {
                let zero = self.iconst(0);
                let one = self.iconst(1);
                let neg_one = self.iconst(-1);
                let i64_min = self.iconst(i64::MIN);
                let is_zero = self.b.ins().icmp(IntCC::Equal, y, zero);
                // overflow == (x == i64::MIN) && (y == -1)
                let x_is_min = self.b.ins().icmp(IntCC::Equal, x, i64_min);
                let y_is_neg1 = self.b.ins().icmp(IntCC::Equal, y, neg_one);
                let is_overflow = self.b.ins().band(x_is_min, y_is_neg1);
                let is_unsafe = self.b.ins().bor(is_zero, is_overflow);
                // Set flags on Ctx. MOD does NOT flag MIN%-1: its Euclidean
                // value is exactly 0, matching the evaluator's MOD (#280) —
                // only DIV's MIN/-1 is a genuine wrapped result.
                self.set_ctx_flag(Ctx::DIV_BY_ZERO_OFFSET, is_zero);
                if matches!(op, BinOp::Div) {
                    self.set_ctx_flag(Ctx::OVERFLOW_OFFSET, is_overflow);
                }
                // Use safe_y=1 to avoid hardware trap
                let safe_y = self.b.ins().select(is_unsafe, one, y);
                let result = if matches!(op, BinOp::Div) {
                    self.b.ins().sdiv(x, safe_y)
                } else {
                    // Euclidean modulo (#280), matching Rust's rem_euclid
                    // (and therefore the evaluator's MOD): the result is
                    // always non-negative — when the truncated remainder is
                    // negative, add |y|. Computed as r + y for positive y
                    // and r - y for negative y, which stays in range even
                    // for y == i64::MIN (r ∈ (MIN, 0) there, so r - MIN ≤
                    // MAX). In the unsafe cases (y == 0 or MIN % -1) srem
                    // runs against safe_y = 1, so r = 0 and no adjustment
                    // fires.
                    let r = self.b.ins().srem(x, safe_y);
                    let r_neg = self.b.ins().icmp(IntCC::SignedLessThan, r, zero);
                    let y_neg = self.b.ins().icmp(IntCC::SignedLessThan, y, zero);
                    let r_plus_y = self.b.ins().iadd(r, y);
                    let r_minus_y = self.b.ins().isub(r, y);
                    let adjusted = self.b.ins().select(y_neg, r_minus_y, r_plus_y);
                    self.b.ins().select(r_neg, adjusted, r)
                };
                // div-by-zero → 0; MIN/-1 → wrapping MIN for div, exact 0 for mod
                self.b.ins().select(is_zero, zero, result)
            }
        }
    }

    fn as_f(&mut self, w: Value) -> Value {
        self.b.ins().bitcast(types::F64, MemFlagsData::trusted(), w)
    }
    fn as_i(&mut self, f: Value) -> Value {
        self.b.ins().bitcast(types::I64, MemFlagsData::trusted(), f)
    }

    fn float_bin(&mut self, op: BinOp, x: Value, y: Value) -> Value {
        let (xf, yf) = (self.as_f(x), self.as_f(y));
        let rf = match op {
            BinOp::Add => self.b.ins().fadd(xf, yf),
            BinOp::Sub => self.b.ins().fsub(xf, yf),
            BinOp::Mul => self.b.ins().fmul(xf, yf),
            BinOp::Div => self.b.ins().fdiv(xf, yf),
            BinOp::Mod => self.b.ins().fdiv(xf, yf), // unreachable: float mod is rejected
        };
        self.as_i(rf)
    }

    /// Evaluate `then`/`else` into a result stack slot and reload — avoids
    /// block-parameter API churn.
    fn branch_to_slot(
        &mut self,
        cond: Value,
        then_f: impl FnOnce(&mut Self) -> Value,
        else_f: impl FnOnce(&mut Self) -> Value,
    ) -> Value {
        let res =
            self.b
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
        let then_b = self.b.create_block();
        let else_b = self.b.create_block();
        let merge_b = self.b.create_block();

        self.b.ins().brif(cond, then_b, &[], else_b, &[]);

        self.b.switch_to_block(then_b);
        self.b.seal_block(then_b);
        let tv = then_f(self);
        self.b.ins().stack_store(tv, res, 0);
        self.b.ins().jump(merge_b, &[]);

        self.b.switch_to_block(else_b);
        self.b.seal_block(else_b);
        let ev = else_f(self);
        self.b.ins().stack_store(ev, res, 0);
        self.b.ins().jump(merge_b, &[]);

        self.b.switch_to_block(merge_b);
        self.b.seal_block(merge_b);
        self.b.ins().stack_load(types::I64, res, 0)
    }

    /// Emit a non-tail call to typed function `id`, guarded by the depth
    /// cap (issue #271): every non-tail call — whether it ends up taking the
    /// direct native `call_indirect` fast path or falling back to the host
    /// trampoline — grows *some* call stack (native or, via the trampoline,
    /// `Ctx::call`'s own Rust recursion) once per call, unbounded without
    /// this. `jit_enter_call`/`jit_exit_call` mirror `Ctx::enter_call`/
    /// `exit_call` so the fast path — which never otherwise touches
    /// `Ctx::call` at all — is covered too, not just the trampoline-routed
    /// cases (which would also be caught by `Ctx::call`'s own guard, one
    /// frame further in).
    fn emit_call(&mut self, id: usize, args: &[Core]) -> Value {
        let vals: Vec<Value> = args.iter().map(|a| self.emit_value(a)).collect();
        let argc = vals.len();
        let buf = self.b.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            (argc.max(1) * 8) as u32,
            3,
        ));
        for (i, v) in vals.iter().enumerate() {
            self.b.ins().stack_store(*v, buf, (i * 8) as i32);
        }
        let buf_addr = self.b.ins().stack_addr(self.ptr, buf, 0);

        let ok = {
            let ec = self.b.ins().call(self.enter_call_ref, &[self.ctx_ptr]);
            self.b.inst_results(ec)[0]
        };
        let allowed = self.b.ins().icmp_imm(IntCC::NotEqual, ok, 0);

        let outer_res =
            self.b
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
        let call_b = self.b.create_block();
        let over_limit_b = self.b.create_block();
        let outer_merge_b = self.b.create_block();
        self.b.ins().brif(allowed, call_b, &[], over_limit_b, &[]);

        // Depth cap hit: `jit_enter_call` already recorded the pending
        // recursion-limit error; skip the call entirely and substitute a
        // zero-length arena buffer — a valid, non-null pointer that is also
        // a harmless plain integer if the caller treats this call's result
        // as a scalar. The membrane discards it once it sees the error.
        self.b.switch_to_block(over_limit_b);
        self.b.seal_block(over_limit_b);
        let zero_len = self.iconst(0);
        let sentinel = self.alloc(zero_len);
        self.b.ins().stack_store(sentinel, outer_res, 0);
        self.b.ins().jump(outer_merge_b, &[]);

        self.b.switch_to_block(call_b);
        self.b.seal_block(call_b);
        let call_result = self.emit_call_unguarded(id, argc, buf_addr);
        self.b.ins().call(self.exit_call_ref, &[self.ctx_ptr]);
        self.b.ins().stack_store(call_result, outer_res, 0);
        self.b.ins().jump(outer_merge_b, &[]);

        self.b.switch_to_block(outer_merge_b);
        self.b.seal_block(outer_merge_b);
        self.b.ins().stack_load(types::I64, outer_res, 0)
    }

    /// The actual call dispatch `emit_call` wraps with the depth guard:
    /// arity-mismatch/native-direct/trampoline selection, unchanged from
    /// before issue #271 other than taking the already-built argument buffer.
    fn emit_call_unguarded(&mut self, id: usize, argc: usize, buf_addr: Value) -> Value {
        // If the call-site arity disagrees with the callee's current param count
        // (callee was redefined with a different arity after this function's Core
        // was elaborated), skip the native fast path and always route through the
        // trampoline.  The arity guard in `invoke_once` will then skip the callee's
        // native edition too, avoiding an out-of-bounds read in its native prologue.
        let arity_mismatch = id < self.param_counts.len() && argc != self.param_counts[id];

        if arity_mismatch {
            let id_v = self.iconst(id as i64);
            let argc_v = self.iconst(argc as i64);
            let sc = self
                .b
                .ins()
                .call(self.tramp_ref, &[self.ctx_ptr, id_v, buf_addr, argc_v]);
            return self.b.inst_results(sc)[0];
        }

        // Load the callee's current native entry from its (heap-stable) cell.
        // Non-zero -> call it directly (no host round-trip); zero -> the callee
        // has no native edition right now, so route through the trampoline,
        // which dispatches to its closure/interpreter edition.
        let cell_v = self.b.ins().iconst(self.ptr, self.cell_addrs[id] as i64);
        let entry = self
            .b
            .ins()
            .load(self.ptr, MemFlagsData::trusted(), cell_v, 0);
        let zero = self.b.ins().iconst(self.ptr, 0);
        let is_native = self.b.ins().icmp(IntCC::NotEqual, entry, zero);

        let res =
            self.b
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
        let direct_b = self.b.create_block();
        let slow_b = self.b.create_block();
        let merge_b = self.b.create_block();
        self.b.ins().brif(is_native, direct_b, &[], slow_b, &[]);

        // Fast path: direct native call_indirect.
        self.b.switch_to_block(direct_b);
        self.b.seal_block(direct_b);
        let dc = self
            .b
            .ins()
            .call_indirect(self.callee_sig, entry, &[buf_addr, self.ctx_ptr]);
        let dr = self.b.inst_results(dc)[0];
        self.b.ins().stack_store(dr, res, 0);
        self.b.ins().jump(merge_b, &[]);

        // Slow path: host trampoline -> registry dispatch.
        self.b.switch_to_block(slow_b);
        self.b.seal_block(slow_b);
        let id_v = self.iconst(id as i64);
        let argc_v = self.iconst(argc as i64);
        let sc = self
            .b
            .ins()
            .call(self.tramp_ref, &[self.ctx_ptr, id_v, buf_addr, argc_v]);
        let sr = self.b.inst_results(sc)[0];
        self.b.ins().stack_store(sr, res, 0);
        self.b.ins().jump(merge_b, &[]);

        self.b.switch_to_block(merge_b);
        self.b.seal_block(merge_b);
        self.b.ins().stack_load(types::I64, res, 0)
    }
}

fn int_cc(op: CmpOp) -> IntCC {
    match op {
        CmpOp::Lt => IntCC::SignedLessThan,
        CmpOp::Gt => IntCC::SignedGreaterThan,
        CmpOp::Le => IntCC::SignedLessThanOrEqual,
        CmpOp::Ge => IntCC::SignedGreaterThanOrEqual,
        CmpOp::Eq => IntCC::Equal,
        CmpOp::Ne => IntCC::NotEqual,
    }
}

fn float_cc(op: CmpOp) -> FloatCC {
    match op {
        CmpOp::Lt => FloatCC::LessThan,
        CmpOp::Gt => FloatCC::GreaterThan,
        CmpOp::Le => FloatCC::LessThanOrEqual,
        CmpOp::Ge => FloatCC::GreaterThanOrEqual,
        CmpOp::Eq => FloatCC::Equal,
        CmpOp::Ne => FloatCC::NotEqual,
    }
}
