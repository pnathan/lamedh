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
    AbiParam, BlockArg, InstBuilder, MemFlagsData, SigRef, Signature, StackSlotData, StackSlotKind,
    Value, types,
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
    jb.symbol("jit_array_oob", super::jit_array_oob as *const u8);
    jb.symbol("jit_bad_char", super::jit_bad_char as *const u8);
    jb.symbol("jit_ftrans", super::jit_ftrans as *const u8);
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

    // Imported out-of-bounds index reporter (issue #282): (ctx, idx, len,
    // is_store) -> (). Called from the else-arm of a guarded FETCH/STORE to
    // record the evaluator's index error on `Ctx`; the native code still
    // yields its memory-safe substitute (fetch → 0, store → no-op).
    let mut oobsig = module.make_signature();
    oobsig.params.push(AbiParam::new(ptr));
    oobsig.params.push(AbiParam::new(types::I64));
    oobsig.params.push(AbiParam::new(types::I64));
    oobsig.params.push(AbiParam::new(types::I64));
    let array_oob_id = module
        .declare_function("jit_array_oob", Linkage::Import, &oobsig)
        .map_err(|e| e.to_string())?;

    // Imported CODE-CHAR range reporter (issue #281): (ctx, n) -> (). Called
    // from the out-of-range arm of a CODE-CHAR narrowing to record the
    // evaluator's range error on `Ctx`; the native code still masks the value
    // to a byte itself.
    let mut bcsig = module.make_signature();
    bcsig.params.push(AbiParam::new(ptr));
    bcsig.params.push(AbiParam::new(types::I64));
    let bad_char_id = module
        .declare_function("jit_bad_char", Linkage::Import, &bcsig)
        .map_err(|e| e.to_string())?;

    // Imported libm float-intrinsic trampoline (`sin`/`cos`/`tan`/`exp`/
    // `round`): (opcode, x) -> result word. Pure math, no `Ctx`.
    let mut ftsig = module.make_signature();
    ftsig.params.push(AbiParam::new(types::I64));
    ftsig.params.push(AbiParam::new(types::F64));
    ftsig.returns.push(AbiParam::new(types::I64));
    let ftrans_id = module
        .declare_function("jit_ftrans", Linkage::Import, &ftsig)
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
        let array_oob_ref = module.declare_func_in_func(array_oob_id, b.func);
        let bad_char_ref = module.declare_func_in_func(bad_char_id, b.func);
        let ftrans_ref = module.declare_func_in_func(ftrans_id, b.func);
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
            array_oob_ref,
            bad_char_ref,
            ftrans_ref,
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
    /// Imported [`super::jit_array_oob`] (issue #282): records the evaluator's
    /// index error on `Ctx` from the out-of-bounds arm of a guarded
    /// `FETCH`/`STORE`.
    array_oob_ref: cranelift_codegen::ir::FuncRef,
    /// Imported [`super::jit_bad_char`] (issue #281): records the CODE-CHAR
    /// range error on `Ctx` from the out-of-range arm of a CODE-CHAR narrowing.
    bad_char_ref: cranelift_codegen::ir::FuncRef,
    /// Imported [`super::jit_ftrans`]: the libm float-intrinsic trampoline
    /// (`sin`/`cos`/`tan`/`exp`/`round`), called from the `FUnary` arm.
    ftrans_ref: cranelift_codegen::ir::FuncRef,
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
                // Narrow int64 -> char by masking to a byte (issue #136). An
                // argument outside 0..=255 is not representable as the typed
                // island's byte-`char`: record the evaluator's CODE-CHAR range
                // error (issue #281) via the host trampoline, then still yield
                // the masked byte as the memory-safe substitute. Only the
                // out-of-range arm calls the trampoline; the in-range hot path
                // is the same single `band` as before.
                let v = self.emit_value(a);
                let masked = self.b.ins().band_imm(v, 0xff);
                let zero = self.iconst(0);
                let max = self.iconst(255);
                let ge0 = self.b.ins().icmp(IntCC::SignedGreaterThanOrEqual, v, zero);
                let le255 = self.b.ins().icmp(IntCC::SignedLessThanOrEqual, v, max);
                let in_range = self.b.ins().band(ge0, le255);
                Emitted::Value(self.branch_to_slot(
                    in_range,
                    |_s| masked,
                    |s| {
                        s.b.ins().call(s.bad_char_ref, &[s.ctx_ptr, v]);
                        masked
                    },
                ))
            }
            Core::FUnary(op, a) => {
                use super::types::FUnOp;
                // Unary float intrinsic: bitcast the arg word to f64, apply the
                // op, and produce the result word. `sqrt` yields a float word;
                // the rounding ops floor/ceil/trunc convert to int64 with a
                // *saturating* fcvt, matching Rust's saturating `f64 as i64` that
                // the evaluator and Core interpreter use. The libm ops
                // (sin/cos/tan/exp/round — no direct instruction) call the
                // `jit_ftrans` trampoline, which returns the result word already
                // (float bits or the integer), computed by `FUnOp::apply_word`.
                let v = self.emit_value(a);
                let af = self.as_f(v);
                let r = if op.is_libm() {
                    let opc = self.iconst(op.opcode() as i64);
                    let call = self.b.ins().call(self.ftrans_ref, &[opc, af]);
                    self.b.inst_results(call)[0]
                } else {
                    match op {
                        FUnOp::Sqrt => {
                            let rf = self.b.ins().sqrt(af);
                            self.as_i(rf)
                        }
                        FUnOp::Floor => {
                            let rf = self.b.ins().floor(af);
                            self.b.ins().fcvt_to_sint_sat(types::I64, rf)
                        }
                        FUnOp::Ceil => {
                            let rf = self.b.ins().ceil(af);
                            self.b.ins().fcvt_to_sint_sat(types::I64, rf)
                        }
                        FUnOp::Trunc => {
                            let rf = self.b.ins().trunc(af);
                            self.b.ins().fcvt_to_sint_sat(types::I64, rf)
                        }
                        _ => unreachable!("libm ops routed through jit_ftrans above"),
                    }
                };
                Emitted::Value(r)
            }
            Core::IntToFloat(a) => {
                // `(float int)`: widen an int64 word to a float64 word.
                let v = self.emit_value(a);
                let f = self.b.ins().fcvt_from_sint(types::F64, v);
                Emitted::Value(self.as_i(f))
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
                    |s| {
                        // Out of bounds: record the evaluator's index error
                        // (issue #282), then substitute 0 as before.
                        s.record_oob(base, idx, false);
                        s.iconst(0)
                    },
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
                    |s| {
                        // Out of bounds: record the evaluator's index error
                        // (issue #282); the store is a no-op, return `val` as
                        // before (the recorded error supersedes the result).
                        s.record_oob(base, idx, true);
                        val
                    },
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
            Core::ArrayMap2(op, kind, out, a, b) => {
                let base_out = self.emit_value(out);
                let base_a = self.emit_value(a);
                let base_b = self.emit_value(b);
                Emitted::Value(self.emit_array_map2(*op, *kind, base_out, base_a, base_b))
            }
            Core::ArraySum(a) => {
                let base_a = self.emit_value(a);
                Emitted::Value(self.emit_array_sum(base_a))
            }
            Core::ArrayDot(a, b) => {
                let base_a = self.emit_value(a);
                let base_b = self.emit_value(b);
                Emitted::Value(self.emit_array_dot(base_a, base_b))
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

    /// Record the evaluator's out-of-range index error (issue #282) from the
    /// out-of-bounds arm of a guarded `FETCH`/`STORE` by calling the
    /// [`super::jit_array_oob`] host trampoline with `idx` and the buffer's
    /// length. The caller still emits the memory-safe substitute value.
    fn record_oob(&mut self, base: Value, idx: Value, is_store: bool) {
        let len = self
            .b
            .ins()
            .load(types::I64, MemFlagsData::trusted(), base, 0);
        let is_store = self.iconst(is_store as i64);
        self.b
            .ins()
            .call(self.array_oob_ref, &[self.ctx_ptr, idx, len, is_store]);
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
            BinOp::BitAnd => self.b.ins().band(x, y),
            BinOp::BitOr => self.b.ins().bor(x, y),
            BinOp::BitXor => self.b.ins().bxor(x, y),
            // `y` is a constant in 1..=63 (the elaborator only emits shifts for a
            // literal in-range `ash`), so Cranelift's shift-amount masking never
            // engages and this matches the evaluator's in-range `ash` exactly.
            BinOp::Shl => self.b.ins().ishl(x, y),
            BinOp::AShr => self.b.ins().sshr(x, y),
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
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::AShr => {
                unreachable!("bitwise/shift ops are int64-only")
            }
        };
        self.as_i(rf)
    }

    /// Lower [`Core::ArrayMap2`]: `out[i] = a[i] OP b[i]` for `i` in
    /// `0..min_len` where `min_len = min(len out, len a, len b)`, as a
    /// 2-lane SIMD loop (`I64X2`/`F64X2` — maps to SSE on x86-64 and NEON on
    /// aarch64) with a scalar tail for a final odd element. Returns
    /// `base_out` unchanged (the node's value is the mutated `out` array).
    ///
    /// Buffer layout: `[len, e0, e1, …]`, element `i` at `base + 8*(i+1)`
    /// (`elem_addr`) — elements `i, i+1` are 16 contiguous bytes, so one
    /// vector load/store per iteration covers both lanes. The buffer is
    /// only 8-byte aligned (allocated as a `u64` buffer), so every vector
    /// access uses **unaligned**, `notrap` `MemFlagsData` — the addresses are
    /// always in-bounds by construction (`i < vec_end <= min_len`), so
    /// `notrap` is sound, but `aligned` would be a lie for an odd `i`.
    fn emit_array_map2(
        &mut self,
        op: BinOp,
        kind: NumKind,
        base_out: Value,
        base_a: Value,
        base_b: Value,
    ) -> Value {
        let trusted = MemFlagsData::trusted();
        let unaligned = MemFlagsData::new().with_notrap();

        let len_a = self.b.ins().load(types::I64, trusted, base_a, 0);
        let len_b = self.b.ins().load(types::I64, trusted, base_b, 0);
        let len_out = self.b.ins().load(types::I64, trusted, base_out, 0);
        let min_ab = self.b.ins().smin(len_a, len_b);
        let min_len = self.b.ins().smin(min_ab, len_out);
        // Largest even n <= min_len (clears the low bit); safe for a
        // negative min_len too (never occurs — buffer lengths are
        // non-negative by construction) since it only ever moves `vec_end`
        // further from 0 in magnitude, and the `i < vec_end` loop guard
        // below still degenerates to zero iterations.
        let vec_end = self.b.ins().band_imm(min_len, -2i64);

        // --- vectorized loop: i = 0, 2, 4, ... while i < vec_end ---------
        let zero = self.iconst(0);
        let loop_b = self.b.create_block();
        self.b.append_block_param(loop_b, types::I64);
        self.b.ins().jump(loop_b, &[BlockArg::from(zero)]);

        self.b.switch_to_block(loop_b);
        // Not sealed yet: the back edge from `body_b` (below) is the loop's
        // second predecessor and hasn't been emitted yet.
        let i = self.b.block_params(loop_b)[0];
        let cont = self.b.ins().icmp(IntCC::SignedLessThan, i, vec_end);
        let body_b = self.b.create_block();
        let after_vec_b = self.b.create_block();
        self.b.ins().brif(cont, body_b, &[], after_vec_b, &[]);

        self.b.switch_to_block(body_b);
        self.b.seal_block(body_b); // single predecessor: loop_b, known now
        let vty = vec_ty(kind);
        let addr_a = self.elem_addr(base_a, i);
        let addr_b = self.elem_addr(base_b, i);
        let addr_out = self.elem_addr(base_out, i);
        let va = self.b.ins().load(vty, unaligned, addr_a, 0);
        let vb = self.b.ins().load(vty, unaligned, addr_b, 0);
        let vr = self.vec_bin(op, kind, va, vb);
        self.b.ins().store(unaligned, vr, addr_out, 0);
        let next_i = self.b.ins().iadd_imm(i, 2);
        self.b.ins().jump(loop_b, &[BlockArg::from(next_i)]);
        self.b.seal_block(loop_b); // both predecessors known now

        self.b.switch_to_block(after_vec_b);
        self.b.seal_block(after_vec_b); // single predecessor: loop_b's brif

        // --- scalar tail: one more element iff min_len is odd -----------
        let has_tail = self.b.ins().icmp(IntCC::NotEqual, min_len, vec_end);
        let tail_b = self.b.create_block();
        let done_b = self.b.create_block();
        self.b.ins().brif(has_tail, tail_b, &[], done_b, &[]);

        self.b.switch_to_block(tail_b);
        self.b.seal_block(tail_b); // single predecessor: after_vec_b's brif
        let addr_a_s = self.elem_addr(base_a, vec_end);
        let addr_b_s = self.elem_addr(base_b, vec_end);
        let addr_out_s = self.elem_addr(base_out, vec_end);
        // Scalar element loads/stores are always 8-byte offsets from an
        // 8-byte-aligned base, so (unlike the vector path) `trusted`'s
        // `aligned` bit is honest here too.
        let sa = self.b.ins().load(types::I64, trusted, addr_a_s, 0);
        let sb = self.b.ins().load(types::I64, trusted, addr_b_s, 0);
        let sr = self.scalar_wrapping_bin(op, kind, sa, sb);
        self.b.ins().store(trusted, sr, addr_out_s, 0);
        self.b.ins().jump(done_b, &[]);

        self.b.switch_to_block(done_b);
        // Two predecessors: tail_b's jump (just emitted) and after_vec_b's
        // brif else-arm (emitted above) — both known now.
        self.b.seal_block(done_b);
        base_out
    }

    /// `x OP y` on 2-lane vector operands (`I64X2`/`F64X2`) — wrapping for
    /// int64 (plain `iadd`/`isub`/`imul` are already two's-complement
    /// wraparound, matching `wrapping_add`/`wrapping_sub`/`wrapping_mul`; no
    /// per-lane overflow flag exists to set, which is the whole reason
    /// [`Core::ArrayMap2`] is defined as wrapping). Only `Add`/`Sub`/`Mul`
    /// are ever constructed for this node.
    fn vec_bin(&mut self, op: BinOp, kind: NumKind, x: Value, y: Value) -> Value {
        match (kind, op) {
            (NumKind::I, BinOp::Add) => self.b.ins().iadd(x, y),
            (NumKind::I, BinOp::Sub) => self.b.ins().isub(x, y),
            (NumKind::I, BinOp::Mul) => self.b.ins().imul(x, y),
            (NumKind::F, BinOp::Add) => self.b.ins().fadd(x, y),
            (NumKind::F, BinOp::Sub) => self.b.ins().fsub(x, y),
            (NumKind::F, BinOp::Mul) => self.b.ins().fmul(x, y),
            _ => unreachable!("ArrayMap2 only ever carries Add/Sub/Mul"),
        }
    }

    /// Scalar counterpart of [`Self::vec_bin`] for the odd-length tail
    /// element: raw `I64` words in, bitcasting to `F64` for the float case
    /// (mirroring [`Self::float_bin`], but — unlike it — never touching the
    /// `OVERFLOW`/`DIV_BY_ZERO` `Ctx` flags, since this whole node family is
    /// defined as flagless/wrapping to match the vectorized body).
    fn scalar_wrapping_bin(&mut self, op: BinOp, kind: NumKind, x: Value, y: Value) -> Value {
        match kind {
            NumKind::I => match op {
                BinOp::Add => self.b.ins().iadd(x, y),
                BinOp::Sub => self.b.ins().isub(x, y),
                BinOp::Mul => self.b.ins().imul(x, y),
                _ => unreachable!("ArrayMap2 only ever carries Add/Sub/Mul"),
            },
            NumKind::F => {
                let (xf, yf) = (self.as_f(x), self.as_f(y));
                let rf = match op {
                    BinOp::Add => self.b.ins().fadd(xf, yf),
                    BinOp::Sub => self.b.ins().fsub(xf, yf),
                    BinOp::Mul => self.b.ins().fmul(xf, yf),
                    _ => unreachable!("ArrayMap2 only ever carries Add/Sub/Mul"),
                };
                self.as_i(rf)
            }
        }
    }

    /// Lower [`Core::ArraySum`]: wrapping sum of every `int64` element of
    /// `a`, as a 2-lane `I64X2` accumulator loop plus a horizontal reduction
    /// and a scalar tail for an odd final element. Sound because wrapping
    /// int64 addition is associative — see [`Core::ArraySum`]'s doc comment.
    ///
    /// Loop shape mirrors [`Self::emit_array_map2`]: block params carry the
    /// induction variable `i` AND the running `I64X2` accumulator (the
    /// vector-lane analogue of accumulating a scalar in a block param).
    /// `elem_addr`/`MemFlagsData` usage (unaligned+notrap for the vector
    /// path, trusted for the always-8-byte-aligned scalar tail) is identical
    /// to [`Self::emit_array_map2`]'s.
    fn emit_array_sum(&mut self, base_a: Value) -> Value {
        let trusted = MemFlagsData::trusted();
        let unaligned = MemFlagsData::new().with_notrap();

        let len_a = self.b.ins().load(types::I64, trusted, base_a, 0);
        // Largest even n <= len_a (clears the low bit) — see
        // `emit_array_map2`'s identical `vec_end` computation.
        let vec_end = self.b.ins().band_imm(len_a, -2i64);

        let zero = self.iconst(0);
        let zero_vec = self.b.ins().splat(types::I64X2, zero);

        // --- vectorized loop: i = 0, 2, 4, ... while i < vec_end ---------
        let loop_b = self.b.create_block();
        self.b.append_block_param(loop_b, types::I64); // i
        self.b.append_block_param(loop_b, types::I64X2); // running accumulator
        self.b
            .ins()
            .jump(loop_b, &[BlockArg::from(zero), BlockArg::from(zero_vec)]);

        self.b.switch_to_block(loop_b);
        // Not sealed yet: the back edge from `body_b` (below) is the loop's
        // second predecessor and hasn't been emitted yet.
        let i = self.b.block_params(loop_b)[0];
        let acc_in = self.b.block_params(loop_b)[1];
        let cont = self.b.ins().icmp(IntCC::SignedLessThan, i, vec_end);
        let body_b = self.b.create_block();
        let after_vec_b = self.b.create_block();
        self.b.ins().brif(cont, body_b, &[], after_vec_b, &[]);

        self.b.switch_to_block(body_b);
        self.b.seal_block(body_b); // single predecessor: loop_b, known now
        let addr_a = self.elem_addr(base_a, i);
        let va = self.b.ins().load(types::I64X2, unaligned, addr_a, 0);
        let acc_out = self.b.ins().iadd(acc_in, va);
        let next_i = self.b.ins().iadd_imm(i, 2);
        self.b
            .ins()
            .jump(loop_b, &[BlockArg::from(next_i), BlockArg::from(acc_out)]);
        self.b.seal_block(loop_b); // both predecessors known now

        self.b.switch_to_block(after_vec_b);
        self.b.seal_block(after_vec_b); // single predecessor: loop_b's brif
        // Horizontal reduce: `acc_in` here is loop_b's accumulator block
        // param at loop exit (a value dominating `after_vec_b`, usable
        // directly without re-passing it through a block arg — same
        // dominance-based visibility `emit_array_map2` relies on for
        // `vec_end`). Lane 0 + lane 1 is the whole reduction for a 2-lane
        // vector.
        let lane0 = self.b.ins().extractlane(acc_in, 0u8);
        let lane1 = self.b.ins().extractlane(acc_in, 1u8);
        let vec_sum = self.b.ins().iadd(lane0, lane1);

        // --- scalar tail: one more element iff len_a is odd -------------
        let has_tail = self.b.ins().icmp(IntCC::NotEqual, len_a, vec_end);
        let tail_b = self.b.create_block();
        let done_b = self.b.create_block();
        self.b.append_block_param(done_b, types::I64); // final sum
        self.b
            .ins()
            .brif(has_tail, tail_b, &[], done_b, &[BlockArg::from(vec_sum)]);

        self.b.switch_to_block(tail_b);
        self.b.seal_block(tail_b); // single predecessor: after_vec_b's brif
        let addr_a_s = self.elem_addr(base_a, vec_end);
        let sa = self.b.ins().load(types::I64, trusted, addr_a_s, 0);
        let tail_sum = self.b.ins().iadd(vec_sum, sa);
        self.b.ins().jump(done_b, &[BlockArg::from(tail_sum)]);

        self.b.switch_to_block(done_b);
        // Two predecessors: tail_b's jump (just emitted) and after_vec_b's
        // brif else-arm (emitted above) — both known now.
        self.b.seal_block(done_b);
        self.b.block_params(done_b)[0]
    }

    /// Lower [`Core::ArrayDot`]: wrapping sum over `i in 0..min(len a, len
    /// b)` of `a[i] * b[i]`, as a 2-lane `I64X2` accumulator loop (`imul` the
    /// loaded vectors, `iadd` into the accumulator) plus the same horizontal
    /// reduction and scalar tail as [`Self::emit_array_sum`]. Sound because
    /// wrapping int64 multiply-then-add is associative in the running sum —
    /// see [`Core::ArrayDot`]'s doc comment.
    fn emit_array_dot(&mut self, base_a: Value, base_b: Value) -> Value {
        let trusted = MemFlagsData::trusted();
        let unaligned = MemFlagsData::new().with_notrap();

        let len_a = self.b.ins().load(types::I64, trusted, base_a, 0);
        let len_b = self.b.ins().load(types::I64, trusted, base_b, 0);
        let min_len = self.b.ins().smin(len_a, len_b);
        let vec_end = self.b.ins().band_imm(min_len, -2i64);

        let zero = self.iconst(0);
        let zero_vec = self.b.ins().splat(types::I64X2, zero);

        // --- vectorized loop: i = 0, 2, 4, ... while i < vec_end ---------
        let loop_b = self.b.create_block();
        self.b.append_block_param(loop_b, types::I64); // i
        self.b.append_block_param(loop_b, types::I64X2); // running accumulator
        self.b
            .ins()
            .jump(loop_b, &[BlockArg::from(zero), BlockArg::from(zero_vec)]);

        self.b.switch_to_block(loop_b);
        let i = self.b.block_params(loop_b)[0];
        let acc_in = self.b.block_params(loop_b)[1];
        let cont = self.b.ins().icmp(IntCC::SignedLessThan, i, vec_end);
        let body_b = self.b.create_block();
        let after_vec_b = self.b.create_block();
        self.b.ins().brif(cont, body_b, &[], after_vec_b, &[]);

        self.b.switch_to_block(body_b);
        self.b.seal_block(body_b);
        let addr_a = self.elem_addr(base_a, i);
        let addr_b = self.elem_addr(base_b, i);
        let va = self.b.ins().load(types::I64X2, unaligned, addr_a, 0);
        let vb = self.b.ins().load(types::I64X2, unaligned, addr_b, 0);
        let vp = self.b.ins().imul(va, vb);
        let acc_out = self.b.ins().iadd(acc_in, vp);
        let next_i = self.b.ins().iadd_imm(i, 2);
        self.b
            .ins()
            .jump(loop_b, &[BlockArg::from(next_i), BlockArg::from(acc_out)]);
        self.b.seal_block(loop_b);

        self.b.switch_to_block(after_vec_b);
        self.b.seal_block(after_vec_b);
        let lane0 = self.b.ins().extractlane(acc_in, 0u8);
        let lane1 = self.b.ins().extractlane(acc_in, 1u8);
        let vec_sum = self.b.ins().iadd(lane0, lane1);

        // --- scalar tail: one more element iff min_len is odd -----------
        let has_tail = self.b.ins().icmp(IntCC::NotEqual, min_len, vec_end);
        let tail_b = self.b.create_block();
        let done_b = self.b.create_block();
        self.b.append_block_param(done_b, types::I64);
        self.b
            .ins()
            .brif(has_tail, tail_b, &[], done_b, &[BlockArg::from(vec_sum)]);

        self.b.switch_to_block(tail_b);
        self.b.seal_block(tail_b);
        let addr_a_s = self.elem_addr(base_a, vec_end);
        let addr_b_s = self.elem_addr(base_b, vec_end);
        let sa = self.b.ins().load(types::I64, trusted, addr_a_s, 0);
        let sb = self.b.ins().load(types::I64, trusted, addr_b_s, 0);
        let sp = self.b.ins().imul(sa, sb);
        let tail_sum = self.b.ins().iadd(vec_sum, sp);
        self.b.ins().jump(done_b, &[BlockArg::from(tail_sum)]);

        self.b.switch_to_block(done_b);
        self.b.seal_block(done_b);
        self.b.block_params(done_b)[0]
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

/// The 2-lane vector type for [`Emitter::emit_array_map2`]: `I64X2` for
/// int64 (SSE2 `paddq`/`psubq`/hand-rolled `pmuludq` shuffle on x86-64, NEON
/// `add`/`sub`/`mul` `v2i64`/`v2i64` — Cranelift picks the ISA-appropriate
/// lowering), `F64X2` for float64 (SSE2 `addpd`/`subpd`/`mulpd`, NEON
/// `fadd`/`fsub`/`fmul` `v2f64`).
fn vec_ty(kind: NumKind) -> types::Type {
    match kind {
        NumKind::I => types::I64X2,
        NumKind::F => types::F64X2,
    }
}
