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
    n_params: usize,
    n_slots: usize,
    cell_addrs: &[usize],
) -> Result<NativeEdition, String> {
    let mut jb = JITBuilder::new(default_libcall_names()).map_err(|e| e.to_string())?;
    jb.symbol("jit_trampoline", jit_trampoline as *const u8);
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

        let mut e = Emitter {
            b: &mut b,
            ptr,
            env_slot,
            ctx_ptr,
            tramp_ref,
            callee_sig,
            cell_addrs,
        };
        let result = e.emit(core);
        b.ins().return_(&[result]);
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
    callee_sig: SigRef,
    cell_addrs: &'c [usize],
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

    fn emit(&mut self, core: &Core) -> Value {
        match core {
            Core::LitI(n) => self.iconst(*n),
            Core::LitF(f) => self.iconst(f.to_bits() as i64),
            Core::Var(i) => self
                .b
                .ins()
                .stack_load(types::I64, self.env_slot, (*i * 8) as i32),
            Core::Bin(k, op, a, b) => {
                let (x, y) = (self.emit(a), self.emit(b));
                match k {
                    NumKind::I => self.int_bin(*op, x, y),
                    NumKind::F => self.float_bin(*op, x, y),
                }
            }
            Core::Cmp(k, op, a, b) => {
                let (x, y) = (self.emit(a), self.emit(b));
                let cmp = match k {
                    NumKind::I => self.b.ins().icmp(int_cc(*op), x, y),
                    NumKind::F => {
                        let (xf, yf) = (self.as_f(x), self.as_f(y));
                        self.b.ins().fcmp(float_cc(*op), xf, yf)
                    }
                };
                self.b.ins().uextend(types::I64, cmp)
            }
            Core::Not(a) => {
                let v = self.emit(a);
                let z = self.iconst(0);
                let c = self.b.ins().icmp(IntCC::Equal, v, z);
                self.b.ins().uextend(types::I64, c)
            }
            Core::And(a, b) => {
                // a ? (b != 0) : 0  — short-circuits.
                let av = self.emit(a);
                let cond = self.truthy(av);
                self.branch_to_slot(cond, |s| s.eval_truthy(b), |s| s.iconst(0))
            }
            Core::Or(a, b) => {
                // a ? 1 : (b != 0)  — short-circuits.
                let av = self.emit(a);
                let cond = self.truthy(av);
                self.branch_to_slot(cond, |s| s.iconst(1), |s| s.eval_truthy(b))
            }
            Core::If(c, t, e) => {
                let cv = self.emit(c);
                let cond = self.truthy(cv);
                self.branch_to_slot(cond, |s| s.emit(t), |s| s.emit(e))
            }
            Core::Let(slot, init, body) => {
                let v = self.emit(init);
                self.b
                    .ins()
                    .stack_store(v, self.env_slot, (*slot * 8) as i32);
                self.emit(body)
            }
            Core::Call(id, args) => self.emit_call(*id, args),
        }
    }

    fn int_bin(&mut self, op: BinOp, x: Value, y: Value) -> Value {
        match op {
            BinOp::Add => self.b.ins().iadd(x, y),
            BinOp::Sub => self.b.ins().isub(x, y),
            BinOp::Mul => self.b.ins().imul(x, y),
            BinOp::Div | BinOp::Mod => {
                // Guard divide-by-zero to yield 0 (matching the interpreter),
                // without a trap: divide by a safe non-zero, then select.
                let zero = self.iconst(0);
                let one = self.iconst(1);
                let is_zero = self.b.ins().icmp(IntCC::Equal, y, zero);
                let safe_y = self.b.ins().select(is_zero, one, y);
                let q = if matches!(op, BinOp::Div) {
                    self.b.ins().sdiv(x, safe_y)
                } else {
                    self.b.ins().srem(x, safe_y)
                };
                self.b.ins().select(is_zero, zero, q)
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

    /// Evaluate `core` and normalize to the boolean word `(value != 0)`.
    fn eval_truthy(&mut self, core: &Core) -> Value {
        let v = self.emit(core);
        let t = self.truthy(v);
        self.b.ins().uextend(types::I64, t)
    }

    fn emit_call(&mut self, id: usize, args: &[Core]) -> Value {
        let vals: Vec<Value> = args.iter().map(|a| self.emit(a)).collect();
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
